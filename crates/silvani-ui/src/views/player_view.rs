use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use silvani_meta::image_cache::ImageCacheManager;
use silvani_meta::models::AnimeItem;
use silvani_player::{PlaybackStatus, PlayerController};
use silvani_scraper::models::Episode;

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
pub type EpChangeHandler = Option<Rc<dyn Fn(&f32, &mut Window, &mut App)>>;
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
    is_seeking_drag: bool,
    episodes: Vec<Episode>,
    image_cache: Arc<ImageCacheManager>,
    seek_slider: Entity<SliderState>,
    volume_slider: Entity<SliderState>,
    on_back: ClickHandler,
    on_switch_episode: EpChangeHandler,
    on_toggle_fullscreen: ClickHandler,
    on_mouse_move: MouseMoveHandler,
    on_retry: ClickHandler,
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
        is_seeking_drag: bool,
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
            is_seeking_drag,
            episodes,
            image_cache,
            seek_slider,
            volume_slider,
            on_back: None,
            on_switch_episode: None,
            on_toggle_fullscreen: None,
            on_mouse_move: None,
            on_retry: None,
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

    fn format_time(secs: f64) -> String {
        let total = secs as u64;
        let h = total / 3600;
        let m = (total % 3600) / 60;
        let s = total % 60;
        if h > 0 {
            format!("{:02}:{:02}:{:02}", h, m, s)
        } else {
            format!("{:02}:{:02}", m, s)
        }
    }
}

