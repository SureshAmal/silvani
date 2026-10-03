use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use silvani_scraper::models::{StreamSource, SubtitleTrack};

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Opening,
    Started,
    Progress(f32),
    Finished,
    Error(String),
}

#[derive(Debug)]
pub enum PlayerError {
    NoStreams,
    ProcessSpawn(std::io::Error),
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerError::NoStreams => write!(f, "No playable video streams available"),
            PlayerError::ProcessSpawn(e) => write!(f, "Failed to launch player: {}", e),
        }
    }
}

impl std::error::Error for PlayerError {}

/// High-level media player abstraction allowing swapping MPV for other backends in the future.
pub trait MediaPlayer: Send + Sync {
    fn play_stream(
        &self,
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<(), PlayerError>;

    fn play_youtube(
        &self,
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<(), PlayerError>;
}

pub struct MpvPlayer;

impl MediaPlayer for MpvPlayer {
    fn play_stream(
        &self,
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<(), PlayerError> {
        let event_cb = on_event;
        event_cb(PlayerEvent::Opening);

        let event_cb_loaded = Arc::clone(&event_cb);
        let on_loaded = move || {
            event_cb_loaded(PlayerEvent::Started);
        };

        let event_cb_prog = Arc::clone(&event_cb);
        let on_progress = move |pct: f32| {
            event_cb_prog(PlayerEvent::Progress(pct));
        };

        let event_cb_end = Arc::clone(&event_cb);
        match MpvPlayer::play_stream_with_callbacks(title, source, resolution, on_loaded, on_progress) {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                    event_cb_end(PlayerEvent::Finished);
                });
                Ok(())
            }
            Err(e) => {
                event_cb_end(PlayerEvent::Error(e.to_string()));
                Err(e)
            }
        }
    }

    fn play_youtube(
        &self,
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<(), PlayerError> {
        let event_cb = on_event;
        event_cb(PlayerEvent::Opening);

        let event_cb_loaded = Arc::clone(&event_cb);
        let on_loaded = move || {
            event_cb_loaded(PlayerEvent::Started);
        };

        let event_cb_prog = Arc::clone(&event_cb);
        let on_progress = move |pct: f32| {
            event_cb_prog(PlayerEvent::Progress(pct));
        };

        let event_cb_end = Arc::clone(&event_cb);
        match MpvPlayer::play_youtube_with_callbacks(title, video_id_or_url, resolution, on_loaded, on_progress) {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                    event_cb_end(PlayerEvent::Finished);
                });
                Ok(())
            }
            Err(e) => {
                event_cb_end(PlayerEvent::Error(e.to_string()));
                Err(e)
            }
        }
    }
}

impl MpvPlayer {
    pub fn play_stream(
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
    ) -> Result<Child, PlayerError> {
        Self::play_stream_with_progress(title, source, resolution, |_| {})
    }

    pub fn play_stream_with_progress<F>(
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
        on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
    {
        Self::play_stream_with_callbacks(title, source, resolution, || {}, on_progress)
    }

    pub fn play_stream_with_callbacks<F, L>(
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
        on_loaded: L,
        on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
        L: FnOnce() + Send + 'static,
    {
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

        Self::play_url_with_callbacks(
            title,
            &stream.url,
            Some(&source.referer),
            &source.subtitles,
            resolution,
            on_loaded,
            on_progress,
        )
    }

    pub fn play_url(
        title: &str,
        url: &str,
        referer: Option<&str>,
        subtitles: &[SubtitleTrack],
        resolution: Option<&str>,
    ) -> Result<Child, PlayerError> {
        Self::play_url_with_callbacks(title, url, referer, subtitles, resolution, || {}, |_| {})
    }

    pub fn play_url_with_progress<F>(
        title: &str,
        url: &str,
        referer: Option<&str>,
        subtitles: &[SubtitleTrack],
        resolution: Option<&str>,
        on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
    {
        Self::play_url_with_callbacks(title, url, referer, subtitles, resolution, || {}, on_progress)
    }

    pub fn play_url_with_callbacks<F, L>(
        title: &str,
        url: &str,
        referer: Option<&str>,
        subtitles: &[SubtitleTrack],
        resolution: Option<&str>,
        on_loaded: L,
        mut on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
        L: FnOnce() + Send + 'static,
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let socket_path = PathBuf::from(format!("/tmp/silvani_mpv_{}_{}.sock", std::process::id(), now));

        let mut cmd = Command::new("mpv");
        cmd.arg(url);
        cmd.arg(format!("--force-media-title={}", title));
        cmd.arg(format!("--input-ipc-server={}", socket_path.to_string_lossy()));

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

        if let Some(ref_url) = referer {
            cmd.arg(format!("--referrer={}", ref_url));
            cmd.arg(format!("--http-header-fields=Referer: {}", ref_url));
        }

        for sub in subtitles {
            cmd.arg(format!("--sub-file={}", sub.url));
        }

        let child = cmd.spawn().map_err(PlayerError::ProcessSpawn)?;

        // Fast poll connection to IPC socket without sleeping first
        let s_path = socket_path.clone();
        let mut on_loaded_opt = Some(on_loaded);
        std::thread::spawn(move || {
            let mut stream = None;
            for _ in 0..120 {
                if let Ok(s) = UnixStream::connect(&s_path) {
                    stream = Some(s);
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }

            if let Some(mut stream) = stream {
                // IPC Connected: MPV window is opened and playback is initialized immediately
                if let Some(cb) = on_loaded_opt.take() {
                    cb();
                }

                let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(800)));
                let cloned = stream.try_clone();

                if let Ok(cloned_stream) = cloned {
                    let mut reader = BufReader::new(cloned_stream);
                    on_progress(0.0);
                    loop {
                        std::thread::sleep(Duration::from_millis(800));
                        let req = "{\"command\": [\"get_property\", \"percent-pos\"]}\n";
                        if stream.write_all(req.as_bytes()).is_err() {
                            break;
                        }

                        let mut line = String::new();
                        if reader.read_line(&mut line).is_ok()
                            && !line.is_empty()
                            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&line)
                            && let Some(pct) = v.get("data").and_then(|d| d.as_f64())
                        {
                            on_progress(pct as f32);
                        }
                    }
                }
            }
            let _ = std::fs::remove_file(&s_path);
        });

        Ok(child)
    }

    pub fn play_youtube(
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
    ) -> Result<Child, PlayerError> {
        Self::play_youtube_with_callbacks(title, video_id_or_url, resolution, || {}, |_| {})
    }

    pub fn play_youtube_with_progress<F>(
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
        on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
    {
        Self::play_youtube_with_callbacks(title, video_id_or_url, resolution, || {}, on_progress)
    }

    pub fn play_youtube_with_callbacks<F, L>(
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
        on_loaded: L,
        on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
        L: FnOnce() + Send + 'static,
    {
        let url = if video_id_or_url.starts_with("http") {
            video_id_or_url.to_string()
        } else {
            format!("https://www.youtube.com/watch?v={}", video_id_or_url)
        };
        Self::play_url_with_callbacks(
            title,
            &url,
            Some("https://www.youtube.com/"),
            &[],
            resolution,
            on_loaded,
            on_progress,
        )
    }
}
