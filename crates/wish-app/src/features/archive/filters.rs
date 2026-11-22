//! The archive lists' filter bar, after genshinlist.vercel.app's: a search
//! field, a "Filter" button that opens the filters in a drawer
//! (each group a grid of buttons, ticked when on), the order as a picker, a
//! button that reverses it, and under the bar the filters that are
//! on, each a chip that removes it.
//!
//! A filter applies as soon as it is clicked; the drawer only gathers them.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::InputState;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, Div, Entity, FocusHandle, FontWeight, ImageSource, IntoElement,
    ParentElement as _, SharedString, Styled as _, Window, div, img, rems,
};

use super::Sort;
use wish_ui::ButtonExt as _;
use wish_ui::{Drawer, DrawerState, PickerItem, PickerState, button, icon_button, input, picker};

/// What a filter or the bar does when clicked.
pub(super) type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One filter: its label, an icon (an element), whether it is on, and what
/// turns it on or off.
pub(super) struct FilterOption {
    pub label: SharedString,
    pub icon: Option<ImageSource>,
    pub on: bool,
    pub toggle: Handler,
}

/// A group of filters in the sheet, under its title.
pub(super) struct FilterGroup {
    pub title: String,
    pub options: Vec<FilterOption>,
}

/// Puts the texts a list worked out ahead in the current language: the
/// sort options and the search's placeholder (after a change of language).
pub(super) fn refresh_texts(
    sort: &Entity<PickerState<Sort>>,
    search: &Entity<InputState>,
    placeholder: String,
    window: &mut Window,
    cx: &mut App,
) {
    sort.update(cx, |sort, cx| sort.set_items(sort_options(), cx));
    search.update(cx, |search, cx| {
        search.set_placeholder(placeholder, window, cx)
    });
}

/// The orders, as the sort picker lists them, newest first.
pub(super) fn sort_options() -> Vec<PickerItem<Sort>> {
    vec![
        PickerItem::new(t!("archive-sort-version"), Sort::Newest),
        PickerItem::new(t!("archive-sort-name"), Sort::Name),
        PickerItem::new(t!("archive-sort-rarity"), Sort::Rarity),
    ]
}

/// The bar above a list: search, Filter (with how many are on), the order
/// and its direction; then a chip for each filter on.
#[allow(clippy::too_many_arguments)]
pub(super) fn filter_bar(
    search: &Entity<InputState>,
    sort: &Entity<PickerState<Sort>>,
    reversed: bool,
    groups: &[FilterGroup],
    open_filters: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    reverse: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    clear: Handler,
    cx: &App,
) -> Div {
    let on: Vec<&FilterOption> = groups
        .iter()
        .flat_map(|group| group.options.iter())
        .filter(|option| option.on)
        .collect();
    let filter = button("open-filters")
        .icon(Icon::new(IconName::Funnel))
        .label(if on.is_empty() {
            t!("archive-filter")
        } else {
            t!("archive-filter-count", count = on.len())
        })
        .on_click(open_filters);
    // The direction shows in the icon: every button has the same fill.
    let direction = icon_button(
        "reverse-order",
        if reversed {
            IconName::ArrowUpNarrowWide
        } else {
            IconName::ArrowDownWideNarrow
        },
        if reversed {
            t!("archive-order-reversed")
        } else {
            t!("archive-reverse-order")
        },
    )
    .on_click(reverse);
    let chips = on.iter().enumerate().map(|(ix, option)| {
        let toggle = option.toggle.clone();
        Button::new(("active-filter", ix))
            .small()
            .h_8()
            .px_3()
            .cursor_pointer()
            .icon(Icon::new(IconName::Close))
            .label(option.label.clone())
            .accessibility_label(t!("archive-remove-filter", filter = option.label.as_ref()))
            .on_click(move |_, window, cx| toggle(window, cx))
    });
    div()
        .v_flex()
        .gap_3()
        .child(
            div()
                .h_flex()
                .gap_3()
                .child(div().flex_1().min_w(rems(12.)).child(input(search, cx)))
                .child(filter)
                .child(
                    div().w(rems(13.)).child(
                        picker("sort", sort)
                            .title_prefix(t!("archive-sort-by"))
                            .accessibility_label(t!("archive-order"))
                            .menu_width(rems(13.)),
                    ),
                )
                .child(direction),
        )
        .when(!on.is_empty(), |this| {
            this.child(
                div().h_flex().flex_wrap().gap_2().children(chips).child(
                    button("clear-filters")
                        .quiet(cx)
                        .h_8()
                        .label(t!("archive-clear-all"))
                        .on_click(move |_, window, cx| clear(window, cx)),
                ),
            )
        })
}

/// The filters in a drawer at the right of the page (`wish_ui::Drawer`):
/// each group a grid of buttons, "Clear all" and Done at the bottom.
pub(super) fn filter_drawer(
    state: DrawerState,
    focus: &FocusHandle,
    groups: Vec<FilterGroup>,
    close: Handler,
    clear: Handler,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let content = groups.into_iter().map(|group| {
        div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(group.title),
            )
            .child(
                div().grid().grid_cols(2).gap_2().children(
                    group
                        .options
                        .into_iter()
                        .map(|option| filter_button(option, cx)),
                ),
            )
    });
    let drawer = Drawer::new(
        "filters",
        state,
        focus,
        t!("archive-filter"),
        t!("archive-close"),
        close.clone(),
    );
    content
        .fold(drawer, Drawer::child)
        .footer_child(
            button("filters-clear")
                .quiet(cx)
                .label(t!("archive-clear-all"))
                .on_click(move |_, window, cx| clear(window, cx)),
        )
        .footer_child(div().flex_1())
        .footer_child(
            button("filters-done")
                .label(t!("archive-done"))
                .on_click(move |_, window, cx| close(window, cx)),
        )
}

/// A filter in the drawer: a 32 px button with its icon and label, outlined
/// at rest; on, filled with the primary colour and ticked.
fn filter_button(option: FilterOption, cx: &App) -> Button {
    let theme = cx.theme();
    let FilterOption {
        label,
        icon,
        on,
        toggle,
    } = option;
    Button::new(SharedString::from(format!("filter-{label}")))
        .quiet(cx)
        .h_8()
        .px_3()
        .border_2()
        .cursor_pointer()
        .map(|this| {
            if on {
                this.bg(theme.primary)
                    .border_color(theme.primary)
                    .text_color(theme.primary_foreground)
            } else {
                this.border_color(theme.border).text_color(theme.foreground)
            }
        })
        .accessibility_label(if on {
            t!("archive-filter-on", filter = label.as_ref())
        } else {
            label.to_string()
        })
        .child(
            div()
                .w_full()
                .h_flex()
                .gap_2()
                .child(
                    div()
                        .size_6()
                        .flex_none()
                        .children(icon.map(|icon| img(icon).size_full())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .when(on, |this| {
                    this.child(
                        Icon::new(IconName::Check)
                            .size_4()
                            .text_color(theme.primary_foreground),
                    )
                }),
        )
        .on_click(move |_, window, cx| toggle(window, cx))
}
