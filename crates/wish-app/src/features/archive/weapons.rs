//! The weapon archive, like the characters': every weapon as a card,
//! filtered by type and rarity; a click opens its page (the wish art, ATK and
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
    AnyElement, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ObjectFit, ParentElement as _, Render, SharedString, Styled as _, StyledImage as _,
    Subscription, Window, div, img, rems,
};
use wish_core::planner::{Level, PlanTarget, Weapon, WeaponGoal};

use super::Sort;
use super::detail::{facts_grid, hero_banner, level_badge, materials, table};
use crate::app::{Library, Session};
use wish_ui::{
    PageFrame, Segmented, badge, button, card, card_title, filter_chip, format, input, theme,
};

const TYPES: [&str; 5] = ["Sword", "Claymore", "Polearm", "Bow", "Catalyst"];

pub struct WeaponsView {
    session: Entity<Session>,
    library: Entity<Library>,
    search: Entity<InputState>,
    /// The weapon types and rarities shown; none means all.
    types: BTreeSet<SharedString>,
    stars: BTreeSet<u8>,
    sort: Sort,
    /// The weapon whose page is open (a genshin-db key).
    open: Option<String>,
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
        let subscriptions = vec![
            cx.observe(&library, |_, _, cx| cx.notify()),
            cx.observe(&session, |_, _, cx| cx.notify()),
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
            sort: Sort::default(),
            open: None,
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

        let query = self.search.read(cx).value().to_lowercase();
        let mut shown: Vec<&Weapon> = planner
            .weapons()
            .iter()
            .filter(|w| query.is_empty() || w.name.to_lowercase().contains(&query))
            .filter(|w| self.types.is_empty() || self.types.contains(w.weapon.as_str()))
            .filter(|w| self.stars.is_empty() || self.stars.contains(&w.rarity))
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

        let theme = cx.theme();
        let cards = shown.iter().map(|weapon| {
            let key = weapon.key.clone();
            let new = latest.is_some() && Some(version(&weapon.key)) == latest;
            let picture = div()
                .relative()
                .w_full()
                .h(rems(7.5))
                .rounded_t(theme.radius_lg)
                .bg(theme::stars_tile(weapon.rarity, cx))
                .children(
                    images
                        .weapon(weapon)
                        .map(|icon| img(icon).size_full().object_fit(ObjectFit::Contain)),
                )
                .when(new, |this| {
                    this.child(div().absolute().top_1().right_1().child(badge(
                        "New",
                        (theme.primary_foreground, theme.primary),
                        cx,
                    )))
                });
            Button::new(SharedString::from(format!("weapon-{}", weapon.key)))
                .ghost()
                .p_0()
                .w(rems(7.5))
                .h_auto()
                .rounded(theme.radius_lg)
                .cursor_pointer()
                .accessibility_label(format!(
                    "{}, {}★ {}",
                    weapon.name, weapon.rarity, weapon.weapon
                ))
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .rounded(theme.radius_lg)
                        .bg(theme.group_box)
                        .child(picture)
                        .child(
                            div()
                                .w_full()
                                .px_2()
                                .py_1p5()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_center()
                                .truncate()
                                .text_color(theme.foreground)
                                .child(weapon.name.clone()),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.show_weapon(&key, cx)))
        });

        let type_filters = TYPES.map(|kind| {
            let on = self.types.contains(kind);
            filter_chip(
                SharedString::from(format!("weapon-type-{kind}")),
                kind,
                theme.primary,
                on,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                Self::toggle(&mut this.types, kind.into());
                cx.notify();
            }))
        });
        let star_filters = [5u8, 4, 3, 2, 1].map(|stars| {
            let on = self.stars.contains(&stars);
            filter_chip(
                SharedString::from(format!("weapon-stars-{stars}")),
                format!("{stars}★"),
                theme::stars_color(stars, cx),
                on,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                Self::toggle(&mut this.stars, stars);
                cx.notify();
            }))
        });
        let sort = Segmented::new("weapon-sort")
            .option("Newest")
            .option("Name")
            .option("Rarity")
            .selected(match self.sort {
                Sort::Newest => 0,
                Sort::Name => 1,
                Sort::Rarity => 2,
            })
            .on_select(cx.listener(|this, ix: &usize, _, cx| {
                this.sort = match ix {
                    1 => Sort::Name,
                    2 => Sort::Rarity,
                    _ => Sort::Newest,
                };
                cx.notify();
            }));
        let toolbar = div()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap_4()
                    .child(div().w(rems(18.)).child(input(&self.search, cx)))
                    .child(sort),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap_x_4()
                    .gap_y_2()
                    .child(div().h_flex().gap_2().children(type_filters))
                    .child(div().h_flex().gap_2().children(star_filters)),
            );

        let count = shown.len();
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
                    .child(div().h_flex().flex_wrap().gap_3().children(cards))
                    .when(count == 0, |this| {
                        this.child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child("No weapon matches these filters."),
                        )
                    }),
            );
        PageFrame::new("Weapons", body)
            .subtitle(format!("{count} of {} weapons", planner.weapons().len()))
            .toolbar(toolbar)
            .body_scrolls()
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
        let goal = WeaponGoal::full(rarity);
        let target = PlanTarget::Weapon { key, goal };
        let account = self.session.read(cx).account();
        let task = self.library.update(cx, |library, cx| {
            library.add_plan_entry(account, target, cx)
        });
        cx.spawn_in(window, async move |_, cx| {
            let message = match task.await {
                Ok(_) => crate::app::notices::Notice::done(format!(
                    "{name} is in the planner, from level 1 to {}.",
                    goal.to.level
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
