//! Genshin Companion's design system, the app's shared layer (like shadcn's `ui/`):
//! its theme and typeface, its icons, number and label formatting, and the
//! presentational components every page uses.
//!
//! Components take plain data and render it; they know nothing of the
//! app's state, which stays in `wish-app`. They follow paimon.moe's layout
//! in Material 3's look (Expressive's typeface): tonal surfaces, pill
//! controls, the secondary container for what is current (see `theme.json`).

mod assets;
pub mod axis;
pub mod components;
pub mod format;
pub mod theme;

pub use assets::{UiAssets, pool_icon};
pub use components::{
    ButtonExt, MaterialTile, NavGroup, NavItem, PageFrame, Pager, PityPoint, Segmented, badge,
    big_number, button, button_style, card, card_title, dialog_close_footer, dialog_footer,
    filter_chip, icon_button, input, item_tile, legend, outcome_badge, pity_chart, rich_text,
    select, snackbar, stat, stat_label, stat_tile,
};

use gpui_kit::App;

/// Registers the typeface and themes; call it after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    theme::install(cx);
}
