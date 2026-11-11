//! The ascension planner, after genshin-center's: the characters to raise,
//! each with where it is and where it should go (range sliders for the level
//! and the three talents), and custom characters for those the game data does
//! not have yet (a leak: a name, its stars and its six materials). Each entry
//! shows its materials; the "Still needed" card adds up the plan, less what
//! the player has. Clicking a material asks how many they have.
//!
//! An entity: the sliders of each entry are its state. The plan and the
//! inventory are the [`Library`]'s; a slider saves its entry when released,
//! and while it is dragged the costs follow it.

mod custom;
mod inventory;

use std::collections::{BTreeSet, HashMap};

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_kit::component::{Icon, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, CursorStyle, Div, Entity, FontWeight, ImageSource,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Pixels, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Subscription,
    Window, div, img, px, relative, rems,
};
use wish_core::planner::{
    CharacterGoal, CustomItem, EntryShare, Inventory, Level, Material, MaterialId, PlanEntry,
    PlanTarget, PlanTotals, PlannerData, WeaponGoal,
};

use crate::app::Library;
use crate::app::images::Images;
use wish_ui::{
    MaterialTile, PageFrame, badge, button, card, card_title, dialog_footer, format, icon_button,
    select, theme,
};

/// The talent rows' names, in the order of the goal's talents.
const TALENTS: [&str; 3] = ["Normal Attack", "Elemental Skill", "Elemental Burst"];

