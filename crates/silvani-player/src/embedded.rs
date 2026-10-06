use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use libloading::{Library, Symbol};
use crate::{PlaybackStatus, PlayerEvent, PlayerError};

pub const MPV_RENDER_PARAM_INVALID: c_int = 0;
pub const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
pub const MPV_RENDER_PARAM_SW_SIZE: c_int = 17;
pub const MPV_RENDER_PARAM_SW_FORMAT: c_int = 18;
pub const MPV_RENDER_PARAM_SW_STRIDE: c_int = 19;
pub const MPV_RENDER_PARAM_SW_POINTER: c_int = 20;

pub const MPV_EVENT_NONE: c_int = 0;
pub const MPV_EVENT_SHUTDOWN: c_int = 1;
pub const MPV_EVENT_FILE_LOADED: c_int = 8;
pub const MPV_EVENT_END_FILE: c_int = 7;

#[repr(C)]
pub struct MpvRenderParam {
    pub param_type: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct MpvEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

pub struct LibMpv {
    _lib: Arc<Library>,
    pub mpv_create: Symbol<'static, unsafe extern "C" fn() -> *mut c_void>,
    pub mpv_initialize: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> c_int>,
    pub mpv_terminate_destroy: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
    pub mpv_set_option_string: Symbol<'static, unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int>,
    pub mpv_set_property_string: Symbol<'static, unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int>,
    pub mpv_command: Symbol<'static, unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int>,
    pub mpv_wait_event: Symbol<'static, unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent>,
    pub mpv_get_property_string: Symbol<'static, unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char>,
    pub mpv_free: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
    pub mpv_render_context_create: Symbol<'static, unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut MpvRenderParam) -> c_int>,
    pub mpv_render_context_render: Symbol<'static, unsafe extern "C" fn(*mut c_void, *mut MpvRenderParam) -> c_int>,
    pub mpv_render_context_update: Symbol<'static, unsafe extern "C" fn(*mut c_void) -> u64>,
    pub mpv_render_context_free: Symbol<'static, unsafe extern "C" fn(*mut c_void)>,
}

// Safety: LibMpv symbols are immutable pointers into dynamically loaded shared library
unsafe impl Send for LibMpv {}
unsafe impl Sync for LibMpv {}

use std::sync::OnceLock;

static LIB_MPV_INSTANCE: OnceLock<Result<Arc<LibMpv>, String>> = OnceLock::new();

impl LibMpv {
    pub fn load() -> Result<Arc<Self>, String> {
        LIB_MPV_INSTANCE
            .get_or_init(|| {
                let candidate_paths = [
                    "libmpv.so.2",
                    "/usr/lib64/libmpv.so.2",
                    "/usr/lib/x86_64-linux-gnu/libmpv.so.2",
                    "libmpv.so",
                ];

                let mut loaded = None;
                for path in candidate_paths {
                    if let Ok(lib) = unsafe { Library::new(path) } {
                        loaded = Some(Arc::new(lib));
                        break;
                    }
                }

                let lib = match loaded {
                    Some(l) => l,
                    None => return Err("Failed to find libmpv.so.2 on system".to_string()),
                };

                unsafe {
                    let mpv_create = match lib.get::<unsafe extern "C" fn() -> *mut c_void>(b"mpv_create\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_initialize = match lib.get::<unsafe extern "C" fn(*mut c_void) -> c_int>(b"mpv_initialize\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_terminate_destroy = match lib.get::<unsafe extern "C" fn(*mut c_void)>(b"mpv_terminate_destroy\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_set_option_string = match lib.get::<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int>(b"mpv_set_option_string\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_set_property_string = match lib.get::<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int>(b"mpv_set_property_string\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_command = match lib.get::<unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int>(b"mpv_command\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_wait_event = match lib.get::<unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent>(b"mpv_wait_event\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_get_property_string = match lib.get::<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char>(b"mpv_get_property_string\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_free = match lib.get::<unsafe extern "C" fn(*mut c_void)>(b"mpv_free\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_render_context_create = match lib.get::<unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut MpvRenderParam) -> c_int>(b"mpv_render_context_create\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_render_context_render = match lib.get::<unsafe extern "C" fn(*mut c_void, *mut MpvRenderParam) -> c_int>(b"mpv_render_context_render\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_render_context_update = match lib.get::<unsafe extern "C" fn(*mut c_void) -> u64>(b"mpv_render_context_update\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };
                    let mpv_render_context_free = match lib.get::<unsafe extern "C" fn(*mut c_void)>(b"mpv_render_context_free\0") {
                        Ok(sym) => std::mem::transmute(sym),
                        Err(e) => return Err(e.to_string()),
                    };

                    Ok(Arc::new(Self {
                        _lib: lib,
                        mpv_create,
                        mpv_initialize,
                        mpv_terminate_destroy,
                        mpv_set_option_string,
                        mpv_set_property_string,
                        mpv_command,
                        mpv_wait_event,
                        mpv_get_property_string,
                        mpv_free,
                        mpv_render_context_create,
                        mpv_render_context_render,
                        mpv_render_context_update,
                        mpv_render_context_free,
                    }))
                }
            })
            .clone()
    }
}

#[derive(Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // Native BGRA format matching GPUI's RenderImage
}

impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("data_len", &self.data.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play,
    Pause,
    TogglePause,
    SeekAbsolute(f64),
    SeekRelative(f64),
    SetVolume(f64),
    SetSubtitleTrack(String),
    ToggleSubtitles,
    NextSubtitle,
    Quit,
}

/// Non-blocking, thread-safe controller for in-process libmpv instance.
/// All control actions (pause, seek, volume) send non-blocking channel messages
/// so the main UI thread never hangs or blocks regardless of network buffering.
#[derive(Clone)]
pub struct EmbeddedPlayerHandle {
    cmd_tx: std::sync::mpsc::Sender<PlayerCommand>,
    is_alive: Arc<AtomicBool>,
    status: Arc<RwLock<PlaybackStatus>>,
}

impl std::fmt::Debug for EmbeddedPlayerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedPlayerHandle")
            .field("is_alive", &self.is_alive.load(Ordering::Relaxed))
            .finish()
    }
}

impl EmbeddedPlayerHandle {
    pub fn play(&self) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::Play).map_err(|e| e.to_string())
    }

