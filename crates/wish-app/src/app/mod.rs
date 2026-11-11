//! The app's state and services, below the views: the [`Library`] model,
//! the [`Session`] (selected account, flow in progress),
//! item images, the embedded assets and the [`Notices`] (messages about work
//! done in the background).

mod assets;
pub mod images;
pub mod library;
pub mod notices;
pub mod session;

pub use assets::AppAssets;
pub use library::{ImageDownloads, Library};
pub use notices::{Notice, Notices, notify};
pub use session::Session;