pub struct PlannerView {
    library: Entity<Library>,
    /// The pickers that add a character and a weapon.
    add_character: Entity<SelectState<Vec<EntryOption>>>,
    add_weapon: Entity<SelectState<Vec<EntryOption>>>,
    /// The sliders of each character and weapon entry, by entry id.
    controls: HashMap<i64, Controls>,
    /// The entry whose card is being dragged, dimmed meanwhile.
    dragging: Option<i64>,
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

/// An entry of a picker.
#[derive(Clone, PartialEq)]
struct EntryOption {
    key: SharedString,
    name: SharedString,
}

impl SearchableListItem for EntryOption {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &SharedString {
        &self.key
    }
}

impl PlannerView {
    pub fn new(library: Entity<Library>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let planner = library.read(cx).planner().clone();
        let picker = |options, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| SelectState::new(options, None, window, cx).searchable(true))
        };
        let add_character = picker(character_options(&planner), window, cx);
        let add_weapon = picker(weapon_options(&planner), window, cx);
        let subscriptions = vec![
            cx.observe_in(&library, window, |this, _, window, cx| {
                this.sync_controls(window, cx);
                cx.notify();
            }),
            cx.subscribe_in(
                &add_character,
                window,
                |this, picker, event: &SelectEvent<Vec<EntryOption>>, window, cx| {
                    if let SelectEvent::Confirm(Some(key)) = event {
                        this.add_character(key.to_string(), window, cx);
                        // Ready for the next one.
                        picker.update(cx, |picker, cx| picker.set_selected_index(None, window, cx));
                    }
                },
            ),
            cx.subscribe_in(
                &add_weapon,
                window,
                |this, picker, event: &SelectEvent<Vec<EntryOption>>, window, cx| {
                    if let SelectEvent::Confirm(Some(key)) = event {
                        this.add_weapon(key.to_string(), window, cx);
                        picker.update(cx, |picker, cx| picker.set_selected_index(None, window, cx));
                    }
                },
            ),
        ];
        let mut view = Self {
            library,
            add_character,
            add_weapon,
            controls: HashMap::new(),
            dragging: None,
            _subscriptions: subscriptions,
        };
        view.sync_controls(window, cx);
        view
    }

    fn add_character(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let target = PlanTarget::Character {
            key,
            goal: CharacterGoal::FULL,
        };
        let task = self
            .library
            .update(cx, |library, cx| library.add_plan_entry(target, cx));
        report_errors(task, window, cx);
    }

    /// Adds a weapon, from level 1 to the top of its rarity.
    fn add_weapon(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let planner = self.library.read(cx).planner();
        let rarity = planner.weapon(&key).map_or(5, |weapon| weapon.rarity);
        let target = PlanTarget::Weapon {
            key,
            goal: WeaponGoal::full(rarity),
        };
        let task = self
            .library
            .update(cx, |library, cx| library.add_plan_entry(target, cx));
        report_errors(task, window, cx);
    }

    /// Opens the custom character dialog: for a new one, or entry `id`.
    pub(crate) fn open_custom_entry(
        &mut self,
        id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entry = id.and_then(|id| {
            let plan = self.library.read(cx).plan();
            plan.iter().find(|entry| entry.id == id).cloned()
        });
        self.open_custom(entry, window, cx);
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
    fn move_entry(&mut self, id: i64, onto: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.dragging = None;
        if id == onto {
            return;
        }
        let task = self
            .library
            .update(cx, |library, cx| library.move_plan_entry(id, onto, cx));
        report_errors(task, window, cx);
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
        let title = SharedString::from(format!("Remove {name} from the plan?"));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let library = library.clone();
            alert
                .title(title.clone())
                .description("Its goal is forgotten. A backup is taken first.")
                .footer(dialog_footer(button("confirm").danger().label("Remove")))
                .on_ok(move |_, window, cx| {
                    let task =
                        library.update(cx, |library, cx| library.remove_plan_entry(entry, cx));
                    if let Ok(task) = task {
                        report_errors(task, window, cx);
                    }
                    true
                })
        });
    }

    /// Makes sliders for new character and weapon entries and drops those of
    /// removed ones. Existing sliders keep their values: the plan they would
    /// be set from is what they saved.
    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        let (plan, planner) = (library.plan().to_vec(), library.planner().clone());
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
        let on_slide = move |this: &mut Self,
                             event: &SliderEvent,
                             window: &mut Window,
                             cx: &mut Context<Self>| {
            if let SliderEvent::Release(_) = event {
                this.save(id, window, cx);
            }
            cx.notify();
        };
        let mut subscriptions =
            vec![
                cx.subscribe_in(&level, window, move |this, _, event, window, cx| {
                    on_slide(this, event, window, cx)
                }),
            ];
        for talent in &talents {
            subscriptions.push(cx.subscribe_in(
                talent,
                window,
                move |this, _, event, window, cx| on_slide(this, event, window, cx),
            ));
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
    fn save(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut entry) = self
            .library
            .read(cx)
            .plan()
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
            let task =
                (self.library).update(cx, |library, cx| library.update_plan_entry(entry, cx));
            report_errors(task, window, cx);
        }
    }

    /// The plan with each entry's goal as its sliders show it.
    fn live_plan(&self, cx: &App) -> Vec<PlanEntry> {
        let mut plan = self.library.read(cx).plan().to_vec();
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
        let (planner, images) = (library.planner().clone(), library.images().clone());
        let inventory = library.inventory().clone();
        let plan = self.live_plan(cx);
        let allocation = planner.allocate(&plan, &inventory);
        // As many cards a row as fit, a rem apart.
        let content = crate::shell::content_width(window);
        let room = content / window.rem_size();
        let columns = (((room + 1.) / (CARD_REMS + 1.)).floor() as u16).clamp(1, MAX_COLUMNS);
        let gaps = window.rem_size() * (f32::from(columns) - 1.);
        let shared = Shared {
            planner: &planner,
            images: &images,
            inventory: &inventory,
            card_width: (content - gaps) / f32::from(columns),
        };

        let cards: Vec<AnyElement> = plan
            .iter()
            .zip(&allocation.entries)
            .map(|(entry, share)| self.render_entry(entry, share, shared, cx))
            .collect();
        let still_needed = card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title("Still needed", cx).child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} {}",
                            plan.len(),
                            format::plural(plan.len() as u32, "entry", "entries")
                        )),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "What the plan lacks once your inventory is shared out, in the plan's \
                         order; lower tiers are crafted up. Click a material to enter how many \
                         you have.",
                    ),
            )
            .child(materials(
                "total",
                &allocation.needed,
                &allocation.missing,
                Count::Plan,
                shared,
                cx,
            ));

        let body: AnyElement = if plan.is_empty() {
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
                                .child("Nothing planned yet"),
                        )
                        .child(div().text_color(cx.theme().muted_foreground).child(
                            "Add a character or a weapon to see what raising it costs, or a \
                             custom one the game data does not have yet.",
                        )),
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
                        .child(still_needed)
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
                div().w(rems(18.)).child(
                    select(&self.add_character, cx)
                        .placeholder("Add a character…")
                        .accessibility_label("Add a character"),
                ),
            )
            .child(
                div().w(rems(18.)).child(
                    select(&self.add_weapon, cx)
                        .placeholder("Add a weapon…")
                        .accessibility_label("Add a weapon"),
                ),
            )
            .child(
                button("add-custom")
                    .icon(IconName::Plus)
                    .label("Custom…")
                    .tooltip(
                        "For a character or a weapon the game data does not have yet: a name and \
                         its materials",
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_custom(None, window, cx)),
                    ),
            );
        PageFrame::new("Ascension planner", body)
            .subtitle("What raising your characters and weapons costs")
            .toolbar(toolbar)
            .body_scrolls()
    }
}

