use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use silvani_meta::models::RelatedAnime;

pub type SeasonSelectHandler = Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct SeasonSelector {
    id: ElementId,
    current_title: String,
    relations: Vec<RelatedAnime>,
    on_select: SeasonSelectHandler,
}

impl SeasonSelector {
    pub fn new(id: impl Into<ElementId>, current_title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            current_title: current_title.into(),
            relations: Vec::new(),
            on_select: None,
        }
    }

    pub fn relations(mut self, relations: Vec<RelatedAnime>) -> Self {
        self.relations = relations;
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SeasonSelector {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let on_select = self.on_select.clone();

        v_flex()
            .id(self.id)
            .gap_2()
            .w_full()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new("Franchise Seasons & Relations")
                            .text_sm()
                            .font_semibold()
                            .text_color(theme.muted_foreground),
                    )
                    .when(self.relations.is_empty(), |d| {
                        d.child(Tag::secondary().child("Single Season / Standalone"))
                    }),
            )
            .when(!self.relations.is_empty(), |container| {
                container.child(
                    h_flex()
                        .id("franchise-season-pills")
                        .items_center()
                        .gap_2()
                        .overflow_x_scroll()
                        .child(
                            Button::new("current-season-tab")
                                .primary()
                                .small()
                                .label(format!("Current: {}", self.current_title)),
                        )
                        .children(self.relations.into_iter().enumerate().map(|(idx, rel)| {
                            let rel_id = rel.id.clone();
                            let rel_type = rel.relation_type.replace('_', " ");
                            let btn_label = format!("[{}] {}", rel_type, rel.title.display());
                            let cb = on_select.clone();

                            Button::new(format!("rel-season-btn-{}", idx))
                                .outline()
                                .small()
                                .label(btn_label)
                                .when_some(cb, move |b, handler| {
                                    let id = rel_id.clone();
                                    b.on_click(move |_ev, win, cx| handler(&id, win, cx))
                                })
                        })),
                )
            })
    }
}
