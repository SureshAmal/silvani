use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::cache::CacheStore;
use crate::error::Result;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource};
use crate::provider::{AnimeProvider, ProviderMetadata};
use crate::providers::*;

static TOKIO_RT: OnceLock<Runtime> = OnceLock::new();

fn get_tokio_handle() -> Handle {
    if let Ok(handle) = Handle::try_current() {
        return handle;
    }

    TOKIO_RT
        .get_or_init(|| {
            Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .expect("Failed to initialize background Scraper Tokio runtime")
        })
        .handle()
        .clone()
}

pub fn provider_matches(registered_name: &str, requested_name: &str) -> bool {
    let r = registered_name.to_lowercase().replace(['_', '-'], "");
    let q = requested_name.to_lowercase().replace(['_', '-'], "");
    r == q || r.contains(&q) || q.contains(&r)
}

#[derive(Clone)]
pub struct ScraperEngine {
    providers: Vec<Arc<dyn AnimeProvider>>,
    cache: CacheStore,
}

impl ScraperEngine {
    fn default_providers() -> Vec<Arc<dyn AnimeProvider>> {
        vec![
            Arc::new(HiAnimeProvider::new()),
            Arc::new(AllAnimeProvider::new()),
            Arc::new(GogoAnimeProvider::new()),
            Arc::new(AnimeOnsenProvider::new()),
            Arc::new(OnePaceProvider::new()),
            Arc::new(AnimeWorldIndiaProvider::new()),
            Arc::new(DesiDubAnimeProvider::new()),
            Arc::new(YouTubeAnimeProvider::muse_india()),
            Arc::new(YouTubeAnimeProvider::ani_one_asia()),
        ]
    }

    pub fn new() -> Result<Self> {
        let cache = CacheStore::in_memory()?;
        Ok(Self {
            providers: Self::default_providers(),
            cache,
        })
    }

    pub fn with_cache_file(db_path: &str) -> Result<Self> {
        let cache = CacheStore::open(db_path)?;
        Ok(Self {
            providers: Self::default_providers(),
            cache,
        })
    }

    pub fn register_provider(&mut self, provider: Arc<dyn AnimeProvider>) {
        self.providers.push(provider);
    }

    pub fn providers(&self) -> &[Arc<dyn AnimeProvider>] {
        &self.providers
    }

    pub fn provider_metadatas(&self) -> Vec<ProviderMetadata> {
        self.providers.iter().map(|p| p.metadata()).collect()
    }

    pub fn filter_providers_by_audio(&self, audio: AudioType) -> Vec<Arc<dyn AnimeProvider>> {
        self.providers
            .iter()
            .filter(|p| p.supported_audio().contains(&audio))
            .cloned()
            .collect()
    }

    pub fn clear_cache(&self) {
        let _ = self.cache.clear();
    }

    /// Reactive Streaming API with fast-first racing: Pushes results immediately as each provider finishes.
    pub fn search_stream(&self, query: &str) -> mpsc::Receiver<Vec<AnimeSearchResult>> {
        self.search_stream_filtered(query, None, None)
    }

    /// Filtered Streaming Search: Allows filtering by language (AudioType) and/or specific enabled provider IDs.
    /// Emits results instantly as individual providers respond (fast-first racing).
    pub fn search_stream_filtered(
        &self,
        query: &str,
        audio_filter: Option<AudioType>,
        provider_ids: Option<Vec<String>>,
    ) -> mpsc::Receiver<Vec<AnimeSearchResult>> {
        let matched_providers: Vec<Arc<dyn AnimeProvider>> = self
            .providers
            .iter()
            .filter(|p| {
                if let Some(audio) = audio_filter {
                    if !p.supported_audio().contains(&audio) {
                        return false;
                    }
                }
                if let Some(ref ids) = provider_ids {
                    if !ids.iter().any(|id| provider_matches(p.name(), id) || provider_matches(p.id(), id)) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let (tx, rx) = mpsc::channel(matched_providers.len().max(1));
        let handle = get_tokio_handle();

        for provider in matched_providers {
            let p = Arc::clone(&provider);
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

                // Race provider search with a 7-second timeout to prevent blocked providers from lingering
                let search_fut = p.search(&q);
                let timeout_fut = tokio::time::timeout(Duration::from_secs(15), search_fut);

                if let Ok(Ok(results)) = timeout_fut.await {
                    if !results.is_empty() {
                        let _ = cache.set_search(&q, p.name(), &results);
                        let _ = sender.send(results).await;
                    }
                }
            });
        }

        rx
    }

    /// Batch Search API: Waits for providers and aggregates ranked results.
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

                        let search_fut = p.search(&q);
                        if let Ok(Ok(results)) = tokio::time::timeout(Duration::from_secs(15), search_fut).await {
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

    /// Targeted Search: Searches only the specified provider with caching
    pub async fn search_provider(&self, provider_name: &str, query: &str) -> Vec<AnimeSearchResult> {
        let provider = match self.providers.iter().find(|p| provider_matches(p.name(), provider_name)) {
            Some(p) => Arc::clone(p),
            None => return Vec::new(),
        };

        let prov_key = provider.name();
        if let Some(cached) = self.cache.get_search(query, prov_key, 3600) {
            return cached;
        }

        if let Ok(results) = provider.search(query).await {
            let _ = self.cache.set_search(query, prov_key, &results);
            results
        } else {
            Vec::new()
        }
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

        let source = provider.stream(episode_id, audio).await?;
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
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_provider_filtering() {
        let engine = ScraperEngine::new().unwrap();
        let hindi_providers = engine.filter_providers_by_audio(AudioType::Hindi);
        assert!(!hindi_providers.is_empty());
        assert!(hindi_providers.iter().any(|p| p.name() == "animeworldindia"));

        let sub_providers = engine.filter_providers_by_audio(AudioType::Sub);
        assert!(sub_providers.iter().any(|p| p.name() == "hianime"));
        assert!(sub_providers.iter().any(|p| p.name() == "allanime"));
    }
}
