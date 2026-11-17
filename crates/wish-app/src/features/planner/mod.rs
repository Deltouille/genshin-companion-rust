//! The ascension planner, after genshin-center's: the characters and weapons
//! to raise, each with where it is and where it should go (range sliders for
//! the level and the three talents), and custom ones for those the game data
//! does not have yet (a name, the stars and the materials). Each entry shows
//! its materials; the "Still needed" card adds up the plan, less what the
//! player has. Clicking a material asks how many they have.
//!
//! An entity: the sliders of each entry are its state. The plan and the
//! inventory are the [`Library`]'s; a slider saves its entry when released,
//! and while it is dragged the costs follow it. What the page shows is the
//! [`PlannerModel`](model::PlannerModel) (`model.rs`, plain data, tested
//! alone), worked out again when the plan, the inventory, the account or a
//! slider changes; the render only reads it, and `sections/` draw it.

mod add;
pub(crate) use add::Kind as AddKind;
mod custom;
mod inventory;
mod model;
pub(crate) use model::talent_names;
mod resin;
mod sections;

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
};
use wish_core::planner::{
    CharacterGoal, CustomItem, Level, MaterialId, PlanEntry, PlanTarget, Role, WeaponGoal,
    exp_series,
};

use crate::app::{Library, Session};
use model::PlannerModel;
use sections::{Actions, CardControls, EntryCard, Look, StillNeeded};
use wish_core::model::AccountId;
use wish_ui::{PageFrame, button, card, dialog_footer};

pub struct PlannerView {
    session: Entity<Session>,
    library: Entity<Library>,
    /// The sliders of each character and weapon entry, by entry id.
    controls: HashMap<i64, Controls>,
    /// The entry whose card is being dragged, dimmed meanwhile.
    dragging: Option<i64>,
    /// What the page shows, worked out again when the plan, the inventory,
    /// the account or a slider changes; the render only reads it.
    model: Rc<PlannerModel>,
    _subscriptions: Vec<Subscription>,
}

/// An entry's sliders: its level, from and to (as indexes into `steps`), and
/// a character's three talents.
struct Controls {
    level: Entity<SliderState>,
    /// The levels the slider goes through: a character's, or those of a
    /// weapon of its rarity.
    steps: Vec<Level>,
    talents: Vec<Entity<SliderState>>,
    _subscriptions: Vec<Subscription>,
}

impl PlannerView {
    pub fn new(session: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let subscriptions = vec![
            cx.observe_in(&library, window, |this, _, window, cx| {
                this.sync_controls(window, cx);
                this.rebuild(cx);
                cx.notify();
            }),
            // Another account: its plan.
            cx.observe_in(&session, window, |this, _, window, cx| {
                this.sync_controls(window, cx);
                this.rebuild(cx);
                cx.notify();
            }),
        ];
        let mut view = Self {
            session,
            library,
            controls: HashMap::new(),
            dragging: None,
            model: Rc::default(),
            _subscriptions: subscriptions,
        };
        view.sync_controls(window, cx);
        view.rebuild(cx);
        view
    }

    /// The account whose plan is shown: the sidebar's.
    fn account(&self, cx: &App) -> Option<AccountId> {
        self.session.read(cx).account()
    }

    /// Adds a character: from the levels and talents the account has (as the
    /// archive's "Add to the planner" does), to 90/90 with its talents to 10.
    fn add_character(&mut self, key: String, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let goal = self.library.read(cx).owned(account).character_goal(&key);
        let target = PlanTarget::Character { key, goal };
        let task = self.library.update(cx, |library, cx| {
            library.add_plan_entry(account, target, cx)
        });
        report_plan(task, cx);
    }

    /// Adds a weapon: from its most levelled copy that can still go up (else
    /// level 1, for a new copy) to the top of its rarity.
    fn add_weapon(&mut self, key: String, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let library = self.library.read(cx);
        let rarity = (library.planner().weapon(&key)).map_or(5, |weapon| weapon.rarity);
        let goal = library.owned(account).weapon_goal(&key, rarity);
        let target = PlanTarget::Weapon { key, goal };
        let task = self.library.update(cx, |library, cx| {
            library.add_plan_entry(account, target, cx)
        });
        report_plan(task, cx);
    }

    /// Opens the custom character dialog: for a new one, or entry `id`.
    pub(crate) fn open_custom_entry(
        &mut self,
        id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let account = self.account(cx);
        let entry = id.and_then(|id| {
            let plan = self.library.read(cx).plan(account);
            plan.iter().find(|entry| entry.id == id).cloned()
        });
        self.open_custom(entry, window, cx);
    }

