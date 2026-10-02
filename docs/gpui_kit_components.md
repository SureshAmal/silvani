# GPUI Kit Component & Event Reference

Comprehensive guide to all UI components, state contracts, and event-handling patterns in `gpui-kit` (`0.7.0`).

---

## 1. Application Architecture & Window Setup

Applications depend solely on `gpui-kit`. The root window must mount `Root` (handled automatically by `gpui_kit::open_window`) to enable overlays (dialogs, sheets, tooltips, context menus, notifications).

```rust
use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx); // MUST be called first

            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| MainView::new(window, cx))
            })
            .expect("failed to open window");
        });
}
```

---

## 2. Event Handling Patterns

GPUI Kit components fall into 4 interaction patterns:

### Pattern A: Command Callbacks (`on_click`)
For stateless action triggers (buttons, clickable items).
```rust
Button::new("submit")
    .primary()
    .label("Submit")
    .on_click(cx.listener(|this, _event, window, cx| {
        this.submit_form(window, cx);
    }))
```

### Pattern B: Controlled Value Requests (`on_change`)
For stateless form toggles (Checkbox, Switch, Radio). The callback requests a state mutation; the application owns and stores the boolean/index.
```rust
Switch::new("autoplay")
    .checked(self.autoplay)
    .on_change(cx.listener(|this, next: &bool, _window, cx| {
        this.autoplay = *next;
        cx.notify();
    }))
```

### Pattern C: Retained Entity Subscriptions (`cx.subscribe_in` / `cx.subscribe`)
For stateful components (`InputState`, `SelectState`, `SliderState`, `ListState`, `TableState`). The view retains `Entity<State>` and keeps subscription handles in a `Vec<Subscription>`.
```rust
// Store subscription handle in the owning view struct
self._subscriptions.push(cx.subscribe_in(&self.search_input, window, |this, state, event, window, cx| {
    match event {
        InputEvent::Change => {
            let query = state.read(cx).value();
            this.filter_results(query, cx);
        }
        InputEvent::PressEnter { .. } => {
            this.execute_search(cx);
        }
        InputEvent::Focus | InputEvent::Blur => {}
    }
}));
```

### Pattern D: Overlay Triggers via Window (`WindowExt`)
Root mounts overlay layers automatically. Open overlays directly via `Window`:
```rust
window.open_dialog(cx, |dialog, _, _| { ... });
window.open_alert_dialog(cx, |alert, _, _| { ... });
window.open_sheet(cx, |sheet, _, _| { ... });
window.push_notification("Saved successfully", cx);
```

---

## 3. Complete Component Catalog

### Buttons & Actions (`gpui_kit::component::button::*`)

| Component | State / Model | Key Methods | Events |
|---|---|---|---|
| `Button` | Stateless | `new(id)`, `label(&str)`, `icon(IconName)`, `primary()`, `danger()`, `warning()`, `success()`, `ghost()`, `link()`, `outline()`, `disabled(bool)`, `loading(bool)`, `selected(bool)`, `xsmall()`, `small()`, `large()`, `tooltip(&str)` | `on_click(impl Fn(&ClickEvent, &mut Window, &mut App))` |
| `ButtonGroup` | Stateless | `new(id)`, `child(Button)`, `outline()`, `compact()` | `on_click(impl Fn(&[usize], &mut Window, &mut App))` |
| `DropdownButton` | Stateless | `new(id)`, `label(&str)`, `menu(impl Fn(&mut Window, &mut App) -> Menu)` | Dispatches menu item selections |
| `Kbd` | Stateless | `new()`, `child(&str)`, `keystroke(&str)` | Static display of hotkey badges |

```rust
Button::new("play")
    .primary()
    .icon(IconName::Play)
    .label("Play Now")
    .on_click(cx.listener(|this, _, _, cx| {
        this.start_playback(cx);
    }))
```

---

### Form Controls & Inputs (`gpui_kit::component::*`)

