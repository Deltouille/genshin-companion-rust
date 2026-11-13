//! The weapon archive, like the characters': every weapon as a card, under
//! the same filter bar (type and rarity in its sheet); a click opens its page (the wish art, ATK and
//! the secondary stat at any level and what reaching it costs, the passive at
//! each refinement).
//!
//! An entity, like [`CharactersView`](super::CharactersView): the filters,
//! the open weapon, its level slider and refinement are its own state.

use std::collections::BTreeSet;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, Render, SharedString, Styled as _,
    StyledImage as _, Subscription, WeakEntity, Window, div, img, rems,
};
use wish_core::collection::OwnedWeapon;
use wish_core::model::Rarity;
use wish_core::planner::{Level, PlanTarget, Weapon, WeaponGoal};

use std::rc::Rc;

use super::Sort;
use super::detail::{facts_grid, hero_banner, level_badge, materials, table};
use super::filters::{self, FilterGroup, FilterOption};
use super::list::{corner_button, section_title};
use super::weapon_copies::CopyControls;
use crate::app::images::Images;
use crate::app::{Library, Session};
use wish_ui::{PageFrame, Segmented, badge, button, card, card_title, format, theme};
use wish_ui::{PickerEvent, PickerState};

const TYPES: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

pub struct WeaponsView {
    pub(super) session: Entity<Session>,
    pub(super) library: Entity<Library>,
    search: Entity<InputState>,
    /// The weapon types and rarities shown; none means all.
    types: BTreeSet<SharedString>,
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
    pub(super) copies: Vec<CopyControls>,
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

    /// The level the stats are shown at.
    fn level(&self, rarity: u8, cx: &Context<Self>) -> Level {
        let levels = Level::weapon_steps(rarity);
        let at = self.level.read(cx).value().end().round() as usize;
        levels[at.min(levels.len() - 1)]
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
                on: this.types.contains(*kind),
                toggle: toggle(Rc::new(move |this| {
                    Self::toggle(&mut this.types, (*kind).into())
                })),
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

    fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images().clone(),
        );
        let version = |key: &str| {
            archive
                .weapon(key)
                .and_then(|page| page.version.as_deref())
                .map(super::list::release)
                .unwrap_or_default()
        };
        let latest = planner.weapons().iter().map(|w| version(&w.key)).max();

        let owned = library.owned(self.account(cx)).clone();
        let favourite = |key: &str| owned.copies(key).any(|c| c.favourite);
        let query = self.search.read(cx).value().to_lowercase();
        let mut shown: Vec<&Weapon> = planner
            .weapons()
            .iter()
            .filter(|w| query.is_empty() || w.name.to_lowercase().contains(&query))
            .filter(|w| self.types.is_empty() || self.types.contains(w.weapon.as_str()))
            .filter(|w| self.stars.is_empty() || self.stars.contains(&w.rarity))
            .filter(|w| !self.favourites || favourite(&w.key))
            .collect();
        match self.sort {
            Sort::Newest => shown.sort_by(|a, b| {
                version(&b.key)
                    .cmp(&version(&a.key))
                    .then_with(|| b.rarity.cmp(&a.rarity))
                    .then_with(|| a.name.cmp(&b.name))
            }),
            Sort::Name => shown.sort_by(|a, b| a.name.cmp(&b.name)),
            Sort::Rarity => {
                shown.sort_by(|a, b| b.rarity.cmp(&a.rarity).then_with(|| a.name.cmp(&b.name)))
            }
        }
        if self.reversed {
            shown.reverse();
        }

