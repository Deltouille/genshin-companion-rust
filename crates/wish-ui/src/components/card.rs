use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{App, Div, FontWeight, ParentElement as _, SharedString, Styled as _, div};

/// A raised surface on the page: cards, panels, chart frames. In the dark
/// theme the surface colour alone sets it apart, as on paimon.moe; on the
/// light page a hairline helps.
pub fn card(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .v_flex()
        .rounded(theme.radius_lg)
        .bg(theme.group_box)
        .text_color(theme.group_box_foreground)
        .when(!theme.is_dark(), |this| {
            this.border_1().border_color(theme.border)
        })
}

/// A card's title row: a semibold title, with room for trailing content
/// (such as a button) added by the caller.
pub fn card_title(title: impl Into<SharedString>, cx: &App) -> Div {
    div().h_flex().gap_2().child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().group_box_foreground)
            .child(title.into()),
    )
}
