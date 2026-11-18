//! The pages and flows of the window, one folder per feature (a group in the
//! sidebar) or app-wide page.
//!
//! - `overview/`: the app's home, the account in figures, one tab each: its
//!   wishes, its characters, its weapons.
//! - `wishes/`: the wish counter (history per banner type, import).
//! - `archive/`: the game's characters, weapons and materials, with what the
//!   account owns of each.
//! - `planner/`: the ascension planner.
//! - `imports/`: every way to bring data in, on one page (the wish history,
//!   the HoYoLAB sync, a GOOD file).
//! - `settings/`, `credits/`: the app's own pages, at the foot of the sidebar.
//!
//! A new feature (such as a quest reader) gets its own folder here, its pages
//! in `shell::Page` and its group in the sidebar.

pub(crate) mod archive;
pub(crate) mod credits;
pub(crate) mod imports;
pub(crate) mod overview;
pub(crate) mod planner;
pub(crate) mod settings;
pub(crate) mod wishes;
