//! What the [`Library`](super::Library) reaches outside the computer: the
//! game's wish history API, HoYoLAB and the image sites. Given once, when it
//! opens, so tests give fakes and never touch the network.

use std::sync::Arc;
use std::time::Duration;

use wish_core::hoyolab;
use wish_core::import::api::{self, Pacing};

/// Whether the library downloads the images the pages ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageDownloads {
    On,
    Off,
}

/// Everything the library reaches outside the computer: [`Services::network`]
/// in the app, [`Services::offline`] in tests.
#[derive(Clone)]
pub struct Services {
    /// The game's wish history API.
    pub game: Arc<dyn api::Transport + Send + Sync>,
    /// How fast the game's API is asked: politely in the app, at once in
    /// tests.
    pub pacing: Pacing,
    pub hoyolab: Arc<dyn hoyolab::Transport + Send + Sync>,
    pub images: ImageDownloads,
}

impl Services {
    /// The real network, as the app uses it.
    pub fn network() -> Self {
        Self {
            game: Arc::new(api::HttpTransport::new()),
            pacing: Pacing::default(),
            hoyolab: Arc::new(hoyolab::HttpTransport::new()),
            images: ImageDownloads::On,
        }
    }

    /// No network at all: every request fails and no image is downloaded.
    /// Tests start from it and put their fakes in.
    pub fn offline() -> Self {
        Self {
            game: Arc::new(Offline),
            pacing: Pacing {
                delay: Duration::ZERO,
                backoff: Duration::ZERO,
                retries: 0,
            },
            hoyolab: Arc::new(Offline),
            images: ImageDownloads::Off,
        }
    }
}

/// A network that isn't there.
struct Offline;

impl api::Transport for Offline {
    fn get(&self, url: &str) -> Result<String, String> {
        Err(format!("no network here ({url})"))
    }
}

impl hoyolab::Transport for Offline {
    fn send(&self, request: &hoyolab::Request) -> hoyolab::Result<String> {
        Err(hoyolab::Error::Network(format!(
            "no network here ({})",
            request.url
        )))
    }
}
