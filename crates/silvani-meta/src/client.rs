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
        })
    }

    pub fn with_cache_file(db_path: &str) -> Result<Self> {
        let cache = MetaCache::open(db_path)?;
        Ok(Self {
            anilist: AniListProvider::new(),
            kitsu: KitsuProvider::new(),
            jikan: JikanProvider::new(),
            cache,
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

    pub fn clear_cache(&self) {
        let _ = self.cache.clear();
    }

    pub fn delete_cache(&self, key: &str) {
        let _ = self.cache.delete(key);
    }
}
