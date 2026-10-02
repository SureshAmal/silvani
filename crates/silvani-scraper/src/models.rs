use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioType {
    Sub,
    Dub,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    pub language: String,
    pub url: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStream {
    pub quality: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeSearchResult {
    pub provider: String,
    pub id: String,
    pub title: String,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub number: f32,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSource {
    pub provider: String,
    pub audio_type: AudioType,
    pub language: String,
    pub streams: Vec<VideoStream>,
    pub subtitles: Vec<SubtitleTrack>,
    pub referer: String,
}