    pub fn pause(&self) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::Pause).map_err(|e| e.to_string())
    }

    pub fn toggle_pause(&self) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::TogglePause).map_err(|e| e.to_string())
    }

    pub fn seek_absolute(&self, secs: f64) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::SeekAbsolute(secs)).map_err(|e| e.to_string())
    }

    pub fn seek_relative(&self, secs: f64) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::SeekRelative(secs)).map_err(|e| e.to_string())
    }

    pub fn set_volume(&self, vol: f64) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::SetVolume(vol)).map_err(|e| e.to_string())
    }

    pub fn set_subtitle_track(&self, sid: &str) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::SetSubtitleTrack(sid.to_string())).map_err(|e| e.to_string())
    }

    pub fn toggle_subtitles(&self) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::ToggleSubtitles).map_err(|e| e.to_string())
    }

    pub fn next_subtitle(&self) -> Result<(), String> {
        self.cmd_tx.send(PlayerCommand::NextSubtitle).map_err(|e| e.to_string())
    }

    pub fn quit(&self) -> Result<(), String> {
        self.is_alive.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(PlayerCommand::Quit);
        Ok(())
    }

    /// Returns the cached playback status in O(1) time without any C-FFI blocking
    pub fn get_status(&self) -> PlaybackStatus {
        self.status.read().map(|s| s.clone()).unwrap_or_default()
    }
}

pub struct EmbeddedVideoEngine;

