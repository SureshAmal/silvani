use std::sync::Arc;

use crate::engine::ScraperEngine;
use crate::error::{Result, ScraperError};
use crate::models::{AudioType, Episode, StreamSource};

#[derive(Clone)]
pub struct AnimeStreamService {
    engine: Arc<ScraperEngine>,
}

impl AnimeStreamService {
    pub fn new(engine: Arc<ScraperEngine>) -> Self {
        Self { engine }
    }

    pub fn engine(&self) -> &Arc<ScraperEngine> {
        &self.engine
    }

    pub fn clear_cache(&self) {
        self.engine.clear_cache();
    }

    /// Finds and loads real episodes for a given title, provider, and audio language.
    /// Returns an empty list if no episodes exist on this provider for this audio.
    /// Never generates fake or placeholder episodes.
    pub async fn fetch_episodes(
        &self,
        title: &str,
        provider: &str,
        audio: AudioType,
    ) -> Result<(String, Vec<Episode>)> {
        // Query formulation: specialized for dub on certain providers
        let search_query = match (&audio, provider) {
            (AudioType::Dub, "gogoanime") => format!("{title} (Dub)"),
            _ => title.to_string(),
        };

        let results = self.engine.search_provider(provider, &search_query).await;

        let target = results.into_iter().find(|r| {
            if audio == AudioType::Dub && provider == "gogoanime" {
                r.title.to_lowercase().contains("dub")
            } else {
                true
            }
        });

        if let Some(matched) = target {
            let episodes = self.engine.episodes(provider, &matched.id).await?;
            Ok((matched.id, episodes))
        } else {
            // No matching anime found on this provider for this audio
            Ok((String::new(), Vec::new()))
        }
    }

    /// Resolves stream source for an episode
    pub async fn resolve_stream(
        &self,
        title: &str,
        provider: &str,
        episode_id: Option<&str>,
        ep_number: f32,
        audio: AudioType,
    ) -> Result<StreamSource> {
        if let Some(eid) = episode_id
            && !eid.is_empty()
        {
            return self.engine.stream(provider, eid, audio).await;
        }

        // Fallback: search provider directly
        let (_aid, episodes) = self.fetch_episodes(title, provider, audio.clone()).await?;
        let target_ep = episodes
            .iter()
            .find(|e| (e.number - ep_number).abs() < 0.1)
            .or_else(|| episodes.first())
            .ok_or_else(|| ScraperError::NotFound("No episodes available for this selection".into()))?;

        self.engine.stream(provider, &target_ep.id, audio).await
    }
}
