//! The core of Genshin Companion: everything but the window. It never
//! depends on GPUI, so all of it tests alone, and `wishctl` (an example)
//! drives it from a terminal.
//!
//! The modules, by layer (`docs/architecture/` has the whole picture):
//!
//! - **Domain**, the game's rules, which reach no database, network or file
//!   (`tests/layers.rs` checks it): [`model`] (wishes, pools, accounts),
//!   [`engine`] (pity, banners, 50/50), [`stats`], [`planner`] (costs,
//!   allocation, resin), [`collection`] (what an account owns,
//!   HoYoLAB syncs, GOOD files), [`archive`] (character and weapon pages),
//!   [`chronicle`] (the Abyss, the Theater and the Diary HoYoLAB showed).
//! - **Game data**, embedded JSON a newer copy can override: [`data`] (items
//!   and banners), with the planner's and the archive's own files.
//! - **Storage**: [`store`] (SQLite, migrations, backups), [`snapshot`] (the
//!   lossless export and the mirror), [`paths`] (the data folder),
//!   [`settings`] (`settings.json`), [`keyring`] (the HoYoLAB login, when
//!   the player asks the app to keep it).
//! - **Imports and exports**: [`import`] (paimon.moe saves, the game's API,
//!   UIGF, spreadsheets, counting by hand), [`uigf`], [`excel`].
//! - **Use cases** shared by the app and `wishctl`: [`service`].
//! - **Cross-cutting**: [`i18n`] (Fluent texts, the game's names in every
//!   language), [`error`], [`images`] (where each game image comes from).

/// A text of the core in the current language: `t!("key", name = value)`
/// (see [`i18n`]).
macro_rules! t {
    ($($args:tt)*) => {
        $crate::tr!($crate::i18n::TEXTS, $($args)*)
    };
}

pub mod archive;
pub mod chronicle;
pub mod collection;
pub mod data;
pub mod engine;
pub mod error;
pub mod excel;
pub mod i18n;
pub mod images;
pub mod import;
pub mod keyring;
pub mod model;
pub mod paths;
pub mod planner;
pub mod service;
pub mod settings;
pub mod snapshot;
pub mod stats;
pub mod store;
pub mod uigf;

pub use error::{Error, Result};
/// The HoYoLAB client, so the app reaches it through the core.
pub use hoyolab;
pub use paths::AppPaths;
pub use store::Store;
