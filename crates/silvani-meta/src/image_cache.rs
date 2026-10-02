use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use reqwest::Client;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::models::AnimeItem;

static GLOBAL_TOKIO_RT: OnceLock<Runtime> = OnceLock::new();

pub fn get_tokio_handle() -> Handle {
    if let Ok(handle) = Handle::try_current() {
        handle
    } else {
        GLOBAL_TOKIO_RT
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to initialize background Tokio runtime for image cache")
            })
            .handle()
            .clone()
    }
}

pub struct ImageCacheManager {
    cache_dir: PathBuf,
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl Default for ImageCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageCacheManager {
    pub fn new() -> Self {
        let base_cache = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(|_| std::env::temp_dir());
        let cache_dir = base_cache.join("silvani").join("covers");
        let _ = std::fs::create_dir_all(&cache_dir);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Allow up to 12 concurrent image downloads in parallel
        let semaphore = Arc::new(Semaphore::new(12));

        Self {
            cache_dir,
            client,
            semaphore,
        }
    }

    pub fn get_cached_path(&self, id: &str) -> Option<PathBuf> {
        let path = self.cache_dir.join(format!("{}.jpg", id));
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub async fn fetch_and_cache(&self, id: String, url: String) -> Option<PathBuf> {
        let dest = self.cache_dir.join(format!("{}.jpg", id));
        if dest.exists() {
            return Some(dest);
        }

        let handle = get_tokio_handle();
        let client = self.client.clone();
        let sem = Arc::clone(&self.semaphore);
        let dest_clone = dest.clone();

        handle
            .spawn(async move {
                let _permit = sem.acquire().await.ok()?;
                if dest_clone.exists() {
                    return Some(dest_clone);
                }

                let res = client.get(&url).send().await.ok()?;
                if !res.status().is_success() {
                    return None;
                }

                let bytes = res.bytes().await.ok()?;
                std::fs::write(&dest_clone, &bytes).ok()?;
                Some(dest_clone)
            })
            .await
            .ok()?
    }

    /// Pre-fetches multiple anime covers in parallel at high speed safely on Tokio runtime
    pub fn prefetch_covers(&self, items: &[AnimeItem]) -> tokio::task::JoinHandle<()> {
        let handle = get_tokio_handle();
        let mut targets = Vec::new();
        for item in items {
            if let Some(url) = &item.cover_image
                && !url.starts_with('/')
                && !url.starts_with("file://")
            {
                let dest = self.cache_dir.join(format!("{}.jpg", item.id));
                if !dest.exists() {
                    targets.push((item.id.clone(), url.clone(), dest));
                }
            }
        }
        if targets.is_empty() {
            return handle.spawn(async {});
        }

        let client = self.client.clone();
        let sem = Arc::clone(&self.semaphore);
        handle.spawn(async move {
            let mut set = JoinSet::new();
            for (_id, url, dest) in targets {
                let client = client.clone();
                let sem = Arc::clone(&sem);
                set.spawn(async move {
                    let _permit = match sem.acquire().await {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    if dest.exists() {
                        return;
                    }
                    if let Ok(res) = client.get(&url).send().await
                        && res.status().is_success()
                        && let Ok(bytes) = res.bytes().await
                    {
                        let _ = std::fs::write(&dest, &bytes);
                    }
                });
            }
            while set.join_next().await.is_some() {}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnimeTitle;

    #[tokio::test]
    async fn test_image_cache_manager() {
        let mgr = ImageCacheManager::new();
        let path = mgr.get_cached_path("non_existent_anime_id_99999");
        assert!(path.is_none());
    }

    #[test]
    fn test_prefetch_without_ambient_runtime() {
        // Run on an OS thread with no ambient Tokio runtime (simulating a GPUI worker thread)
        let handle = std::thread::spawn(|| {
            let mgr = ImageCacheManager::new();
            let items = vec![AnimeItem {
                id: "test-prefetch-1".to_string(),
                title: AnimeTitle {
                    english: Some("Test Anime".to_string()),
                    romaji: None,
                    native: None,
                },
                cover_image: Some("https://example.com/test.jpg".to_string()),
                ..Default::default()
            }];
            // Must not panic with "there is no reactor running"
            mgr.prefetch_covers(&items);
        });
        handle.join().expect("Prefetch on worker thread panicked!");
    }
}
