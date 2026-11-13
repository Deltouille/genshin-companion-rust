//! Icons: GPUI Kit's component icons plus the few Lucide icons the design
//! uses (only those listed here are compiled in).

use std::borrow::Cow;

use gpui_kit::assets::IconName;
use gpui_kit::{AssetSource, Result, SharedString};
use wish_core::model::Pool;

gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        Sparkles,
        Swords,
        ScrollText,
        Sprout,
        Circle,
        Gem,
        Calculator,
        GripVertical,
        Diamond,
        Sparkle,
        Funnel,
        ArrowDownWideNarrow,
        ArrowUpNarrowWide,
        FlaskConical,
        Import,
        CloudDownload,
        UserPlus
    ]
);

/// The design system's assets. An app with assets of its own serves those
/// first and falls back to this.
pub struct UiAssets;

impl AssetSource for UiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

/// The icon that stands for a banner type, in the sidebar and on its cards.
pub fn pool_icon(pool: Pool) -> IconName {
    match pool {
        Pool::CharacterEvent => IconName::Sparkles,
        Pool::WeaponEvent => IconName::Swords,
        Pool::Standard => IconName::Star,
        Pool::Chronicled => IconName::ScrollText,
        Pool::Beginners => IconName::Sprout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_icons_are_served() {
        // Every icon of `ExtraIcons` that the app uses.
        for icon in [
            IconName::Gem,
            IconName::Calculator,
            IconName::Circle,
            IconName::GripVertical,
        ] {
            let path = gpui_kit::component::IconNamed::path(icon);
            assert!(
                UiAssets.load(&path).unwrap().is_some(),
                "{path} is not served"
            );
        }
    }
}