    /// Opens the dialog for a new custom character on the choice of its
    /// `role`'s material (the screenshot tool uses it).
    pub(crate) fn open_custom_picker(
        &mut self,
        role: Role,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let form = self.open_custom(None, window, cx);
        form.update(cx, |form, cx| form.pick(role, window, cx));
    }

    /// Asks how many of material `id` (and the rest of its series) the
    /// player has.
    pub(crate) fn open_inventory_of(
        &mut self,
        id: MaterialId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_inventory_for(CustomItem::Material(id), window, cx);
    }

    /// Moves entry `id` to where entry `onto` is.
    fn move_entry(&mut self, id: i64, onto: i64, cx: &mut Context<Self>) {
        self.dragging = None;
        if id == onto {
            return;
        }
        let account = self.account(cx);
        let task = self.library.update(cx, |library, cx| {
            library.move_plan_entry(account, id, onto, cx)
        });
        report_plan(task, cx);
    }

    /// Opens the confirmation to remove an entry.
    fn confirm_remove(
        &mut self,
        entry: i64,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let library = self.library.downgrade();
        let title = SharedString::from(t!("planner-remove-title", name = name.as_str()));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let library = library.clone();
            alert
                .title(title.clone())
                .description(t!("planner-remove-help"))
                .footer(dialog_footer(
                    button("confirm").danger().label(t!("planner-remove")),
                ))
                .on_ok(move |_, _, cx| {
                    let task =
                        library.update(cx, |library, cx| library.remove_plan_entry(entry, cx));
                    if let Ok(task) = task {
                        report_plan(task, cx);
                    }
                    true
                })
        });
    }

    /// Makes sliders for new character and weapon entries and drops those of
    /// removed ones. Existing sliders keep their values: the plan they would
    /// be set from is what they saved.
    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let library = self.library.read(cx);
        let (plan, planner) = (library.plan(account).to_vec(), library.planner().clone());
        self.controls
            .retain(|id, _| plan.iter().any(|entry| entry.id == *id));
        for entry in plan {
            if self.controls.contains_key(&entry.id) {
                continue;
            }
            let controls = match &entry.target {
                PlanTarget::CustomWeapon { stars, goal, .. } => {
                    let steps = Level::weapon_steps(*stars);
                    self.make_controls(entry.id, steps, (goal.from, goal.to), None, window, cx)
                }
                PlanTarget::Weapon { key, goal } => {
                    let rarity = planner.weapon(key).map_or(5, |weapon| weapon.rarity);
                    let steps = Level::weapon_steps(rarity);
                    self.make_controls(entry.id, steps, (goal.from, goal.to), None, window, cx)
                }
                target => match target.goal() {
                    Some(goal) => self.make_controls(
                        entry.id,
                        Level::steps(),
                        (goal.from, goal.to),
                        Some(goal.talents),
                        window,
                        cx,
                    ),
                    None => continue,
                },
            };
            self.controls.insert(entry.id, controls);
        }
    }

    fn make_controls(
        &mut self,
        id: i64,
        steps: Vec<Level>,
        (from, to): (Level, Level),
        talents: Option<[(u8, u8); 3]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Controls {
        let step = |level: Level| steps.iter().position(|s| *s == level).unwrap_or(0) as f32;
        let last = (steps.len() - 1) as f32;
        let (from, to) = (step(from), step(to).min(last));
        let level = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(last)
                .step(1.0)
                .default_value((from, to))
        });
        let talents: Vec<_> = talents
            .into_iter()
            .flatten()
            .map(|(from, to)| {
                cx.new(|_| {
                    SliderState::new()
                        .min(1.0)
                        .max(10.0)
                        .step(1.0)
                        .default_value((f32::from(from), f32::from(to)))
                })
            })
            .collect();
        // The costs follow a slider while it is dragged; released, it saves.
        let on_slide = move |this: &mut Self, event: &SliderEvent, cx: &mut Context<Self>| {
            if let SliderEvent::Release(_) = event {
                this.save(id, cx);
            }
            this.rebuild(cx);
            cx.notify();
        };
        let mut subscriptions =
            vec![
                cx.subscribe_in(&level, window, move |this, _, event, _, cx| {
                    on_slide(this, event, cx)
                }),
            ];
        for talent in &talents {
            subscriptions.push(
                cx.subscribe_in(talent, window, move |this, _, event, _, cx| {
                    on_slide(this, event, cx)
                }),
            );
        }
        Controls {
            level,
            steps,
            talents,
            _subscriptions: subscriptions,
        }
    }

    /// The level range an entry's slider shows now.
    fn levels(&self, id: i64, cx: &App) -> Option<(Level, Level)> {
        let controls = self.controls.get(&id)?;
        let (from, to) = slider_range(&controls.level, cx);
        let last = controls.steps.len() - 1;
        Some((controls.steps[from.min(last)], controls.steps[to.min(last)]))
    }

    /// The goal a character entry's sliders show now.
    fn goal(&self, id: i64, cx: &App) -> Option<CharacterGoal> {
        let (from, to) = self.levels(id, cx)?;
        let controls = self.controls.get(&id)?;
        let talents: Vec<(u8, u8)> = (controls.talents.iter())
            .map(|talent| {
                let (from, to) = slider_range(talent, cx);
                (from as u8, to as u8)
            })
            .collect();
        Some(CharacterGoal {
            from,
            to,
            talents: talents.try_into().ok()?,
        })
    }

    /// The goal a weapon entry's slider shows now.
    fn weapon_goal(&self, id: i64, cx: &App) -> Option<WeaponGoal> {
        let (from, to) = self.levels(id, cx)?;
        Some(WeaponGoal { from, to })
    }

    /// Saves an entry's goal from its sliders.
    fn save(&mut self, id: i64, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let Some(mut entry) = self
            .library
            .read(cx)
            .plan(account)
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            return;
        };
        let changed = match (self.goal(id, cx), self.weapon_goal(id, cx)) {
            (Some(goal), _) if entry.target.goal().is_some() => {
                let saved = entry.target.goal_mut().expect("a character's goal");
                std::mem::replace(saved, goal) != goal
            }
            (_, Some(goal)) => match entry.target.weapon_goal_mut() {
                Some(saved) => std::mem::replace(saved, goal) != goal,
                None => false,
            },
            _ => false,
        };
        if changed {
            let task = (self.library).update(cx, |library, cx| {
                library.update_plan_entry(account, entry, cx)
            });
            report_plan(task, cx);
        }
    }

    /// Works the page out again: the plan as the sliders show it, shared out
    /// over the inventory, and the resin.
    fn rebuild(&mut self, cx: &App) {
        let account = self.account(cx);
        let plan = self.live_plan(cx);
        let library = self.library.read(cx);
        let model = PlannerModel::build(
            &plan,
            library.inventory(account),
            library.planner(),
            &library.settings(cx).resin,
        );
        self.model = Rc::new(model);
    }

    /// Opens a custom entry's dialog, as its sliders show it.
    fn edit_custom(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.model.entry(id).map(|entry| entry.entry.clone()) {
            self.open_custom(Some(entry), window, cx);
        }
    }

    /// What the page's pieces do when used.
    fn actions(&self, cx: &Context<Self>) -> Actions {
        let view = cx.entity().downgrade();
        let (open, edit, remove) = (view.clone(), view.clone(), view.clone());
        let (drag, drop, resin) = (view.clone(), view.clone(), view);
        Actions {
            open_material: Rc::new(move |item, window, cx| {
                open.update(cx, |view, cx| view.open_inventory_for(item, window, cx))
                    .ok();
            }),
            edit: Rc::new(move |id, window, cx| {
                edit.update(cx, |view, cx| view.edit_custom(id, window, cx))
                    .ok();
            }),
            remove: Rc::new(move |id, name, window, cx| {
                remove
                    .update(cx, |view, cx| view.confirm_remove(id, name, window, cx))
                    .ok();
            }),
            start_drag: Rc::new(move |id, cx| {
                drag.update(cx, |view, cx| {
                    view.dragging = Some(id);
                    cx.notify();
                })
                .ok();
            }),
            drop_onto: Rc::new(move |id, onto, cx| {
                drop.update(cx, |view, cx| view.move_entry(id, onto, cx))
                    .ok();
            }),
            resin_options: Rc::new(move |window, cx| {
                resin
                    .update(cx, |view, cx| view.open_resin_options(window, cx))
                    .ok();
            }),
        }
    }

    /// The plan with each entry's goal as its sliders show it.
    fn live_plan(&self, cx: &App) -> Vec<PlanEntry> {
        let mut plan = self.library.read(cx).plan(self.account(cx)).to_vec();
        for entry in &mut plan {
            if let Some(goal) = entry.target.goal_mut()
                && let Some(live) = self.goal(entry.id, cx)
            {
                *goal = live;
            }
            if let Some(goal) = entry.target.weapon_goal_mut()
                && let Some(live) = self.weapon_goal(entry.id, cx)
            {
                *goal = live;
            }
        }
        plan
    }
}

