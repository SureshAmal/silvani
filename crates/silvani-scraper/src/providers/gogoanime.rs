use regex::Regex;
use std::sync::LazyLock;

use crate::error::{Result, ScraperError};
use crate::http::ScraperHttpClient;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture, ProviderMetadata, ProviderPriority};

static SEARCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a\s+href="[^"]*/category/([^"]*)"\s+title="([^"]*)""#).unwrap()
});

static EPISODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="[^"]*-episode-([0-9.]+)[^"]*""#).unwrap()
});

static IFRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<iframe\s+src="([^"]+)""#).unwrap()
});

const MIRRORS: &[&str] = &[
    "https://anitaku.pe",
    "https://anitaku.so",
];

pub struct GogoAnimeProvider {
    client: ScraperHttpClient,
}

impl Default for GogoAnimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GogoAnimeProvider {
    pub fn new() -> Self {
        Self {
            client: ScraperHttpClient::new(),
        }
    }
}

impl AnimeProvider for GogoAnimeProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            id: "gogoanime",
            name: "gogoanime",
            display_name: "GogoAnime (AniTaku)",
            description: "Massive classic & modern anime catalog (Sub & Dub)",
            supported_audio: vec![AudioType::Sub, AudioType::Dub],
            priority: ProviderPriority::High,
            is_official: false,
            primary_domain: "anitaku.pe",
            mirror_domains: MIRRORS.to_vec(),
        }
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let query_param = query.replace(' ', "+");
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/search.html?keyword={query_param}"))
                .collect();

            let (_effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

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
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/category/{anime_id}"))
                .collect();

            let (_effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

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
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/{episode_id}"))
                .collect();

            let (effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

            let iframe_cap = IFRAME_RE
                .captures(&html)
                .ok_or_else(|| ScraperError::NotFound("Player iframe not found".into()))?;

            let stream_url = if iframe_cap[1].starts_with("//") {
                format!("https:{}", &iframe_cap[1])
            } else {
                iframe_cap[1].to_string()
            };

            let domain = effective_url
                .split('/')
                .take(3)
                .collect::<Vec<_>>()
                .join("/");

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: match audio {
                    AudioType::Dub => "English".to_string(),
                    _ => "Japanese".to_string(),
                },
                streams: vec![VideoStream {
                    quality: "auto".to_string(),
                    url: stream_url,
                }],
                subtitles: Vec::new(),
                referer: domain,
            })
        })
    }
}
