//! Presentational building blocks shared by the pages. Each takes plain data
//! and the theme; none reads or changes app state.

mod badge;
mod card;
mod controls;
mod item_tile;
mod legend;
mod material_tile;
mod nav_group;
mod nav_item;
mod page_frame;
mod pager;
mod pity_chart;
mod rich_text;
mod segmented;
mod snackbar;
mod stat;

pub use badge::{badge, outcome_badge};
pub use card::{card, card_title};
pub use controls::{
    ButtonExt, button, button_style, dialog_close_footer, dialog_footer, filter_chip, icon_button,
    input, select,
};
pub use item_tile::item_tile;
pub use legend::legend;
pub use material_tile::MaterialTile;
pub use nav_group::NavGroup;
pub use nav_item::NavItem;
pub use page_frame::PageFrame;
pub use pager::Pager;
pub use pity_chart::{PityPoint, pity_chart};
pub use rich_text::rich_text;
pub use segmented::Segmented;
pub use snackbar::snackbar;
pub use stat::{big_number, stat, stat_label, stat_tile};
