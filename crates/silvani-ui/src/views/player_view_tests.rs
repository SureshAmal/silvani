use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::{Root, input::InputState, slider::SliderState};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window, px, size,
};
use silvani_meta::{AnimeItem, ImageCacheManager};
use silvani_player::{MpvController, PlaybackStatus, PlayerController};
use silvani_scraper::Episode;

use super::EmbeddedPlayerView;
use crate::player_chrome::PlayerChromeState;

struct ChromeHarness {
    chrome: PlayerChromeState,
    seek: Entity<SliderState>,
    volume: Entity<SliderState>,
    image_cache: Arc<ImageCacheManager>,
    search: Entity<InputState>,
}

impl Render for ChromeHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        EmbeddedPlayerView::new(
            AnimeItem::default(),
            1.0,
            "Episode 1",
            "test",
            "sub",
            "auto",
            PlaybackStatus {
                duration_secs: 600.0,
                paused: true,
                ..Default::default()
            },
            Some(PlayerController::Socket(MpvController::new(
                "/tmp/silvani-chrome-unused".into(),
            ))),
            None,
            false,
            false,
            self.chrome.is_visible(),
            None,
            vec![
                Episode {
                    id: "episode-1".into(),
                    number: 1.0,
                    title: Some("A Captain and His Crew with a Very Long Episode Name".into()),
                },
                Episode {
                    id: "episode-2".into(),
                    number: 2.0,
                    title: Some("The Voyage".into()),
                },
            ],
            self.image_cache.clone(),
            self.seek.clone(),
            self.volume.clone(),
        )
        .playlist_search(self.search.clone())
        .on_mouse_move(cx.listener(|this, _, _, cx| {
            this.chrome.activity(Instant::now());
            cx.notify();
        }))
        .on_player_hover(cx.listener(|this, inside, _, cx| {
            this.chrome.set_pointer_inside(*inside);
            if *inside {
                this.chrome.activity(Instant::now());
            }
            cx.notify();
        }))
    }
}

#[gpui_kit::test]
fn idle_player_hides_details_and_transport_without_hiding_outside_cursor(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut harness = None;
    let handle = cx.open_window(size(px(1200.0), px(700.0)), |window, cx| {
        let view = cx.new(|cx| ChromeHarness {
            chrome: PlayerChromeState::new(Instant::now()),
            seek: cx.new(|_| SliderState::new().min(0.0).max(600.0)),
            volume: cx.new(|_| SliderState::new().min(0.0).max(100.0)),
            image_cache: Arc::new(ImageCacheManager::new()),
            search: cx.new(|cx| {
                InputState::new(window, cx).placeholder("Search episode number or title…")
            }),
        });
        harness = Some(view.clone());
        Root::new(view, window, cx)
    });
    let harness = harness.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("video-viewport").bounds();
        assert!(window.try_find("player-back-btn").is_some());
        assert!(window.try_find("play-pause").is_some());
        window.hover("side-ep-1", cx);
        harness.update(cx, |view, cx| {
            view.chrome
                .expire(Instant::now() + Duration::from_secs(3), false);
            assert!(!view.chrome.should_hide_cursor());
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("player-back-btn").is_none());
        assert!(window.try_find("play-pause").is_none());
        assert!(window.try_find("subtitle-menu").is_none());
        assert_eq!(window.find("video-viewport").bounds(), bounds);
        assert!(window.find("side-ep-1").visible());
        window.hover("video-canvas", cx);
        window.render_frame(cx);
        assert!(window.try_find("play-pause").is_some());
        assert!(harness.read(cx).chrome.is_visible());
        harness.update(cx, |view, cx| {
            view.chrome
                .expire(Instant::now() + Duration::from_secs(3), false);
            assert!(view.chrome.should_hide_cursor());
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("play-pause").is_none());
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("playlist-search", cx);
        window.input("voyage", cx);
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("side-ep-1").is_none());
        assert_eq!(window.find("side-ep-2").label(), Some("2 · The Voyage"));
    })
    .unwrap();
}

#[test]
fn episode_names_and_search_handle_missing_titles_and_fractional_numbers() {
    let episode = Episode {
        id: "special".into(),
        number: 12.5,
        title: Some("The Voyage".into()),
    };
    assert_eq!(super::episode_label(&episode), "12.5 · The Voyage");
    assert!(super::episode_matches(&episode, " VOYAGE "));
    assert!(super::episode_matches(&episode, "12.5"));
    assert!(!super::episode_matches(&episode, "13"));
    assert_eq!(
        super::episode_label(&Episode {
            title: Some("Episode 12.5".into()),
            ..episode
        }),
        "Episode 12.5"
    );
}
