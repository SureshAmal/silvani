use regex::Regex;
use std::sync::LazyLock;

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture};

static POST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<h2\s+class="entry-title"><a\s+href="([^"]*)"[^>]*>([^<]*)</a>"#).unwrap()
});

static EPISODE_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="([^"]*)"[^>]*>.*?Episode\s+([0-9.]+)"#).unwrap()
});

static IFRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<iframe[^>]+src="([^"]+)""#).unwrap()
});

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const BASE_URL: &str = "https://animeworldindia.net";

pub struct AnimeWorldIndiaProvider {
    client: reqwest::Client,
}

impl Default for AnimeWorldIndiaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimeWorldIndiaProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl AnimeProvider for AnimeWorldIndiaProvider {
    fn name(&self) -> &'static str {
        "animeworldindia"
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let url = format!("{BASE_URL}/?s={}", query.replace(' ', "+"));
            let html = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .text()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?;

            let mut results = Vec::new();
            for cap in POST_RE.captures_iter(&html) {
                let link = cap[1].to_string();
                let slug = link
                    .trim_end_matches('/')
                    .split('/')
                    .next_back()
                    .unwrap_or_default()
                    .to_string();

                results.push(AnimeSearchResult {
                    provider: self.name().to_string(),
                    id: slug,
                    title: cap[2].trim().to_string(),
                    poster_url: None,
                });
            }
            Ok(results)
        })
    }

    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            let url = format!("{BASE_URL}/{anime_id}/");
            let html = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .text()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?;

            let mut episodes = Vec::new();
            for cap in EPISODE_LINK_RE.captures_iter(&html) {
                let num = cap[2].parse::<f32>().unwrap_or(1.0);
                episodes.push(Episode {
                    id: cap[1].to_string(),
                    number: num,
                    title: Some(format!("Hindi Dub Ep {num}")),
                });
            }

            if episodes.is_empty() {
                episodes.push(Episode {
                    id: format!("{BASE_URL}/{anime_id}/"),
                    number: 1.0,
                    title: Some("Full Release".into()),
                });
            }

            Ok(episodes)
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let html = self
                .client
                .get(episode_id)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .text()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?;

            let stream_url = if let Some(cap) = IFRAME_RE.captures(&html) {
                cap[1].to_string()
            } else {
                episode_id.to_string()
            };

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: "Hindi".to_string(),
                streams: vec![VideoStream {
                    quality: "auto".to_string(),
                    url: stream_url,
                }],
                subtitles: Vec::new(),
                referer: BASE_URL.to_string(),
            })
        })
    }
}
