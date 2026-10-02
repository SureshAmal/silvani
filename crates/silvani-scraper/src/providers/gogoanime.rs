use regex::Regex;
use std::sync::LazyLock;

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture};

static SEARCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a\s+href="[^"]*/category/([^"]*)"\s+title="([^"]*)""#).unwrap()
});

static EPISODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="[^"]*-episode-([0-9.]+)[^"]*""#).unwrap()
});

static IFRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<iframe\s+src="([^"]+)""#).unwrap()
});

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const BASE_URL: &str = "https://anitaku.pe";

pub struct GogoAnimeProvider {
    client: reqwest::Client,
}

impl Default for GogoAnimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GogoAnimeProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl AnimeProvider for GogoAnimeProvider {
    fn name(&self) -> &'static str {
        "gogoanime"
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let url = format!("{BASE_URL}/search.html?keyword={}", query.replace(' ', "+"));
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
            for cap in SEARCH_RE.captures_iter(&html) {
                results.push(AnimeSearchResult {
                    provider: self.name().to_string(),
                    id: cap[1].to_string(),
                    title: cap[2].to_string(),
                    poster_url: None,
                });
            }
            Ok(results)
        })
    }

    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            let page_url = format!("{BASE_URL}/category/{anime_id}");
            let html = self
                .client
                .get(&page_url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .text()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?;

            let mut episodes = Vec::new();
            for cap in EPISODE_RE.captures_iter(&html) {
                let num = cap[1].parse::<f32>().unwrap_or(0.0);
                episodes.push(Episode {
                    id: format!("{anime_id}-episode-{}", &cap[1]),
                    number: num,
                    title: None,
                });
            }
            episodes.reverse();
            Ok(episodes)
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let ep_url = format!("{BASE_URL}/{episode_id}");
            let html = self
                .client
                .get(&ep_url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .text()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?;

            let iframe_cap = IFRAME_RE
                .captures(&html)
                .ok_or_else(|| ScraperError::NotFound("Player iframe not found".into()))?;

            let stream_url = if iframe_cap[1].starts_with("//") {
                format!("https:{}", &iframe_cap[1])
            } else {
                iframe_cap[1].to_string()
            };

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: "Japanese".to_string(),
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
