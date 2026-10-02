use base64::Engine;
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, SubtitleTrack, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture};

static SEARCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"class="film-name">\s*<a href="[^"]*/([^"/]*)"\s*title="([^"]*)""#).unwrap()
});

static EP_ITEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"data-number="([^"]*)"[^>]*data-id="([0-9]+)""#).unwrap()
});

static BLOB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"window\.__P="([^"]*)""#).unwrap()
});

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const XOR_KEY: &[u8] = b"otaku-embed-v1";

#[derive(Deserialize)]
struct EpisodeListResponse {
    #[serde(default)]
    html: String,
}

#[derive(Deserialize)]
struct DecryptedPayload {
    src: String,
    #[serde(default)]
    subtitles: Vec<RawSubtitle>,
}

#[derive(Deserialize)]
struct RawSubtitle {
    src: String,
    label: Option<String>,
    #[serde(default)]
    default: bool,
}

pub struct HiAnimeProvider {
    client: reqwest::Client,
}

impl Default for HiAnimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HiAnimeProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_default();
        Self { client }
    }

    fn deobfuscate(blob: &str) -> Result<DecryptedPayload> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(blob)
            .map_err(|e| ScraperError::Crypto(e.to_string()))?;

        let decrypted: Vec<u8> = bytes
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ XOR_KEY[i % XOR_KEY.len()])
            .collect();

        serde_json::from_slice(&decrypted).map_err(|e| ScraperError::Parse(e.to_string()))
    }
}

impl AnimeProvider for HiAnimeProvider {
    fn name(&self) -> &'static str {
        "hianime"
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let query_param = query.replace(' ', "+");
            let url = format!("https://hianime.at/search?keyword={query_param}");

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
            let numeric_id = anime_id
                .split('-')
                .next_back()
                .ok_or_else(|| ScraperError::Parse("Invalid anime slug".into()))?;

            let url = format!("https://hianime.at/api/theme/episode/list/{numeric_id}");
            let res: EpisodeListResponse = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

            let mut episodes = Vec::new();
            for cap in EP_ITEM_RE.captures_iter(&res.html) {
                let num = cap[1].parse::<f32>().unwrap_or(0.0);
                episodes.push(Episode {
                    id: cap[2].to_string(),
                    number: num,
                    title: None,
                });
            }
            Ok(episodes)
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let mode_str = match audio {
                AudioType::Sub => "sub",
                AudioType::Dub => "dub",
            };

            let servers_url = format!("https://hianime.at/api/theme/episode/servers?episodeId={episode_id}");
            let resp: serde_json::Value = self
                .client
                .get(&servers_url)
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .json()
                .await
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

            let html = resp["html"]
                .as_str()
                .ok_or_else(|| ScraperError::Parse("Missing HTML in server response".into()))?;

            let re_hash = Regex::new(&format!(
                r#"data-type="{mode_str}"\s*data-server-name="ZokoAnime"\s*data-hash="([^"]*)""#
            ))
            .map_err(|e| ScraperError::Parse(e.to_string()))?;

            let hash_cap = re_hash
                .captures(html)
                .ok_or_else(|| ScraperError::NotFound(format!("ZokoAnime server not found for {mode_str}")))?;

            let embed_url_bytes = base64::engine::general_purpose::STANDARD
                .decode(&hash_cap[1])
                .map_err(|e| ScraperError::Crypto(e.to_string()))?;

            let embed_url = String::from_utf8(embed_url_bytes)
                .map_err(|e| ScraperError::Crypto(e.to_string()))?;

            let embed_html = self
                .client
                .get(&embed_url)
                .header("Referer", "https://zokoanime.video/")
                .send()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?
                .text()
                .await
                .map_err(|e| ScraperError::Network(e.to_string()))?;

            let blob_cap = BLOB_RE
                .captures(&embed_html)
                .ok_or_else(|| ScraperError::NotFound("window.__P blob not found".into()))?;

            let payload = Self::deobfuscate(&blob_cap[1])?;

            let subtitles = payload
                .subtitles
                .into_iter()
                .map(|s| SubtitleTrack {
                    language: s.label.unwrap_or_else(|| "English".into()),
                    url: s.src,
                    is_default: s.default,
                })
                .collect();

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: "Japanese".to_string(),
                streams: vec![VideoStream {
                    quality: "auto".to_string(),
                    url: payload.src,
                }],
                subtitles,
                referer: "https://zokoanime.video/".to_string(),
            })
        })
    }
}