        // What the account owns first, its favourites leading; the others
        // after, each part in the chosen order.
        let (mut mine, others): (Vec<&Weapon>, Vec<&Weapon>) = shown
            .into_iter()
            .partition(|w| owned.copies(&w.key).next().is_some());
        mine.sort_by_key(|w| !favourite(&w.key));
        let is_new = |w: &Weapon| latest.is_some() && Some(version(&w.key)) == latest;
        let mine_cards: Vec<AnyElement> = mine
            .iter()
            .map(|w| {
                let copies: Vec<OwnedWeapon> = owned.copies(&w.key).cloned().collect();
                self.weapon_card(w, &copies, is_new(w), &images, cx)
            })
            .collect();
        let other_cards: Vec<AnyElement> = others
            .iter()
            .map(|w| self.weapon_card(w, &[], is_new(w), &images, cx))
            .collect();
        let theme = cx.theme();

        let view = cx.entity();
        let groups = self.filter_groups(view.downgrade());
        let toolbar = filters::filter_bar(
            &self.search,
            &self.sort_select,
            self.reversed,
            &groups,
            cx.listener(|this, _, window, cx| this.open_filters(window, cx)),
            cx.listener(|this, _, _, cx| {
                this.reversed = !this.reversed;
                cx.notify();
            }),
            Self::clear_filters(&view),
            cx,
        );

