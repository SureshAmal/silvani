use regex::Regex;
use std::sync::LazyLock;

use crate::error::{Result, ScraperError};
use crate::http::ScraperHttpClient;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture, ProviderMetadata, ProviderPriority};

static POST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<h2\s+class="entry-title"><a\s+href="([^"]*)"[^>]*>([^<]*)</a>"#).unwrap()
});

static IFRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<iframe[^>]+src="([^"]+)""#).unwrap()
});

const MIRRORS: &[&str] = &[
    "https://desidubanime.me",
    "https://desidub.in",
];

pub struct DesiDubAnimeProvider {
    client: ScraperHttpClient,
}

impl Default for DesiDubAnimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DesiDubAnimeProvider {
    pub fn new() -> Self {
        Self {
            client: ScraperHttpClient::new(),
        }
    }
}

impl AnimeProvider for DesiDubAnimeProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            id: "desidubanime",
            name: "desidubanime",
            display_name: "DesiDub Anime",
            description: "Dedicated Hindi dubbed and subbed anime aggregator",
            supported_audio: vec![AudioType::Hindi, AudioType::Dub],
            priority: ProviderPriority::High,
            is_official: false,
            primary_domain: "desidubanime.me",
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
            Ok(vec![Episode {
                id: anime_id.to_string(),
                number: 1.0,
                title: Some("Hindi Dubbed Episode 1".into()),
            }])
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/{episode_id}/"))
                .collect();

            let (effective_url, html) = self.client.get_html_with_mirrors(&urls, None).await?;

            let iframe_cap = IFRAME_RE
                .captures(&html)
                .ok_or_else(|| ScraperError::NotFound("Player iframe not found".into()))?;

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
                    url: iframe_cap[1].to_string(),
                }],
                subtitles: Vec::new(),
                referer: domain,
            })
        })
    }
}
