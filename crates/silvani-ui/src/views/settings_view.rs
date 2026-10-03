use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable, Theme};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::settings::{self, AppSettings};

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
pub type ThemeSelectHandler = Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>;
pub type SettingsSaveHandler = Option<Rc<dyn Fn(&AppSettings, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct SettingsView {
    settings: AppSettings,
    on_back: ClickHandler,
    on_theme_select: ThemeSelectHandler,
    on_toggle_mode: ClickHandler,
    on_update_settings: SettingsSaveHandler,
}

impl SettingsView {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            settings,
            on_back: None,
            on_theme_select: None,
            on_toggle_mode: None,
            on_update_settings: None,
        }
    }

    pub fn on_back(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }

    pub fn on_theme_select(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_theme_select = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_mode(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_mode = Some(Rc::new(handler));
        self
    }

    pub fn on_update_settings(
        mut self,
        handler: impl Fn(&AppSettings, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_update_settings = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SettingsView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let on_back = self.on_back.clone();
        let on_theme_select = self.on_theme_select.clone();
        let on_update = self.on_update_settings.clone();

        let current_theme_name = Theme::global(cx).theme_name().to_string();
        let all_themes = settings::available_themes(cx);

        v_flex()
            .id("settings-view-container")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .p_6()
            .gap_6()
            .overflow_y_scroll()
            // Top Nav
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Button::new("settings-back-btn")
                            .outline()
                            .label("← Back to Browse")
                            .when_some(on_back, |b, handler| {
                                b.on_click(move |ev, win, cx| handler(ev, win, cx))
                            }),
                    )
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        Label::new("Application Settings")
                            .text_2xl()
                            .font_bold(),
                    )
            )
            .child(
                v_flex()
                .gap_6()
                    .child(
                        Label::new("Installed Themes")
                            .text_lg()
                            .font_bold(),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_3()
                            .children(all_themes.into_iter().map(|th| {
                                let th_name = th.name.to_string();
                                let is_active = th_name == current_theme_name;
                                let mode_str = if th.mode.is_dark() { "dark" } else { "light" };
                                let cb = on_theme_select.clone();

                                Button::new(format!("theme-card-{}", th_name))
                                    .label(format!("{} ({})", th_name, mode_str))
                                    .when(is_active, |b| b.primary())
                                    .when(!is_active, |b| b.outline())
                                    .when_some(cb, move |b, handler| {
                                        let name = th_name.clone();
                                        b.on_click(move |_ev, win, cx| handler(&name, win, cx))
                                    })
                            })),
                    ),
            )
            .child(
                v_flex()
                .gap_6()
                    .child(
                        Label::new("Stream & Audio Preferences")
                            .text_lg()
                            .font_bold(),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Default Provider:").font_semibold().text_sm())
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .children(self.settings.providers.iter().filter(|p| p.enabled).map(|p| {
                                        let id = p.id.clone();
                                        let name = p.name.clone();
                                        let is_sel = self.settings.default_provider == id;
                                        let mut new_s = self.settings.clone();
                                        new_s.default_provider = id.clone();
                                        let cb = on_update.clone();
                                        Button::new(format!("set-prov-{}", id))
                                            .small()
                                            .label(name)
                                            .when(is_sel, |b| b.primary())
                                            .when(!is_sel, |b| b.outline())
                                            .when_some(cb, move |b, handler| {
                                                let s_clone = new_s.clone();
                                                b.on_click(move |_ev, win, cx| handler(&s_clone, win, cx))
                                            })
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Default Audio:").font_semibold().text_sm())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .children([
                                        ("sub", "Subtitles (JP)"),
                                        ("dub", "English Dub"),
                                        ("hindi", "Hindi Dub"),
                                    ].into_iter().map(|(id, name)| {
                                        let is_sel = self.settings.default_audio == id;
                                        let mut new_s = self.settings.clone();
                                        new_s.default_audio = id.to_string();
                                        let cb = on_update.clone();

                                        Button::new(format!("set-aud-{}", id))
                                            .small()
                                            .label(name)
                                            .when(is_sel, |b| b.primary())
                                            .when(!is_sel, |b| b.outline())
                                            .when_some(cb, move |b, handler| {
                                                let s_clone = new_s.clone();
                                                b.on_click(move |_ev, win, cx| handler(&s_clone, win, cx))
                                            })
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Default Resolution:").font_semibold().text_sm())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .children(["1080p", "720p", "480p", "Auto"].into_iter().map(|res| {
                                        let is_sel = self.settings.default_resolution == res;
                                        let mut new_s = self.settings.clone();
                                        new_s.default_resolution = res.to_string();
                                        let cb = on_update.clone();

                                        Button::new(format!("set-res-{}", res))
                                            .small()
                                            .label(res)
                                            .when(is_sel, |b| b.primary())
                                            .when(!is_sel, |b| b.outline())
                                            .when_some(cb, move |b, handler| {
                                                let s_clone = new_s.clone();
                                                b.on_click(move |_ev, win, cx| handler(&s_clone, win, cx))
                                            })
                                    })),
                            ),
                    ),
            )
    }
}