impl PlannerView {
    fn render_entry(
        &self,
        entry: &PlanEntry,
        share: &EntryShare,
        shared: Shared,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = entry.id;
        let scope = format!("entry-{id}");
        let tiles = || {
            materials(
                &scope,
                &share.needed,
                &share.missing,
                Count::Entry,
                shared,
                cx,
            )
        };
        let preview = |portrait, name: &str, detail, goal| {
            CardPreview::new(portrait, name, detail, goal, share, shared)
        };
        let character_steps = Level::steps();
        let ranges = |goal| Ranges::of(&character_steps, goal);
        match &entry.target {
            PlanTarget::Character { key, goal } => {
                let character = shared.planner.character(key);
                let name = character.map_or(key.clone(), |c| c.name.clone());
                let portrait = Portrait::Icon(
                    character.and_then(|c| shared.images.character(c)),
                    character.map_or(4, |c| c.rarity),
                );
                let detail = character.map(|c| format!("{} · {}", c.element, c.weapon));
                let header = entry_header(
                    preview(portrait, &name, detail, Some(ranges(goal))),
                    id,
                    None,
                    cx,
                );
                let warning = character
                    .is_none()
                    .then_some("This character is no longer in the game data.");
                self.character_card(id, &name, ranges(goal), header, warning, tiles(), cx)
            }
            PlanTarget::Weapon { key, goal } => {
                let weapon = shared.planner.weapon(key);
                let name = weapon.map_or(key.clone(), |w| w.name.clone());
                let rarity = weapon.map_or(5, |w| w.rarity);
                let portrait = Portrait::Icon(weapon.and_then(|w| shared.images.weapon(w)), rarity);
                let detail = weapon.map(|w| format!("{} · {}★", w.weapon, w.rarity));
                let steps = Level::weapon_steps(rarity);
                let goal = Ranges {
                    steps: &steps,
                    from: goal.from,
                    to: goal.to,
                    talents: &[],
                };
                let header =
                    entry_header(preview(portrait, &name, detail, Some(goal)), id, None, cx);
                let warning = weapon
                    .is_none()
                    .then_some("This weapon is no longer in the game data.");
                self.character_card(id, &name, goal, header, warning, tiles(), cx)
            }
            PlanTarget::CustomCharacter {
                name, stars, goal, ..
            } => {
                let edit = icon_button(
                    SharedString::from(format!("edit-plan-{id}")),
                    IconName::Settings,
                    format!("Edit {name}"),
                )
                .ghost()
                .on_click(cx.listener({
                    let entry = entry.clone();
                    move |this, _, window, cx| this.open_custom(Some(entry.clone()), window, cx)
                }));
                let detail = Some(format!("Custom character · {stars}★"));
                let header = entry_header(
                    preview(Portrait::Custom(*stars), name, detail, Some(ranges(goal))),
                    id,
                    Some(edit.into_any_element()),
                    cx,
                );
                self.character_card(id, name, ranges(goal), header, None, tiles(), cx)
            }
            PlanTarget::CustomWeapon {
                name, stars, goal, ..
            } => {
                let edit = icon_button(
                    SharedString::from(format!("edit-plan-{id}")),
                    IconName::Settings,
                    format!("Edit {name}"),
                )
                .ghost()
                .on_click(cx.listener({
                    let entry = entry.clone();
                    move |this, _, window, cx| this.open_custom(Some(entry.clone()), window, cx)
                }));
                let steps = Level::weapon_steps(*stars);
                let goal = Ranges {
                    steps: &steps,
                    from: goal.from,
                    to: goal.to,
                    talents: &[],
                };
                let detail = Some(format!("Custom weapon · {stars}★"));
                let header = entry_header(
                    preview(Portrait::CustomWeapon(*stars), name, detail, Some(goal)),
                    id,
                    Some(edit.into_any_element()),
                    cx,
                );
                self.character_card(id, name, goal, header, None, tiles(), cx)
            }
            PlanTarget::Custom { name, .. } => {
                // The free-form entries of earlier versions: shown, and
                // removable, but no longer made.
                let detail = Some("Custom entry".to_owned());
                let card = card(cx)
                    .p_4()
                    .gap_4()
                    .child(entry_header(
                        preview(Portrait::Entry, name, detail, None),
                        id,
                        None,
                        cx,
                    ))
                    .child(tiles());
                droppable(card, id, self, cx).into_any_element()
            }
        }
    }