/// A range slider's ends, as whole steps.
fn slider_range(state: &Entity<SliderState>, cx: &App) -> (usize, usize) {
    match state.read(cx).value() {
        SliderValue::Range(from, to) => (from.round() as usize, to.round() as usize),
        SliderValue::Single(to) => (0, to.round() as usize),
    }
}

/// A character card is never narrower: room for its sliders and four tiles a
/// row.
const CARD_REMS: f32 = 19.;
/// At most this many cards a row.
const MAX_COLUMNS: u16 = 4;

impl Render for PlannerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let library = self.library.read(cx);
        // As many cards a row as fit, a rem apart.
        let content = crate::shell::content_width(window);
        let room = content / window.rem_size();
        let columns = (((room + 1.) / (CARD_REMS + 1.)).floor() as u16).clamp(1, MAX_COLUMNS);
        let gaps = window.rem_size() * (f32::from(columns) - 1.);
        let look = Look {
            planner: library.planner().clone(),
            images: library.images(cx).clone(),
            resin: library.settings(cx).resin.clone(),
            card_width: (content - gaps) / f32::from(columns),
        };
        let actions = self.actions(cx);
        let model = self.model.clone();

        let cards: Vec<AnyElement> = (model.entries.iter().enumerate())
            .map(|(index, entry)| {
                let id = entry.id();
                EntryCard {
                    model: model.clone(),
                    index,
                    controls: self.controls.get(&id).map(|controls| CardControls {
                        level: controls.level.clone(),
                        talents: controls.talents.clone(),
                    }),
                    dragged: self.dragging == Some(id),
                    look: look.clone(),
                    actions: actions.clone(),
                }
                .into_any_element()
            })
            .collect();

        let body: AnyElement = if model.entries.is_empty() {
            div()
                .px_6()
                .child(
                    card(cx)
                        .p_6()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("planner-empty")),
                        )
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(t!("planner-empty-help")),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .id("plan")
                .size_full()
                .overflow_y_scrollbar()
                .child(
                    div()
                        .v_flex()
                        .gap_4()
                        .px_6()
                        .pt_2()
                        .pb_6()
                        .child(StillNeeded {
                            model: model.clone(),
                            look,
                            actions,
                        })
                        .child(
                            div()
                                .grid()
                                .grid_cols(columns)
                                .gap_4()
                                .items_start()
                                .children(cards),
                        ),
                )
                .into_any_element()
        };

        let toolbar = div()
            .h_flex()
            .gap_3()
            .child(
                button("add-character")
                    .primary()
                    .icon(IconName::UserPlus)
                    .label(t!("planner-add-character"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_add(add::Kind::Character, window, cx)
                    })),
            )
            .child(
                button("add-weapon")
                    .icon(IconName::Swords)
                    .label(t!("planner-add-weapon"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_add(add::Kind::Weapon, window, cx)
                    })),
            )
            .child(
                button("add-custom")
                    .icon(IconName::Plus)
                    .label(t!("planner-custom"))
                    .tooltip(t!("planner-custom-tooltip"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_custom(None, window, cx);
                    })),
            );
        PageFrame::new(t!("planner-title"), body)
            .subtitle(t!("planner-subtitle"))
            .toolbar(toolbar)
            .body_scrolls()
    }
}

