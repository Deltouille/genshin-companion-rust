//! The app's assets: the design system's icons ([`wish_ui::UiAssets`]). The
//! game's images are not assets: they are downloaded on first use (see
//! [`super::images`]).

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};
use wish_ui::UiAssets;

/// The app's asset source; register it with `Application::with_assets`.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        UiAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        UiAssets.list(path)
    }
}