| Component | State / Model | Key Methods | Events |
|---|---|---|---|
| `Input` (`input::*`) | Retained `Entity<InputState>` | `new(&entity)`, `cleanable(bool)`, `disabled(bool)`, `prefix(element)`, `suffix(element)`, `content_type(InputContentType)`, `mask_toggle()`, `appearance(bool)` | `InputEvent::Change`, `InputEvent::PressEnter`, `InputEvent::Focus`, `InputEvent::Blur` |
| `SearchInput` | Retained `Entity<InputState>` | `new(&entity)`, built-in search icon and clear button | Inherits `InputEvent` |
| `NumberInput` | Retained `Entity<NumberInputState>` | `new(&entity)`, `step(f64)`, `min(f64)`, `max(f64)` | Emits value change events |
| `Textarea` | Retained `Entity<InputState>` | `new(&entity)`, multi-line input mode | Inherits `InputEvent` |
| `Checkbox` (`checkbox::*`) | Controlled `bool` | `new(id)`, `checked(bool)`, `label(&str)`, `disabled(bool)` | `on_change(cx.listener(\|this, next: &bool, _, cx\| ...))` |
| `Switch` (`switch::*`) | Controlled `bool` | `new(id)`, `checked(bool)`, `disabled(bool)` | `on_change(cx.listener(\|this, next: &bool, _, cx\| ...))` |
| `Radio` (`radio::*`) | Controlled `bool` | `new(id)`, `checked(bool)`, `label(&str)` | `on_change(cx.listener(\|this, next: &bool, _, cx\| ...))` |
| `RadioGroup` | Controlled `Option<usize>` | `new(id)`, `selected_index(Option<usize>)`, `child(Radio)` | `on_change(cx.listener(\|this, index: &usize, _, cx\| ...))` |
| `Slider` (`slider::*`) | Retained `Entity<SliderState>` | `new(&entity)`, `horizontal()`, `vertical()`, `disabled(bool)`, `reverse()` | `SliderEvent::Change(SliderValue)` |
| `ColorPicker` | Controlled / Retained | `new(id)`, `color(Hsla)`, `palette(&[Hsla])` | `on_change(impl Fn(Hsla))` |
| `Rating` | Controlled `f32` | `new(id)`, `value(f32)`, `max(u32)`, `icon(IconName)` | `on_change(impl Fn(f32))` |
| `Form` (`form::*`) | Container | `new()`, `v_form()`, `h_form()`, `label_layout(Axis)`, `columns(usize)`, `child(Field)`, `footer(element)` | Structural layout |
| `Field` (`form::*`) | Container | `new()`, `field()`, `label(&str)`, `description(&str)`, `col_span(usize)`, `child(element)` | Structural field pairing |

```rust
// Retained Text Input
let name_input = cx.new(|cx| {
    InputState::new(window, cx)
        .placeholder("Enter anime title...")
        .cleanable(true)
});

// Slider (e.g. video seeker or volume)
let volume_slider = cx.new(|_| SliderState::new(0.8).min(0.0).max(1.0).step(0.05));
cx.subscribe(&volume_slider, |this, _, event: &SliderEvent, cx| {
    if let SliderEvent::Change(val) = event {
        this.set_volume(val.end(), cx);
    }
});
```

---

### Selections & Dropdowns (`gpui_kit::component::*`)

| Component | State / Model | Key Methods | Events |
|---|---|---|---|
| `Select` (`select::*`) | Retained `Entity<SelectState<T>>` | `new(&entity)`, `placeholder(&str)`, `disabled(bool)`, `cleanable(bool)` | `SelectEvent::Select(IndexPath)` |
| `Combobox` (`combobox::*`) | Retained `Entity<ComboboxState<T>>` | `new(&entity)`, `searchable(true)`, `placeholder(&str)` | Emits selection & filter updates |
| `DatePicker` (`time::date_picker::*`) | Retained `Entity<DatePickerState>` | `new(&entity)`, `format(&str)` | Emits date selection updates |
| `Calendar` (`time::calendar::*`) | Retained `Entity<CalendarState>` | `new(&entity)`, single or range selection | Emits date click events |
| `TimeField` (`time::time_field::*`) | Retained `Entity<TimeFieldState>` | `new(&entity)`, `step(u32)` | Emits time value updates |

```rust
let provider_select = cx.new(|cx| {
    SelectState::new(
        vec!["HiAnime", "GogoAnime", "AnimeOnsen", "Muse India"],
        Some(IndexPath::default()),
        window,
        cx,
    )
});
```

---

### Collections & Tables (`gpui_kit::component::*`)

