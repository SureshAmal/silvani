use std::sync::{Arc, OnceLock};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::cache::CacheStore;
use crate::error::Result;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource};
use crate::provider::AnimeProvider;
use crate::providers::{
    AnimeOnsenProvider, AnimeWorldIndiaProvider, DesiDubAnimeProvider, GogoAnimeProvider,
    HiAnimeProvider, OnePaceProvider, YouTubeAnimeProvider,
};

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
                    .expect("Failed to initialize background Tokio runtime for scrapers")
            })
            .handle()
            .clone()
    }
}

pub struct ScraperEngine {
    providers: Vec<Arc<dyn AnimeProvider>>,
    cache: CacheStore,
}

impl Default for ScraperEngine {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

fn provider_matches(p_name: &str, target: &str) -> bool {
    let p_clean = p_name.replace(['_', '-'], "").to_lowercase();
    let t_clean = target.replace(['_', '-'], "").to_lowercase();
    p_clean == t_clean
        || (p_clean == "animeworldindia" && (t_clean == "animeworld" || t_clean == "animeworldindia"))
        || (p_clean == "desidubanime" && (t_clean == "desidub" || t_clean == "desidubanime"))
}

impl ScraperEngine {
    pub fn new() -> Result<Self> {
        let cache = CacheStore::in_memory()?;
        let providers: Vec<Arc<dyn AnimeProvider>> = vec![
            Arc::new(HiAnimeProvider::new()),
            Arc::new(GogoAnimeProvider::new()),
            Arc::new(AnimeOnsenProvider::new()),
            Arc::new(OnePaceProvider::new()),
            Arc::new(AnimeWorldIndiaProvider::new()),
            Arc::new(DesiDubAnimeProvider::new()),
            Arc::new(YouTubeAnimeProvider::muse_india()),
            Arc::new(YouTubeAnimeProvider::ani_one_asia()),
        ];
        Ok(Self { providers, cache })
    }

    pub fn with_cache_file(db_path: &str) -> Result<Self> {
        let cache = CacheStore::open(db_path)?;
        let providers: Vec<Arc<dyn AnimeProvider>> = vec![
            Arc::new(HiAnimeProvider::new()),
            Arc::new(GogoAnimeProvider::new()),
            Arc::new(AnimeOnsenProvider::new()),
            Arc::new(OnePaceProvider::new()),
            Arc::new(AnimeWorldIndiaProvider::new()),
            Arc::new(DesiDubAnimeProvider::new()),
            Arc::new(YouTubeAnimeProvider::muse_india()),
            Arc::new(YouTubeAnimeProvider::ani_one_asia()),
        ];
        Ok(Self { providers, cache })
    }

    pub fn register_provider(&mut self, provider: Arc<dyn AnimeProvider>) {
        self.providers.push(provider);
    }

    pub fn clear_cache(&self) {
        let _ = self.cache.clear();
    }

    /// Reactive Streaming API: Pushes results immediately as each provider finishes.
    /// UI pulls from Receiver in real-time without waiting for other providers.
    pub fn search_stream(&self, query: &str) -> mpsc::Receiver<Vec<AnimeSearchResult>> {
        let (tx, rx) = mpsc::channel(self.providers.len().max(1));
        let handle = get_tokio_handle();

        for provider in &self.providers {
            let p = Arc::clone(provider);
            let q = query.to_string();
            let cache = self.cache.clone();
            let sender = tx.clone();

            handle.spawn(async move {
                if let Some(cached) = cache.get_search(&q, p.name(), 3600) {
                    if !cached.is_empty() {
                        let _ = sender.send(cached).await;
                    }
                    return;
                }

                if let Ok(results) = p.search(&q).await
                    && !results.is_empty() {
                        let _ = cache.set_search(&q, p.name(), &results);
                        let _ = sender.send(results).await;
                    }
            });
        }

        rx
    }

    /// Batch Search API: Waits for all providers to complete and aggregates results.
    pub async fn search(&self, query: &str) -> Vec<AnimeSearchResult> {
        let handle = get_tokio_handle();
        let providers = self.providers.clone();
        let cache = self.cache.clone();
        let q = query.to_string();

        handle
            .spawn(async move {
                let mut set = JoinSet::new();

                for provider in providers {
                    let p = Arc::clone(&provider);
                    let q = q.clone();
                    let cache = cache.clone();

                    set.spawn(async move {
                        if let Some(cached) = cache.get_search(&q, p.name(), 3600) {
                            return cached;
                        }
                        if let Ok(results) = p.search(&q).await {
                            let _ = cache.set_search(&q, p.name(), &results);
                            results
                        } else {
                            Vec::new()
                        }
                    });
                }

                let mut aggregated = Vec::new();
                while let Some(res) = set.join_next().await {
                    if let Ok(results) = res {
                        aggregated.extend(results);
                    }
                }

                let q_clean = q.to_lowercase().replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");
                let q_words: Vec<&str> = q_clean.split_whitespace().collect();

                aggregated.sort_by(|a, b| {
                    let score = |title: &str| -> i32 {
                        let t_clean = title.to_lowercase().replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");
                        if t_clean == q_clean {
                            return 1000;
                        }
                        if t_clean.starts_with(&q_clean) {
                            return 500;
                        }
                        if t_clean.contains(&q_clean) {
                            return 300;
                        }
                        let mut match_count = 0;
                        for w in &q_words {
                            if t_clean.contains(w) {
                                match_count += 10;
                            }
                        }
                        match_count
                    };
                    score(&b.title).cmp(&score(&a.title))
                });

                aggregated
            })
            .await
            .unwrap_or_default()
    }

    /// High-level API: Fetch episode list for anime on specific provider
    pub async fn episodes(&self, provider_name: &str, anime_id: &str) -> Result<Vec<Episode>> {
        let provider = self
            .providers
            .iter()
            .find(|p| provider_matches(p.name(), provider_name))
            .ok_or_else(|| crate::error::ScraperError::NotFound("Provider not found".into()))?;

        provider.episodes(anime_id).await
    }

    /// High-level API: Get playable video stream with caching
    pub async fn stream(
        &self,
        provider_name: &str,
        episode_id: &str,
        audio: AudioType,
    ) -> Result<StreamSource> {
        let provider = self
            .providers
            .iter()
            .find(|p| provider_matches(p.name(), provider_name))
            .ok_or_else(|| crate::error::ScraperError::NotFound("Provider not found".into()))?;

        let prov_key = provider.name();
        if let Some(cached) = self.cache.get_stream(prov_key, episode_id, &audio, 7200) {
            return Ok(cached);
        }

        let source = provider.stream(episode_id, audio.clone()).await?;
        let _ = self.cache.set_stream(prov_key, episode_id, &audio, &source);
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_stream_without_ambient_runtime() {
        let engine = ScraperEngine::new().unwrap();
        let mut rx = engine.search_stream("Naruto");
        // Verify receiver is active and no panic occurs
        assert!(rx.try_recv().is_err());
    }
}