    /// A character's or weapon's card: its header, the sliders of its goal one
    /// above the other, its materials.
    #[allow(clippy::too_many_arguments)]
    fn character_card(
        &self,
        id: i64,
        name: &str,
        goal: Ranges,
        header: Div,
        warning: Option<&'static str>,
        materials: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let Some(controls) = self.controls.get(&id) else {
            return card(cx).p_4().child(header).into_any_element();
        };
        let level_row = slider_row(
            "Level",
            format!(
                "{}/{} → {}/{}",
                goal.from.level,
                goal.from.cap(),
                goal.to.level,
                goal.to.cap()
            ),
            Slider::new(&controls.level)
                .cursor_pointer()
                .accessibility_label(format!("{name}'s level")),
            cx,
        );
        let talent_rows = TALENTS.iter().zip(&controls.talents).zip(goal.talents).map(
            |((label, state), &(from, to))| {
                slider_row(
                    label,
                    format!("{from} → {to}"),
                    Slider::new(state)
                        .cursor_pointer()
                        .accessibility_label(format!("{name}'s {label}")),
                    cx,
                )
            },
        );
        let card = card(cx)
            .p_4()
            .gap_4()
            .child(header)
            .when_some(warning, |this, warning| {
                this.child(div().text_sm().text_color(theme.warning).child(warning))
            })
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(level_row)
                    .children(talent_rows),
            )
            .child(materials);
        droppable(card, id, self, cx).into_any_element()
    }

    /// Asks how many of `item` the player has: with the other tiers of its
    /// series, when it is crafted from lower tiers.
    fn open_inventory_for(
        &mut self,
        item: CustomItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let library = self.library.read(cx);
        let (planner, images) = (library.planner().clone(), library.images().clone());
        let needed = planner.plan_totals(&self.live_plan(cx));
        let materials = match &item {
            CustomItem::Material(id) => {
                let series = planner
                    .craft_family(*id)
                    .map_or_else(|| vec![*id], <[MaterialId]>::to_vec);
                series
                    .into_iter()
                    .filter_map(|id| {
                        let material = planner.material(id)?;
                        Some(inventory::Material {
                            item: CustomItem::Material(id),
                            name: material.name.clone(),
                            icon: images.material(material),
                            stars: material.rarity,
                            needed: needed.materials.get(&id).copied().unwrap_or(0),
                        })
                    })
                    .collect()
            }
            CustomItem::Named(name) => vec![inventory::Material {
                item: item.clone(),
                name: name.clone(),
                icon: None,
                stars: 1,
                needed: needed.named.get(name).copied().unwrap_or(0),
            }],
        };
        if !materials.is_empty() {
            self.open_inventory(materials, window, cx);
        }
    }
}