| Component | State / Model | Key Methods | Events |
|---|---|---|---|
| `VirtualList` | Stateless / Delegate | `v_virtual_list(id, count, render_item)`, `h_virtual_list(...)`, `track_scroll(handle)` | Dynamic lazy rendering on scroll |
| `List` (`list::*`) | Retained `Entity<ListState<D>>` | `new(&entity)`, `searchable(bool)` | `ListEvent::Select(IndexPath)` |
| `ListItem` | Stateless item wrapper | `new(id)`, `child(element)`, `selected(bool)`, `disabled(bool)` | `on_click(cx.listener(...))` |
| `Table` / `DataTable` (`table::*`) | Retained `Entity<TableState<D>>` | `new(&entity)`, sorting, column resizing, row selection | `TableEvent::Select(usize)`, `TableEvent::Sort(column)` |
| `Tree` (`tree::*`) | Retained `Entity<TreeState<T>>` | Hierarchical tree with expand/collapse nodes | Node expansion and click events |

```rust
// Fast virtualized list for episode rows
v_virtual_list("episodes-list", self.episodes.len(), move |idx, _window, _cx| {
    let ep = &this.episodes[idx];
    div()
        .h(px(40.))
        .p_2()
        .child(format!("Episode {}: {}", ep.number, ep.title))
        .into_any_element()
})
```

---

### Navigation & Layout Containers (`gpui_kit::component::*`)

| Component | State / Model | Key Methods | Events |
|---|---|---|---|
| `TabBar` (`tab::*`) | Controlled `usize` | `new(id)`, `child(Tab)`, `selected_index(usize)` | `on_change(cx.listener(\|this, idx: &usize, _, cx\| ...))` |
| `Tab` (`tab::*`) | Child part | `new(id)`, `child(element)`, `icon(IconName)`, `closeable(bool)` | `on_close(cx.listener(...))` |
| `Sidebar` (`sidebar::*`) | Container | `new(id)`, `header(element)`, `footer(element)`, `child(element)`, `collapsed(bool)` | Toggle collapse actions |
| `Accordion` (`accordion::*`) | Controlled / Retained | `new(id)`, `child(AccordionItem)` | Expand/collapse callbacks |
| `Collapsible` (`collapsible::*`) | Controlled `bool` | `new(id)`, `open(bool)`, `trigger(element)`, `child(element)` | Open state toggles |
| `ResizablePanelGroup` (`resizable::*`) | Panel Group State | `h_resizable(id, state)`, `v_resizable(...)`, `child(ResizablePanel)` | `ResizablePanelEvent::Resize` |
| `ResizablePanel` | Panel child | `new()`, `initial_size(Pixels / DefiniteLength)`, `min_size(Pixels)` | Resize geometry |
| `Dock` (`dock::*`) | Workspace Dock State | Multi-panel draggable IDE-style workspace panes | Pane focus and dock events |
| `Pagination` (`pagination::*`) | Controlled `usize` | `new(id)`, `total(usize)`, `current(usize)`, `page_size(usize)` | `on_change(impl Fn(usize))` |
| `Stepper` (`stepper::*`) | Controlled `usize` | `new(id)`, `current_step(usize)`, `child(Step)` | Step click callbacks |

---

### Feedback, Overlays & Dialogs (`gpui_kit::component::*`)

| Component | Trigger Context | Key Methods | Events / Callbacks |
|---|---|---|---|
| `Dialog` (`dialog::*`) | `window.open_dialog` | `title(&str)`, `child(element)`, `footer(DialogFooter)` | `DialogAction`, `DialogClose` buttons |
| `AlertDialog` | `window.open_alert_dialog` | `title(&str)`, `description(&str)`, `confirm()`, `ok_text(&str)`, `ok_variant(ButtonVariant)` | `on_ok(impl Fn(&mut Window, &mut App) -> bool)`, `on_cancel(...)` |
| `Sheet` (`sheet::*`) | `window.open_sheet` | `title(&str)`, `side(Side::Left / Right / Top / Bottom)`, `child(element)` | Slide-out overlay dismissal |
| `Popover` (`popover::*`) | Attached to Element | `new(id)`, `trigger(element)`, `content(element)`, `placement(Placement)` | Anchor click opens popup |
| `Tooltip` (`tooltip::*`) | `.tooltip(...)` on element | `new(&str)` | Mouse hover shows tooltip |
| `Notification` | `window.push_notification` | `new(&str)`, `info()`, `success()`, `warning()`, `error()`, `message(&str)` | Timed auto-dismissal |
| `Alert` (`alert::*`) | Inline component | `new()`, `info()`, `warning()`, `danger()`, `title(&str)`, `child(element)` | Inline banners |
| `Badge` (`badge::*`) | Inline indicator | `new()`, `label(&str)`, `count(usize)`, `primary()`, `danger()`, `dot()` | Visual label |
| `Tag` (`tag::*`) | Inline chip | `new(id)`, `label(&str)`, `closable(bool)` | `on_close(cx.listener(...))` |
| `Spinner` (`spinner::*`) | Loading indicator | `new()`, `small()`, `medium()`, `large()` | Animated loading spinner |
| `Progress` (`progress::*`) | Loading bar | `new(id)`, `value(f32)` (0.0 to 1.0) | Visual progress bar |
| `ProgressCircle` | Loading circle | `new(id)`, `value(f32)` | Circular progress |
| `Skeleton` (`skeleton::*`) | Placeholder loader | `new()`, `w(DefiniteLength)`, `h(...)`, `rounded()` | Shimmering placeholder |

