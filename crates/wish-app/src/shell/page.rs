//! Where the window can go: the app's pages and its features' pages.

use wish_core::model::Pool;

/// A destination in the sidebar: a page of a feature, or one of the app's own
/// pages at the foot of the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The app's home: the selected account in figures, one tab each.
    Overview(OverviewTab),
    Wishes(WishPage),
    Archive(ArchivePage),
    /// The ascension planner.
    Planner,
    Chronicle(ChroniclePage),
    /// Every way to bring data in.
    Imports,
    Settings,
    Credits,
}

/// The overview's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewTab {
    /// Key figures, a card per banner type, wishes and pity over time.
    #[default]
    Wishes,
    /// The characters owned, against the game's.
    Characters,
    /// The weapons owned, against the game's.
    Weapons,
}

impl OverviewTab {
    pub const ALL: [OverviewTab; 3] = [Self::Wishes, Self::Characters, Self::Weapons];

    pub fn label(self) -> String {
        match self {
            Self::Wishes => t!("app-nav-wishes"),
            Self::Characters => t!("app-nav-characters"),
            Self::Weapons => t!("app-nav-weapons"),
        }
    }
}

/// The wish counter's pages: one per banner type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WishPage {
    History(Pool),
}

/// The archive's pages: the game's characters and weapons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchivePage {
    /// Every character, and one character's page when one is open.
    Characters,
    /// Every weapon, and one weapon's page when one is open.
    Weapons,
    /// Every material, by type, with how many the account has.
    Resources,
}

/// The chronicle's pages: what HoYoLAB shows for a while only, kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChroniclePage {
    /// The Spiral Abyss, period by period.
    Abyss,
    /// The Imaginarium Theater, period by period.
    Theater,
    /// The Traveler's Diary, month by month.
    Diary,
}

/// A feature: a group in the sidebar, holding its pages. A new one (a quest
/// reader…) adds a variant here, one to [`Page`] with its own page enum, a
/// group in `sidebar.rs` and a folder in `features/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    Wishes,
    Archive,
    Planner,
    Chronicle,
}

impl Page {
    /// The feature the page belongs to; `None` for the app's own pages.
    pub fn feature(self) -> Option<Feature> {
        match self {
            Page::Wishes(_) => Some(Feature::Wishes),
            Page::Archive(_) => Some(Feature::Archive),
            Page::Planner => Some(Feature::Planner),
            Page::Chronicle(_) => Some(Feature::Chronicle),
            Page::Overview(_) | Page::Imports | Page::Settings | Page::Credits => None,
        }
    }
}

impl Feature {
    /// The page the feature opens on. The wish counter opens on its first
    /// banner type; without wishes, the shell shows the overview instead.
    pub fn home(self) -> Page {
        match self {
            Feature::Wishes => Page::Wishes(WishPage::History(Pool::CharacterEvent)),
            Feature::Archive => Page::Archive(ArchivePage::Characters),
            Feature::Planner => Page::Planner,
            Feature::Chronicle => Page::Chronicle(ChroniclePage::Abyss),
        }
    }
}
