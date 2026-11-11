use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Div, FontWeight, Hsla, IntoElement, ParentElement as _, SharedString, Styled as _, div,
};

/// A recessed tile inside a card, holding one figure: a label and its
/// detail on the left, the value large on the right (paimon.moe's counter).
/// The label may carry a badge after its text.
pub fn stat_tile(
    label: impl IntoElement,
    detail: impl IntoElement,
    value: impl IntoElement,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    div()
        .h_flex()
        .items_center()
        .gap_4()
        .px_4()
        .py_3()
        .rounded(theme.radius_lg)
        .bg(theme.muted)
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().h_flex().gap_2().child(label))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(detail),
                ),
        )
        .child(value)
}

/// A large, heavy number, as paimon.moe draws counts.
pub fn big_number(value: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_3xl()
        .font_weight(FontWeight::BLACK)
        .text_color(color)
        .child(value.into())
}

/// A small muted label above a value.
pub fn stat_label(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
}

/// A figure in DaisyUI's "stat" layout: a small title, a large value, and a
/// muted line under it.
pub fn stat(
    title: impl Into<SharedString>,
    value: impl IntoElement,
    description: impl IntoElement,
    cx: &App,
) -> Div {
    div()
        .v_flex()
        .gap_1()
        .child(stat_label(title, cx).text_sm())
        .child(value)
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}
