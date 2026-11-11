use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Div, ImageSource, ParentElement as _, Styled as _, div, img};
use wish_core::model::Rarity;

use crate::theme;

/// An item's icon on its rarity tile (paimon.moe's gradient), or the empty
/// tile when there is no icon, so names beside it stay aligned. 28 px; the
/// caller may resize it.
pub fn item_tile(rarity: Rarity, icon: Option<ImageSource>, cx: &App) -> Div {
    div()
        .size_7()
        .flex_shrink_0()
        .rounded(cx.theme().radius)
        .bg(theme::rarity_tile(rarity, cx))
        .children(icon.map(|icon| img(icon).size_full()))
}
