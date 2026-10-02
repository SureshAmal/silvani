use youtube_dl::{YoutubeDl, YoutubeDlOutput};

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture};

pub struct YouTubeAnimeProvider {
    channel_name: &'static str,
    channel_query: &'static str,
    default_language: &'static str,
}

impl YouTubeAnimeProvider {
    pub fn muse_india() -> Self {
        Self {
            channel_name: "muse_india",
            channel_query: "Muse India",
            default_language: "Hindi",
        }
    }

    pub fn ani_one_asia() -> Self {
        Self {
            channel_name: "ani_one_asia",
            channel_query: "Ani-One Asia",
            default_language: "English",
        }
    }
}

impl AnimeProvider for YouTubeAnimeProvider {
    fn name(&self) -> &'static str {
        self.channel_name
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let search_term = format!("ytsearch10:{} {}", self.channel_query, query);

            let output = tokio::task::spawn_blocking(move || {
                YoutubeDl::new(&search_term)
                    .flat_playlist(true)
                    .extra_arg("--no-warnings")
                    .run()
            })
            .await
            .map_err(|e| ScraperError::Network(e.to_string()))?
            .map_err(|e| ScraperError::Network(e.to_string()))?;

            let mut results = Vec::new();

            let is_channel_match = |uploader: Option<&str>, channel: Option<&str>| -> bool {
                let check = |name: &str| {
                    let n = name.to_lowercase();
                    match self.channel_name {
                        "muse_india" => n.contains("muse"),
                        "ani_one_asia" => n.contains("ani-one") || n.contains("anione"),
                        _ => true,
                    }
                };
                uploader.map(check).unwrap_or(false) || channel.map(check).unwrap_or(false)
            };

            let query_words: Vec<String> = query
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .map(|s| s.to_string())
                .collect();

            let is_title_relevant = |title: &str| -> bool {
                if query_words.is_empty() {
                    return true;
                }
                let title_lower = title.to_lowercase();
                query_words.iter().any(|w| title_lower.contains(w))
            };

            if let YoutubeDlOutput::Playlist(playlist) = output {
                if let Some(entries) = playlist.entries {
                    for entry in entries {
                        if !is_channel_match(entry.uploader.as_deref(), entry.channel.as_deref()) {
                            continue;
                        }

                        let id = entry.id;
                        let title = entry.title.unwrap_or_else(|| "Unknown".into());
                        if !is_title_relevant(&title) {
                            continue;
                        }

                        results.push(AnimeSearchResult {
                            provider: self.name().to_string(),
                            id: id.clone(),
                            title,
                            poster_url: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
                        });
                    }
                }
            } else if let YoutubeDlOutput::SingleVideo(video) = output
                && is_channel_match(video.uploader.as_deref(), video.channel.as_deref())
            {
                let title = video.title.unwrap_or_else(|| "Unknown".into());
                if is_title_relevant(&title) {
                    results.push(AnimeSearchResult {
                        provider: self.name().to_string(),
                        id: video.id.clone(),
                        title,
                        poster_url: Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video.id)),
                    });
                }
            }

            Ok(results)
        })
    }

    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            Ok(vec![Episode {
                id: anime_id.to_string(),
                number: 1.0,
                title: Some("Full Episode".into()),
            }])
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let video_url = format!("https://www.youtube.com/watch?v={episode_id}");

            let output = tokio::task::spawn_blocking(move || {
                YoutubeDl::new(&video_url)
                    .format("best[ext=mp4]/best")
                    .run()
            })
            .await
            .map_err(|e| ScraperError::Network(e.to_string()))?
            .map_err(|e| ScraperError::Network(e.to_string()))?;

            let stream_url = match output {
                YoutubeDlOutput::SingleVideo(v) => v.url.unwrap_or_default(),
                YoutubeDlOutput::Playlist(p) => {
                    p.entries
                        .and_then(|mut e| e.pop())
                        .and_then(|v| v.url)
                        .unwrap_or_default()
                }
            };

            if stream_url.is_empty() {
                return Err(ScraperError::NotFound("Stream URL not found".into()));
            }

            Ok(StreamSource {
                provider: self.name().to_string(),
                audio_type: audio,
                language: self.default_language.to_string(),
                streams: vec![VideoStream {
                    quality: "1080p".to_string(),
                    url: stream_url,
                }],
                subtitles: Vec::new(),
                referer: "https://www.youtube.com/".to_string(),
            })
        })
    }
}