/// An entry's picture, name and detail, with its edit and remove buttons.
fn entry_header(
    preview: CardPreview,
    id: i64,
    edit: Option<AnyElement>,
    cx: &Context<PlannerView>,
) -> Div {
    let theme = cx.theme();
    let (name, detail) = (preview.name.clone(), preview.detail.clone());
    let picture = preview.portrait.render(cx);
    let remove_name = name.to_string();
    let dragged = DraggedEntry { id, card: preview };
    let view = cx.entity().downgrade();
    // The picture and the name are the handle that moves the card.
    let handle = div()
        .id(SharedString::from(format!("plan-handle-{id}")))
        .h_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .cursor(CursorStyle::OpenHand)
        .on_drag(dragged, move |dragged, _, _, cx| {
            let id = dragged.id;
            view.update(cx, |view, cx| {
                view.dragging = Some(id);
                cx.notify();
            })
            .ok();
            cx.new(|_| dragged.clone())
        })
        .tooltip(|window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(
                "Drag to change the order: the first characters take from your inventory first",
            )
            .build(window, cx)
        })
        .child(
            Icon::new(IconName::GripVertical)
                .size_4()
                .flex_shrink_0()
                .text_color(theme.muted_foreground),
        )
        .child(picture)
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(name.clone()),
                )
                .children(detail.map(|detail| {
                    div()
                        .text_sm()
                        .truncate()
                        .text_color(theme.muted_foreground)
                        .child(detail)
                })),
        );
    div().h_flex().gap_1().child(handle).children(edit).child(
        icon_button(
            SharedString::from(format!("remove-plan-{id}")),
            IconName::Close,
            format!("Remove {name} from the plan"),
        )
        .ghost()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.confirm_remove(id, remove_name.clone(), window, cx)
        })),
    )
}

/// A slider with its label and the range it shows.
fn slider_row(label: &str, value: String, slider: Slider, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .v_flex()
        .gap_1p5()
        .child(
            div()
                .h_flex()
                .justify_between()
                .text_sm()
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(label.to_owned()),
                )
                .child(div().font_weight(FontWeight::SEMIBOLD).child(value)),
        )
        .child(slider)
}

/// A card being dragged to another place in the plan, and what follows the
/// pointer meanwhile.
#[derive(Clone)]
struct DraggedEntry {
    id: i64,
    card: CardPreview,
}

/// A copy of a character card, without its controls: what follows the
/// pointer while the card is moved.
#[derive(Clone)]
struct CardPreview {
    width: Pixels,
    portrait: Portrait,
    name: SharedString,
    detail: Option<SharedString>,
    /// Each slider: its label, its range as text, and where the range starts
    /// and ends (0 to 1).
    ranges: Vec<(SharedString, SharedString, f32, f32)>,
    /// Each tile: icon, stars, count, done.
    tiles: Vec<(Option<ImageSource>, u8, SharedString, bool)>,
    /// What the game data does not have: label, done.
    named: Vec<(SharedString, bool)>,
}

/// What an entry's sliders are set to: its level range over its steps (a
/// character's, or a weapon's of its rarity), and a character's talents.
#[derive(Clone, Copy)]
struct Ranges<'a> {
    steps: &'a [Level],
    from: Level,
    to: Level,
    talents: &'a [(u8, u8)],
}

impl<'a> Ranges<'a> {
    /// A character's goal over `steps`.
    fn of(steps: &'a [Level], goal: &'a CharacterGoal) -> Self {
        Self {
            steps,
            from: goal.from,
            to: goal.to,
            talents: &goal.talents,
        }
    }
}

/// What a card shows in the corner.
#[derive(Clone)]
enum Portrait {
    /// A character's or weapon's icon, on its rarity tile.
    Icon(Option<ImageSource>, u8),
    Custom(u8),
    CustomWeapon(u8),
    Entry,
}

