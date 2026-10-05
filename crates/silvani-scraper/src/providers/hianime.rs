use base64::Engine;
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

use crate::error::{Result, ScraperError};
use crate::http::ScraperHttpClient;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, SubtitleTrack, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture, ProviderMetadata, ProviderPriority};

static SEARCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"class="film-name">\s*<a href="[^"]*/([^"/]*)"\s*title="([^"]*)""#).unwrap()
});

static EP_ITEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"data-number="([^"]*)"[^>]*data-id="([0-9]+)""#).unwrap()
});

static BLOB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"window\.__P="([^"]*)""#).unwrap()
});

const XOR_KEY: &[u8] = b"otaku-embed-v1";
const MIRRORS: &[&str] = &[
    "https://hianime.at",
    "https://hianime.nz",
    "https://aniwatchtv.to",
    "https://hianime.to",
];

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
    client: ScraperHttpClient,
}

impl Default for HiAnimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HiAnimeProvider {
    pub fn new() -> Self {
        Self {
            client: ScraperHttpClient::new(),
        }
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
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            id: "hianime",
            name: "hianime",
            display_name: "HiAnime",
            description: "High quality 1080p HLS streaming with soft subtitles (Sub & Dub)",
            supported_audio: vec![AudioType::Sub, AudioType::Dub],
            priority: ProviderPriority::High,
            is_official: false,
            primary_domain: "hianime.to",
            mirror_domains: MIRRORS.to_vec(),
        }
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let query_param = query.replace(' ', "+");
            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/search?keyword={query_param}"))
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
            let numeric_id = anime_id
                .split('-')
                .next_back()
                .ok_or_else(|| ScraperError::Parse("Invalid anime slug".into()))?;

            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/api/theme/episode/list/{numeric_id}"))
                .collect();

            let (_effective_url, res): (String, EpisodeListResponse) =
                self.client.get_json_with_mirrors(&urls, None).await?;

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
                AudioType::Dub => "dub",
                _ => "sub",
            };

            let urls: Vec<String> = MIRRORS
                .iter()
                .map(|m| format!("{m}/api/theme/episode/servers?episodeId={episode_id}"))
                .collect();

            let (server_base_url, resp): (String, serde_json::Value) =
                self.client.get_json_with_mirrors(&urls, None).await?;

            let html = resp["html"]
                .as_str()
                .ok_or_else(|| ScraperError::Parse("Missing HTML in server response".into()))?;

            let server_item_re = Regex::new(r#"<div[^>]*class="[^"]*server-item[^"]*"[^>]*>"#)
                .map_err(|e| ScraperError::Parse(e.to_string()))?;
            let attr_type_re = Regex::new(r#"data-type="([^"]*)""#)
                .map_err(|e| ScraperError::Parse(e.to_string()))?;
            let attr_hash_re = Regex::new(r#"data-hash="([^"]*)""#)
                .map_err(|e| ScraperError::Parse(e.to_string()))?;

            let mut servers: Vec<(String, String)> = Vec::new();
            for cap in server_item_re.find_iter(html) {
                let tag = cap.as_str();
                if let Some(h) = attr_hash_re.captures(tag) {
                    let stype = attr_type_re
                        .captures(tag)
                        .map(|c| c[1].to_string())
                        .unwrap_or_else(|| "sub".to_string());
                    servers.push((stype, h[1].to_string()));
                }
            }

            // Find requested audio mode first, or gracefully fall back to available server (e.g. sub)
            let (actual_mode, hash_str) = servers
                .iter()
                .find(|(t, _)| t == mode_str)
                .or_else(|| servers.first())
                .cloned()
                .ok_or_else(|| ScraperError::NotFound(format!("No streaming server found for episode {episode_id}")))?;

            let actual_audio = if actual_mode == "dub" {
                AudioType::Dub
            } else {
                AudioType::Sub
            };

            let embed_url_bytes = base64::engine::general_purpose::STANDARD
                .decode(&hash_str)
                .map_err(|e| ScraperError::Crypto(e.to_string()))?;

            let embed_url = String::from_utf8(embed_url_bytes)
                .map_err(|e| ScraperError::Crypto(e.to_string()))?;

            let embed_urls = vec![embed_url.clone()];
            let (_eff, embed_html) = self
                .client
                .get_html_with_mirrors(&embed_urls, Some("https://zokoanime.video/"))
                .await?;

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

            let is_dub = actual_audio == AudioType::Dub;
            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: actual_audio,
                language: if is_dub {
                    "English".to_string()
                } else {
                    "Japanese".to_string()
                },
                streams: vec![VideoStream {
                    quality: "auto".to_string(),
                    url: payload.src,
                }],
                subtitles,
                referer: if server_base_url.contains("zokoanime") {
                    "https://zokoanime.video/".to_string()
                } else {
                    "https://megacloud.tv/".to_string()
                },
            })
        })
    }
}
