use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{h_flex, ActiveTheme};
use gpui_kit::prelude::*;
use gpui_kit::*;

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct SearchBar {
    id: ElementId,
    search_input: Entity<InputState>,
    on_search: ClickHandler,
    on_history: ClickHandler,
}

impl SearchBar {
    pub fn new(id: impl Into<ElementId>, search_input: &Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            search_input: search_input.clone(),
            on_search: None,
            on_history: None,
        }
    }

    pub fn on_search(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_search = Some(Rc::new(handler));
        self
    }

    pub fn on_history(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_history = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SearchBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .id(self.id)
            .items_center()
            .gap_3()
            .w_full()
            .child(
                Label::new("SILVANI")
                    .text_xl()
                    .font_bold()
                    .text_color(theme.primary),
            )
            .child(
                h_flex()
                    .flex_1()
                    .child(Input::new(&self.search_input).cleanable(true)),
            )
            .child(
                Button::new("search-btn")
                    .primary()
                    .label("Search")
                    .when_some(self.on_search, |b, cb| {
                        b.on_click(move |ev, win, cx| cb(ev, win, cx))
                    }),
            )
            .child(
                Button::new("history-btn")
                    .outline()
                    .label("Watch History")
                    .when_some(self.on_history, |b, cb| {
                        b.on_click(move |ev, win, cx| cb(ev, win, cx))
                    }),
            )
    }
}
