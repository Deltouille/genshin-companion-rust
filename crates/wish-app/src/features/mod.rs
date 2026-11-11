//! The pages and flows of the window, one folder per feature (a group in the
//! sidebar) or app-wide page.
//!
//! - `wishes/`: the wish counter (overview, history per banner type, import).
//! - `settings/`, `credits/`: the app's own pages, at the foot of the sidebar.
//!
//! A new feature (such as an ascension planner) gets its own folder here, its
//! pages in `shell::Page` and its group in the sidebar.

pub(crate) mod archive;
pub(crate) mod credits;
pub(crate) mod planner;
pub(crate) mod settings;
pub(crate) mod wishes;
