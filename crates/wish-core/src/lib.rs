//! Core of the app: the wish counter's data model, banner/item data, storage,
//! importers and pity / 50-50 engine, and the ascension planner's data and
//! costs.
//!
//! This crate must never depend on GPUI so it stays unit-testable and reusable.
//! See `PLAN.md` at the repository root for the roadmap.

pub mod archive;
pub mod data;
pub mod engine;
pub mod error;
pub mod excel;
pub mod images;
pub mod import;
pub mod model;
pub mod paths;
pub mod planner;
pub mod settings;
pub mod snapshot;
pub mod stats;
pub mod store;
pub mod uigf;

pub use error::{Error, Result};
pub use paths::AppPaths;
pub use store::Store;
