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
    Started(MpvController),
    Progress(f32),
    StatusUpdate(PlaybackStatus),
    Finished,
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct PlaybackStatus {
    pub position_secs: f64,
    pub duration_secs: f64,
    pub paused: bool,
    pub volume: f64,
    pub sub_track: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MpvController {
    pub socket_path: PathBuf,
}

impl MpvController {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn send_command(&self, cmd: serde_json::Value) -> Result<serde_json::Value, String> {
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_millis(300)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_millis(300)))
            .map_err(|e| e.to_string())?;

        let mut req_str = serde_json::to_string(&cmd).map_err(|e| e.to_string())?;
        req_str.push('\n');

        stream.write_all(req_str.as_bytes()).map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;

        serde_json::from_str(&line).map_err(|e| e.to_string())
    }

    pub fn play(&self) -> Result<(), String> {
        self.send_command(serde_json::json!({ "command": ["set_property", "pause", false] }))
            .map(|_| ())
    }

    pub fn pause(&self) -> Result<(), String> {
        self.send_command(serde_json::json!({ "command": ["set_property", "pause", true] }))
            .map(|_| ())
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        self.send_command(serde_json::json!({ "command": ["cycle", "pause"] }))
            .map(|_| ())
    }

    pub fn seek_absolute(&self, secs: f64) -> Result<(), String> {
        self.send_command(serde_json::json!({ "command": ["seek", secs, "absolute"] }))
            .map(|_| ())
    }

    pub fn seek_relative(&self, secs: f64) -> Result<(), String> {
        self.send_command(serde_json::json!({ "command": ["seek", secs, "relative"] }))
            .map(|_| ())
    }

    pub fn set_volume(&self, vol: f64) -> Result<(), String> {
        let clean_vol = vol.clamp(0.0, 100.0);
        self.send_command(serde_json::json!({ "command": ["set_property", "volume", clean_vol] }))
            .map(|_| ())
    }

    pub fn set_subtitle_track(&self, track_id: &str) -> Result<(), String> {
        self.send_command(serde_json::json!({ "command": ["set_property", "sid", track_id] }))
            .map(|_| ())
    }

    pub fn get_status(&self) -> PlaybackStatus {
        let pos = self
            .send_command(serde_json::json!({ "command": ["get_property", "time-pos"] }))
            .ok()
            .and_then(|v| v["data"].as_f64())
            .unwrap_or(0.0);

        let dur = self
            .send_command(serde_json::json!({ "command": ["get_property", "duration"] }))
            .ok()
            .and_then(|v| v["data"].as_f64())
            .unwrap_or(0.0);

        let paused = self
            .send_command(serde_json::json!({ "command": ["get_property", "pause"] }))
            .ok()
            .and_then(|v| v["data"].as_bool())
            .unwrap_or(false);

        let vol = self
            .send_command(serde_json::json!({ "command": ["get_property", "volume"] }))
            .ok()
            .and_then(|v| v["data"].as_f64())
            .unwrap_or(100.0);

        PlaybackStatus {
            position_secs: pos,
            duration_secs: dur,
            paused,
            volume: vol,
            sub_track: None,
        }
    }
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
        let event_cb = Arc::clone(&on_event);
        event_cb(PlayerEvent::Opening);

        let event_cb_end = Arc::clone(&on_event);
        match MpvPlayer::play_stream_with_event_cb(title, source, resolution, on_event) {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                    event_cb_end(PlayerEvent::Finished);
                });
                Ok(())
            }
            Err(e) => {
                event_cb_end(player_error_to_event(&e));
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
        let event_cb = Arc::clone(&on_event);
        event_cb(PlayerEvent::Opening);

        let event_cb_end = Arc::clone(&on_event);
        match MpvPlayer::play_youtube_with_event_cb(title, video_id_or_url, resolution, on_event) {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                    event_cb_end(PlayerEvent::Finished);
                });
                Ok(())
            }
            Err(e) => {
                event_cb_end(player_error_to_event(&e));
                Err(e)
            }
        }
    }
}

fn player_error_to_event(e: &PlayerError) -> PlayerEvent {
    PlayerEvent::Error(e.to_string())
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

    pub fn play_stream_with_event_cb(
        title: &str,
        source: &StreamSource,
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
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

        Self::play_url_with_event_cb(
            title,
            &stream.url,
            Some(&source.referer),
            &source.subtitles,
            resolution,
            on_event,
        )
    }

    pub fn play_youtube_with_event_cb(
        title: &str,
        video_id_or_url: &str,
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<Child, PlayerError> {
        let url = if video_id_or_url.starts_with("http") {
            video_id_or_url.to_string()
        } else {
            format!("https://www.youtube.com/watch?v={}", video_id_or_url)
        };
        Self::play_url_with_event_cb(
            title,
            &url,
            Some("https://www.youtube.com/"),
            &[],
            resolution,
            on_event,
        )
    }

    pub fn play_url_with_event_cb(
        title: &str,
        url: &str,
        referer: Option<&str>,
        subtitles: &[SubtitleTrack],
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<Child, PlayerError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let socket_path = PathBuf::from(format!("/tmp/silvani_mpv_{}_{}.sock", std::process::id(), now));

        let mut cmd = Command::new("mpv");
        cmd.arg(url);
        cmd.arg(format!("--force-media-title={}", title));
        cmd.arg(format!("--input-ipc-server={}", socket_path.to_string_lossy()));
        cmd.arg("--hwdec=auto-safe");
        cmd.arg("--vo=gpu-next,gpu");
        cmd.arg("--gpu-api=auto");

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

        let s_path = socket_path.clone();
        let controller = MpvController::new(socket_path.clone());
        let event_cb = on_event;

        std::thread::spawn(move || {
            let mut stream = None;
            for _ in 0..120 {
                if let Ok(s) = UnixStream::connect(&s_path) {
                    stream = Some(s);
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }

            if stream.is_some() {
                event_cb(PlayerEvent::Started(controller.clone()));

                loop {
                    std::thread::sleep(Duration::from_millis(400));
                    let status = controller.get_status();
                    if status.duration_secs > 0.0 {
                        let pct = (status.position_secs / status.duration_secs * 100.0) as f32;
                        event_cb(PlayerEvent::Progress(pct));
                        event_cb(PlayerEvent::StatusUpdate(status));
                    } else if !controller.socket_path.exists() {
                        break;
                    }
                }
            }

            let _ = std::fs::remove_file(&s_path);
        });

        Ok(child)
    }

    pub fn play_url_with_callbacks<F, L>(
        title: &str,
        url: &str,
        referer: Option<&str>,
        subtitles: &[SubtitleTrack],
        resolution: Option<&str>,
        on_loaded: L,
        on_progress: F,
    ) -> Result<Child, PlayerError>
    where
        F: FnMut(f32) + Send + 'static,
        L: FnOnce() + Send + 'static,
    {
        let on_loaded_cell = std::sync::Mutex::new(Some(on_loaded));
        let on_progress_cell = std::sync::Mutex::new(on_progress);

        let event_cb = Arc::new(move |ev: PlayerEvent| match ev {
            PlayerEvent::Started(_) => {
                if let Ok(mut guard) = on_loaded_cell.lock()
                    && let Some(cb) = guard.take()
                {
                    cb();
                }
            }
            PlayerEvent::Progress(p) => {
                if let Ok(mut guard) = on_progress_cell.lock() {
                    (guard)(p);
                }
            }
            _ => {}
        });

        Self::play_url_with_event_cb(title, url, referer, subtitles, resolution, event_cb)
    }
}
