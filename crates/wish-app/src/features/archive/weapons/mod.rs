//! The weapon archive, like the characters': every weapon as a card, under
//! the same filter bar (type and rarity in its sheet); a click opens its page
//! (the wish art, ATK and the secondary stat at any level and what reaching
//! it costs, the passive at each refinement).
//!
//! An entity, like [`CharactersView`](super::CharactersView): the filters,
//! the open weapon, its level slider and refinement are its own state (this
//! file, with its actions). What the pages show is `model.rs` (pure, tested
//! alone); `list.rs` and `page.rs` draw it, `copies.rs` the account's
//! copies of the open weapon.

use std::collections::BTreeSet;

mod copies;
mod list;
mod model;
mod page;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, Render, Subscription, WeakEntity,
    Window,
};
use wish_core::planner::{Level, PlanTarget};

use std::rc::Rc;

use super::Sort;
use super::filters::{self, FilterGroup, FilterOption};
use crate::app::{Library, Session};
use copies::CopyControls;
use wish_ui::{PickerEvent, PickerState};

const TYPES: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

pub struct WeaponsView {
    pub(super) session: Entity<Session>,
    pub(super) library: Entity<Library>,
    search: Entity<InputState>,
    /// The weapon types and rarities shown; none means all.
    types: BTreeSet<&'static str>,
    stars: BTreeSet<u8>,
    /// Only the weapons with a copy marked as a favourite.
    favourites: bool,
    sort: Sort,
    /// The sort pill.
    sort_select: Entity<PickerState<Sort>>,
    /// The order turned round.
    reversed: bool,
    /// The filter drawer, and what has the keyboard while it is open.
    drawer: filters::Drawer,
    drawer_focus: FocusHandle,
    /// The weapon whose page is open (a genshin-db key).
    pub(super) open: Option<String>,
    /// The pickers of the open weapon's copies.
    copies: Vec<CopyControls>,
    /// An index into the open weapon's levels ([`Level::weapon_steps`]).
    level: Entity<SliderState>,
    /// The refinement its passive is shown at, 1 to 5.
    refinement: usize,
    _level: Subscription,
    _subscriptions: Vec<Subscription>,
}