```rust
// Confirmation Alert Dialog
window.open_alert_dialog(cx, |alert, _window, _cx| {
    alert
        .title("Clear Watch History?")
        .description("This will remove all saved watch progress from your local database.")
        .confirm()
        .ok_text("Clear History")
        .ok_variant(ButtonVariant::Danger)
        .on_ok(|_window, _cx| {
            // return true to close dialog
            true
        })
});
```

---

### Menus & Context Menus (`gpui_kit::component::menu::*`)

| Component | Constructor | Key Methods | Events |
|---|---|---|---|
| `Menu` | `Menu::new()` | `child(MenuItem)`, `separator()` | Container for menu items |
| `MenuItem` | `MenuItem::new(id)` | `label(&str)`, `icon(IconName)`, `shortcut(Keystroke)`, `disabled(bool)` | `on_click(cx.listener(\|this, _, window, cx\| ...))` |
| `ContextMenu` | `.context_menu(...)` on any element | Attaches right-click contextual popup to any `div()` | Right-click trigger |
| `DropdownMenu` | Attached to `DropdownButton` | Attaches left-click popup to button | Click trigger |

```rust
div()
    .id("anime-card")
    .context_menu(|menu, _window, _cx| {
        menu
            .child(MenuItem::new("play").label("Play Now").icon(IconName::Play))
            .child(MenuItem::new("favorite").label("Add to Favorites").icon(IconName::Heart))
            .separator()
            .child(MenuItem::new("copy_link").label("Copy Stream Link").icon(IconName::Copy))
    })
```

---

### Layout Helpers & Theme Access

```rust
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName};

// Flex helpers
h_flex().gap_2().items_center() // Horizontal row with gap 2 and vertically centered items
v_flex().gap_4().flex_1()       // Vertical column taking full height

// Theming tokens
cx.theme().primary      // Accent color
cx.theme().background   // Main background
cx.theme().surface      // Card / container surface
cx.theme().border       // Subtle border color
cx.theme().foreground   // Primary text
cx.theme().muted        // Secondary text
cx.theme().destructive  // Error / Danger color
```

---

## 4. Downloaded Skills Reference

The complete official skills from `longbridge/gpui-kit` are installed locally in `docs/skills/`:

- **[`docs/skills/gpui-kit/SKILL.md`](file:///home/suresh/dev/projects/silvani/docs/skills/gpui-kit/SKILL.md)**: Main architecture, setup, state ownership, element identity, and component conventions.
- **[`docs/skills/gpui-kit/references/coding-guides.md`](file:///home/suresh/dev/projects/silvani/docs/skills/gpui-kit/references/coding-guides.md)**: Normative coding rules, contexts, phases, async rules, and testing checklist.
- **[`docs/skills/gpui-kit/references/conventions.md`](file:///home/suresh/dev/projects/silvani/docs/skills/gpui-kit/references/conventions.md)**: Component-family conventions (`on_click` vs `on_change`, Form layouts, trait imports).
- **[`docs/skills/gpui-kit/references/usage.md`](file:///home/suresh/dev/projects/silvani/docs/skills/gpui-kit/references/usage.md)**: Usage guide and component recipes.
- **[`docs/skills/gpui-kit-design-guides/SKILL.md`](file:///home/suresh/dev/projects/silvani/docs/skills/gpui-kit-design-guides/SKILL.md)**: Official Design Guides for layout, spacing, colors, interaction states, and copy guidelines.