impl PlannerView {
    /// Asks how many of `item` the player has: with the other tiers of its
    /// series, when it is crafted from lower tiers, or with the smaller books
    /// or ores, for an EXP material.
    fn open_inventory_for(
        &mut self,
        item: CustomItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let library = self.library.read(cx);
        let (planner, images) = (library.planner().clone(), library.images(cx).clone());
        let materials = match &item {
            CustomItem::Material(id) => {
                // Lowest first, with the EXP each gives for books and ores.
                let series: Vec<(MaterialId, Option<u64>)> = match exp_series(*id) {
                    Some(series) => (series.iter().rev())
                        .map(|(id, gives)| (*id, Some(*gives)))
                        .collect(),
                    None => planner
                        .craft_family(*id)
                        .map_or_else(|| vec![*id], <[MaterialId]>::to_vec)
                        .into_iter()
                        .map(|id| (id, None))
                        .collect(),
                };
                series
                    .into_iter()
                    .filter_map(|(id, exp)| {
                        let material = planner.material(id)?;
                        Some(inventory::Material {
                            item: CustomItem::Material(id),
                            name: material.name.to_string(),
                            icon: images.material(material),
                            stars: material.rarity,
                            exp,
                        })
                    })
                    .collect()
            }
            CustomItem::Named(name) => vec![inventory::Material {
                item: item.clone(),
                name: name.clone(),
                icon: None,
                stars: 1,
                exp: None,
            }],
        };
        if !materials.is_empty() {
            self.open_inventory(materials, window, cx);
        }
    }
}

/// Shows a notice if a plan write fails.
fn report_plan<T: 'static>(task: gpui_kit::Task<anyhow::Result<T>>, cx: &mut App) {
    crate::app::notices::report(task, t!("planner-not-saved"), cx);
}