impl WeaponsView {
    pub fn new(session: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search a weapon"));
        let (level, level_subscription) = Self::level_slider(5, cx);
        let sort_select = cx.new(|cx| PickerState::new(filters::sort_options(), Some(0), cx));
        let subscriptions = vec![
            // New data, or another account: the copies' pickers follow.
            cx.observe(&library, |this, _, cx| {
                this.sync_copies(cx);
                cx.notify();
            }),
            cx.observe(&session, |this, _, cx| {
                this.sync_copies(cx);
                cx.notify();
            }),
            cx.subscribe(&sort_select, |this, _, event: &PickerEvent<Sort>, cx| {
                let PickerEvent::Confirm(sort) = event;
                this.sort = *sort;
                cx.notify();
            }),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self {
            session,
            library,
            search,
            types: BTreeSet::new(),
            stars: BTreeSet::new(),
            favourites: false,
            sort: Sort::default(),
            sort_select,
            reversed: false,
            drawer: filters::Drawer::default(),
            drawer_focus: cx.focus_handle(),
            open: None,
            copies: Vec::new(),
            level,
            refinement: 1,
            _level: level_subscription,
            _subscriptions: subscriptions,
        }
    }

    /// A level slider for a weapon of `rarity` stars, at its top.
    fn level_slider(rarity: u8, cx: &mut Context<Self>) -> (Entity<SliderState>, Subscription) {
        let last = (Level::weapon_steps(rarity).len() - 1) as f32;
        let level = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(last)
                .step(1.0)
                .default_value(last)
        });
        let subscription = cx.subscribe(&level, |_, _, _: &SliderEvent, cx| cx.notify());
        (level, subscription)
    }

    /// Back to the list.
    pub fn show_list(&mut self, cx: &mut Context<Self>) {
        self.open = None;
        self.sync_copies(cx);
        cx.notify();
    }

    /// Opens `key`'s page, at its top level and first refinement.
    pub fn show_weapon(&mut self, key: &str, cx: &mut Context<Self>) {
        let rarity = (self.library.read(cx).planner().weapon(key)).map_or(5, |w| w.rarity);
        let (level, subscription) = Self::level_slider(rarity, cx);
        self.level = level;
        self._level = subscription;
        self.refinement = 1;
        self.open = Some(key.to_owned());
        self.sync_copies(cx);
        cx.notify();
    }

    fn toggle<T: Ord>(set: &mut BTreeSet<T>, value: T) {
        if !set.remove(&value) {
            set.insert(value);
        }
    }

    /// The list's filters, grouped as the sheet shows them: types, rarities.
    fn filter_groups(&self, view: WeakEntity<Self>) -> Vec<FilterGroup> {
        let this = self;
        let weak = view;
        let toggle = |change: Rc<dyn Fn(&mut Self)>| -> filters::Handler {
            let weak = weak.clone();
            Rc::new(move |_, cx| {
                weak.update(cx, |this, cx| {
                    change(this);
                    cx.notify();
                })
                .ok();
            })
        };
        let types = TYPES
            .iter()
            .map(|kind| FilterOption {
                label: (*kind).into(),
                icon: None,
                on: this.types.contains(kind),
                toggle: toggle(Rc::new(move |this| Self::toggle(&mut this.types, *kind))),
            })
            .collect();
        let stars = [5u8, 4, 3, 2, 1]
            .into_iter()
            .map(|stars| FilterOption {
                label: format!("{stars}★").into(),
                icon: None,
                on: this.stars.contains(&stars),
                toggle: toggle(Rc::new(move |this| Self::toggle(&mut this.stars, stars))),
            })
            .collect();
        let collection = vec![FilterOption {
            label: "Favourites".into(),
            icon: None,
            on: this.favourites,
            toggle: toggle(Rc::new(|this| this.favourites = !this.favourites)),
        }];
        vec![
            FilterGroup {
                title: "Type",
                options: types,
            },
            FilterGroup {
                title: "Rarity",
                options: stars,
            },
            FilterGroup {
                title: "Collection",
                options: collection,
            },
        ]
    }

    /// Turns every filter off.
    fn clear_filters(view: &Entity<Self>) -> filters::Handler {
        let weak = view.downgrade();
        Rc::new(move |_, cx| {
            weak.update(cx, |this, cx| {
                this.types.clear();
                this.stars.clear();
                this.favourites = false;
                cx.notify();
            })
            .ok();
        })
    }

    /// Opens the filter drawer, with the keyboard in it (Escape closes it).
    fn open_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.drawer = filters::Drawer::Open;
        self.drawer_focus.focus(window, cx);
        cx.notify();
    }

    /// Slides the filter drawer out, then forgets it.
    fn close_filters(&mut self, cx: &mut Context<Self>) {
        if self.drawer != filters::Drawer::Open {
            return;
        }
        self.drawer = filters::Drawer::Closing;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(filters::DRAWER_SLIDE).await;
            this.update(cx, |this, cx| {
                if this.drawer == filters::Drawer::Closing {
                    this.drawer = filters::Drawer::Closed;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn add_to_plan(&mut self, key: String, name: String, rarity: u8, cx: &mut Context<Self>) {
        let account = self.session.read(cx).account();
        // From its most levelled copy that can still go up, else level 1.
        let goal = self
            .library
            .read(cx)
            .owned(account)
            .weapon_goal(&key, rarity);
        let target = PlanTarget::Weapon { key, goal };
        let task = self.library.update(cx, |library, cx| {
            library.add_plan_entry(account, target, cx)
        });
        crate::app::notices::report_outcome(
            task,
            format!("{name} was not added to the planner"),
            move |_| {
                format!(
                    "{name} is in the planner, from level {} to {}.",
                    goal.from.level, goal.to.level
                )
            },
            cx,
        );
    }
}

impl Render for WeaponsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.open.clone() {
            Some(key) => self.render_weapon(&key, window, cx),
            None => self.render_list(cx),
        }
    }
}
