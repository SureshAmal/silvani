use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum AudioType {
    #[default]
    Sub,
    Dub,
    Hindi,
}

impl AudioType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sub => "sub",
            Self::Dub => "dub",
            Self::Hindi => "hindi",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sub => "Sub",
            Self::Dub => "Dub",
            Self::Hindi => "Hindi Dub",
        }
    }
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
