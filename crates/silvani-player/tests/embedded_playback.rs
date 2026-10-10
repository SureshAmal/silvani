//! Local smoke test: cargo test -p silvani-player --test embedded_playback -- --ignored
use silvani_player::{EmbeddedVideoEngine, PlaybackStatus};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn wait_for(
    handle: &silvani_player::EmbeddedPlayerHandle,
    predicate: impl Fn(&PlaybackStatus) -> bool,
) -> PlaybackStatus {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let status = handle.get_status();
        if predicate(&status) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for playback state: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "requires system libmpv, ffmpeg, and an audio output"]
fn embedded_player_selects_tracks_and_seeks_a_local_video() {
    let dir = std::env::temp_dir().join(format!("silvani-player-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let subtitles = dir.join("captions.srt");
    std::fs::write(
        &subtitles,
        "1\n00:00:00,000 --> 00:00:19,000\nTest captions\n",
    )
    .unwrap();
    let video = dir.join("fixture.mkv");
    let output = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:r=24",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=48000:cl=mono",
            "-i",
        ])
        .arg(&subtitles)
        .args([
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-map",
            "1:a",
            "-map",
            "2:s",
            "-t",
            "20",
            "-c:v",
            "mpeg4",
            "-c:a",
            "aac",
            "-c:s",
            "srt",
            "-metadata:s:a:0",
            "language=jpn",
            "-metadata:s:a:1",
            "language=eng",
            "-metadata:s:s:0",
            "language=eng",
        ])
        .arg(&video)
        .output()
        .expect("ffmpeg must be installed");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let handle = EmbeddedVideoEngine::start_playback(
        video.to_str().unwrap(),
        None,
        &[],
        None,
        Arc::new(|_| {}),
    )
    .unwrap();
    let status = wait_for(&handle, |s| {
        s.duration_secs > 0.0 && s.audio_tracks.len() == 2 && s.subtitle_tracks.len() == 1
    });
    let subtitle = status.subtitle_tracks[0].id().to_owned();
    let english = status
        .audio_tracks
        .iter()
        .find(|t| t.label() == "eng")
        .unwrap()
        .id()
        .to_owned();
    handle.set_subtitle_track("no").unwrap();
    wait_for(&handle, |s| !s.subtitles_visible);
    handle.set_subtitle_track(&subtitle).unwrap();
    wait_for(&handle, |s| {
        s.subtitles_visible
            && s.subtitle_tracks
                .iter()
                .any(|t| t.id() == subtitle && t.is_selected())
    });
    handle.set_audio_track(&english).unwrap();
    wait_for(&handle, |s| {
        s.audio_tracks
            .iter()
            .any(|t| t.id() == english && t.is_selected())
    });
    handle.pause().unwrap();
    wait_for(&handle, |s| s.paused);
    handle.seek_absolute(10.0).unwrap();
    wait_for(&handle, |s| {
        s.paused && (s.position_secs - 10.0).abs() < 0.5
    });
    handle.quit().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