        let (mine_count, other_count) = (mine_cards.len(), other_cards.len());
        let filtered = !query.is_empty()
            || !self.types.is_empty()
            || !self.stars.is_empty()
            || self.favourites;
        let muted = theme.muted_foreground;
        let body = div()
            .id("weapon-grid")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_6()
                    .pt_2()
                    .pb_6()
                    .child(section_title("Owned", mine_count, cx))
                    .when(mine_count == 0, |this| {
                        this.child(div().text_sm().text_color(muted).child(if filtered {
                            "None of your weapons matches these filters."
                        } else {
                            "The weapons you have show here, with their refinement and how \
                             many copies. Add them with + on their card."
                        }))
                    })
                    .child(div().h_flex().flex_wrap().gap_5().children(mine_cards))
                    .when(other_count > 0 || mine_count == 0, |this| {
                        this.child(
                            div()
                                .pt_2()
                                .child(section_title("Not owned", other_count, cx)),
                        )
                    })
                    .child(div().h_flex().flex_wrap().gap_5().children(other_cards))
                    .when(mine_count + other_count == 0, |this| {
                        this.child(
                            div()
                                .text_color(muted)
                                .child("No weapon matches these filters."),
                        )
                    }),
            );
        let distinct = planner
            .weapons()
            .iter()
            .filter(|w| owned.copies(&w.key).next().is_some())
            .count();
        let page = PageFrame::new("Weapons", body)
            .subtitle(format!(
                "{distinct} owned · {} weapons",
                planner.weapons().len()
            ))
            .toolbar(toolbar)
            .body_scrolls();
        let drawer = (self.drawer != filters::Drawer::Closed).then(|| {
            let weak = view.downgrade();
            let close: filters::Handler = Rc::new(move |_, cx| {
                weak.update(cx, |this, cx| this.close_filters(cx)).ok();
            });
            filters::filter_drawer(
                self.drawer,
                &self.drawer_focus,
                self.filter_groups(view.downgrade()),
                close,
                Self::clear_filters(&view),
                cx,
            )
        });
        div()
            .relative()
            .size_full()
            .child(page)
            .children(drawer)
            .into_any_element()
    }

    /// A weapon's card: its picture with what the account has of it (the
    /// best refinement, and how many copies), its name; the corner buttons
    /// mark it as a favourite or add a copy.
    fn weapon_card(
        &self,
        weapon: &Weapon,
        copies: &[OwnedWeapon],
        new: bool,
        images: &Images,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (ink, parchment) = theme::tile_label(cx);
        let (key, name) = (weapon.key.clone(), weapon.name.clone());
        let owned = !copies.is_empty();
        let add = corner_button(format!("add-copy-{key}"), IconName::Plus, "Add a copy", cx)
            .on_click(cx.listener({
                let (key, name) = (key.clone(), name.clone());
                move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.add_copy(key.clone(), name.clone(), cx);
                }
            }));
        let corner = if owned {
            let favourite = copies.iter().any(|c| c.favourite);
            let gold = theme::rarity(Rarity::Five, cx);
            let star = corner_button(
                format!("favourite-{key}"),
                if favourite {
                    IconName::StarFill
                } else {
                    IconName::Star
                },
                if favourite {
                    "Remove from favourites"
                } else {
                    "Add to favourites"
                },
                cx,
            )
            .when(favourite, |this| this.text_color(gold))
            .on_click(cx.listener({
                let key = key.clone();
                move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_weapon_favourite(&key, cx);
                }
            }));
            div().h_flex().gap_1().child(star).child(add)
        } else {
            div().child(add)
        };
        let summary = owned.then(|| {
            let best = copies.iter().map(|c| c.refinement).max().unwrap_or(1);
            match copies.len() {
                1 => format!("R{best}"),
                n => format!("R{best} · ×{n}"),
            }
        });
        let picture = div()
            .relative()
            .w_full()
            .h(rems(8.25))
            .rounded_t(theme.radius_lg)
            .rounded_br(rems(2.))
            .bg(if owned {
                theme::stars_tile(weapon.rarity, cx)
            } else {
                // Faded with its picture: the card reads as one not owned.
                theme::stars_tile(weapon.rarity, cx).opacity(0.4)
            })
            .children(images.weapon(weapon).map(|icon| {
                img(icon)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .when(!owned, |this| this.opacity(0.5))
            }))
            .child(div().absolute().top_1().right_1().child(corner))
            .child(
                div()
                    .absolute()
                    .bottom_1()
                    .left_1()
                    .right_1()
                    .h_flex()
                    .gap_1()
                    .children(summary.map(|text| {
                        badge(text, (theme.foreground, theme.background.opacity(0.85)), cx)
                    }))
                    .child(div().flex_1())
                    .when(new, |this| {
                        this.child(badge("New", (theme.primary_foreground, theme.primary), cx))
                    }),
            );
        Button::new(SharedString::from(format!("weapon-{key}")))
            .ghost()
            .p_0()
            .w(rems(8.25))
            .h_auto()
            .rounded(theme.radius_lg)
            .cursor_pointer()
            .accessibility_label(format!(
                "{}, {}★ {}{}",
                weapon.name,
                weapon.rarity,
                weapon.weapon,
                if owned { ", owned" } else { "" }
            ))
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .rounded(theme.radius_lg)
                    .bg(if owned { parchment } else { theme.muted })
                    .child(picture)
                    .child(
                        div()
                            .w_full()
                            .px_2()
                            .py_1p5()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_center()
                            .truncate()
                            .text_color(if owned { ink } else { theme.muted_foreground })
                            .child(weapon.name.clone()),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.show_weapon(&key, cx)))
            .into_any_element()
    }

    fn render_weapon(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images().clone(),
        );
        let back = button("back-to-weapons")
            .ghost()
            .icon(IconName::ArrowLeft)
            .label("All weapons")
            .on_click(cx.listener(|this, _, _, cx| this.show_list(cx)));
        let (Some(weapon), Some(page)) = (planner.weapon(key), archive.weapon(key)) else {
            return PageFrame::new(
                "Weapon",
                div().px_6().child("This weapon is not in the game data."),
            )
            .action(back)
            .into_any_element();
        };
        // What the account has of it, under the banner.
        let refinable = page
            .passive
            .as_ref()
            .is_some_and(|p| p.refinements.len() > 1);
        let copies = self.my_copies(weapon, refinable, cx);
        let theme = cx.theme();
        let width = crate::shell::content_width(window);

        // The banner: the stars, the description and the facts beside the
        // wish art, which stands upright (half as wide as tall).
        let info = div()
            .child(
                div()
                    .text_2xl()
                    .text_color(theme::stars_color(weapon.rarity, cx))
                    .child("★".repeat(usize::from(weapon.rarity))),
            )
            .children(
                page.description
                    .as_deref()
                    .map(|text| div().text_lg().line_clamp(3).child(text.to_owned())),
            )
            .child(facts_grid(
                &[
                    ("Type", Some(weapon.weapon.as_str())),
                    ("Secondary", page.stats.special.as_deref()),
                    ("Passive", page.passive.as_ref().map(|p| p.name.as_str())),
                    ("Released in", page.version.as_deref()),
                ],
                cx,
            ));
        let hero = hero_banner(
            info,
            images
                .splash(page.art.as_deref())
                .map(|art| (art, 0.5, 1.15)),
            images.backdrop(page.art.as_deref()),
            width,
            window,
            cx,
        );

        let level = self.level(weapon.rarity, cx);
        let level_label = format!("Lv. {}/{}", level.level, level.cap());
        let stats = archive.weapon_stats(page, level);
        let mut rows = vec![(
            "Base ATK".to_owned(),
            format::count(stats.attack.round() as u64),
        )];
        if let (Some(name), Some(value)) = (&page.stats.special, stats.special) {
            let value = if page.stats.special_percent {
                format!("{:.1}%", value * 100.0)
            } else {
                format::count(value.round() as u64)
            };
            rows.push((name.clone(), value));
        }
        let stats_card = card(cx)
            .p_4()
            .gap_3()
            .child(card_title("Stats", cx).child(level_badge(&level_label, cx)))
            .child(
                Slider::new(&self.level)
                    .cursor_pointer()
                    .accessibility_label("Level"),
            )
            .child(table(rows, cx));
        let need = planner.weapon_cost(
            weapon,
            &WeaponGoal {
                from: Level::START,
                to: level,
            },
        );
        let cost_card = card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title(format!("To reach {level_label}"), cx).child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("from level 1"),
                ),
            )
            .child(materials(&need.materials_with_exp(), &planner, &images, cx));

        let passive = page.passive.as_ref().map(|passive| {
            let refinement = self.refinement.clamp(1, passive.refinements.len());
            let text = passive.refinements[refinement - 1].clone();
            let levels = (passive.refinements.len() > 1).then(|| {
                (1..=passive.refinements.len())
                    .fold(Segmented::new("refinement"), |tabs, r| {
                        tabs.option(format!("R{r}"))
                    })
                    .selected(refinement - 1)
                    .on_select(cx.listener(|this, ix: &usize, _, cx| {
                        this.refinement = ix + 1;
                        cx.notify();
                    }))
            });
            card(cx)
                .p_4()
                .gap_3()
                .child(card_title(passive.name.clone(), cx).children(levels))
                .child(div().text_color(theme.foreground).child(text))
        });

        let body = div()
            .id("weapon-page")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_6()
                    .pt_2()
                    .pb_6()
                    .child(hero)
                    .child(copies)
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_4()
                            .items_start()
                            .child(stats_card)
                            .child(cost_card),
                    )
                    .children(passive),
            );
        let plan = button("plan-weapon")
            .primary()
            .icon(IconName::Plus)
            .label("Add to the planner")
            .on_click(cx.listener({
                let (key, name, rarity) = (weapon.key.clone(), weapon.name.clone(), weapon.rarity);
                move |this, _, window, cx| {
                    this.add_to_plan(key.clone(), name.clone(), rarity, window, cx)
                }
            }));
        PageFrame::new(weapon.name.clone(), body)
            .subtitle(weapon.weapon.clone())
            .action(plan)
            .action(back)
            .body_scrolls()
            .into_any_element()
    }

    fn add_to_plan(
        &mut self,
        key: String,
        name: String,
        rarity: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        cx.spawn_in(window, async move |_, cx| {
            let message = match task.await {
                Ok(_) => crate::app::notices::Notice::done(format!(
                    "{name} is in the planner, from level {} to {}.",
                    goal.from.level, goal.to.level
                )),
                Err(err) => crate::app::notices::Notice::failed(format!(
                    "{name} was not added to the planner: {err:#}"
                )),
            };
            cx.update(|_, cx| crate::app::notices::notify(message, cx))
                .ok();
        })
        .detach();
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
