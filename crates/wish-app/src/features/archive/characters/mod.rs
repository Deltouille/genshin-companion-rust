//! The character archive's entity: the filters, the open character and its
//! level sliders, kept while the user goes elsewhere (this file); `list.rs`
//! and `page.rs` draw the list and a character's page, `collection.rs` what
//! the account owns of it. What they show is `model.rs` (pure, tested alone).

mod collection;
mod list;
mod model;
mod page;

use std::collections::BTreeSet;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, Render, Subscription, Window,
};
use wish_core::planner::Level;

use super::{Sort, filters, level_options};
use crate::app::{Library, Session};
use wish_ui::{PickerEvent, PickerState};

pub struct CharactersView {
    session: Entity<Session>,
    library: Entity<Library>,
    search: Entity<InputState>,
    /// The elements, weapon types and rarities shown; none means all.
    elements: BTreeSet<&'static str>,
    weapons: BTreeSet<&'static str>,
    stars: BTreeSet<u8>,
    /// Only the characters marked as favourites.
    favourites: bool,
    sort: Sort,
    /// The sort pill.
    sort_select: Entity<PickerState<Sort>>,
    /// The order turned round.
    reversed: bool,
    /// The filter drawer, and what has the keyboard while it is open.
    drawer: filters::Drawer,
    drawer_focus: FocusHandle,
    /// The character whose page is open (a genshin-db key).
    open: Option<String>,
    /// An index into [`Level::steps`]: the level the stats are shown at.
    level: Entity<SliderState>,
    /// The talent level the values are shown at, 1 to 15.
    talent_level: Entity<SliderState>,
    /// The owned character's level, on its page.
    owned_level: Entity<PickerState<usize>>,
    /// What the page's controls were last set from: the character and its
    /// owned level. The sliders open at the level the player has.
    synced: Option<(String, Option<Level>)>,
    _subscriptions: Vec<Subscription>,
}

/// The talent level the values open at.
const DEFAULT_TALENT_LEVEL: f32 = 10.0;

impl CharactersView {
    pub fn new(session: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("archive-search-character")));
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
        let sort_select = cx.new(|cx| PickerState::new(filters::sort_options(), Some(0), cx));
        let owned_level = cx.new(|cx| PickerState::new(level_options(&Level::steps()), None, cx));
        let subscriptions = vec![
            cx.observe_in(&library, window, |this, _, window, cx| {
                // A change of language among others.
                let placeholder = t!("archive-search-character");
                filters::refresh_texts(&this.sort_select, &this.search, placeholder, window, cx);
                cx.notify()
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&level, |_, _, _: &SliderEvent, cx| cx.notify()),
            cx.subscribe(&sort_select, |this, _, event: &PickerEvent<Sort>, cx| {
                let PickerEvent::Confirm(sort) = event;
                this.sort = *sort;
                cx.notify();
            }),
            cx.subscribe(&talent_level, |_, _, _: &SliderEvent, cx| cx.notify()),
            cx.subscribe_in(
                &owned_level,
                window,
                |this, _, event: &PickerEvent<usize>, _, cx| {
                    let PickerEvent::Confirm(index) = event;
                    if let Some(level) = Level::steps().get(*index) {
                        this.set_owned_level(*level, cx);
                    }
                },
            ),
        ];
        Self {
            session,
            library,
            search,
            elements: BTreeSet::new(),
            weapons: BTreeSet::new(),
            stars: BTreeSet::new(),
            favourites: false,
            sort: Sort::default(),
            sort_select,
            reversed: false,
            drawer: filters::Drawer::default(),
            drawer_focus: cx.focus_handle(),
            open: None,
            level,
            talent_level,
            owned_level,
            synced: None,
            _subscriptions: subscriptions,
        }
    }

    /// Back to the list.
    pub fn show_list(&mut self, cx: &mut Context<Self>) {
        self.open = None;
        cx.notify();
    }

    /// Opens `key`'s page.
    pub fn show_character(&mut self, key: &str, cx: &mut Context<Self>) {
        self.open = Some(key.to_owned());
        self.synced = None;
        cx.notify();
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
