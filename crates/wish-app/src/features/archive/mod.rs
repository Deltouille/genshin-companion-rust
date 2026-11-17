//! The character archive, after paimon.moe's character list and Project
//! Amber's, and the player's own characters, after genshin-tracker's: the
//! characters the selected account owns first (level and constellation on the
//! card, a star for favourites), then the others, filtered by element, weapon
//! and rarity; a click opens the character's page (what the player has,
//! profile with stats at any level, talents, constellations).
//!
//! An entity: the filters, the open character and the level sliders
//! are its own state, kept while the user goes elsewhere. What the account
//! owns is the [`Library`]'s, per account, followed through the [`Session`].
//! The shell opens the list from the sidebar and a character with
//! [`CharactersView::show_character`]. The weapons are [`WeaponsView`]'s, the
//! materials and how many the account has [`ResourcesView`]'s.

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
