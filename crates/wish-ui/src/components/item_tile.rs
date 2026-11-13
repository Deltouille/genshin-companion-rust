use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Div, ImageSource, ParentElement as _, Rems, Styled as _, div, img, rems};
use wish_core::model::Rarity;

use crate::theme;

/// An item's icon on its rarity tile, `size` square, its bottom right corner
/// rounded wide as on the game's item cards, or the empty tile when there is
/// no icon, so names beside it stay aligned.
pub fn item_tile(rarity: Rarity, icon: Option<ImageSource>, size: Rems, cx: &App) -> Div {
    div()
        .size(size)
        .flex_shrink_0()
        .rounded(cx.theme().radius)
        // The same share of the side as `MaterialTile`'s corner.
        .rounded_br(rems(size.0 * 0.3))
        .bg(theme::rarity_tile(rarity, cx))
        .children(icon.map(|icon| img(icon).size_full()))
}
