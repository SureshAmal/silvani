use regex::Regex;
use std::sync::LazyLock;

use crate::error::Result;
use crate::http::ScraperHttpClient;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture, ProviderMetadata, ProviderPriority};

static POST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<h2\s+class="entry-title"><a\s+href="([^"]*)"[^>]*>([^<]*)</a>"#).unwrap()
});

static EPISODE_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="([^"]*)"[^>]*>.*?Episode\s+([0-9.]+)"#).unwrap()
});

static IFRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<iframe[^>]+src="([^"]+)""#).unwrap()
});

const MIRRORS: &[&str] = &[
    "https://animeworldindia.net",
    "https://animeworld.ac",
];

pub struct AnimeWorldIndiaProvider {
    client: ScraperHttpClient,
}

impl Default for AnimeWorldIndiaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimeWorldIndiaProvider {
    pub fn new() -> Self {
        Self {
            client: ScraperHttpClient::new(),
        }
    }
}

impl AnimeProvider for AnimeWorldIndiaProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            id: "animeworldindia",
            name: "animeworldindia",
            display_name: "AnimeWorld India",
            description: "Direct Hindi dubbed anime and movies with multi-audio",
            supported_audio: vec![AudioType::Hindi, AudioType::Sub],
            priority: ProviderPriority::High,
            is_official: false,
            primary_domain: "animeworldindia.net",
            mirror_domains: MIRRORS.to_vec(),
        }
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let query_param = query.replace(' ', "+");
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/?s={query_param}"))
                .collect();

            let (_effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

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
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/{anime_id}/"))
                .collect();

            let (effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

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
                    id: effective_url,
                    number: 1.0,
                    title: Some("Full Release".into()),
                });
            }

            Ok(episodes)
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let urls = vec![episode_id.to_string()];
            let (effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

            let stream_url = if let Some(cap) = IFRAME_RE.captures(&html) {
                cap[1].to_string()
            } else {
                effective_url.clone()
            };

            let domain = effective_url
                .split('/')
                .take(3)
                .collect::<Vec<_>>()
                .join("/");

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: "Hindi".to_string(),
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
