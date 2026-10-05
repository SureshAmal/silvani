use serde::Deserialize;

use crate::error::Result;
use crate::http::ScraperHttpClient;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, SubtitleTrack, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture, ProviderMetadata, ProviderPriority};

const MIRROR_APIS: &[&str] = &[
    "https://api.animeonsen.xyz/v4",
    "https://www.animeonsen.xyz/api/v4",
];

#[derive(Deserialize)]
struct SearchItem {
    id: String,
    title: String,
}

#[derive(Deserialize)]
struct EpisodeItem {
    id: String,
    number: f32,
    title: Option<String>,
}

#[derive(Deserialize)]
struct VideoItem {
    uri: String,
    #[serde(default)]
    subtitles: Vec<SubItem>,
}

#[derive(Deserialize)]
struct SubItem {
    lang: String,
    src: String,
}

pub struct AnimeOnsenProvider {
    client: ScraperHttpClient,
}

impl Default for AnimeOnsenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimeOnsenProvider {
    pub fn new() -> Self {
        Self {
            client: ScraperHttpClient::new(),
        }
    }
}

impl AnimeProvider for AnimeOnsenProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            id: "animeonsen",
            name: "animeonsen",
            display_name: "AnimeOnsen",
            description: "Direct ad-free 720p streams with soft subtitles",
            supported_audio: vec![AudioType::Sub],
            priority: ProviderPriority::Medium,
            is_official: false,
            primary_domain: "animeonsen.xyz",
            mirror_domains: vec!["api.animeonsen.xyz", "animeonsen.xyz", "www.animeonsen.xyz"],
        }
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let encoded_q = query.replace(' ', "%20");
            let urls: Vec<String> = MIRROR_APIS
                .iter()
                .map(|base| format!("{base}/search/{encoded_q}"))
                .collect();

            let (_effective_url, items): (String, Vec<SearchItem>) =
                self.client.get_json_with_mirrors(&urls, Some("https://animeonsen.xyz/")).await?;

            Ok(items
                .into_iter()
                .map(|item| AnimeSearchResult {
                    provider: self.name().to_string(),
                    id: item.id,
                    title: item.title,
                    poster_url: None,
                })
                .collect())
        })
    }

    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            let urls: Vec<String> = MIRROR_APIS
                .iter()
                .map(|base| format!("{base}/anime/{anime_id}/episodes"))
                .collect();

            let (_effective_url, items): (String, Vec<EpisodeItem>) =
                self.client.get_json_with_mirrors(&urls, Some("https://animeonsen.xyz/")).await?;

            Ok(items
                .into_iter()
                .map(|item| Episode {
                    id: item.id,
                    number: item.number,
                    title: item.title,
                })
                .collect())
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let urls: Vec<String> = MIRROR_APIS
                .iter()
                .map(|base| format!("{base}/episode/{episode_id}/video"))
                .collect();

            let (_effective_url, item): (String, VideoItem) =
                self.client.get_json_with_mirrors(&urls, Some("https://animeonsen.xyz/")).await?;

            let subtitles = item
                .subtitles
                .into_iter()
                .map(|s| SubtitleTrack {
                    language: s.lang,
                    url: s.src,
                    is_default: true,
                })
                .collect();

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: "Japanese".to_string(),
                streams: vec![VideoStream {
                    quality: "720p".to_string(),
                    url: item.uri,
                }],
                subtitles,
                referer: "https://animeonsen.xyz/".to_string(),
            })
        })
    }
}