impl Portrait {
    fn render(&self, cx: &App) -> Div {
        let theme = cx.theme();
        let tile = div().size_12().flex_shrink_0().rounded(theme.radius);
        match self {
            Portrait::Icon(icon, stars) => tile
                .overflow_hidden()
                .bg(theme::stars_tile(*stars, cx))
                .children(
                    icon.clone()
                        .map(|icon| img(icon).size_full().object_fit(ObjectFit::Contain)),
                ),
            Portrait::Custom(stars) => tile
                .bg(theme::stars_tile(*stars, cx))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Sparkles)
                        .size_6()
                        .text_color(gpui_kit::white().opacity(0.9)),
                ),
            Portrait::CustomWeapon(stars) => tile
                .bg(theme::stars_tile(*stars, cx))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Swords)
                        .size_6()
                        .text_color(gpui_kit::white().opacity(0.9)),
                ),
            Portrait::Entry => tile
                .bg(theme.muted)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Sparkles)
                        .size_6()
                        .text_color(theme.muted_foreground),
                ),
        }
    }
}

impl CardPreview {
    fn new(
        portrait: Portrait,
        name: &str,
        detail: Option<String>,
        goal: Option<Ranges>,
        share: &EntryShare,
        shared: Shared,
    ) -> Self {
        let mut ranges = Vec::new();
        if let Some(goal) = goal {
            let last = (goal.steps.len().max(2) - 1) as f32;
            let step = |level: Level| {
                goal.steps.iter().position(|s| *s == level).unwrap_or(0) as f32 / last
            };
            ranges.push((
                "Level".into(),
                format!(
                    "{}/{} → {}/{}",
                    goal.from.level,
                    goal.from.cap(),
                    goal.to.level,
                    goal.to.cap()
                )
                .into(),
                step(goal.from),
                step(goal.to),
            ));
            for (label, &(from, to)) in TALENTS.iter().zip(goal.talents) {
                let at = |level: u8| f32::from(level.saturating_sub(1)) / 9.;
                ranges.push((
                    (*label).into(),
                    format!("{from} → {to}").into(),
                    at(from),
                    at(to),
                ));
            }
        }
        let tiles = known_materials(&share.needed, &share.missing, shared.planner)
            .into_iter()
            .map(|(material, _, lack)| {
                (
                    shared.images.material(material),
                    material.rarity,
                    format::compact(lack).into(),
                    lack == 0,
                )
            })
            .collect();
        let named = named_materials(&share.needed, &share.missing)
            .into_iter()
            .map(|(name, _, lack)| {
                (
                    format!("{} × {name}", format::count(lack)).into(),
                    lack == 0,
                )
            })
            .collect();
        Self {
            width: shared.card_width,
            portrait,
            name: name.to_owned().into(),
            detail: detail.map(Into::into),
            ranges,
            tiles,
            named,
        }
    }
}

impl Render for DraggedEntry {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Drawn where the card was: the pointer took it by its header, which
        // sits a padding (one rem) in.
        let rem = window.rem_size();
        card_copy(&self.card, cx).ml(-rem).mt(-rem)
    }
}

/// The copy of a card that follows the pointer.
fn card_copy(card: &CardPreview, cx: &App) -> Div {
    {
        let theme = cx.theme();
        let ranges = card.ranges.iter().map(|(label, value, from, to)| {
            div()
                .v_flex()
                .gap_1p5()
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .text_sm()
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child(label.clone()),
                        )
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(value.clone())),
                )
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(6.))
                        .my(px(5.))
                        .rounded_full()
                        .bg(theme.primary.opacity(0.25))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .h(px(6.))
                                .left(relative(*from))
                                .w(relative(to - from))
                                .rounded_full()
                                .bg(theme.primary),
                        ),
                )
        });
        let tiles = card.tiles.iter().map(|(icon, stars, label, done)| {
            MaterialTile::new(icon.clone(), *stars, label.clone()).done(*done)
        });
        let named = card.named.iter().map(|(label, done)| {
            badge(label.clone(), (theme.foreground, theme.muted), cx)
                .text_sm()
                .when(*done, |this| this.opacity(0.35))
        });
        div()
            .w(card.width)
            .p_4()
            .v_flex()
            .gap_4()
            .rounded(theme.radius_lg)
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.primary)
            .shadow_lg()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        Icon::new(IconName::GripVertical)
                            .size_4()
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground),
                    )
                    .child(card.portrait.render(cx))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(card.name.clone()),
                            )
                            .children(card.detail.clone().map(|detail| {
                                div()
                                    .text_sm()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(detail)
                            })),
                    ),
            )
            .when(!card.ranges.is_empty(), |this| {
                this.child(div().v_flex().gap_3().children(ranges))
            })
            .child(div().h_flex().flex_wrap().gap_2().children(tiles))
            .when(!card.named.is_empty(), |this| {
                this.child(div().h_flex().flex_wrap().gap_2().children(named))
            })
    }
}

