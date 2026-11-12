//! The archive lists' filter bar, after genshinlist.vercel.app's: a search
//! field, a "Filter" button that opens the filters in a drawer
//! (each group a grid of buttons, ticked when on), the order as a parchment
//! pill, a button that reverses it, and under the bar the filters that are
//! on, each a chip that removes it.
//!
//! A filter applies as soon as it is clicked; the drawer only gathers them.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, App, ClickEvent, Div, Entity, FocusHandle, FontWeight,
    ImageSource, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    SharedString, Styled as _, Window, deferred, div, ease_out_quint, img, rems,
};

use super::Sort;
use wish_ui::{PickerItem, PickerState, button, icon_button, input, picker};

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
    pub title: &'static str,
    pub options: Vec<FilterOption>,
}

/// The orders, as the sort pill lists them, newest first.
pub(super) fn sort_options() -> Vec<PickerItem<Sort>> {
    vec![
        PickerItem::new("Version", Sort::Newest),
        PickerItem::new("Name", Sort::Name),
        PickerItem::new("Rarity", Sort::Rarity),
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
            "Filter".to_owned()
        } else {
            format!("Filter · {}", on.len())
        })
        .on_click(open_filters);
    // The direction shows in the icon: every button has the same parchment.
    let direction = icon_button(
        "reverse-order",
        if reversed {
            IconName::ArrowUpNarrowWide
        } else {
            IconName::ArrowDownWideNarrow
        },
        if reversed {
            "Order reversed"
        } else {
            "Reverse the order"
        },
    )
    .on_click(reverse);
    let chips = on.iter().enumerate().map(|(ix, option)| {
        let toggle = option.toggle.clone();
        Button::new(("active-filter", ix))
            .small()
            .h_8()
            .px_3()
            .rounded_full()
            .cursor_pointer()
            .icon(Icon::new(IconName::Close))
            .label(option.label.clone())
            .accessibility_label(format!("Remove the filter {}", option.label))
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
                            .title_prefix("Sort by ")
                            .accessibility_label("Order")
                            .menu_width(rems(13.)),
                    ),
                )
                .child(direction),
        )
        .when(!on.is_empty(), |this| {
            this.child(
                div().h_flex().flex_wrap().gap_2().children(chips).child(
                    button("clear-filters")
                        .ghost()
                        .h_8()
                        .label("Clear all")
                        .on_click(move |_, window, cx| clear(window, cx)),
                ),
            )
        })
}

/// Whether the filter drawer shows: sliding in and open, or sliding out
/// before it goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Drawer {
    #[default]
    Closed,
    Open,
    Closing,
}

/// How long the drawer takes to slide in or out.
pub(super) const DRAWER_SLIDE: Duration = Duration::from_millis(200);

/// The drawer's width.
const DRAWER_REMS: f32 = 26.;

/// Above the page, under the dialogs (GPUI Kit's start at 10).
const DRAWER_PRIORITY: usize = 5;

/// The filters in a drawer at the right of the page, over a veil, the whole
/// page's height: it slides in, and out again when closed (Done, the cross,
/// Escape, or a click on the veil). It is drawn from the list's own state, so
/// a click shows at once.
pub(super) fn filter_drawer(
    state: Drawer,
    focus: &FocusHandle,
    groups: Vec<FilterGroup>,
    close: Handler,
    clear: Handler,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let gold = wish_ui::theme::palette(cx).gold;
    let closing = state == Drawer::Closing;
    // Each way its own animation, so closing starts from the open drawer.
    let (slide, fade) = if closing {
        ("drawer-out", "veil-out")
    } else {
        ("drawer-in", "veil-in")
    };
    let shown = move |delta: f32| if closing { 1. - delta } else { delta };
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
    let veil = div()
        .id("filter-veil")
        .absolute()
        .inset_0()
        .bg(theme.overlay)
        .on_mouse_down(MouseButton::Left, {
            let close = close.clone();
            move |_, window, cx| close(window, cx)
        })
        .with_animation(
            fade,
            Animation::new(DRAWER_SLIDE).with_easing(ease_out_quint()),
            move |this, delta| this.opacity(shown(delta)),
        );
    let panel = div()
        .id("filter-drawer")
        .track_focus(focus)
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(rems(DRAWER_REMS))
        .v_flex()
        .bg(theme.group_box)
        .text_color(theme.foreground)
        .border_l_1()
        .border_color(theme.border)
        .shadow_xl()
        // Clicks inside stay inside; Escape closes it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_key_down({
            let close = close.clone();
            move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    close(window, cx);
                }
            }
        })
        .child(
            div()
                .h_flex()
                .px_5()
                .pt_5()
                .pb_3()
                .child(
                    div()
                        .flex_1()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(gold)
                        .child("Filter"),
                )
                .child(
                    icon_button("close-filters", IconName::Close, "Close")
                        .ghost()
                        .on_click({
                            let close = close.clone();
                            move |_, window, cx| close(window, cx)
                        }),
                ),
        )
        .child(
            div()
                .id("filter-groups")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(div().v_flex().gap_6().px_5().pb_4().children(content)),
        )
        .child(
            div()
                .h_flex()
                .gap_2()
                .px_5()
                .py_4()
                .border_t_1()
                .border_color(theme.border)
                .child(
                    button("filters-clear")
                        .ghost()
                        .label("Clear all")
                        .on_click(move |_, window, cx| clear(window, cx)),
                )
                .child(div().flex_1())
                .child(
                    button("filters-done")
                        .label("Done")
                        .on_click(move |_, window, cx| close(window, cx)),
                ),
        )
        .with_animation(
            slide,
            Animation::new(DRAWER_SLIDE).with_easing(ease_out_quint()),
            move |this, delta| this.right(rems(-DRAWER_REMS * (1. - shown(delta)))),
        );
    deferred(div().absolute().inset_0().child(veil).child(panel)).with_priority(DRAWER_PRIORITY)
}

/// A filter in the drawer: a 48 px button with its icon and label, outlined
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
        .ghost()
        .h_12()
        .px_3()
        .rounded_full()
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
            format!("{label}, on")
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
