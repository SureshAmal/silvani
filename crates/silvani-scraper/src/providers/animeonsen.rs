use serde::Deserialize;

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, SubtitleTrack, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture};

const BASE_API: &str = "https://api.animeonsen.xyz/v4";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

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
    client: reqwest::Client,
}

impl Default for AnimeOnsenProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimeOnsenProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl AnimeProvider for AnimeOnsenProvider {
    fn name(&self) -> &'static str {
        "animeonsen"
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let url = format!("{BASE_API}/search/{}", query.replace(' ', "%20"));
            let items: Vec<SearchItem> = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

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
            let url = format!("{BASE_API}/anime/{anime_id}/episodes");
            let items: Vec<EpisodeItem> = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

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
            let url = format!("{BASE_API}/episode/{episode_id}/video");
            let item: VideoItem = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

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
