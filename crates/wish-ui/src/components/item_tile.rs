//! An item's icon on its rarity tile, as the game's item cards draw it.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Div, ImageSource, ParentElement as _, Rems, Styled as _, div, img};
use wish_core::model::Rarity;

use crate::theme;

/// An item's icon on its rarity tile, `size` square with the theme's small
/// corners, or the empty tile when there is no icon, so names beside it stay
/// aligned.
pub fn item_tile(rarity: Rarity, icon: Option<ImageSource>, size: Rems, cx: &App) -> Div {
    div()
        .size(size)
        .flex_shrink_0()
        .rounded(cx.theme().radius)
        .bg(theme::rarity_tile(rarity, cx))
        .children(icon.map(|icon| img(icon).size_full()))
}
