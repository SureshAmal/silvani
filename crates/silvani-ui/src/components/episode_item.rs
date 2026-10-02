use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct EpisodeItem {
    id: ElementId,
    number: f32,
    title: Option<String>,
    is_watched: bool,
    on_play: ClickHandler,
}

impl EpisodeItem {
    pub fn new(id: impl Into<ElementId>, number: f32) -> Self {
        Self {
            id: id.into(),
            number,
            title: None,
            is_watched: false,
            on_play: None,
        }
    }

    pub fn title(mut self, title: Option<String>) -> Self {
        self.title = title;
        self
    }

    pub fn is_watched(mut self, watched: bool) -> Self {
        self.is_watched = watched;
        self
    }

    pub fn on_play(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_play = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for EpisodeItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ep_num_label = format!("EP {}", self.number);
        let ep_title = self.title.unwrap_or_else(|| format!("Episode {}", self.number));

        h_flex()
            .id(self.id)
            .items_center()
            .justify_between()
            .p_2()
            .rounded_md()
            .bg(if self.is_watched { theme.secondary.opacity(0.6) } else { theme.secondary })
            .border_1()
            .border_color(if self.is_watched { theme.primary.opacity(0.3) } else { theme.border })
            .hover(|s| s.border_color(theme.primary).bg(theme.accent))
            .cursor_pointer()
            .gap_2()
            .w(px(280.))
            .when_some(self.on_play, |d, cb| {
                d.on_click(move |ev, win, cx| cb(ev, win, cx))
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        Tag::secondary()
                            .child(ep_num_label)
                    )
                    .child(
                        Label::new(ep_title)
                            .text_sm()
                            .line_clamp(1),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .when(self.is_watched, |d| {
                        d.child(Tag::success().child("Watched ✓"))
                    })
                    .when(!self.is_watched, |d| {
                        d.child(
                            Button::new("play-btn")
                                .ghost()
                                .small()
                                .icon(IconName::Play),
                        )
                    }),
            )
    }
}
