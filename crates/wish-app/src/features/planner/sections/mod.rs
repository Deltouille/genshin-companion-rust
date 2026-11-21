//! The planner page's pieces, each drawing its part of the
//! [`PlannerModel`](super::model::PlannerModel): the "Still needed" card, an
//! entry's card, a list of materials, the card that follows the pointer
//! while one is moved. They pass the clicks back through [`Actions`], the
//! view's; none reads or writes the library.

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{App, Pixels, Window};
use wish_core::planner::{CustomItem, PlannerData, ResinOptions};

use crate::app::images::Images;

mod card;
mod drag;
mod materials;
mod still_needed;

pub(super) use card::{CardControls, EntryCard};
pub(super) use still_needed::StillNeeded;

/// What a click on a material does.
pub(super) type OnMaterial = Rc<dyn Fn(CustomItem, &mut Window, &mut App)>;
/// What a click on an entry's button does.
pub(super) type OnEntry = Rc<dyn Fn(i64, &mut Window, &mut App)>;
/// What removing an entry does, given its name.
pub(super) type OnRemove = Rc<dyn Fn(i64, String, &mut Window, &mut App)>;
/// What taking a card does.
pub(super) type OnDrag = Rc<dyn Fn(i64, &mut App)>;
/// What dropping a card onto another does: (the one moved, the one it takes
/// the place of).
pub(super) type OnDrop = Rc<dyn Fn(i64, i64, &mut App)>;
/// What a button opening a dialog does.
pub(super) type OnOpen = Rc<dyn Fn(&mut Window, &mut App)>;

/// What the pieces draw from, besides the model: the game data and the
/// images their pictures come from, how the resin counts, a card's width.
#[derive(Clone)]
pub(super) struct Look {
    pub planner: Arc<PlannerData>,
    pub images: Arc<Images>,
    pub resin: ResinOptions,
    /// A card's width, for the copy that follows the pointer.
    pub card_width: Pixels,
}

/// What the pieces do when used: the view's actions.
#[derive(Clone)]
pub(super) struct Actions {
    /// A material clicked: how many the player has.
    pub open_material: OnMaterial,
    /// A custom entry's settings.
    pub edit: OnEntry,
    /// An entry's ×: remove it, after asking (its name).
    pub remove: OnRemove,
    /// A card taken by its header, to be moved.
    pub start_drag: OnDrag,
    /// A card dropped onto another.
    pub drop_onto: OnDrop,
    pub resin_options: OnOpen,
}
