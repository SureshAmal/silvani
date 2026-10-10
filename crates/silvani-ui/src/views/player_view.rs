use std::rc::Rc;
use std::sync::Arc;

use crate::components::VideoPlayerControls;
use crate::components::video_player_controls::{MenuOpenHandler, ResolutionHandler};
use gpui_kit::base::{StyledExt, TestSupportExt};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use silvani_scraper::AudioType;

use silvani_meta::image_cache::ImageCacheManager;
use silvani_meta::models::AnimeItem;
use silvani_player::{PlaybackStatus, PlayerController};
use silvani_scraper::models::Episode;

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
pub type EpChangeHandler = Option<Rc<dyn Fn(&f32, &mut Window, &mut App)>>;
type HoverHandler = Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>;
type ActivityHandler = Option<Rc<dyn Fn(&(), &mut Window, &mut App)>>;
type AudioChangeHandler = Option<Rc<dyn Fn(&AudioType, &mut Window, &mut App)>>;
pub type MouseMoveHandler = Option<Rc<dyn Fn(&MouseMoveEvent, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct EmbeddedPlayerView {
    anime_item: AnimeItem,
    ep_number: f32,
    ep_title: String,
    provider: String,
    audio: String,
    resolution: String,
    status: PlaybackStatus,
    controller: Option<PlayerController>,
    current_frame: Option<Arc<gpui::RenderImage>>,
    is_loading: bool,
    is_fullscreen: bool,
    is_hud_visible: bool,
    error: Option<String>,
    episodes: Vec<Episode>,
    image_cache: Arc<ImageCacheManager>,
    seek_slider: Entity<SliderState>,
    volume_slider: Entity<SliderState>,
    on_back: ClickHandler,
    on_switch_episode: EpChangeHandler,
    on_toggle_fullscreen: ClickHandler,
    on_mouse_move: MouseMoveHandler,
    on_retry: ClickHandler,
    audio_options: Vec<AudioType>,
    on_audio_change: AudioChangeHandler,
    on_player_hover: HoverHandler,
    on_activity: ActivityHandler,
    on_menu_open_change: MenuOpenHandler,
    playlist_search: Option<Entity<InputState>>,
    resolutions: Vec<String>,
    on_resolution: ResolutionHandler,
}

impl EmbeddedPlayerView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        anime_item: AnimeItem,
        ep_number: f32,
        ep_title: impl Into<String>,
        provider: impl Into<String>,
        audio: impl Into<String>,
        resolution: impl Into<String>,
        status: PlaybackStatus,
        controller: Option<PlayerController>,
        current_frame: Option<Arc<gpui::RenderImage>>,
        is_loading: bool,
        is_fullscreen: bool,
        is_hud_visible: bool,
        error: Option<String>,
        episodes: Vec<Episode>,
        image_cache: Arc<ImageCacheManager>,
        seek_slider: Entity<SliderState>,
        volume_slider: Entity<SliderState>,
    ) -> Self {
        Self {
            anime_item,
            ep_number,
            ep_title: ep_title.into(),
            provider: provider.into(),
            audio: audio.into(),
            resolution: resolution.into(),
            status,
            controller,
            current_frame,
            is_loading,
            is_fullscreen,
            is_hud_visible,
            error,
            episodes,
            image_cache,
            seek_slider,
            volume_slider,
            on_back: None,
            on_switch_episode: None,
            on_toggle_fullscreen: None,
            on_mouse_move: None,
            on_retry: None,
            audio_options: Vec::new(),
            on_audio_change: None,
            on_player_hover: None,
            on_activity: None,
            on_menu_open_change: None,
            playlist_search: None,
            resolutions: Vec::new(),
            on_resolution: None,
        }
    }

    pub fn on_retry(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    pub fn on_back(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }

    pub fn on_switch_episode(
        mut self,
        handler: impl Fn(&f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_switch_episode = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_fullscreen(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_fullscreen = Some(Rc::new(handler));
        self
    }

    pub fn on_mouse_move(
        mut self,
        handler: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_move = Some(Rc::new(handler));
        self
    }

    pub fn on_player_hover(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_player_hover = Some(Rc::new(handler));
        self
    }

    pub fn on_activity(mut self, handler: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.on_activity = Some(Rc::new(handler));
        self
    }

    pub fn on_menu_open_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_menu_open_change = Some(Rc::new(handler));
        self
    }

    pub fn playlist_search(mut self, input: Entity<InputState>) -> Self {
        self.playlist_search = Some(input);
        self
    }

    pub fn resolutions(
        mut self,
        options: Vec<String>,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.resolutions = options;
        self.on_resolution = Some(Rc::new(handler));
        self
    }

    pub fn audio_options(mut self, options: Vec<AudioType>) -> Self {
        self.audio_options = options;
        self
    }

    pub fn on_audio_change(
        mut self,
        handler: impl Fn(&AudioType, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_audio_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for EmbeddedPlayerView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let query = self
            .playlist_search
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let filtered: Vec<&Episode> = self
            .episodes
            .iter()
            .filter(|episode| episode_matches(episode, &query))
            .collect();
        let external = matches!(self.controller, Some(PlayerController::Socket(_)));
        let finished = !self.is_loading && self.controller.is_none() && self.error.is_none();
        let on_back = if self.is_fullscreen {
            self.on_toggle_fullscreen.clone()
        } else {
            self.on_back.clone()
        };
        let on_retry = self.on_retry.clone();
        let ep_cb = self.on_switch_episode.clone();
        let fs_canvas_cb = self.on_toggle_fullscreen.clone();

        let anime_title = self.anime_item.display_title().to_string();
        let current_ep_num = self.ep_number;

        // Check if previous or next episode exists
        let prev_ep = self
            .episodes
            .iter()
            .filter(|e| e.number < current_ep_num)
            .map(|e| e.number)
            .max_by(f32::total_cmp);
        let next_ep = self
            .episodes
            .iter()
            .filter(|e| e.number > current_ep_num)
            .map(|e| e.number)
            .min_by(f32::total_cmp);

        // Poster image fallback
        let poster_source: Option<ImageSource> =
            match self.image_cache.get_cached_path(&self.anime_item.id) {
                Some(p) if p.exists() => Some(ImageSource::from(p)),
                _ => self
                    .anime_item
                    .cover_image
                    .as_ref()
                    .map(|url| ImageSource::from(url.clone())),
            };

        v_flex()
            .id("embedded-player-view")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                // Main Area: Video Viewport (Flex 1) + Optional Episode Sidebar
                h_flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    // Video Viewport Canvas
                    .child(
                        v_flex()
                            .id("video-viewport")
                            .test_support()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .relative()
                            .bg(gpui::black())
                            .group("video-viewport")
                            .child(
                                // Visual Screen Canvas
                                div()
                                    .id("video-canvas")
                                    .test_support()
                                    .w_full()
                                    .flex_1()
                                    .min_h_0()
                                    .relative()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_click(move |ev, win, cx| {
                                        if ev.click_count() == 2 {
                                            if let Some(ref handler) = fs_canvas_cb {
                                                handler(ev, win, cx);
                                            }
                                        }
                                    })
                                    .when_some(self.current_frame.clone(), |d, frame| {
                                        d.child(
                                            img(ImageSource::Render(frame))
                                                .size_full()
                                                .object_fit(ObjectFit::Contain),
                                        )
                                    })
                                    .when(self.current_frame.is_none(), |d| {
                                        d.when_some(poster_source, |d2, src| {
                                            d2.child(
                                                img(src)
                                                    .size_full()
                                                    .object_fit(ObjectFit::Cover)
                                                    .opacity(0.25),
                                            )
                                        })
                                    })
                                    .when_some(self.error.clone(), |d, err_msg| {
                                        let retry_cb = on_retry.clone();
                                        d.child(
                                            v_flex()
                                                .absolute()
                                                .inset_0()
                                                .items_center()
                                                .justify_center()
                                                .gap_3()
                                                .bg(theme.background.opacity(0.95))
                                                .child(
                                                    Label::new(err_msg)
                                                        .text_sm()
                                                        .font_semibold()
                                                        .text_color(theme.foreground),
                                                )
                                                .child(
                                                    Button::new("player-retry-btn")
                                                        .primary()
                                                        .small()
                                                        .label("Retry")
                                                        .when_some(retry_cb, |b, handler| {
                                                            b.on_click(move |ev, win, cx| {
                                                                handler(ev, win, cx)
                                                            })
                                                        }),
                                                ),
                                        )
                                    })
                                    .when(
                                        self.error.is_none()
                                            && !finished
                                            && !external
                                            && (self.is_loading || self.current_frame.is_none()),
                                        |d| {
                                            let msg =
                                                format!("Loading {} stream...", self.provider);
                                            d.child(
                                                v_flex()
                                                    .absolute()
                                                    .inset_0()
                                                    .items_center()
                                                    .justify_center()
                                                    .gap_3()
                                                    .bg(theme.background.opacity(0.9))
                                                    .child(Spinner::new())
                                                    .child(
                                                        Label::new(msg)
                                                            .text_sm()
                                                            .font_semibold()
                                                            .text_color(theme.foreground),
                                                    ),
                                            )
                                        },
                                    )
                                    .when(external || finished, |canvas| {
                                        let retry = on_retry.clone();
                                        canvas.child(
                                            v_flex()
                                                .absolute()
                                                .inset_0()
                                                .items_center()
                                                .justify_center()
                                                .gap_3()
                                                .bg(theme.background.opacity(0.9))
                                                .child(
                                                    Label::new(if external {
                                                        "Playing in MPV"
                                                    } else {
                                                        "Playback ended"
                                                    })
                                                    .text_sm(),
                                                )
                                                .when(finished, |overlay| {
                                                    overlay.child(
                                                        Button::new("replay-episode")
                                                            .primary()
                                                            .label("Replay episode")
                                                            .when_some(retry, |button, handler| {
                                                                button.on_click(
                                                                    move |event, window, cx| {
                                                                        handler(event, window, cx)
                                                                    },
                                                                )
                                                            }),
                                                    )
                                                }),
                                        )
                                    })
                                    .when(
                                        self.error.is_none()
                                            && self.current_frame.is_some()
                                            && !self.is_loading
                                            && self.status.is_buffering,
                                        |d| {
                                            d.child(
                                                v_flex()
                                                    .absolute()
                                                    .inset_0()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        h_flex()
                                                            .px_4()
                                                            .py_2()
                                                            .rounded(theme.radius_lg)
                                                            .bg(theme.background.opacity(0.9))
                                                            .gap_2()
                                                            .items_center()
                                                            .child(Spinner::new())
                                                            .child(
                                                                Label::new("Buffering...")
                                                                    .text_xs()
                                                                    .font_semibold()
                                                                    .text_color(theme.foreground),
                                                            ),
                                                    ),
                                            )
                                        },
                                    ),
                            )
                            .when_some(self.on_mouse_move.clone(), |v, handler| {
                                v.on_mouse_move(move |ev, win, cx| handler(ev, win, cx))
                            })
                            .when_some(self.on_player_hover, |viewport, handler| {
                                viewport
                                    .on_hover(move |inside, window, cx| handler(inside, window, cx))
                            })
                            .when_some(self.on_activity, |viewport, handler| {
                                let scroll_handler = handler.clone();
                                viewport
                                    .capture_any_mouse_down(move |_, window, cx| {
                                        handler(&(), window, cx)
                                    })
                                    .on_scroll_wheel(move |_, window, cx| {
                                        scroll_handler(&(), window, cx)
                                    })
                            })
                            .when(self.is_hud_visible, |v| {
                                v.child(
                                    h_flex()
                                        .id("player-details-overlay")
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .right_0()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .px_6()
                                        .py_3()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .bg(theme.background.opacity(0.95))
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_4()
                                                .child(
                                                    Button::new("player-back-btn")
                                                        .outline()
                                                        .label(if self.is_fullscreen {
                                                            "Exit fullscreen"
                                                        } else {
                                                            "Back to anime"
                                                        })
                                                        .when_some(
                                                            on_back.clone(),
                                                            |b, handler| {
                                                                b.on_click(move |ev, win, cx| {
                                                                    handler(ev, win, cx)
                                                                })
                                                            },
                                                        ),
                                                )
                                                .child(
                                                    v_flex()
                                                        .gap_0p5()
                                                        .child(
                                                            Label::new(anime_title.clone())
                                                                .font_bold()
                                                                .text_sm()
                                                                .line_clamp(1),
                                                        )
                                                        .child(
                                                            Label::new(format!(
                                                                "Episode {:.0} - {}",
                                                                self.ep_number, self.ep_title
                                                            ))
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Label::new(format!("Audio: {}", self.audio))
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground),
                                                )
                                                .child(
                                                    Label::new(self.provider.clone())
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground),
                                                )
                                                .child(
                                                    Label::new(self.resolution.clone())
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground),
                                                ),
                                        ),
                                )
                            })
                            .when(self.is_hud_visible, |viewport| {
                                viewport.child(
                                    VideoPlayerControls::new(
                                        "embedded-transport",
                                        "",
                                        self.status.clone(),
                                    )
                                    .overlay()
                                    .resolutions(
                                        self.resolution.clone(),
                                        self.resolutions,
                                        self.on_resolution,
                                    )
                                    .on_menu_open_change(self.on_menu_open_change)
                                    .controller(self.controller.clone())
                                    .sliders(self.seek_slider.clone(), self.volume_slider.clone())
                                    .episodes(prev_ep, next_ep, self.on_switch_episode.clone())
                                    .fullscreen(
                                        self.is_fullscreen,
                                        self.on_toggle_fullscreen.clone(),
                                    )
                                    .audio_options(
                                        match self.audio.to_lowercase().as_str() {
                                            "dub" => AudioType::Dub,
                                            "hindi" | "hindi dub" => AudioType::Hindi,
                                            _ => AudioType::Sub,
                                        },
                                        self.audio_options,
                                        self.on_audio_change,
                                    ),
                                )
                            }),
                    )
                    // Episode Quick Playlist Sidebar (hidden in Fullscreen mode)
                    .when(!self.is_fullscreen, |v| {
                        v.child(
                            v_flex()
                                .w_72()
                                .h_full()
                                .border_l_1()
                                .border_color(theme.border)
                                .bg(theme.secondary.opacity(0.15))
                                .p_3()
                                .gap_2()
                                .child(
                                    Label::new(format!(
                                        "Playlist ({} Episodes)",
                                        self.episodes.len()
                                    ))
                                    .font_bold()
                                    .text_sm(),
                                )
                                .when_some(self.playlist_search, |sidebar, input| {
                                    sidebar.child(
                                        Input::new(&input).id("playlist-search").small().w_full(),
                                    )
                                })
                                .when(filtered.is_empty(), |sidebar| {
                                    sidebar.child(
                                        Label::new("No matching episodes")
                                            .text_sm()
                                            .text_color(theme.muted_foreground),
                                    )
                                })
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_h_0()
                                        .id("ep-playlist-scroll")
                                        .overflow_y_scroll()
                                        .gap_1()
                                        .children(filtered.into_iter().map(|ep| {
                                            let ep_num = ep.number;
                                            let is_current = (ep_num - current_ep_num).abs() < 0.1;
                                            let title_disp = episode_label(ep);
                                            let cb = ep_cb.clone();

                                            Button::new(format!("side-ep-{}", ep_num))
                                                .small()
                                                .w_full()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .tooltip(title_disp.clone())
                                                .accessibility_label(title_disp.clone())
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .min_w_0()
                                                        .text_left()
                                                        .text_ellipsis()
                                                        .child(title_disp),
                                                )
                                                .when(is_current, |b| b.primary())
                                                .when(!is_current, |b| b.outline())
                                                .when_some(cb, move |b, handler| {
                                                    b.on_click(move |_ev, win, cx| {
                                                        handler(&ep_num, win, cx)
                                                    })
                                                })
                                        })),
                                ),
                        )
                    }),
            )
    }
}

fn episode_label(episode: &Episode) -> String {
    let number = episode.number.to_string();
    let fallback = format!("Episode {number}");
    match episode
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        Some(title) if !title.eq_ignore_ascii_case(&fallback) => format!("{number} · {title}"),
        _ => fallback,
    }
}

fn episode_matches(episode: &Episode, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || episode_label(episode).to_lowercase().contains(&query)
}

#[cfg(test)]
#[path = "player_view_tests.rs"]
mod tests;