/// Makes `card`, entry `id`'s, a place to drop another entry: it takes this
/// one's place in the plan.
fn droppable(
    card: Div,
    id: i64,
    view: &PlannerView,
    cx: &Context<PlannerView>,
) -> gpui_kit::Stateful<Div> {
    // The card being moved stays dimmed in its place meanwhile.
    let dragged = cx.has_active_drag() && view.dragging == Some(id);
    card.id(SharedString::from(format!("plan-card-{id}")))
        .when(dragged, |this| this.opacity(0.35))
        .drag_over::<DraggedEntry>(move |style, dragged, _, cx| {
            if dragged.id == id {
                style
            } else {
                style.bg(cx.theme().primary.opacity(0.15))
            }
        })
        .on_drop(
            cx.listener(move |this, dragged: &DraggedEntry, window, cx| {
                this.move_entry(dragged.id, id, window, cx)
            }),
        )
}

/// The known materials of a list, in the game's inventory order: each with
/// what is needed and what is still lacking. (EXP lacking may come out as
/// other books than the EXP needed.)
fn known_materials<'a>(
    needed: &PlanTotals,
    missing: &PlanTotals,
    planner: &'a PlannerData,
) -> Vec<(&'a Material, u64, u64)> {
    let ids: BTreeSet<MaterialId> = (needed.materials.keys())
        .chain(missing.materials.keys())
        .copied()
        .collect();
    let mut known: Vec<_> = ids
        .into_iter()
        .filter_map(|id| {
            let need = needed.materials.get(&id).copied().unwrap_or(0);
            let lack = missing.materials.get(&id).copied().unwrap_or(0);
            Some((planner.material(id)?, need, lack))
        })
        .collect();
    known.sort_by_key(|(material, _, _)| (material.sort, material.id));
    known
}

/// The materials of a list the game data does not have, by name: each with
/// what is needed and what is still lacking.
fn named_materials<'a>(
    needed: &'a PlanTotals,
    missing: &'a PlanTotals,
) -> Vec<(&'a str, u64, u64)> {
    let mut names: BTreeSet<&String> = needed.named.keys().collect();
    names.extend(missing.named.keys());
    names
        .into_iter()
        .map(|name| {
            let need = needed.named.get(name).copied().unwrap_or(0);
            let lack = missing.named.get(name).copied().unwrap_or(0);
            (name.as_str(), need, lack)
        })
        .collect()
}

/// What the counts under a list of materials say.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Count {
    /// An entry's: what it still lacks once the entries before it took their
    /// part of the inventory.
    Entry,
    /// The plan's: what it still lacks.
    Plan,
}

/// What every list of materials on the page draws from.
#[derive(Clone, Copy)]
struct Shared<'a> {
    planner: &'a PlannerData,
    images: &'a Images,
    inventory: &'a Inventory,
    /// A character card's width, for the copy that follows the pointer.
    card_width: Pixels,
}

