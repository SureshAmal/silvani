use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::prelude::*;
use gpui_kit::*;

use silvani_player::{PlaybackStatus, PlayerController};

pub type ControlHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct VideoPlayerControls {
    id: ElementId,
    title: String,
    status: PlaybackStatus,
    controller: Option<PlayerController>,
    on_close: ControlHandler,
}

impl VideoPlayerControls {
    pub fn new(id: impl Into<ElementId>, title: impl Into<String>, status: PlaybackStatus) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status,
            controller: None,
            on_close: None,
        }
    }

    pub fn controller(mut self, controller: Option<PlayerController>) -> Self {
        self.controller = controller;
        self
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    fn format_time(secs: f64) -> String {
        let total = secs as u64;
        let m = total / 60;
        let s = total % 60;
        format!("{:02}:{:02}", m, s)
    }
}

impl RenderOnce for VideoPlayerControls {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let cur_time = Self::format_time(self.status.position_secs);
        let dur_time = Self::format_time(self.status.duration_secs);
        let time_str = format!("{} / {}", cur_time, dur_time);

        let pct = if self.status.duration_secs > 0.0 {
            (self.status.position_secs / self.status.duration_secs * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };

        let ctrl_play = self.controller.clone();
        let ctrl_rev = self.controller.clone();
        let ctrl_fwd = self.controller.clone();
        let ctrl_vol = self.controller.clone();

        v_flex()
            .id(self.id)
            .w_full()
            .p_4()
            .gap_3()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius_lg)
            .shadow_lg()
            // Title and Close bar
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        Label::new(self.title)
                            .text_sm()
                            .font_semibold()
                    )
                    .when_some(self.on_close, |d, cb| {
                        d.child(
                            Button::new("close-player-btn")
                                .ghost()
                                .icon(IconName::Close)
                                .on_click(move |ev, win, cx| cb(ev, win, cx)),
                        )
                    }),
            )
            // Progress Bar / Scrub Track
            .child(
                v_flex()
                    .w_full()
                    .h_2()
                    .rounded_full()
                    .bg(theme.muted)
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(theme.primary)
                            .w(gpui::relative(pct / 100.0)),
                    ),
            )
            // Control Buttons & Time Display
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            // Skip -10s
                            .child(
                                Button::new("rewind-10")
                                    .ghost()
                                    .icon(IconName::ArrowLeft)
                                    .on_click(move |_, _, _| {
                                        if let Some(ctrl) = &ctrl_rev {
                                            let _ = ctrl.seek_relative(-10.0);
                                        }
                                    }),
                            )
                            // Play / Pause Toggle
                            .child(
                                Button::new("toggle-play")
                                    .primary()
                                    .icon(if self.status.paused {
                                        IconName::Play
                                    } else {
                                        IconName::Pause
                                    })
                                    .on_click(move |_, _, _| {
                                        if let Some(ctrl) = &ctrl_play {
                                            let _ = ctrl.toggle_pause();
                                        }
                                    }),
                            )
                            // Skip +10s
                            .child(
                                Button::new("forward-10")
                                    .ghost()
                                    .icon(IconName::ArrowRight)
                                    .on_click(move |_, _, _| {
                                        if let Some(ctrl) = &ctrl_fwd {
                                            let _ = ctrl.seek_relative(10.0);
                                        }
                                    }),
                            ),
                    )
                    // Time indicator & Volume toggle
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                Label::new(time_str)
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                Button::new("vol-toggle")
                                    .ghost()
                                    .icon(IconName::Volume)
                                    .on_click(move |_, _, _| {
                                        if let Some(ctrl) = &ctrl_vol {
                                            let new_vol = if ctrl.get_status().volume > 0.0 { 0.0 } else { 100.0 };
                                            let _ = ctrl.set_volume(new_vol);
                                        }
                                    }),
                            ),
                    ),
            )
    }
}
