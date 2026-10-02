use serde::Deserialize;

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture};

const BASE_URL: &str = "https://onepace.net/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Deserialize)]
struct ArcItem {
    id: String,
    invariant_title: String,
    #[serde(default)]
    episodes: Vec<ArcEpisode>,
}

#[derive(Deserialize)]
struct ArcEpisode {
    id: String,
    part: Option<u32>,
    title: Option<String>,
    #[serde(default)]
    downloads: Vec<DownloadItem>,
}

#[derive(Deserialize)]
struct DownloadItem {
    type_: String,
    uri: String,
}

pub struct OnePaceProvider {
    client: reqwest::Client,
}

impl Default for OnePaceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OnePaceProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl AnimeProvider for OnePaceProvider {
    fn name(&self) -> &'static str {
        "onepace"
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let q_lower = query.to_lowercase();
            if !("one piece".contains(&q_lower) || "one pace".contains(&q_lower) || q_lower.contains("luffy")) {
                return Ok(Vec::new());
            }

            let url = format!("{BASE_URL}/arcs");
            let arcs: Vec<ArcItem> = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

            Ok(arcs
                .into_iter()
                .map(|a| AnimeSearchResult {
                    provider: self.name().to_string(),
                    id: a.id,
                    title: format!("One Pace - {}", a.invariant_title),
                    poster_url: None,
                })
                .collect())
        })
    }

    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            let url = format!("{BASE_URL}/arcs");
            let arcs: Vec<ArcItem> = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

            let target_arc = arcs
                .into_iter()
                .find(|a| a.id == anime_id)
                .ok_or_else(|| ScraperError::NotFound("Arc not found".into()))?;

            Ok(target_arc
                .episodes
                .into_iter()
                .enumerate()
                .map(|(i, ep)| Episode {
                    id: ep.id,
                    number: ep.part.unwrap_or((i + 1) as u32) as f32,
                    title: ep.title,
                })
                .collect())
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let url = format!("{BASE_URL}/arcs");
            let arcs: Vec<ArcItem> = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

            for a in arcs {
                for ep in a.episodes {
                    if ep.id == episode_id {
                        let stream_uri = ep
                            .downloads
                            .into_iter()
                            .find(|d| d.type_ == "pixeldrain" || d.type_ == "stream" || d.type_ == "telegram")
                            .map(|d| d.uri)
                            .ok_or_else(|| ScraperError::NotFound("No stream link available".into()))?;

                        return Ok(StreamSource {
                            provider: self.name().to_string(),
                            audio_type: audio,
                            language: "Dual (Jap/Eng)".to_string(),
                            streams: vec![VideoStream {
                                quality: "1080p".to_string(),
                                url: stream_uri,
                            }],
                            subtitles: Vec::new(),
                            referer: "https://onepace.net/".to_string(),
                        });
                    }
                }
            }

            Err(ScraperError::NotFound("Episode ID not found".into()))
        })
    }
}