/// Known materials as the game shows them (its inventory order), then the
/// ones the game data does not have, by name. Each opens the inventory
/// dialog. `scope` keeps the tiles' ids apart from other lists'.
fn materials(
    scope: &str,
    needed: &PlanTotals,
    missing: &PlanTotals,
    count: Count,
    shared: Shared,
    cx: &Context<PlannerView>,
) -> AnyElement {
    let theme = cx.theme();
    let known = known_materials(needed, missing, shared.planner);
    let names = named_materials(needed, missing);
    if known.is_empty() && names.is_empty() {
        return div()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child("Nothing to spend.")
            .into_any_element();
    }
    let tip = |name: &str, item: &CustomItem, need: u64, lack: u64| match count {
        Count::Entry if lack == 0 => {
            format!(
                "{name}: {} needed, covered by your inventory",
                format::count(need)
            )
        }
        Count::Entry if lack < need => format!(
            "{name}: {} needed, {} still to get",
            format::count(need),
            format::count(lack)
        ),
        Count::Entry => format!("{name}: {} needed", format::count(need)),
        Count::Plan => format!(
            "{name}: {} for the whole plan, {} owned, {} still to get",
            format::count(need),
            format::count(shared.inventory.owned(item)),
            format::count(lack)
        ),
    };
    let tiles = known.into_iter().map(|(material, need, lack)| {
        let item = CustomItem::Material(material.id);
        let text = tip(&material.name, &item, need, lack);
        clickable(format!("{scope}-{}", material.id), item, text, cx).child(
            MaterialTile::new(
                shared.images.material(material),
                material.rarity,
                format::compact(lack),
            )
            .done(lack == 0),
        )
    });
    let named = names.into_iter().map(|(name, need, lack)| {
        let item = CustomItem::Named(name.to_owned());
        let label = format!("{} × {name}", format::count(lack));
        let text = format!("{} (not in the game data)", tip(name, &item, need, lack));
        clickable(format!("{scope}-named-{name}"), item, text, cx).child(
            badge(label, (theme.foreground, theme.muted), cx)
                .text_sm()
                .when(lack == 0, |this| this.opacity(0.35)),
        )
    });
    let named: Vec<_> = named.collect();
    div()
        .v_flex()
        .gap_2()
        .child(div().h_flex().flex_wrap().gap_2().children(tiles))
        .when(!named.is_empty(), |this| {
            this.child(div().h_flex().flex_wrap().gap_2().children(named))
        })
        .into_any_element()
}

/// A material that opens the inventory dialog when clicked.
fn clickable(
    id: String,
    item: CustomItem,
    tip: String,
    cx: &Context<PlannerView>,
) -> gpui_kit::Stateful<Div> {
    let tip = SharedString::from(tip);
    div()
        .id(SharedString::from(id))
        .rounded(cx.theme().radius)
        .cursor_pointer()
        .hover(|style| style.opacity(0.8))
        .on_click(
            cx.listener(move |this, _, window, cx| {
                this.open_inventory_for(item.clone(), window, cx)
            }),
        )
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
        })
}

fn character_options(planner: &PlannerData) -> Vec<EntryOption> {
    let mut options: Vec<EntryOption> = planner
        .characters()
        .iter()
        .map(|c| EntryOption {
            key: c.key.clone().into(),
            name: c.name.clone().into(),
        })
        .collect();
    options.sort_by(|a, b| a.name.cmp(&b.name));
    options
}

fn weapon_options(planner: &PlannerData) -> Vec<EntryOption> {
    let mut options: Vec<EntryOption> = planner
        .weapons()
        .iter()
        .map(|w| EntryOption {
            key: w.key.clone().into(),
            name: w.name.clone().into(),
        })
        .collect();
    options.sort_by(|a, b| a.name.cmp(&b.name));
    options
}

/// Shows a notification if a plan write fails.
fn report_errors<T: 'static>(
    task: gpui_kit::Task<anyhow::Result<T>>,
    window: &mut Window,
    cx: &mut App,
) {
    window
        .spawn(cx, async move |cx| {
            if let Err(err) = task.await {
                cx.update(|_, cx| {
                    crate::app::notices::notify(
                        crate::app::notices::Notice::failed(format!(
                            "The plan was not saved: {err:#}"
                        )),
                        cx,
                    );
                })
                .ok();
            }
        })
        .detach();
}
