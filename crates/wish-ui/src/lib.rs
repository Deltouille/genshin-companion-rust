//! Genshin Companion's design system, the app's shared layer (like shadcn's `ui/`):
//! its theme and typeface, its icons, number and label formatting, and the
//! presentational components every page uses.
//!
//! Components take plain data and render it; they know nothing of the
//! app's state, which stays in `wish-app`. The look is the game's menus,
//! without any of the game's assets or its font ([`theme`]): ink-blue slate
//! cards on a plain page, parchment text and pill controls, gold for focus and
//! sliders, item cards on their rarity tiles; one rule for corners (every
//! control a pill, surfaces `radius.lg`, item tiles `radius`).
//!
//! The app never uses GPUI Kit's `Button`, `Select`, `Input`, `Pagination`,
//! `ButtonGroup` or `Toggle` directly: they come at the kit's 32 px and 14 px,
//! which look like another app here. It uses the controls of [`components`]
//! instead (`button`, `picker`, `input`, `Segmented`, `Pager`…), which
//! also give everything clickable the pointing hand.
//!
//! - [`theme`]: the themes (`theme.json`), the typeface, the app's own
//!   colour roles ([`theme::palette`]), rarity tiles and colours.
//! - [`components`]: the controls and presentational pieces.
//! - [`format`](mod@format): numbers, dates, servers and the game's fixed terms in the
//!   current language.
//! - [`axis`]: chart ticks that stay on whole numbers.
//! - `assets`: the icons ([`UiAssets`]).

/// A text of the design system in the current language (see
/// `wish_core::i18n`).
macro_rules! t {
    ($($args:tt)*) => {
        wish_core::tr!($crate::TEXTS, $($args)*)
    };
}

/// The design system's texts.
pub(crate) static TEXTS: std::sync::LazyLock<wish_core::i18n::Texts> =
    std::sync::LazyLock::new(|| wish_core::texts!("ui"));

mod assets;
pub mod axis;
pub mod components;
pub mod format;
pub mod theme;

pub use assets::{UiAssets, pool_icon};
pub use components::{
    ButtonExt, MaterialTile, MenuButton, NavGroup, NavItem, PageFrame, Pager, Picker, PickerEvent,
    PickerItem, PickerState, PityPoint, Segmented, badge, big_number, button, button_style, card,
    card_title, confirm_destructive, dialog_close_footer, dialog_footer, filter_chip, form_dialog,
    icon_button, input, item_tile, legend, menu_button, outcome_badge, picker, pity_chart,
    rich_text, snackbar, stat, stat_label, stat_tile,
};

use gpui_kit::App;

/// Registers the typeface and themes; call it after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    theme::install(cx);
}
