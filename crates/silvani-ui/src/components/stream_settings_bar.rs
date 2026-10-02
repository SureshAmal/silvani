use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use silvani_scraper::models::AudioType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedAudio {
    Sub,
    Dub,
    Hindi,
}

impl SelectedAudio {
    pub fn to_audio_type(&self) -> AudioType {
        match self {
            Self::Sub => AudioType::Sub,
            Self::Dub | Self::Hindi => AudioType::Dub,
        }
    }
}

pub const AVAILABLE_PROVIDERS: [(&str, &str); 7] = [
    ("hianime", "HiAnime"),
    ("gogoanime", "GogoAnime"),
    ("animeonsen", "AnimeOnsen"),
    ("animeworldindia", "AnimeWorld (HI)"),
    ("desidubanime", "DesiDub (HI)"),
    ("muse_india", "Muse India (YT)"),
    ("ani_one_asia", "Ani-One Asia (YT)"),
];

pub const AVAILABLE_RESOLUTIONS: [&str; 4] = ["1080p", "720p", "480p", "Auto"];

pub type AudioChangeHandler = Option<Rc<dyn Fn(&SelectedAudio, &mut Window, &mut App)>>;
pub type ProviderChangeHandler = Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>;
pub type ResolutionChangeHandler = Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct StreamSettingsBar {
    id: ElementId,
    selected_audio: SelectedAudio,
    selected_provider: String,
    selected_resolution: String,
    on_audio_change: AudioChangeHandler,
    on_provider_change: ProviderChangeHandler,
    on_resolution_change: ResolutionChangeHandler,
}

impl StreamSettingsBar {
    pub fn new(
        id: impl Into<ElementId>,
        selected_audio: SelectedAudio,
        selected_provider: impl Into<String>,
        selected_resolution: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            selected_audio,
            selected_provider: selected_provider.into(),
            selected_resolution: selected_resolution.into(),
            on_audio_change: None,
            on_provider_change: None,
            on_resolution_change: None,
        }
    }

    pub fn on_audio_change(
        mut self,
        handler: impl Fn(&SelectedAudio, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_audio_change = Some(Rc::new(handler));
        self
    }

    pub fn on_provider_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_provider_change = Some(Rc::new(handler));
        self
    }

    pub fn on_resolution_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolution_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for StreamSettingsBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let audio_cb = self.on_audio_change.clone();
        let prov_cb = self.on_provider_change.clone();
        let res_cb = self.on_resolution_change.clone();

        v_flex()
            .id(self.id)
            .gap_3()
            .p_3()
            .rounded_lg()
            .bg(theme.secondary.opacity(0.4))
            .border_1()
            .border_color(theme.border)
            .w_full()
            // Row 1: Audio / Language & Resolution Controls
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Label::new("Audio:")
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(theme.muted_foreground),
                            )
                            .child({
                                let cb = audio_cb.clone();
                                let is_active = self.selected_audio == SelectedAudio::Sub;
                                Button::new("audio-sub")
                                    .small()
                                    .label("Sub (JP)")
                                    .when(is_active, |b| b.primary())
                                    .when(!is_active, |b| b.outline())
                                    .when_some(cb, |b, h| {
                                        b.on_click(move |_ev, win, cx| h(&SelectedAudio::Sub, win, cx))
                                    })
                            })
                            .child({
                                let cb = audio_cb.clone();
                                let is_active = self.selected_audio == SelectedAudio::Dub;
                                Button::new("audio-dub")
                                    .small()
                                    .label("Dub (EN)")
                                    .when(is_active, |b| b.primary())
                                    .when(!is_active, |b| b.outline())
                                    .when_some(cb, |b, h| {
                                        b.on_click(move |_ev, win, cx| h(&SelectedAudio::Dub, win, cx))
                                    })
                            })
                            .child({
                                let cb = audio_cb;
                                let is_active = self.selected_audio == SelectedAudio::Hindi;
                                Button::new("audio-hi")
                                    .small()
                                    .label("Dub (Hindi)")
                                    .when(is_active, |b| b.primary())
                                    .when(!is_active, |b| b.outline())
                                    .when_some(cb, |b, h| {
                                        b.on_click(move |_ev, win, cx| h(&SelectedAudio::Hindi, win, cx))
                                    })
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Label::new("Quality:")
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(theme.muted_foreground),
                            )
                            .children(AVAILABLE_RESOLUTIONS.iter().map(|&res| {
                                let cb = res_cb.clone();
                                let is_active = self.selected_resolution.eq_ignore_ascii_case(res);
                                Button::new(format!("res-{}", res))
                                    .small()
                                    .label(res)
                                    .when(is_active, |b| b.primary())
                                    .when(!is_active, |b| b.outline())
                                    .when_some(cb, move |b, h| {
                                        b.on_click(move |_ev, win, cx| h(res, win, cx))
                                    })
                            })),
                    ),
            )
            // Row 2: Streaming Provider Selector
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .w_full()
                    .child(
                        Label::new("Provider:")
                            .text_xs()
                            .font_semibold()
                            .text_color(theme.muted_foreground)
                            .flex_shrink_0(),
                    )
                    .children(AVAILABLE_PROVIDERS.iter().map(|&(prov_key, prov_name)| {
                        let cb = prov_cb.clone();
                        let is_active = self.selected_provider.eq_ignore_ascii_case(prov_key);
                        Button::new(format!("prov-{}", prov_key))
                            .small()
                            .label(prov_name)
                            .when(is_active, |b| b.primary())
                            .when(!is_active, |b| b.outline())
                            .when_some(cb, move |b, h| {
                                b.on_click(move |_ev, win, cx| h(prov_key, win, cx))
                            })
                    }))
                    .child(
                        Tag::secondary()
                            .child("Auto Fallback Enabled")
                    ),
            )
    }
}
