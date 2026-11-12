//! A picker in the game's style: a parchment pill showing the choice (or a
//! placeholder in faded ink), opening a parchment list of the options in ink,
//! like the game's dropdowns. The option under the keyboard is shaded, the
//! chosen one ticked; a long list gets a search field.
//!
//! GPUI Kit's `Select` draws its list on the popover surface and its rows with
//! the theme's hover colours, which can't be made parchment and ink without
//! breaking every ghost button; so the app keeps its own, on the kit's
//! `Popover` (which places it and closes it on Escape or a click outside).
//!
//! Keyboard: Up and Down move through the options, Enter picks one.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable as _, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rems,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled as _, Subscription, Window, div, rems,
};

use super::controls::input;
use crate::theme;

/// An option of a picker: what it shows, and what it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct PickerItem<V> {
    pub title: SharedString,
    pub value: V,
}

impl<V> PickerItem<V> {
    pub fn new(title: impl Into<SharedString>, value: V) -> Self {
        Self {
            title: title.into(),
            value,
        }
    }
}

/// What a picker tells its owner.
pub enum PickerEvent<V> {
    /// An option was picked.
    Confirm(V),
}

/// A picker's options, choice and open state.
pub struct PickerState<V: Clone + PartialEq + 'static> {
    items: Vec<PickerItem<V>>,
    selected: Option<usize>,
    /// The option the keyboard is on, as a position in the options shown.
    highlighted: usize,
    open: bool,
    search: Option<Entity<InputState>>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    _search_changes: Option<Subscription>,
}

impl<V: Clone + PartialEq + 'static> EventEmitter<PickerEvent<V>> for PickerState<V> {}

/// The handle the open list takes the keyboard with.
impl<V: Clone + PartialEq + 'static> gpui_kit::Focusable for PickerState<V> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl<V: Clone + PartialEq + 'static> PickerState<V> {
    pub fn new(items: Vec<PickerItem<V>>, selected: Option<usize>, cx: &mut Context<Self>) -> Self {
        Self {
            items,
            selected,
            highlighted: 0,
            open: false,
            search: None,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            _search_changes: None,
        }
    }

    /// A picker with a search field over its options, for a long list.
    pub fn searchable(
        items: Vec<PickerItem<V>>,
        selected: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let changes = cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.highlighted = 0;
                cx.notify();
            }
        });
        Self {
            search: Some(search),
            _search_changes: Some(changes),
            ..Self::new(items, selected, cx)
        }
    }

    /// Replaces the options, keeping the choice when it is still one.
    pub fn set_items(&mut self, items: Vec<PickerItem<V>>, cx: &mut Context<Self>) {
        let chosen = self.selected_value().cloned();
        self.items = items;
        self.selected = chosen.and_then(|value| self.items.iter().position(|i| i.value == value));
        self.highlighted = 0;
        cx.notify();
    }

    pub fn selected_value(&self) -> Option<&V> {
        self.selected
            .and_then(|ix| self.items.get(ix))
            .map(|item| &item.value)
    }

    /// Chooses the option standing for `value`, or none when there is none.
    pub fn set_selected_value(&mut self, value: &V, cx: &mut Context<Self>) {
        self.selected = self.items.iter().position(|item| &item.value == value);
        cx.notify();
    }

    /// Chooses option `ix`, or none (a picker that adds something, ready for
    /// the next).
    pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
        self.selected = ix.filter(|ix| *ix < self.items.len());
        cx.notify();
    }

    /// The options shown: those matching the search, in order (indexes into
    /// the options).
    fn shown(&self, cx: &App) -> Vec<usize> {
        let query = self
            .search
            .as_ref()
            .map(|search| search.read(cx).value().to_lowercase())
            .unwrap_or_default();
        (0..self.items.len())
            .filter(|ix| query.is_empty() || self.items[*ix].title.to_lowercase().contains(&query))
            .collect()
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open = open;
        if open {
            match &self.search {
                Some(search) => {
                    search.update(cx, |search, cx| search.set_value("", window, cx));
                    search.read(cx).focus_handle(cx).focus(window, cx);
                }
                None => self.focus.focus(window, cx),
            }
            let shown = self.shown(cx);
            self.highlighted = self
                .selected
                .and_then(|chosen| shown.iter().position(|ix| *ix == chosen))
                .unwrap_or(0);
            self.scroll.scroll_to_item(self.highlighted);
        }
        cx.notify();
    }

    fn move_highlight(&mut self, by: isize, cx: &mut Context<Self>) {
        let count = self.shown(cx).len();
        if count == 0 {
            return;
        }
        let to = (self.highlighted as isize + by).rem_euclid(count as isize);
        self.highlighted = to as usize;
        self.scroll.scroll_to_item(self.highlighted);
        cx.notify();
    }

    /// Picks option `ix` (an index into the options).
    fn confirm(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(ix) else {
            return;
        };
        let value = item.value.clone();
        self.selected = Some(ix);
        self.open = false;
        cx.emit(PickerEvent::Confirm(value));
        cx.notify();
    }

    fn confirm_highlighted(&mut self, cx: &mut Context<Self>) {
        if let Some(ix) = self.shown(cx).get(self.highlighted).copied() {
            self.confirm(ix, cx);
        }
    }
}

