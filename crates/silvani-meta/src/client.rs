use anyhow::Result;

use crate::cache::MetaCache;
use crate::models::{AnimeItem, DashboardData};
use crate::providers::anilist::AniListProvider;
use crate::providers::jikan::JikanProvider;
use crate::providers::kitsu::KitsuProvider;

pub struct AnimeMetadataClient {
    anilist: AniListProvider,
    kitsu: KitsuProvider,
    jikan: JikanProvider,
    cache: MetaCache,
    episode_titles_lock: tokio::sync::Mutex<()>,
}

impl Default for AnimeMetadataClient {
    fn default() -> Self {
        Self::new().expect("Failed to initialize AnimeMetadataClient")
    }
}

impl AnimeMetadataClient {
    pub fn new() -> Result<Self> {
        let cache = MetaCache::in_memory()?;
        Ok(Self {
            anilist: AniListProvider::new(),
            kitsu: KitsuProvider::new(),
            jikan: JikanProvider::new(),
            cache,
            episode_titles_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn with_cache_file(db_path: &str) -> Result<Self> {
        let cache = MetaCache::open(db_path)?;
        Ok(Self {
            anilist: AniListProvider::new(),
            kitsu: KitsuProvider::new(),
            jikan: JikanProvider::new(),
            cache,
            episode_titles_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub async fn get_dashboard(&self, per_page: u32) -> Result<DashboardData> {
        let cache_key = format!("dashboard:{}", per_page);
        if let Some(json_str) = self.cache.get(&cache_key, 3600)
            && let Ok(cached) = serde_json::from_str::<DashboardData>(&json_str) {
                return Ok(cached);
            }

        let result = match self.kitsu.fetch_dashboard(per_page).await {
            Ok(data) => data,
            Err(_) => match self.anilist.fetch_dashboard(per_page).await {
                Ok(data) => data,
                Err(_) => self.jikan.fetch_dashboard(per_page).await?,
            },
        };

        if let Ok(serialized) = serde_json::to_string(&result) {
            let _ = self.cache.set(&cache_key, &serialized);
        }

        Ok(result)
    }

    pub async fn search(&self, query: &str, page: u32, per_page: u32) -> Result<Vec<AnimeItem>> {
        let cache_key = format!("search:{}:{}:{}", query.to_lowercase(), page, per_page);
        if let Some(json_str) = self.cache.get(&cache_key, 7200)
            && let Ok(cached) = serde_json::from_str::<Vec<AnimeItem>>(&json_str) {
                return Ok(cached);
            }

        let result = match self.kitsu.search(query, per_page).await {
            Ok(items) if !items.is_empty() => items,
            _ => match self.anilist.search(query, page, per_page).await {
                Ok(items) if !items.is_empty() => items,
                _ => self.jikan.search(query, per_page).await?,
            },
        };

        if let Ok(serialized) = serde_json::to_string(&result) {
            let _ = self.cache.set(&cache_key, &serialized);
        }

        Ok(result)
    }

    pub async fn get_by_genre(&self, genre: &str, page: u32, per_page: u32) -> Result<Vec<AnimeItem>> {
        let cache_key = format!("genre:{}:{}:{}", genre.to_lowercase(), page, per_page);
        if let Some(json_str) = self.cache.get(&cache_key, 21600)
            && let Ok(cached) = serde_json::from_str::<Vec<AnimeItem>>(&json_str) {
                return Ok(cached);
            }

        let result = match self.kitsu.fetch_by_genre(genre, per_page).await {
            Ok(items) if !items.is_empty() => items,
            _ => self.anilist.fetch_by_genre(genre, page, per_page).await.unwrap_or_default(),
        };

        if let Ok(serialized) = serde_json::to_string(&result) {
            let _ = self.cache.set(&cache_key, &serialized);
        }

        Ok(result)
    }

    pub async fn get_details(&self, id: &str) -> Result<Option<AnimeItem>> {
        let cache_key = format!("details:{}", id);
        if let Some(json_str) = self.cache.get(&cache_key, 86400)
            && let Ok(cached) = serde_json::from_str::<AnimeItem>(&json_str) {
                return Ok(Some(cached));
            }

        let result = if id.starts_with("kitsu-") {
            self.kitsu.fetch_details(id).await?
        } else {
            match self.anilist.fetch_details(id).await {
                Ok(Some(item)) => Some(item),
                _ => self.kitsu.fetch_details(id).await.unwrap_or(None),
            }
        };

        if let Some(item) = &result
            && let Ok(serialized) = serde_json::to_string(item) {
                let _ = self.cache.set(&cache_key, &serialized);
            }

        Ok(result)
    }

    /// Resolve by catalog identity, never by a fuzzy anime-title search.
    pub async fn get_episode_titles(
        &self,
        item: &AnimeItem,
    ) -> Result<std::collections::BTreeMap<u32, String>> {
        let _guard = self.episode_titles_lock.lock().await;
        let key = format!("episode-titles-v2:{}", item.id);
        if let Some(json) = self.cache.get(&key, 86400)
            && let Ok(titles) = serde_json::from_str(&json)
        {
            return Ok(titles);
        }
        let mut native_titles = if item.id.starts_with("kitsu-") {
            self.kitsu
                .fetch_episode_titles(&item.id)
                .await
                .unwrap_or_default()
        } else {
            std::collections::BTreeMap::new()
        };
        if !native_titles.is_empty()
            && item
                .episodes
                .is_none_or(|count| native_titles.len() >= count as usize)
        {
            self.cache
                .set(&key, &serde_json::to_string(&native_titles)?)?;
            return Ok(native_titles);
        }
        let mal_id = if let Some(id) = item.mal_id.filter(|id| *id > 0) {
            Some(id)
        } else if item.id.starts_with("kitsu-") {
            match self.kitsu.resolve_mal_id(&item.id).await {
                Ok(id) => id,
                Err(error) if native_titles.is_empty() => return Err(error),
                Err(_) => None,
            }
        } else if let Some(id) = item.id.strip_prefix("mal-") {
            Some(id.parse::<i64>()?)
        } else {
            self.anilist
                .fetch_details(&item.id)
                .await?
                .and_then(|item| item.mal_id)
        };
        if let Some(mal_id) = mal_id {
            match self.jikan.fetch_episode_titles(mal_id).await {
                Ok(titles) => {
                    for (number, title) in titles {
                        native_titles.entry(number).or_insert(title);
                    }
                }
                Err(error) if native_titles.is_empty() => return Err(error),
                Err(_) => {}
            }
        }
        if !native_titles.is_empty() {
            self.cache
                .set(&key, &serde_json::to_string(&native_titles)?)?;
        }
        Ok(native_titles)
    }

    pub fn clear_cache(&self) {
        let _ = self.cache.clear();
    }

    pub fn delete_cache(&self, key: &str) {
        let _ = self.cache.delete(key);
    }
}

#[cfg(test)]
mod episode_metadata_tests {
    #[tokio::test]
    #[ignore = "requires the public episode metadata APIs"]
    async fn live_episode_titles_resolve_kitsu_identity_and_cache_real_names() {
        let client = super::AnimeMetadataClient::new().unwrap();
        let anime = crate::AnimeItem { id: "kitsu-1".into(), ..Default::default() };
        let titles = client.get_episode_titles(&anime).await.unwrap();
        assert_eq!(titles.get(&1).map(String::as_str), Some("Asteroid Blues"));
        assert!(titles.contains_key(&26));
        assert_eq!(client.get_episode_titles(&anime).await.unwrap(), titles);
    }
}
