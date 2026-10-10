use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use silvani_player::{PlaybackStatus, PlayerController};
use silvani_scraper::AudioType;

pub type ControlHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
type EpisodeHandler = Rc<dyn Fn(&f32, &mut Window, &mut App)>;
pub(crate) type MenuOpenHandler = Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>;
pub(crate) type ResolutionHandler = Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>;
type AudioHandler = Rc<dyn Fn(&AudioType, &mut Window, &mut App)>;

/// Presentation of the transport. The application owns sliders and playback subscriptions.
#[derive(IntoElement)]
pub struct VideoPlayerControls {
    id: ElementId,
    title: String,
    status: PlaybackStatus,
    controller: Option<PlayerController>,
    seek_slider: Option<Entity<SliderState>>,
    volume_slider: Option<Entity<SliderState>>,
    previous: Option<f32>,
    next: Option<f32>,
    on_episode: Option<EpisodeHandler>,
    on_fullscreen: ControlHandler,
    fullscreen: bool,
    on_close: ControlHandler,
    audio_options: Vec<AudioType>,
    audio: AudioType,
    on_audio: Option<AudioHandler>,
    resolution: String,
    resolutions: Vec<String>,
    on_resolution: ResolutionHandler,
    overlay: bool,
    on_menu_open_change: MenuOpenHandler,
}

