//! The character archive, after paimon.moe's character list and Project
//! Amber's: every character as a card, filtered by element, weapon and
//! rarity; a click opens the character's page (profile with stats at any
//! level, talents, constellations).
//!
//! An entity: the filters, the open character, its tab and the level sliders
//! are its own state, kept while the user goes elsewhere. The shell opens the
//! list from the sidebar and a character with [`CharactersView::show_character`].

mod detail;
mod list;
mod weapons;

pub use weapons::WeaponsView;

use std::collections::BTreeSet;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Render, SharedString, Subscription, Window,
};
use wish_core::planner::Level;

use crate::app::Library;

/// What a character's page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CharacterTab {
    #[default]
    Profile,
    Talents,
    Constellations,
}

/// The order of the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Sort {
    /// The latest release first, as Project Amber lists them.
    #[default]
    Newest,
    Name,
    Rarity,
}

pub struct CharactersView {
    library: Entity<Library>,
    search: Entity<InputState>,
    /// The elements, weapon types and rarities shown; none means all.
    elements: BTreeSet<SharedString>,
    weapons: BTreeSet<SharedString>,
    stars: BTreeSet<u8>,
    sort: Sort,
    /// The character whose page is open (a genshin-db key).
    open: Option<String>,
    tab: CharacterTab,
    /// An index into [`Level::steps`]: the level the stats are shown at.
    level: Entity<SliderState>,
    /// The talent level the values are shown at, 1 to 15.
    talent_level: Entity<SliderState>,
    _subscriptions: Vec<Subscription>,
}

/// The talent level the values open at.
const DEFAULT_TALENT_LEVEL: f32 = 10.0;

impl CharactersView {
    pub fn new(library: Entity<Library>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search a character"));
        let last = (Level::steps().len() - 1) as f32;
        let level = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(last)
                .step(1.0)
                .default_value(last)
        });
        let talent_level = cx.new(|_| {
            SliderState::new()
                .min(1.0)
                .max(f32::from(wish_core::archive::MAX_TALENT_LEVEL))
                .step(1.0)
                .default_value(DEFAULT_TALENT_LEVEL)
        });
        let subscriptions = vec![
            cx.observe(&library, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&level, |_, _, _: &SliderEvent, cx| cx.notify()),
            cx.subscribe(&talent_level, |_, _, _: &SliderEvent, cx| cx.notify()),
        ];
        Self {
            library,
            search,
            elements: BTreeSet::new(),
            weapons: BTreeSet::new(),
            stars: BTreeSet::new(),
            sort: Sort::default(),
            open: None,
            tab: CharacterTab::default(),
            level,
            talent_level,
            _subscriptions: subscriptions,
        }
    }

    /// Back to the list.
    pub fn show_list(&mut self, cx: &mut Context<Self>) {
        self.open = None;
        cx.notify();
    }

    /// Opens `key`'s page on its profile.
    pub fn show_character(&mut self, key: &str, cx: &mut Context<Self>) {
        self.open = Some(key.to_owned());
        self.tab = CharacterTab::Profile;
        cx.notify();
    }

    pub fn set_tab(&mut self, tab: CharacterTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    /// The level the stats are shown at.
    fn level(&self, cx: &Context<Self>) -> Level {
        let levels = Level::steps();
        let at = self.level.read(cx).value().end().round() as usize;
        levels[at.min(levels.len() - 1)]
    }

    fn talent_level(&self, cx: &Context<Self>) -> u8 {
        self.talent_level.read(cx).value().end().round() as u8
    }

    fn toggle<T: Ord>(set: &mut BTreeSet<T>, value: T) {
        if !set.remove(&value) {
            set.insert(value);
        }
    }
}

impl Render for CharactersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.open.clone() {
            Some(key) => self.render_character(&key, window, cx),
            None => self.render_list(window, cx),
        }
    }
}