/// A picker on `state`, in its parchment pill.
pub fn picker<V: Clone + PartialEq + 'static>(
    id: impl Into<ElementId>,
    state: &Entity<PickerState<V>>,
) -> Picker<V> {
    Picker {
        id: id.into(),
        state: state.clone(),
        placeholder: "Choose…".into(),
        prefix: None,
        label: None,
        menu_width: rems(16.),
    }
}

/// See [`picker`].
#[derive(IntoElement)]
pub struct Picker<V: Clone + PartialEq + 'static> {
    id: ElementId,
    state: Entity<PickerState<V>>,
    placeholder: SharedString,
    prefix: Option<SharedString>,
    label: Option<SharedString>,
    menu_width: Rems,
}

impl<V: Clone + PartialEq + 'static> Picker<V> {
    /// Shown, in faded ink, while nothing is chosen.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Written before the choice: "Sort by " → "Sort by Name".
    pub fn title_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// What the picker chooses, for assistive technology.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The list's width (16 rem by default).
    pub fn menu_width(mut self, width: Rems) -> Self {
        self.menu_width = width;
        self
    }
}

impl<V: Clone + PartialEq + 'static> RenderOnce for Picker<V> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let state = self.state.read(cx);
        let chosen = state.selected.and_then(|ix| state.items.get(ix));
        let title: SharedString = match chosen {
            Some(item) => match &self.prefix {
                Some(prefix) => format!("{prefix}{}", item.title).into(),
                None => item.title.clone(),
            },
            None => self.placeholder.clone(),
        };
        let (open, focus) = (state.open, state.focus.clone());
        let label = match &self.label {
            Some(label) => format!("{label}: {title}"),
            None => title.to_string(),
        };
        // Faded ink, still 4.9:1 on parchment.
        let faded = palette.ink.opacity(0.8);
        let trigger = Button::new(self.id.clone())
            .large()
            .h_12()
            .px_5()
            .w_full()
            .rounded_full()
            .cursor_pointer()
            .accessibility_label(label)
            .child(
                div()
                    .w_full()
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(chosen.is_none(), |this| this.text_color(faded))
                            .child(title),
                    )
                    .child(Icon::new(IconName::ChevronDown).size_4().flex_none()),
            );
        let (on_change, for_menu) = (self.state.clone(), self.state.clone());
        let mut trigger_style = StyleRefinement::default();
        trigger_style.size.width = Some(gpui_kit::relative(1.).into());
        Popover::new(self.id)
            .trigger(trigger)
            .trigger_style(trigger_style)
            .open(open)
            .on_open_change(move |open, window, cx| {
                on_change.update(cx, |state, cx| state.set_open(*open, window, cx));
            })
            .track_focus(&focus)
            .bg(palette.parchment)
            .rounded(rems(1.5))
            .text_color(palette.ink)
            .p_1()
            .w(self.menu_width)
            .content(move |_, window, cx| menu(&for_menu, window, cx))
    }
}

/// The open list: the search field (if any), then the options.
fn menu<V: Clone + PartialEq + 'static>(
    state: &Entity<PickerState<V>>,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ink = theme::palette(cx).ink;
    let picker = state.read(cx);
    let shown = picker.shown(cx);
    let (highlighted, selected) = (picker.highlighted, picker.selected);
    let rows = shown.iter().enumerate().map(|(position, ix)| {
        let ix = *ix;
        let item = &picker.items[ix];
        let chosen = selected == Some(ix);
        let state = state.clone();
        div()
            .id(("picker-option", ix))
            .h_flex()
            .h_10()
            .px_3()
            .gap_2()
            .rounded_full()
            .cursor_pointer()
            .when(position == highlighted, |this| this.bg(ink.opacity(0.14)))
            .hover(|this| this.bg(ink.opacity(0.08)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(if chosen {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .child(item.title.clone()),
            )
            .when(chosen, |this| {
                this.child(Icon::new(IconName::Check).size_4().flex_none())
            })
            .on_click(move |_, _, cx| {
                state.update(cx, |state, cx| state.confirm(ix, cx));
            })
    });
    let keys = state.clone();
    let search = picker.search.clone();
    let none = shown.is_empty();
    div()
        .v_flex()
        .gap_1()
        .capture_key_down(move |event, _, cx| {
            let key = event.keystroke.key.as_str();
            match key {
                "down" => keys.update(cx, |state, cx| state.move_highlight(1, cx)),
                "up" => keys.update(cx, |state, cx| state.move_highlight(-1, cx)),
                "enter" => keys.update(cx, |state, cx| state.confirm_highlighted(cx)),
                _ => return,
            }
            cx.stop_propagation();
        })
        .children(search.map(|search| div().p_1().child(input(&search, cx).small())))
        .child(
            div()
                .id("picker-options")
                .max_h(rems(20.))
                .overflow_y_scroll()
                .track_scroll(&picker.scroll)
                .v_flex()
                .gap_0p5()
                .children(rows),
        )
        .when(none, |this| {
            this.child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(ink.opacity(0.8))
                    .child("Nothing matches."),
            )
        })
        .into_any_element()
}