impl VideoPlayerControls {
    pub fn new(id: impl Into<ElementId>, title: impl Into<String>, status: PlaybackStatus) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status,
            controller: None,
            seek_slider: None,
            volume_slider: None,
            previous: None,
            next: None,
            on_episode: None,
            on_fullscreen: None,
            fullscreen: false,
            on_close: None,
            audio_options: Vec::new(),
            audio: AudioType::Sub,
            on_audio: None,
            resolution: "auto".into(),
            resolutions: Vec::new(),
            on_resolution: None,
            overlay: false,
            on_menu_open_change: None,
        }
    }

    pub fn controller(mut self, controller: Option<PlayerController>) -> Self {
        self.controller = controller;
        self
    }

    pub fn sliders(mut self, seek: Entity<SliderState>, volume: Entity<SliderState>) -> Self {
        self.seek_slider = Some(seek);
        self.volume_slider = Some(volume);
        self
    }

    pub fn episodes(
        mut self,
        previous: Option<f32>,
        next: Option<f32>,
        handler: Option<EpisodeHandler>,
    ) -> Self {
        self.previous = previous;
        self.next = next;
        self.on_episode = handler;
        self
    }

    pub fn fullscreen(mut self, fullscreen: bool, handler: ControlHandler) -> Self {
        self.fullscreen = fullscreen;
        self.on_fullscreen = handler;
        self
    }

    pub fn audio_options(
        mut self,
        audio: AudioType,
        options: Vec<AudioType>,
        handler: Option<AudioHandler>,
    ) -> Self {
        self.audio = audio;
        self.audio_options = options;
        self.on_audio = handler;
        self
    }

    pub(crate) fn resolutions(
        mut self,
        selected: String,
        options: Vec<String>,
        handler: ResolutionHandler,
    ) -> Self {
        self.resolution = selected;
        self.resolutions = options;
        self.on_resolution = handler;
        self
    }

    pub fn overlay(mut self) -> Self {
        self.overlay = true;
        self
    }

    pub(crate) fn on_menu_open_change(mut self, handler: MenuOpenHandler) -> Self {
        self.on_menu_open_change = handler;
        self
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

pub(crate) fn format_time(secs: f64) -> String {
    let total = if secs.is_finite() {
        secs.max(0.0) as u64
    } else {
        0
    };
    let (hours, minutes, seconds) = (total / 3600, (total / 60) % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

impl RenderOnce for VideoPlayerControls {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ready = self.controller.is_some();
        let seekable =
            ready && self.status.duration_secs.is_finite() && self.status.duration_secs > 0.0;
        let position = self
            .seek_slider
            .as_ref()
            .map(|slider| slider.read(cx).value().end() as f64)
            .unwrap_or(self.status.position_secs);
        let time = format!(
            "{} / {}",
            format_time(position),
            format_time(self.status.duration_secs)
        );
        let play = self.controller.clone();
        let rewind = self.controller.clone();
        let forward = self.controller.clone();
        let volume = self.controller.clone();
        let current_volume = self.status.volume;
        let subtitles = self.status.subtitle_tracks.clone();
        let subtitle_menu_callback = self.on_menu_open_change.clone();
        let audio_menu_callback = self.on_menu_open_change.clone();
        let resolution_menu_callback = self.on_menu_open_change;
        let subtitle_controller = self.controller.clone();
        let subtitles_off =
            !self.status.subtitles_visible || self.status.sub_track.as_deref() == Some("no");
        let subtitle_label = if subtitles.is_empty() {
            "Unavailable".to_string()
        } else if subtitles_off {
            "Off".to_string()
        } else {
            subtitles
                .iter()
                .find(|t| t.is_selected())
                .map(|t| t.label().to_owned())
                .unwrap_or_else(|| "Auto".into())
        };
        let audio_tracks = self.status.audio_tracks.clone();
        let audio_controller = self.controller.clone();
        let audio_label = audio_tracks
            .iter()
            .find(|t| t.is_selected())
            .map(|t| t.label().to_owned())
            .unwrap_or_else(|| self.audio.display_name().into());
        let options = self.audio_options;
        let audio = self.audio;
        let audio_handler = self.on_audio;

        v_flex()
            .id(self.id)
            .when(self.overlay, |controls| {
                controls.absolute().bottom_0().left_0().right_0()
            })
            .w_full()
            .px_4()
            .py_3()
            .gap_2()
            .bg(theme.background)
            .text_color(theme.foreground)
            .border_t_1()
            .border_color(theme.border)
            .when(!self.title.is_empty(), |controls| {
                controls.child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(Label::new(self.title).text_sm())
                        .when_some(self.on_close, |row, close| {
                            row.child(
                                Button::new("close-player")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Close)
                                    .tooltip("Close player")
                                    .on_click(move |event, window, cx| close(event, window, cx)),
                            )
                        }),
                )
            })
            .when_some(self.seek_slider, |controls, slider| {
                controls.child(Slider::new(&slider).w_full().disabled(!seekable))
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("previous-episode")
                            .ghost()
                            .small()
                            .icon(IconName::SkipBack)
                            .tooltip("Previous episode")
                            .disabled(self.previous.is_none() || self.on_episode.is_none())
                            .when_some(self.on_episode.clone(), |button, handler| {
                                let previous = self.previous;
                                button.on_click(move |_, window, cx| {
                                    if let Some(number) = previous {
                                        handler(&number, window, cx);
                                    }
                                })
                            }),
                    )
                    .child(
                        Button::new("rewind")
                            .ghost()
                            .small()
                            .icon(IconName::RotateCcw)
                            .tooltip("Back 10 seconds")
                            .disabled(!seekable)
                            .on_click(move |_, _, _| {
                                if let Some(controller) = &rewind {
                                    let _ = controller.seek_relative(-10.0);
                                }
                            }),
                    )
                    .child(
                        Button::new("play-pause")
                            .primary()
                            .small()
                            .icon(if self.status.paused {
                                IconName::Play
                            } else {
                                IconName::Pause
                            })
                            .tooltip(if self.status.paused { "Play" } else { "Pause" })
                            .disabled(!ready)
                            .on_click(move |_, _, _| {
                                if let Some(controller) = &play {
                                    let _ = controller.toggle_pause();
                                }
                            }),
                    )
                    .child(
                        Button::new("forward")
                            .ghost()
                            .small()
                            .icon(IconName::RotateCw)
                            .tooltip("Forward 10 seconds")
                            .disabled(!seekable)
                            .on_click(move |_, _, _| {
                                if let Some(controller) = &forward {
                                    let _ = controller.seek_relative(10.0);
                                }
                            }),
                    )
                    .child(
                        Button::new("next-episode")
                            .ghost()
                            .small()
                            .icon(IconName::SkipForward)
                            .tooltip("Next episode")
                            .disabled(self.next.is_none() || self.on_episode.is_none())
                            .when_some(self.on_episode, |button, handler| {
                                let next = self.next;
                                button.on_click(move |_, window, cx| {
                                    if let Some(number) = next {
                                        handler(&number, window, cx);
                                    }
                                })
                            }),
                    )
                    .child(
                        Label::new(time)
                            .text_xs()
                            .text_color(theme.muted_foreground),
                    )
                    .child(h_flex().flex_1())
                    .child(
                        Button::new("mute")
                            .ghost()
                            .small()
                            .icon(if self.status.volume > 0.0 {
                                IconName::Volume2
                            } else {
                                IconName::VolumeX
                            })
                            .tooltip(if self.status.volume > 0.0 {
                                "Mute"
                            } else {
                                "Unmute"
                            })
                            .disabled(!ready)
                            .on_click(move |_, _, _| {
                                if let Some(controller) = &volume {
                                    let _ = controller.set_volume(if current_volume > 0.0 {
                                        0.0
                                    } else {
                                        100.0
                                    });
                                }
                            }),
                    )
                    .when_some(self.volume_slider, |row, slider| {
                        row.child(Slider::new(&slider).flex_none().w_24().disabled(!ready))
                    })
                    .when_some(self.on_fullscreen, |row, handler| {
                        row.child(
                            Button::new("fullscreen")
                                .ghost()
                                .small()
                                .icon(if self.fullscreen {
                                    IconName::Minimize
                                } else {
                                    IconName::Maximize
                                })
                                .tooltip(if self.fullscreen {
                                    "Exit fullscreen"
                                } else {
                                    "Fullscreen"
                                })
                                .on_click(move |event, window, cx| handler(event, window, cx)),
                        )
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("subtitle-menu")
                            .ghost()
                            .small()
                            .icon(IconName::Captions)
                            .label(format!("Subtitles: {subtitle_label}"))
                            .disabled(!ready || subtitles.is_empty())
                            .tooltip(if subtitles.is_empty() {
                                "No subtitle tracks available"
                            } else {
                                "Choose subtitles"
                            })
                            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                                let off_controller = subtitle_controller.clone();
                                menu = menu.item(
                                    PopupMenuItem::new("Off").checked(subtitles_off).on_click(
                                        move |_, _, _| {
                                            if let Some(controller) = &off_controller {
                                                let _ = controller.set_subtitle_track("no");
                                            }
                                        },
                                    ),
                                );
                                for track in &subtitles {
                                    let controller = subtitle_controller.clone();
                                    let id = track.id().to_owned();
                                    menu = menu.item(
                                        PopupMenuItem::new(track.label().to_owned())
                                            .checked(track.is_selected() && !subtitles_off)
                                            .on_click(move |_, _, _| {
                                                if let Some(controller) = &controller {
                                                    let _ = controller.set_subtitle_track(&id);
                                                }
                                            }),
                                    );
                                }
                                menu
                            })
                            .on_open_change(move |open, window, cx| {
                                if let Some(handler) = &subtitle_menu_callback {
                                    handler(open, window, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("audio-menu")
                            .ghost()
                            .small()
                            .icon(IconName::Languages)
                            .label(format!("Audio: {audio_label}"))
                            .disabled(
                                !ready
                                    || (audio_tracks.len() < 2
                                        && (options.len() < 2 || audio_handler.is_none())),
                            )
                            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                                if !audio_tracks.is_empty() {
                                    menu = menu.label("Audio tracks");
                                    for track in &audio_tracks {
                                        let controller = audio_controller.clone();
                                        let id = track.id().to_owned();
                                        menu = menu.item(
                                            PopupMenuItem::new(track.label().to_owned())
                                                .checked(track.is_selected())
                                                .on_click(move |_, _, _| {
                                                    if let Some(controller) = &controller {
                                                        let _ = controller.set_audio_track(&id);
                                                    }
                                                }),
                                        );
                                    }
                                }
                                if let Some(handler) = &audio_handler {
                                    menu = menu.separator().label("Stream language");
                                    for option in &options {
                                        let handler = handler.clone();
                                        let option = *option;
                                        menu = menu.item(
                                            PopupMenuItem::new(option.display_name())
                                                .checked(option == audio)
                                                .on_click(move |_, window, cx| {
                                                    handler(&option, window, cx)
                                                }),
                                        );
                                    }
                                }
                                menu
                            })
                            .on_open_change(move |open, window, cx| {
                                if let Some(handler) = &audio_menu_callback {
                                    handler(open, window, cx);
                                }
                            }),
                    )
                    .when_some(self.on_resolution, |row, handler| {
                        row.child(
                            Button::new("resolution-menu")
                                .ghost()
                                .small()
                                .label(format!("Quality: {}", self.resolution))
                                .disabled(!ready || self.resolutions.len() < 2)
                                .dropdown_menu_with_anchor(
                                    Anchor::BottomLeft,
                                    move |mut menu, _, _| {
                                        for quality in &self.resolutions {
                                            let handler = handler.clone();
                                            let quality = quality.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(
                                                    if quality.eq_ignore_ascii_case("auto") {
                                                        "Auto".into()
                                                    } else {
                                                        quality.clone()
                                                    },
                                                )
                                                .checked(
                                                    quality.eq_ignore_ascii_case(&self.resolution),
                                                )
                                                .on_click(move |_, window, cx| {
                                                    handler(&quality, window, cx)
                                                }),
                                            );
                                        }
                                        menu
                                    },
                                )
                                .on_open_change(move |open, window, cx| {
                                    if let Some(handler) = &resolution_menu_callback {
                                        handler(open, window, cx);
                                    }
                                }),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::format_time;
    #[test]
    fn timeline_formats_hours_and_invalid_positions() {
        assert_eq!(format_time(65.9), "01:05");
        assert_eq!(format_time(3661.0), "1:01:01");
        assert_eq!(format_time(-1.0), "00:00");
        assert_eq!(format_time(f64::NAN), "00:00");
    }
}
