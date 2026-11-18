//! The app's state and services, below the views: the [`Library`] (all the
//! player's data, and the writes), its [`Preferences`](library::Preferences)
//! and [`ImageService`](library::ImageService), the [`Session`] (selected
//! account, flow in progress), the [`Services`] it reaches outside, the
//! embedded assets and the [`Notices`] (messages about work done in the
//! background). No views.

mod assets;
pub mod image_service;
pub mod images;
pub mod library;
pub mod notices;
pub mod preferences;
pub mod services;
pub mod session;

pub use assets::AppAssets;
pub use library::Library;
pub use notices::{Notice, Notices, notify, report, report_outcome};
pub use services::{ImageDownloads, Services};
pub use session::Session;