impl EmbeddedVideoEngine {
    pub fn start_playback(
        url: &str,
        referer: Option<&str>,
        subtitles: &[silvani_scraper::models::SubtitleTrack],
        resolution: Option<&str>,
        on_event: Arc<dyn Fn(PlayerEvent) + Send + Sync + 'static>,
    ) -> Result<EmbeddedPlayerHandle, PlayerError> {
        let lib = LibMpv::load().map_err(|e| PlayerError::ProcessSpawn(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;

        let mpv = unsafe { (lib.mpv_create)() };
        if mpv.is_null() {
            return Err(PlayerError::ProcessSpawn(std::io::Error::new(std::io::ErrorKind::Other, "Failed to create mpv instance")));
        }

        unsafe {
            // Force headless libmpv VO (NO external window popup!)
            let opt = |name: &str, val: &str| {
                let c_name = CString::new(name).unwrap();
                let c_val = CString::new(val).unwrap();
                (lib.mpv_set_option_string)(mpv, c_name.as_ptr(), c_val.as_ptr());
            };

            opt("vo", "libmpv");
            opt("ao", "pulse,pipewire,alsa,auto");
            opt("volume", "100");
            opt("mute", "no");
            opt("aid", "auto");
            opt("hwdec", "auto-safe");
            opt("video-sync", "audio");
            opt("demuxer-max-bytes", "150M");
            opt("demuxer-readahead-secs", "30");
            opt("cache", "yes");
            opt("cache-secs", "30");
            opt("cache-pause-initial", "no");
            opt("stream-buffer-size", "512k");
            opt("demuxer-lavf-o", "probesize=32768,analyzeduration=0");
            opt("hr-seek", "yes");
            opt("cursor-autohide", "2500");
            opt("cursor-autohide-fs-only", "no");
            opt("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");
            // Start paused so audio does NOT prematurely play while the video stream buffers
            opt("pause", "yes");

            if let Some(ref_url) = referer {
                opt("referrer", ref_url);
                let headers = format!("Referer: {},Origin: {}", ref_url, ref_url.trim_end_matches('/'));
                opt("http-header-fields", &headers);
                let ytdl_opt = format!("referer={},add-header=Origin:{}", ref_url, ref_url.trim_end_matches('/'));
                opt("ytdl-raw-options", &ytdl_opt);
            }

            if let Some(res) = resolution && !res.eq_ignore_ascii_case("auto") {
                let clean_res = res.trim_end_matches('p');
                if let Ok(h) = clean_res.parse::<u32>() {
                    let ytdl_fmt = format!("best[height<={h}]/bestvideo[height<={h}]+bestaudio/best");
                    opt("ytdl-format", &ytdl_fmt);
                }
            }

            if (lib.mpv_initialize)(mpv) < 0 {
                (lib.mpv_terminate_destroy)(mpv);
                return Err(PlayerError::ProcessSpawn(std::io::Error::new(std::io::ErrorKind::Other, "Failed to init mpv")));
            }
        }

        let is_alive = Arc::new(AtomicBool::new(true));
        let status = Arc::new(RwLock::new(PlaybackStatus::default()));
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<PlayerCommand>();

        let handle = EmbeddedPlayerHandle {
            cmd_tx,
            is_alive: Arc::clone(&is_alive),
            status: Arc::clone(&status),
        };

        let handle_for_worker = handle.clone();
        let url_owned = url.to_string();
        let subs_owned: Vec<silvani_scraper::models::SubtitleTrack> = subtitles.to_vec();
        let status_for_worker = Arc::clone(&status);

        // Immediately notify UI that embedded player handle is active
        let mpv_addr = mpv as usize;
        let lib_worker = Arc::clone(&lib);
        std::thread::spawn(move || {
            let mpv = mpv_addr as *mut c_void;
            let lib = lib_worker;

            // Create software render context for frame decoding
            let mut rd_ctx: *mut c_void = std::ptr::null_mut();
            let sw_type = CString::new("sw").unwrap();
            let mut init_params = [
                MpvRenderParam {
                    param_type: MPV_RENDER_PARAM_API_TYPE,
                    data: sw_type.as_ptr() as *mut c_void,
                },
                MpvRenderParam {
                    param_type: MPV_RENDER_PARAM_INVALID,
                    data: std::ptr::null_mut(),
                },
            ];

            let rd_res = unsafe {
                (lib.mpv_render_context_create)(&mut rd_ctx, mpv, init_params.as_mut_ptr())
            };

            if rd_res < 0 || rd_ctx.is_null() {
                on_event(PlayerEvent::Error("Failed to initialize libmpv render context".to_string()));
                unsafe { (lib.mpv_terminate_destroy)(mpv); }
                is_alive.store(false, Ordering::SeqCst);
                return;
            }

            // Load file & attach subtitle tracks
            {
                let c_cmd = CString::new("loadfile").unwrap();
                let c_url = CString::new(url_owned.as_str()).unwrap();
                let mut ptrs = [c_cmd.as_ptr(), c_url.as_ptr(), std::ptr::null()];
                unsafe { (lib.mpv_command)(mpv, ptrs.as_mut_ptr()); }

                for sub in &subs_owned {
                    if let (Ok(c_sadd), Ok(c_surl), Ok(c_flags)) = (
                        CString::new("sub-add"),
                        CString::new(sub.url.as_str()),
                        CString::new(if sub.is_default { "select" } else { "auto" }),
                    ) {
                        let mut sub_ptrs = [c_sadd.as_ptr(), c_surl.as_ptr(), c_flags.as_ptr(), std::ptr::null()];
                        unsafe { (lib.mpv_command)(mpv, sub_ptrs.as_mut_ptr()); }
                    }
                }
            }

            // Native BGRA buffer setup matching GPUI's RenderImage
            let width: i32 = 960;
            let height: i32 = 540;
            let mut stride: usize = (width as usize) * 4;
            let mut frame_buf = vec![0u8; stride * (height as usize)];
            let mut size = [width, height];
            let fmt = CString::new("bgra").unwrap();

            let mut has_started = false;
            let mut frames_rendered: u64 = 0;
            let mut last_status_tick = std::time::Instant::now();
            let mut last_log_tick = std::time::Instant::now();

            eprintln!("[silvani-player] playback thread started for URL: {}", url_owned);

            while is_alive.load(Ordering::SeqCst) {
                // 1. Process all pending UI commands without blocking the UI thread
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        PlayerCommand::Play => {
                            let c_p = CString::new("pause").unwrap();
                            let c_v = CString::new("no").unwrap();
                            unsafe { (lib.mpv_set_property_string)(mpv, c_p.as_ptr(), c_v.as_ptr()); }
                        }
                        PlayerCommand::Pause => {
                            let c_p = CString::new("pause").unwrap();
                            let c_v = CString::new("yes").unwrap();
                            unsafe { (lib.mpv_set_property_string)(mpv, c_p.as_ptr(), c_v.as_ptr()); }
                        }
                        PlayerCommand::TogglePause => {
                            let c_cmd = CString::new("cycle").unwrap();
                            let c_arg = CString::new("pause").unwrap();
                            let mut ptrs = [c_cmd.as_ptr(), c_arg.as_ptr(), std::ptr::null()];
                            unsafe { (lib.mpv_command)(mpv, ptrs.as_mut_ptr()); }
                        }
                        PlayerCommand::SeekAbsolute(secs) => {
                            let c_cmd = CString::new("seek").unwrap();
                            let c_arg1 = CString::new(secs.to_string()).unwrap();
                            let c_arg2 = CString::new("absolute").unwrap();
                            let mut ptrs = [c_cmd.as_ptr(), c_arg1.as_ptr(), c_arg2.as_ptr(), std::ptr::null()];
                            unsafe { (lib.mpv_command)(mpv, ptrs.as_mut_ptr()); }
                        }
                        PlayerCommand::SeekRelative(secs) => {
                            let c_cmd = CString::new("seek").unwrap();
                            let c_arg1 = CString::new(secs.to_string()).unwrap();
                            let c_arg2 = CString::new("relative").unwrap();
                            let mut ptrs = [c_cmd.as_ptr(), c_arg1.as_ptr(), c_arg2.as_ptr(), std::ptr::null()];
                            unsafe { (lib.mpv_command)(mpv, ptrs.as_mut_ptr()); }
                        }
                        PlayerCommand::SetVolume(vol) => {
                            let c_p = CString::new("volume").unwrap();
                            let c_v = CString::new(vol.clamp(0.0, 100.0).to_string()).unwrap();
                            unsafe { (lib.mpv_set_property_string)(mpv, c_p.as_ptr(), c_v.as_ptr()); }
                        }
                        PlayerCommand::SetSubtitleTrack(sid) => {
                            if let Ok(c_v) = CString::new(sid) {
                                let c_p = CString::new("sid").unwrap();
                                unsafe { (lib.mpv_set_property_string)(mpv, c_p.as_ptr(), c_v.as_ptr()); }
                            }
                        }
                        PlayerCommand::ToggleSubtitles => {
                            let c_cmd = CString::new("cycle").unwrap();
                            let c_arg = CString::new("sub-visibility").unwrap();
                            let mut ptrs = [c_cmd.as_ptr(), c_arg.as_ptr(), std::ptr::null()];
                            unsafe { (lib.mpv_command)(mpv, ptrs.as_mut_ptr()); }
                        }
                        PlayerCommand::NextSubtitle => {
                            let c_cmd = CString::new("cycle").unwrap();
                            let c_arg = CString::new("sub").unwrap();
                            let mut ptrs = [c_cmd.as_ptr(), c_arg.as_ptr(), std::ptr::null()];
                            unsafe { (lib.mpv_command)(mpv, ptrs.as_mut_ptr()); }
                        }
                        PlayerCommand::Quit => {
                            is_alive.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }

                if !is_alive.load(Ordering::SeqCst) {
                    break;
                }

                // 2. Poll MPV events
                let ev_ptr = unsafe { (lib.mpv_wait_event)(mpv, 0.0) };
                if !ev_ptr.is_null() {
                    let ev_id = unsafe { (*ev_ptr).event_id };
                    if ev_id == MPV_EVENT_FILE_LOADED {
                        eprintln!("[silvani-player] MPV_EVENT_FILE_LOADED received");
                        if !has_started {
                            has_started = true;
                            on_event(PlayerEvent::EmbeddedStarted(handle_for_worker.clone()));
                        }
                    } else if ev_id == MPV_EVENT_END_FILE || ev_id == MPV_EVENT_SHUTDOWN {
                        eprintln!("[silvani-player] MPV end of file / shutdown event: {}", ev_id);
                        break;
                    }
                }

                // 3. Render video frame if updated or pending first frame
                let flags = unsafe { (lib.mpv_render_context_update)(rd_ctx) };
                let need_render = (flags & 1) != 0 || (has_started && frames_rendered == 0);
                if need_render {
                    let mut render_params = [
                        MpvRenderParam {
                            param_type: MPV_RENDER_PARAM_SW_SIZE,
                            data: size.as_mut_ptr() as *mut c_void,
                        },
                        MpvRenderParam {
                            param_type: MPV_RENDER_PARAM_SW_FORMAT,
                            data: fmt.as_ptr() as *mut c_void,
                        },
                        MpvRenderParam {
                            param_type: MPV_RENDER_PARAM_SW_STRIDE,
                            data: &mut stride as *mut usize as *mut c_void,
                        },
                        MpvRenderParam {
                            param_type: MPV_RENDER_PARAM_SW_POINTER,
                            data: frame_buf.as_mut_ptr() as *mut c_void,
                        },
                        MpvRenderParam {
                            param_type: MPV_RENDER_PARAM_INVALID,
                            data: std::ptr::null_mut(),
                        },
                    ];

                    let render_res = unsafe {
                        (lib.mpv_render_context_render)(rd_ctx, render_params.as_mut_ptr())
                    };

                    if render_res >= 0 {
                        frames_rendered += 1;
                        if frames_rendered == 1 {
                            eprintln!("[silvani-player] First frame ready (size {}x{}), unpausing playback!", width, height);
                            // Unpause MPV so audio and video play simultaneously from frame 1
                            let c_pause = CString::new("pause").unwrap();
                            let c_no = CString::new("no").unwrap();
                            unsafe { (lib.mpv_set_property_string)(mpv, c_pause.as_ptr(), c_no.as_ptr()); }
                        } else if frames_rendered % 300 == 0 {
                            eprintln!("[silvani-player] Rendered frame #{} (size {}x{})", frames_rendered, width, height);
                        }
                        let frame_data = std::mem::replace(&mut frame_buf, vec![0u8; stride * (height as usize)]);
                        let video_frame = VideoFrame {
                            width: width as u32,
                            height: height as u32,
                            data: frame_data,
                        };
                        on_event(PlayerEvent::FrameReady(video_frame));
                    }
                }

                // 4. Update playback status cache in background thread
                if last_status_tick.elapsed() >= Duration::from_millis(100) {
                    last_status_tick = std::time::Instant::now();
                    let get_str = |prop: &str| -> Option<String> {
                        let c_prop = CString::new(prop).ok()?;
                        unsafe {
                            let ptr = (lib.mpv_get_property_string)(mpv, c_prop.as_ptr());
                            if ptr.is_null() {
                                None
                            } else {
                                let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                                (lib.mpv_free)(ptr as *mut c_void);
                                Some(s)
                            }
                        }
                    };

                    let pos = get_str("time-pos").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let dur = get_str("duration").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let paused = get_str("pause").map(|s| s == "yes").unwrap_or(false);
                    let vol = get_str("volume").and_then(|s| s.parse::<f64>().ok()).unwrap_or(100.0);
                    let cache_buffering = get_str("paused-for-cache").map(|s| s == "yes").unwrap_or(false);
                    let seeking = get_str("seeking").map(|s| s == "yes").unwrap_or(false);

                    let new_status = PlaybackStatus {
                        position_secs: pos,
                        duration_secs: dur,
                        paused,
                        volume: vol,
                        sub_track: None,
                        is_buffering: cache_buffering || seeking,
                    };

                    if let Ok(mut lock) = status_for_worker.write() {
                        *lock = new_status.clone();
                    }

                    if dur > 0.0 {
                        let pct = (pos / dur * 100.0) as f32;
                        on_event(PlayerEvent::Progress(pct));
                    }
                    if last_log_tick.elapsed() >= Duration::from_secs(2) {
                        last_log_tick = std::time::Instant::now();
                        eprintln!(
                            "[silvani-player] status: pos={:.1}s, dur={:.1}s, paused={}, buffering={}, frames={}",
                            pos, dur, paused, new_status.is_buffering, frames_rendered
                        );
                    }
                    on_event(PlayerEvent::StatusUpdate(new_status));
                }

                std::thread::sleep(Duration::from_millis(15));
            }

            // Cleanup
            unsafe {
                (lib.mpv_render_context_free)(rd_ctx);
                (lib.mpv_terminate_destroy)(mpv);
            }
            is_alive.store(false, Ordering::SeqCst);
            on_event(PlayerEvent::Finished);
        });

        Ok(handle)
    }
}
