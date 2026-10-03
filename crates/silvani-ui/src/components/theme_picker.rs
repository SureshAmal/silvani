use std::rc::Rc;
use gpui_kit::component::{
    IndexPath,
    list::{ListDelegate, ListItem, ListState},
    ThemeConfig,
};
use gpui_kit::{
    App, Context, Entity, FocusHandle, ParentElement as _, Styled as _, Subscription, Task, Window,
};

pub struct ThemePickerDelegate {
    all_themes: Vec<Rc<ThemeConfig>>,
    filtered_themes: Vec<Rc<ThemeConfig>>,
    selected_index: Option<IndexPath>,
}

impl ThemePickerDelegate {
    pub fn new(themes: Vec<Rc<ThemeConfig>>) -> Self {
        let filtered_themes = themes.clone();
        let selected_index = if !filtered_themes.is_empty() {
            Some(IndexPath::new(0))
        } else {
            None
        };
        Self {
            all_themes: themes,
            filtered_themes,
            selected_index,
        }
    }

    pub fn get_theme(&self, ix: IndexPath) -> Option<Rc<ThemeConfig>> {
        self.filtered_themes.get(ix.row).cloned()
    }
}

impl ListDelegate for ThemePickerDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.filtered_themes.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_themes = self.all_themes.clone();
        } else {
            self.filtered_themes = self
                .all_themes
                .iter()
                .filter(|t| t.name.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        self.selected_index = if !self.filtered_themes.is_empty() {
            Some(IndexPath::new(0))
        } else {
            None
        };
        cx.notify();
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let theme = self.filtered_themes.get(ix.row)?;
        let selected = self.selected_index == Some(ix);

        // Simple and direct like reference image: no icons, no badges, pure theme name
        Some(
            ListItem::new(("theme-row", ix.row))
                .px_3()
                .py_1()
                .selected(selected)
                .child(theme.name.clone()),
        )
    }
}

pub struct ThemePickerState {
    pub list: Entity<ListState<ThemePickerDelegate>>,
    pub trap_handle: FocusHandle,
    pub _subscription: Subscription,
}
