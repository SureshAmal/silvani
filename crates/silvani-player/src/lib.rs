use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use silvani_scraper::models::{StreamSource, SubtitleTrack};

pub mod embedded;
pub use embedded::{EmbeddedPlayerHandle, EmbeddedVideoEngine, VideoFrame};

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Opening,
    Started(MpvController),
    EmbeddedStarted(EmbeddedPlayerHandle),
    FrameReady(VideoFrame),
    Progress(f32),
    StatusUpdate(PlaybackStatus),
    Finished,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrack {
    id: String,
    label: String,
    selected: bool,
}

impl MediaTrack {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn is_selected(&self) -> bool {
        self.selected
    }
}

/// Read MPV's actual track inventory, including externally attached subtitles.
pub(crate) fn read_tracks(get: impl Fn(&str) -> Option<String>, kind: &str) -> Vec<MediaTrack> {
    let count = get("track-list/count")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    (0..count)
        .filter_map(|ix| {
            let key = |field: &str| format!("track-list/{ix}/{field}");
            if get(&key("type")).as_deref() != Some(kind) {
                return None;
            }
            let id = get(&key("id"))?;
            let title = get(&key("title")).filter(|s| !s.is_empty());
            let language = get(&key("lang")).filter(|s| !s.is_empty());
            let label = match (title, language) {
                (Some(t), Some(l)) => format!("{t} ({l})"),
                (Some(t), None) => t,
                (None, Some(l)) => l,
                _ => format!("Track {id}"),
            };
            Some(MediaTrack {
                id,
                label,
                selected: get(&key("selected")).is_some_and(|v| v == "yes"),
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct PlaybackStatus {
    pub position_secs: f64,
    pub duration_secs: f64,
    pub paused: bool,
    pub volume: f64,
    pub sub_track: Option<String>,
    pub is_buffering: bool,
    pub subtitles_visible: bool,
    pub subtitle_tracks: Vec<MediaTrack>,
    pub audio_tracks: Vec<MediaTrack>,
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        Self {
            position_secs: 0.0,
            duration_secs: 0.0,
            paused: false,
            volume: 100.0,
            sub_track: None,
            is_buffering: false,
            subtitles_visible: true,
            subtitle_tracks: Vec::new(),
            audio_tracks: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PlayerController {
    Socket(MpvController),
    Embedded(EmbeddedPlayerHandle),
}

impl PlayerController {
    pub fn play(&self) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.play(),
            Self::Embedded(h) => h.play(),
        }
    }

    pub fn pause(&self) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.pause(),
            Self::Embedded(h) => h.pause(),
        }
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.toggle_pause(),
            Self::Embedded(h) => h.toggle_pause(),
        }
    }

    pub fn seek_absolute(&self, secs: f64) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.seek_absolute(secs),
            Self::Embedded(h) => h.seek_absolute(secs),
        }
    }

    pub fn seek_relative(&self, secs: f64) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.seek_relative(secs),
            Self::Embedded(h) => h.seek_relative(secs),
        }
    }

    pub fn set_volume(&self, vol: f64) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.set_volume(vol),
            Self::Embedded(h) => h.set_volume(vol),
        }
    }

    pub fn get_status(&self) -> PlaybackStatus {
        match self {
            Self::Socket(c) => c.get_status(),
            Self::Embedded(h) => h.get_status(),
        }
    }

    pub fn set_subtitle_track(&self, sid: &str) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.set_subtitle_track(sid),
            Self::Embedded(h) => h.set_subtitle_track(sid),
        }
    }

    pub fn set_audio_track(&self, id: &str) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.send_command(serde_json::json!({ "command": ["set_property", "aid", id] })).map(|_| ()),
            Self::Embedded(h) => h.set_audio_track(id),
        }
    }

    pub fn toggle_subtitles(&self) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.send_command(serde_json::json!({"command": ["cycle", "sub-visibility"]})).map(|_| ()),
            Self::Embedded(h) => h.toggle_subtitles(),
        }
    }

    pub fn next_subtitle(&self) -> Result<(), String> {
        match self {
            Self::Socket(c) => c.send_command(serde_json::json!({"command": ["cycle", "sid"]})).map(|_| ()),
            Self::Embedded(h) => h.next_subtitle(),
        }
    }

    pub fn quit(&self) -> Result<(), String> {
        match self {
            Self::Socket(c) => {
                let _ = c.send_command(serde_json::json!({"command": ["quit"]}));
                Ok(())
            }
            Self::Embedded(h) => h.quit(),
        }
    }
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

        let response: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if response["error"].as_str().is_some_and(|error| error != "success") {
            return Err(response["error"].as_str().unwrap_or("MPV command failed").to_string());
        }
        Ok(response)
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
        self.send_command(serde_json::json!({ "command": ["seek", secs, "absolute+exact"] }))
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
        self.send_command(serde_json::json!({ "command": ["set_property", "sid", track_id] }))?;
        self.send_command(serde_json::json!({ "command": ["set_property", "sub-visibility", track_id != "no"] }))
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

        let get = |prop: &str| self.send_command(serde_json::json!({"command": ["get_property", prop]}))
            .ok().and_then(|v| match &v["data"] {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Bool(b) => Some(if *b { "yes" } else { "no" }.into()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            });
        let inventory = self.send_command(serde_json::json!({"command": ["get_property", "track-list"]}))
            .ok().and_then(|v| v.get("data").cloned()).unwrap_or(serde_json::Value::Null);
        let track_property = |path: &str| {
            if path == "track-list/count" { return inventory.as_array().map(|a| a.len().to_string()); }
            let mut parts = path.strip_prefix("track-list/")?.split('/');
            let ix = parts.next()?.parse::<usize>().ok()?;
            let value = &inventory[ix][parts.next()?];
            match value {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Bool(b) => Some(if *b { "yes" } else { "no" }.into()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            }
        };
        PlaybackStatus {
            position_secs: pos,
            duration_secs: dur,
            paused,
            volume: vol,
            sub_track: get("sid"),
            is_buffering: get("paused-for-cache").as_deref() == Some("yes"),
            subtitles_visible: get("sub-visibility").as_deref() == Some("yes"),
            subtitle_tracks: read_tracks(track_property, "sub"),
            audio_tracks: read_tracks(track_property, "audio"),
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

        // Try in-process embedded video engine (strictly in-app, zero external windows)
        if let Ok(_handle) = EmbeddedVideoEngine::start_playback(
            &stream.url,
            Some(&source.referer),
            &source.subtitles,
            resolution,
            Arc::clone(&on_event),
        ) {
            return Ok(());
        }

        // Fallback to standalone spawn if libmpv failed to load
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

        let url = if video_id_or_url.starts_with("http") {
            video_id_or_url.to_string()
        } else {
            format!("https://www.youtube.com/watch?v={}", video_id_or_url)
        };

        if let Ok(_handle) = EmbeddedVideoEngine::start_playback(
            &url,
            Some("https://www.youtube.com/"),
            &[],
            resolution,
            Arc::clone(&on_event),
        ) {
            return Ok(());
        }

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
        cmd.arg("--no-border");
        cmd.arg("--no-osc");
        cmd.arg("--osd-level=0");
        cmd.arg("--keep-open=yes");

        cmd.arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");

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
            cmd.arg(format!("--http-header-fields=Referer: {},Origin: {}", ref_url, ref_url.trim_end_matches('/')));
            cmd.arg(format!("--ytdl-raw-options=referer={},add-header=Origin:{}", ref_url, ref_url.trim_end_matches('/')));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn track_inventory_preserves_mpv_ids_languages_and_selection() {
        let properties: HashMap<&str, &str> = [
            ("track-list/count", "4"),
            ("track-list/0/type", "video"),
            ("track-list/1/type", "audio"),
            ("track-list/1/id", "2"),
            ("track-list/1/lang", "eng"),
            ("track-list/1/selected", "yes"),
            ("track-list/2/type", "sub"),
            ("track-list/2/id", "7"),
            ("track-list/2/title", "English captions"),
            ("track-list/2/lang", "eng"),
            ("track-list/2/selected", "yes"),
            ("track-list/3/type", "sub"),
            ("track-list/3/id", "9"),
        ]
        .into();
        let get = |key: &str| properties.get(key).map(|v| v.to_string());
        let subtitles = read_tracks(get, "sub");
        assert_eq!(subtitles.len(), 2);
        assert_eq!(
            subtitles[0],
            MediaTrack {
                id: "7".into(),
                label: "English captions (eng)".into(),
                selected: true
            }
        );
        assert_eq!(subtitles[1].label, "Track 9");
        assert!(!subtitles[1].selected);
        assert_eq!(read_tracks(get, "audio")[0].label, "eng");
        assert!(read_tracks(|_| None, "sub").is_empty());
    }

    #[test]
    fn ipc_subtitle_selection_restores_visibility_and_rejects_mpv_errors() {
        use std::os::unix::net::UnixListener;
        let path =
            std::env::temp_dir().join(format!("silvani-ipc-test-{}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let worker = std::thread::spawn(move || {
            let expected = [
                serde_json::json!({"command": ["set_property", "sid", "7"]}),
                serde_json::json!({"command": ["set_property", "sub-visibility", true]}),
                serde_json::json!({"command": ["set_property", "sid", "no"]}),
                serde_json::json!({"command": ["set_property", "sub-visibility", false]}),
                serde_json::json!({"command": ["set_property", "aid", "99"]}),
            ];
            for (ix, command) in expected.iter().enumerate() {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = String::new();
                BufReader::new(&socket).read_line(&mut request).unwrap();
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&request).unwrap(),
                    *command
                );
                let error = if ix == 4 {
                    "property unavailable"
                } else {
                    "success"
                };
                writeln!(socket, "{}", serde_json::json!({"error": error})).unwrap();
            }
        });
        let controller = PlayerController::Socket(MpvController::new(path.clone()));
        controller.set_subtitle_track("7").unwrap();
        controller.set_subtitle_track("no").unwrap();
        assert_eq!(
            controller.set_audio_track("99").unwrap_err(),
            "property unavailable"
        );
        worker.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
