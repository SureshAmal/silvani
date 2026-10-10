use gpui_kit::component::{
    Root,
    slider::{SliderEvent, SliderState},
};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Render, Subscription, TestAppContext, Window, point,
    px, size,
};
use silvani_player::{MediaTrack, MpvController, PlaybackStatus, PlayerController};
use silvani_scraper::AudioType;

use super::video_player_controls::VideoPlayerControls;

struct TransportHarness {
    seek: Entity<SliderState>,
    volume: Entity<SliderState>,
    committed: Option<f32>,
    ready: bool,
    _subscriptions: Vec<Subscription>,
    audio: AudioType,
    menu_open: bool,
    quality: String,
}

impl Render for TransportHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Commands are never issued by this test; only the application's slider events are inspected.
        let controller = self.ready.then(|| {
            PlayerController::Socket(MpvController::new("/tmp/silvani-ui-test-unused".into()))
        });
        VideoPlayerControls::new(
            "transport",
            "",
            PlaybackStatus {
                duration_secs: 600.0,
                subtitle_tracks: vec![MediaTrack::new("7", "English").selected(true)],
                audio_tracks: vec![
                    MediaTrack::new("1", "Japanese").selected(true),
                    MediaTrack::new("2", "English"),
                ],
                ..Default::default()
            },
        )
        .controller(controller)
        .on_menu_open_change(Some(std::rc::Rc::new(cx.listener(|this, open, _, cx| {
            this.menu_open = *open;
            cx.notify();
        }))))
        .sliders(self.seek.clone(), self.volume.clone())
        .resolutions(
            self.quality.clone(),
            vec!["auto".into(), "720p".into()],
            Some(std::rc::Rc::new(cx.listener(
                |this, quality: &str, _, cx| {
                    this.quality = quality.to_string();
                    cx.notify();
                },
            ))),
        )
        .audio_options(
            self.audio,
            vec![AudioType::Sub, AudioType::Dub],
            Some(std::rc::Rc::new(cx.listener(|this, audio, _, cx| {
                this.audio = *audio;
                cx.notify();
            }))),
        )
    }
}

#[gpui_kit::test]
fn timeline_click_commits_seconds_and_disabled_controls_reject_input(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut harness = None;
    let handle = cx.open_window(size(px(640.0), px(240.0)), |window, cx| {
        let view = cx.new(|cx| {
            let seek = cx.new(|_| SliderState::new().min(0.0).max(600.0).step(0.1));
            let volume = cx.new(|_| SliderState::new().min(0.0).max(100.0).default_value(100.0));
            let subscription = cx.subscribe_in(
                &seek,
                window,
                |this: &mut TransportHarness, _, event, _, cx| {
                    if let SliderEvent::Release(value) = event {
                        this.committed = Some(value.end());
                    }
                    cx.notify();
                },
            );
            TransportHarness {
                seek,
                volume,
                committed: None,
                ready: true,
                _subscriptions: vec![subscription],
                audio: AudioType::Sub,
                menu_open: false,
                quality: "auto".into(),
            }
        });
        harness = Some(view.clone());
        Root::new(view, window, cx)
    });
    let harness = harness.unwrap();
    let seek_id = harness.read_with(cx, |view, _| ("slider", view.seek.entity_id()));
    let volume_id = harness.read_with(cx, |view, _| ("slider", view.volume.entity_id()));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find(seek_id).bounds();
        assert!(bounds.size.width > px(500.0));
        let volume_width = window.find(volume_id).bounds().size.width;
        assert!(
            volume_width >= px(80.0) && volume_width <= px(112.0),
            "volume must remain compact: {volume_width:?}"
        );
        window.click_at(
            seek_id,
            point(bounds.size.width * 0.25, bounds.size.height * 0.5),
            cx,
        );
    })
    .unwrap();
    cx.update(|cx| {
        let committed = harness
            .read(cx)
            .committed
            .expect("track click must commit a seek");
        assert!(
            (committed - 150.0).abs() < 2.0,
            "seek is in seconds: {committed}"
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find(seek_id).bounds();
        window.drag(
            point(bounds.left() + bounds.size.width * 0.25, bounds.center().y),
            point(bounds.left() + bounds.size.width * 0.5, bounds.center().y),
            cx,
        );
    })
    .unwrap();
    cx.update(|cx| {
        let committed = harness.read(cx).committed.expect("drag must commit a seek");
        assert!(
            (committed - 300.0).abs() < 2.0,
            "dragged seek is in seconds: {committed}"
        );
        harness.update(cx, |view, cx| {
            view.ready = false;
            view.committed = None;
            cx.notify();
        });
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find(seek_id).bounds();
        window.click_at(
            seek_id,
            point(bounds.size.width * 0.75, bounds.size.height * 0.5),
            cx,
        );
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(harness.read(cx).committed, None);
    });
    cx.update(|cx| {
        harness.update(cx, |view, cx| {
            view.ready = true;
            cx.notify();
        });
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("subtitle-menu", cx);
        window.render_frame(cx);
        assert!(harness.read(cx).menu_open);
        assert_eq!(
            window.within("popup-menu").find(0usize).label(),
            Some("Off")
        );
        assert_eq!(
            window.within("popup-menu").find(1usize).label(),
            Some("English")
        );
        window.press("escape", cx);
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("audio-menu", cx);
        window.render_frame(cx);
        assert!(harness.read(cx).menu_open);
        assert_eq!(
            window.within("popup-menu").find(1usize).label(),
            Some("Japanese")
        );
        assert_eq!(
            window.within("popup-menu").find(2usize).label(),
            Some("English")
        );
        assert_eq!(
            window.within("popup-menu").find(6usize).label(),
            Some("Dub")
        );
        window.within("popup-menu").click(6usize, cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(harness.read(cx).audio, AudioType::Dub);
        assert!(!harness.read(cx).menu_open);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("resolution-menu", cx);
        window.render_frame(cx);
        assert!(harness.read(cx).menu_open);
        assert_eq!(
            window.within("popup-menu").find(1usize).label(),
            Some("720p")
        );
        window.within("popup-menu").click(1usize, cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(harness.read(cx).quality, "720p");
        assert!(!harness.read(cx).menu_open);
    });
}
