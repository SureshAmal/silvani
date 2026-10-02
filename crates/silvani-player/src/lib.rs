use std::process::{Child, Command};
use silvani_scraper::models::{StreamSource, SubtitleTrack};

#[derive(Debug)]
pub enum PlayerError {
    NoStreams,
    ProcessSpawn(std::io::Error),
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerError::NoStreams => write!(f, "No playable video streams available"),
            PlayerError::ProcessSpawn(e) => write!(f, "Failed to launch mpv: {}", e),
        }
    }
}

impl std::error::Error for PlayerError {}

pub struct MpvPlayer;

impl MpvPlayer {
    pub fn play_stream(
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
    ) -> Result<Child, PlayerError> {
        let stream = if let Some(res) = resolution {
            if res.eq_ignore_ascii_case("auto") {
                source.streams.first()
            } else {
                source
                    .streams
                    .iter()
                    .find(|s| s.quality.to_lowercase().contains(&res.to_lowercase()))
                    .or_else(|| source.streams.first())
            }
        } else {
            source.streams.first()
        }
        .ok_or(PlayerError::NoStreams)?;

        Self::play_url(
            title,
            &stream.url,
            Some(&source.referer),
            &source.subtitles,
            resolution,
        )
    }

    pub fn play_url(
        title: &str,
        url: &str,
        referer: Option<&str>,
        subtitles: &[SubtitleTrack],
        resolution: Option<&str>,
    ) -> Result<Child, PlayerError> {
        let mut cmd = Command::new("mpv");
        cmd.arg(url);
        cmd.arg(format!("--force-media-title={}", title));

        if let Some(res) = resolution
            && !res.eq_ignore_ascii_case("auto")
        {
            let clean_res = res.trim_end_matches('p');
            if let Ok(height) = clean_res.parse::<u32>() {
                cmd.arg(format!(
                    "--ytdl-format=bestvideo[height<={height}]+bestaudio/best[height<={height}]"
                ));
            }
        }

        if let Some(ref_url) = referer
            && !ref_url.is_empty()
        {
            cmd.arg(format!("--referrer={}", ref_url));
        }

        for sub in subtitles {
            if !sub.url.is_empty() {
                cmd.arg(format!("--sub-file={}", sub.url));
            }
        }

        cmd.spawn().map_err(PlayerError::ProcessSpawn)
    }

    pub fn play_youtube(
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
    ) -> Result<Child, PlayerError> {
        let url = if video_id_or_url.starts_with("http") {
            video_id_or_url.to_string()
        } else {
            format!("https://www.youtube.com/watch?v={}", video_id_or_url)
        };
        Self::play_url(title, &url, Some("https://www.youtube.com/"), &[], resolution)
    }
}
