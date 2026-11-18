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
//!
//! - this file: the view, its actions, and the page's layout;
//! - `controls`: each entry's sliders, and saving them;
//! - `day`: the game's day, for "Farm today";
//! - `model`: what the page shows, as plain data;
//! - `sections/`: the cards that draw it (`today`, `still_needed`, `card`,
//!   `materials`, `drag`);
//! - the dialogs: `add` (characters and weapons as cards), `custom` (a
//!   custom character or weapon), `inventory` (how many of a material the
//!   player has), `resin` (the estimate's options).

mod add;
mod controls;
mod day;
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
use gpui_kit::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Task, Window, div,
};
use wish_core::planner::{CustomItem, MaterialId, PlanTarget, Role, exp_series, game_day};

use crate::app::{Library, Session};
use controls::Controls;
use day::now_on;
use model::{GameDay, PlannerModel};
use sections::{Actions, CardControls, EntryCard, FarmToday, Look, StillNeeded};
use wish_core::model::AccountId;
use wish_ui::{PageFrame, button, card, dialog_footer};

/// The planner page: its entries' sliders, the card being dragged, and the
/// model the page draws.
pub struct PlannerView {
    session: Entity<Session>,
    library: Entity<Library>,
    /// The sliders of each character and weapon entry, by entry id.
    controls: HashMap<i64, Controls>,
    /// The entry whose card is being dragged, dimmed meanwhile.
    dragging: Option<i64>,
    /// What the page shows, worked out again when the plan, the inventory,
    /// the account, a slider or the game's day changes; the render only
    /// reads it.
    model: Rc<PlannerModel>,
    /// The game's day the model is for.
    day: GameDay,
    /// Wakes when the game's day changes, for its domains.
    _day_watch: Task<()>,
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
        let day = GameDay {
            day: game_day(now_on(None)),
            server: None,
        };
        let mut view = Self {
            session,
            library,
            controls: HashMap::new(),
            dragging: None,
            model: Rc::default(),
            day,
            _day_watch: Self::watch_the_day(cx),
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

    /// Opens the custom character or weapon dialog: for a new one, or entry
    /// `id`.
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
    /// Works the page out again: the plan as the sliders show it, shared out
    /// over the inventory, and the resin.
    fn rebuild(&mut self, cx: &App) {
        let account = self.account(cx);
        let plan = self.live_plan(cx);
        self.day = self.game_day(cx);
        let library = self.library.read(cx);
        let model = PlannerModel::build(
            &plan,
            library.inventory(account),
            library.planner(),
            &library.settings(cx).resin,
            self.day,
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
                        .children(model.today.clone().map(|today| FarmToday {
                            today,
                            look: look.clone(),
                            actions: actions.clone(),
                        }))
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
