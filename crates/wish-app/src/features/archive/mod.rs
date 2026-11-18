//! The archive: the game's characters, weapons and materials, after
//! Project Amber's and genshinlist.vercel.app's lists, with what the selected
//! account owns of each, after genshin-tracker's. Three pages, each an entity
//! of its own, kept while the player goes elsewhere:
//!
//! - `characters/` ([`CharactersView`]): the list (owned first, then the
//!   others faded), filters and sort, and a character's page (the hero banner,
//!   "My character", stats at any level, ascension, talents, constellations);
//! - `weapons/` ([`WeaponsView`]): the same for weapons, with the account's
//!   copies of each;
//! - `resources/` ([`ResourcesView`]): every material by type, with how many
//!   the account has; a click opens the planner's inventory dialog.
//!
//! What they share: `filters.rs` (the filter bar and its drawer), `shared.rs`
//! (the list card, the hero banner, the stats table…), and here the sort
//! order and the level pickers. What the account owns is the [`Library`]'s,
//! per account, followed through the [`Session`]. The shell opens a list from
//! the sidebar, and a page with [`CharactersView::show_character`] or
//! [`WeaponsView::show_weapon`].

mod characters;
mod filters;
mod resources;
mod shared;
mod weapons;

pub use characters::CharactersView;
pub use resources::{EditMaterial, ResourcesView};
pub use weapons::WeaponsView;

use wish_core::planner::Level;
use wish_ui::PickerItem;

/// The order of the list (the sort pill's options; `reversed` turns it round).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Sort {
    /// The latest release first, as Project Amber lists them.
    #[default]
    Newest,
    Name,
    Rarity,
}

/// `levels` as picker options ("80/90"), standing for their index.
fn level_options(levels: &[Level]) -> Vec<PickerItem<usize>> {
    levels
        .iter()
        .enumerate()
        .map(|(index, level)| PickerItem::new(format!("{}/{}", level.level, level.cap()), index))
        .collect()
}
