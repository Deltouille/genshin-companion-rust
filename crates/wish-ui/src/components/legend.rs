use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Div, Hsla, ParentElement as _, SharedString, Styled as _, div};

/// A chart's legend: one swatch and name per series, in the order the series
/// are drawn. The swatch carries the colour; the names stay in text colour.
pub fn legend(series: impl IntoIterator<Item = (Hsla, SharedString)>, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .h_flex()
        .flex_wrap()
        .gap_x_4()
        .gap_y_1()
        .text_sm()
        .text_color(theme.muted_foreground)
        .children(series.into_iter().map(|(color, name)| {
            div()
                .h_flex()
                .gap_1p5()
                .child(div().size_2p5().rounded_sm().bg(color))
                .child(name)
        }))
}