impl RenderOnce for EmbeddedPlayerView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let cur_time = Self::format_time(self.status.position_secs);
        let dur_time = Self::format_time(self.status.duration_secs);
        let time_str = format!("{} / {}", cur_time, dur_time);

        let seek_slider_ref = self.seek_slider.read(cx);
        let seek_val = seek_slider_ref.value().end() as f64;
        let seek_pct = if self.status.duration_secs > 0.0 {
            (seek_val / self.status.duration_secs).clamp(0.0, 1.0) as f32
        } else {
            seek_slider_ref.percentage().end.clamp(0.0, 1.0)
        };
        let seek_time_str = Self::format_time(seek_val);
        let tooltip_offset = -44.0 * seek_pct;

        let on_back = self.on_back.clone();
        let on_retry = self.on_retry.clone();
        let ep_cb = self.on_switch_episode.clone();
        let ep_prev_cb = self.on_switch_episode.clone();
        let ep_next_cb = self.on_switch_episode.clone();
        let fs_cb = self.on_toggle_fullscreen.clone();
        let fs_canvas_cb = self.on_toggle_fullscreen.clone();
        let fs_floating_cb = self.on_toggle_fullscreen.clone();
        let ctrl_play = self.controller.clone();
        let ctrl_rev10 = self.controller.clone();
        let ctrl_fwd10 = self.controller.clone();
        let ctrl_vol_toggle = self.controller.clone();
        let ctrl_vol_scroll = self.controller.clone();
        let ctrl_sub_toggle = self.controller.clone();
        let ctrl_sub_cycle = self.controller.clone();

        let anime_title = self.anime_item.display_title().to_string();
        let current_ep_num = self.ep_number;
        let volume_pct = self.status.volume;

        // Check if previous or next episode exists
        let prev_ep = self.episodes.iter().find(|e| (e.number - (current_ep_num - 1.0)).abs() < 0.1).map(|e| e.number);
        let next_ep = self.episodes.iter().find(|e| (e.number - (current_ep_num + 1.0)).abs() < 0.1).map(|e| e.number);

        // Poster image fallback
        let poster_source: Option<ImageSource> = match self.image_cache.get_cached_path(&self.anime_item.id) {
            Some(p) if p.exists() => Some(ImageSource::from(p)),
            _ => self.anime_item.cover_image.as_ref().map(|url| ImageSource::from(url.clone())),
        };

        v_flex()
            .id("embedded-player-view")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // Static Top Header Nav (hidden in Fullscreen mode)
            .when(!self.is_fullscreen, |v| {
                v.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .px_6()
                        .py_3()
                        .border_b_1()
                        .border_color(theme.border)
                        .bg(theme.secondary.opacity(0.3))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_4()
                                .child(
                                    Button::new("player-back-btn")
                                        .outline()
                                        .label("← Back to Anime")
                                        .when_some(on_back.clone(), |b, handler| {
                                            b.on_click(move |ev, win, cx| handler(ev, win, cx))
                                        }),
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
                                            Label::new(format!("Episode {:.0} - {}", self.ep_number, self.ep_title))
                                                .text_xs()
                                                .text_color(theme.muted_foreground),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Tag::secondary().child(format!("Audio: {}", self.audio.to_uppercase())))
                                .child(Tag::primary().child(format!("Provider: {}", self.provider)))
                                .child(Tag::info().child(self.resolution.clone())),
                        ),
                )
            })
            .child(
                // Main Area: Video Viewport (Flex 1) + Optional Episode Sidebar
                h_flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    // Video Viewport Canvas
                    .child(
                        v_flex()
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
                                    .size_full()
                                    .relative()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(self.is_hud_visible, |d| d.cursor_pointer())
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
                                                .bg(gpui::black().opacity(0.85))
                                                .child(
                                                    Label::new(format!("Network error: {}", err_msg))
                                                        .text_sm()
                                                        .font_semibold()
                                                        .text_color(gpui::white()),
                                                )
                                                .child(
                                                    Button::new("player-retry-btn")
                                                        .primary()
                                                        .small()
                                                        .label("Retry")
                                                        .when_some(retry_cb, |b, handler| {
                                                            b.on_click(move |ev, win, cx| handler(ev, win, cx))
                                                        }),
                                                ),
                                        )
                                    })
                                    .when(self.error.is_none() && (self.is_loading || self.current_frame.is_none()), |d| {
                                        let msg = format!("Loading {} stream...", self.provider);
                                        d.child(
                                            v_flex()
                                                .absolute()
                                                .inset_0()
                                                .items_center()
                                                .justify_center()
                                                .gap_3()
                                                .bg(gpui::black().opacity(0.6))
                                                .child(Spinner::new())
                                                .child(
                                                    Label::new(msg)
                                                        .text_sm()
                                                        .font_semibold()
                                                        .text_color(gpui::white()),
                                                ),
                                        )
                                    })
                                    .when(self.error.is_none() && self.current_frame.is_some() && !self.is_loading && self.status.is_buffering, |d| {
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
                                                        .rounded_lg()
                                                        .bg(gpui::black().opacity(0.6))
                                                        .gap_2()
                                                        .items_center()
                                                        .child(Spinner::new())
                                                        .child(
                                                            Label::new("Buffering...")
                                                                .text_xs()
                                                                .font_semibold()
                                                                .text_color(gpui::white()),
                                                        ),
                                                ),
                                        )
                                    }),
                            )
                            .when_some(self.on_mouse_move.clone(), |v, handler| {
                                v.on_mouse_move(move |ev, win, cx| handler(ev, win, cx))
                            })
                            // Floating Top Bar Overlay in Fullscreen
                            .when(self.is_fullscreen, |v| {
                                v.child(
                                    h_flex()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .right_0()
                                        .px_6()
                                        .py_3()
                                        .items_center()
                                        .justify_between()
                                        .bg(gpui::black().opacity(0.85))
                                        .opacity(if self.is_hud_visible { 1.0 } else { 0.0 })
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_4()
                                                .child(
                                                    Button::new("floating-back-btn")
                                                        .outline()
                                                        .label("← Exit Fullscreen")
                                                        .when_some(fs_floating_cb, |b, handler| {
                                                            b.on_click(move |ev, win, cx| handler(ev, win, cx))
                                                        }),
                                                )
                                                .child(
                                                    Label::new(format!("{} - Episode {:.0}", anime_title, self.ep_number))
                                                        .font_bold()
                                                        .text_sm()
                                                        .text_color(gpui::white()),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(Tag::secondary().child(format!("Audio: {}", self.audio.to_uppercase())))
                                                .child(Tag::primary().child(format!("Provider: {}", self.provider))),
                                        ),
                                )
                            })
                            // Bottom Player Controls
                            .child(
                                v_flex()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .px_6()
                                    .py_3()
                                    .gap_2()
                                    .bg(gpui::black().opacity(0.85))
                                    .opacity(if self.is_hud_visible { 1.0 } else { 0.0 })
                                    // Interactive Continuous Slider Seek Bar with Seek Time Tooltip
                                    .child(
                                        div()
                                            .w_full()
                                            .relative()
                                            .flex()
                                            .items_center()
                                            .child(Slider::new(&self.seek_slider).w_full())
                                            .when(self.is_seeking_drag, |parent| {
                                                parent.child(
                                                    div()
                                                        .absolute()
                                                        .bottom(gpui::px(22.0))
                                                        .left(gpui::relative(seek_pct))
                                                        .child(
                                                            div()
                                                                .relative()
                                                                .left(gpui::px(tooltip_offset))
                                                                .px_2()
                                                                .py_0p5()
                                                                .rounded_md()
                                                                .bg(theme.primary)
                                                                .text_color(theme.primary_foreground)
                                                                .text_xs()
                                                                .font_semibold()
                                                                .shadow_md()
                                                                .child(seek_time_str.clone()),
                                                        ),
                                                )
                                            }),
                                    )
                                    // Controls Row: Play/Pause, Rewind, Forward, Prev/Next, Subtitles, Volume Slider, Time, Fullscreen
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_2()
                                                    // Previous Episode
                                                    .when_some(prev_ep, |f, ep_val| {
                                                        let cb = ep_prev_cb.clone();
                                                        f.child(
                                                            Button::new("prev-ep-btn")
                                                                .ghost()
                                                                .small()
                                                                .icon(IconName::SkipBack)
                                                                .when_some(cb, move |btn, handler| {
                                                                    btn.on_click(move |_ev, win, cx| handler(&ep_val, win, cx))
                                                                }),
                                                        )
                                                    })
                                                    // Rewind -10s
                                                    .child(
                                                        Button::new("rewind-10")
                                                            .ghost()
                                                            .small()
                                                            .icon(IconName::RotateCcw)
                                                            .on_click(move |_, win, _| {
                                                                if let Some(ctrl) = &ctrl_rev10 {
                                                                    let _ = ctrl.seek_relative(-10.0);
                                                                    win.refresh();
                                                                }
                                                            }),
                                                    )
                                                    // Play / Pause
                                                    .child(
                                                        Button::new("toggle-play")
                                                            .primary()
                                                            .small()
                                                            .icon(if self.status.paused {
                                                                IconName::Play
                                                            } else {
                                                                IconName::Pause
                                                            })
                                                            .on_click(move |_, win, _| {
                                                                if let Some(ctrl) = &ctrl_play {
                                                                    let _ = ctrl.toggle_pause();
                                                                    win.refresh();
                                                                }
                                                            }),
                                                    )
                                                    // Forward +10s
                                                    .child(
                                                        Button::new("forward-10")
                                                            .ghost()
                                                            .small()
                                                            .icon(IconName::RotateCw)
                                                            .on_click(move |_, win, _| {
                                                                if let Some(ctrl) = &ctrl_fwd10 {
                                                                    let _ = ctrl.seek_relative(10.0);
                                                                    win.refresh();
                                                                }
                                                            }),
                                                    )
                                                    // Next Episode
                                                    .when_some(next_ep, |f, ep_val| {
                                                        let cb = ep_next_cb.clone();
                                                        f.child(
                                                            Button::new("next-ep-btn")
                                                                .ghost()
                                                                .small()
                                                                .icon(IconName::SkipForward)
                                                                .when_some(cb, move |btn, handler| {
                                                                    btn.on_click(move |_ev, win, cx| handler(&ep_val, win, cx))
                                                                }),
                                                        )
                                                    })
                                                    // Time Label
                                                    .child(
                                                        Label::new(time_str)
                                                            .text_xs()
                                                            .font_semibold()
                                                            .text_color(gpui::white().opacity(0.85)),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .flex_shrink_0()
                                                    // Subtitles Toggle Button
                                                    .child(
                                                        Button::new("sub-toggle-btn")
                                                            .ghost()
                                                            .small()
                                                            .icon(IconName::Captions)
                                                            .on_click(move |_, win, _| {
                                                                if let Some(ctrl) = &ctrl_sub_toggle {
                                                                    let _ = ctrl.toggle_subtitles();
                                                                    win.refresh();
                                                                }
                                                            }),
                                                    )
                                                    // Subtitles Track Cycle Button
                                                    .child(
                                                        Button::new("sub-cycle-btn")
                                                            .ghost()
                                                            .small()
                                                            .icon(IconName::Languages)
                                                            .on_click(move |_, win, _| {
                                                                if let Some(ctrl) = &ctrl_sub_cycle {
                                                                    let _ = ctrl.next_subtitle();
                                                                    win.refresh();
                                                                }
                                                            }),
                                                    )
                                                    // Smooth Volume Slider Section
                                                    .child(
                                                        h_flex()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                Button::new("vol-toggle")
                                                                    .ghost()
                                                                    .small()
                                                                    .icon(if volume_pct > 0.0 { IconName::Volume2 } else { IconName::VolumeX })
                                                                    .on_click(move |_, win, _| {
                                                                        if let Some(ctrl) = &ctrl_vol_toggle {
                                                                            let cur = ctrl.get_status().volume;
                                                                            let new_vol = if cur > 0.0 { 0.0 } else { 100.0 };
                                                                            let _ = ctrl.set_volume(new_vol);
                                                                            win.refresh();
                                                                        }
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(gpui::px(112.0))
                                                                    .h_5()
                                                                    .flex()
                                                                    .items_center()
                                                                    .on_scroll_wheel(move |ev, win, _| {
                                                                        if let Some(ctrl) = &ctrl_vol_scroll {
                                                                            let delta_y = match ev.delta {
                                                                                gpui::ScrollDelta::Lines(p) => p.y,
                                                                                gpui::ScrollDelta::Pixels(p) => p.y.into(),
                                                                            };
                                                                            let cur = ctrl.get_status().volume;
                                                                            let diff = if delta_y > 0.0 { 5.0 } else if delta_y < 0.0 { -5.0 } else { 0.0 };
                                                                            if diff != 0.0 {
                                                                                let new_vol = (cur + diff).clamp(0.0, 100.0);
                                                                                let _ = ctrl.set_volume(new_vol);
                                                                                win.refresh();
                                                                            }
                                                                        }
                                                                    })
                                                                    .child(Slider::new(&self.volume_slider).w_full())
                                                            )
                                                            .child(
                                                                Label::new(format!("{:.0}%", volume_pct))
                                                                    .text_xs()
                                                                    .font_semibold()
                                                                    .text_color(gpui::white().opacity(0.85)),
                                                            ),
                                                    )
                                                    // Fullscreen Toggle Button
                                                    .child(
                                                        Button::new("fullscreen-toggle-btn")
                                                            .ghost()
                                                            .small()
                                                            .icon(if self.is_fullscreen { IconName::Minimize } else { IconName::Maximize })
                                                            .when_some(fs_cb, |b, handler| {
                                                                b.on_click(move |ev, win, cx| handler(ev, win, cx))
                                                            }),
                                                    ),
                                            ),
                                    ),
                            ),
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
                                    Label::new(format!("Playlist ({} Episodes)", self.episodes.len()))
                                        .font_bold()
                                        .text_sm(),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .id("ep-playlist-scroll")
                                        .overflow_y_scroll()
                                        .gap_1()
                                        .children(self.episodes.iter().map(|ep| {
                                            let ep_num = ep.number;
                                            let is_current = (ep_num - current_ep_num).abs() < 0.1;
                                            let title_disp = ep.title.clone().unwrap_or_else(|| format!("Episode {:.0}", ep_num));
                                            let cb = ep_cb.clone();

                                            Button::new(format!("side-ep-{}", ep_num))
                                                .small()
                                                .w_full()
                                                .label(format!("Ep {:.0}: {}", ep_num, title_disp))
                                                .when(is_current, |b| b.primary())
                                                .when(!is_current, |b| b.outline())
                                                .when_some(cb, move |b, handler| {
                                                    b.on_click(move |_ev, win, cx| handler(&ep_num, win, cx))
                                                })
                                        })),
                                ),
                        )
                    }),
            )
    }
}
