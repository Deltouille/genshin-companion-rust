//! The inventory dialog, after genshin-center's: clicking a material of the
//! plan asks how many the player has, with a second field to add or subtract
//! (what a domain run just gave), and for Mora a ley line's worth.
//!
//! A material crafted from lower tiers (gems, talent books, enemy drops)
//! shows its whole series side by side, since the plan crafts the spare of a
//! tier up, three for one; Hero's Wit (or Mystic Enhancement Ore) shows the
//! three books (or ores), each counted for its EXP. Under the fields, what
//! the plan needs, what it crafts and what it still lacks follow what is
//! typed: the plan is shared out again on each change.

use std::sync::Arc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, FontWeight, ImageSource, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, rems,
};
use wish_core::planner::{Allocation, CustomItem, Inventory, MORA, PlanEntry, PlannerData};

use super::{PlannerView, report_errors};
use wish_ui::{MaterialTile, button, dialog_footer, format, input};

/// What a ley line outcrop of wealth gives at world level 8.
const LEY_LINE_MORA: i64 = 60_000;

/// A material the dialog is about, as the plan shows it.
#[derive(Clone)]
pub(super) struct Material {
    pub item: CustomItem,
    pub name: String,
    pub icon: Option<ImageSource>,
    pub stars: u8,
    /// The EXP it gives, for an EXP book or ore.
    pub exp: Option<u64>,
}

/// The plan the dialog shares the typed inventory out over.
struct Plan {
    planner: Arc<PlannerData>,
    entries: Vec<PlanEntry>,
    /// What the player has, as saved.
    inventory: Inventory,
}

/// One material's fields.
struct Row {
    material: Material,
    owned: u64,
    have: Entity<InputState>,
    change: Entity<InputState>,
}

/// What the plan makes of one material with the inventory as typed.
#[derive(Clone, Copy)]
struct Figures {
    needed: u64,
    crafted: u64,
    missing: u64,
}

pub(super) struct InventoryForm {
    rows: Vec<Row>,
    plan: Plan,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl InventoryForm {
    /// `materials`: one, or a series lowest tier first.
    fn new(
        materials: Vec<Material>,
        plan: Plan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A series' fields are narrow.
        let hint = if materials.len() > 1 {
            "±"
        } else {
            "+60,000 or -20"
        };
        let mut subscriptions = Vec::new();
        let rows: Vec<Row> = materials
            .into_iter()
            .map(|material| {
                let owned = plan.inventory.owned(&material.item);
                let have = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("0")
                        .default_value(format::count(owned))
                });
                let change = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
                // The figures under the fields follow what is typed.
                for state in [&have, &change] {
                    subscriptions.push(cx.subscribe(state, |_, _, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            cx.notify();
                        }
                    }));
                }
                Row {
                    material,
                    owned,
                    have,
                    change,
                }
            })
            .collect();
        if let Some(first) = rows.first() {
            first.have.update(cx, |state, cx| state.focus(window, cx));
        }
        Self {
            rows,
            plan,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// Adds a ley line's Mora to the change.
    fn add_ley_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows.first() else {
            return;
        };
        let change = format::parse_number(&row.change.read(cx).value()).unwrap_or(0);
        let total = change.saturating_add(LEY_LINE_MORA);
        let sign = if total < 0 { "-" } else { "+" };
        let text = format!("{sign}{}", format::count(total.unsigned_abs()));
        row.change
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }

    /// How many of a row the player has with what is typed, or why it
    /// doesn't read.
    fn typed(row: &Row, cx: &App) -> Result<u64, &'static str> {
        let read = |state: &Entity<InputState>| {
            let text = state.read(cx).value();
            if text.trim().is_empty() {
                Some(0)
            } else {
                format::parse_number(&text)
            }
        };
        match (read(&row.have), read(&row.change)) {
            (Some(have), _) if have < 0 => Err("You can't have fewer than none."),
            (Some(have), Some(change)) => Ok(have.saturating_add(change).max(0) as u64),
            _ => Err("Type whole numbers, such as 360,000 or -20."),
        }
    }

    /// The plan shared out with the inventory as typed (a field that
    /// doesn't read keeps what was saved).
    fn allocation(&self, cx: &App) -> Allocation {
        let mut inventory = self.plan.inventory.clone();
        for row in &self.rows {
            if let Ok(count) = Self::typed(row, cx) {
                inventory.set(&row.material.item, count);
            }
        }
        self.plan.planner.allocate(&self.plan.entries, &inventory)
    }

    /// The counts that changed, or `None` with the reason shown.
    fn submit(&mut self, cx: &mut Context<Self>) -> Option<Vec<(CustomItem, u64)>> {
        let mut changed = Vec::new();
        let mut problem = None;
        for row in &self.rows {
            match Self::typed(row, cx) {
                Ok(count) if count != row.owned => {
                    changed.push((row.material.item.clone(), count));
                }
                Ok(_) => {}
                Err(reason) => {
                    problem.get_or_insert(reason);
                }
            }
        }
        self.error = problem.map(SharedString::from);
        cx.notify();
        problem.is_none().then_some(changed)
    }

    /// One material: its tile and name, then its two fields.
    fn render_one(&self, row: &Row, figures: Figures, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let material = &row.material;
        let field = |label: &'static str, state: &Entity<InputState>| {
            div()
                .v_flex()
                .gap_1p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(input(state, cx))
        };
        let still = match figures.missing {
            0 => "nothing still needed".to_owned(),
            n => format!("{} still needed", format::count(n)),
        };
        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .child(MaterialTile::bare(material.icon.clone(), material.stars))
                    .child(
                        div()
                            .v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(material.name.clone()),
                            )
                            .child(div().text_sm().text_color(theme.muted_foreground).child(
                                format!(
                                    "The plan needs {}, {still}",
                                    format::count(figures.needed)
                                ),
                            )),
                    ),
            )
            .child(field("You have", &row.have))
            .child(field("Add or subtract", &row.change))
            .when(material.item == CustomItem::Material(MORA), |this| {
                this.child(
                    div().child(
                        button("ley-line")
                            .ghost()
                            .label(format!(
                                "Add a ley line (+{})",
                                format::count(LEY_LINE_MORA as u64)
                            ))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_ley_line(window, cx)),
                            ),
                    ),
                )
            })
    }

    /// A series: a column per tier, rows for the tiles, the two fields, then
    /// what the plan needs, crafts and still lacks of each tier.
    fn render_series(&self, figures: &[Figures], cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let label = |text: &'static str| {
            div()
                .w(rems(LABEL_REMS))
                .h_full()
                .flex()
                .items_center()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        let cell = || div().w(rems(CELL_REMS)).flex().justify_center();
        // An EXP series counts as its largest item: the others need nothing
        // of their own.
        let largest = self.rows.iter().filter_map(|row| row.material.exp).max();
        let exp = largest.is_some();
        let counted = |row: &Row| !exp || row.material.exp == largest;

        let tiles = self.rows.iter().map(|row| {
            cell().child(
                div()
                    .v_flex()
                    .items_center()
                    .gap_1()
                    .child(MaterialTile::bare(
                        row.material.icon.clone(),
                        row.material.stars,
                    ))
                    .child(
                        div()
                            .w(rems(CELL_REMS))
                            .text_xs()
                            .text_center()
                            .text_color(muted)
                            .line_clamp(2)
                            .child(row.material.name.clone()),
                    )
                    .when_some(row.material.exp, |this, gives| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("{} EXP", format::count(gives))),
                        )
                    }),
            )
        });
        let fields = |pick: fn(&Row) -> &Entity<InputState>| {
            self.rows
                .iter()
                .map(move |row| cell().child(input(pick(row), cx)))
                .collect::<Vec<_>>()
        };
        let numbers = |value: fn(&Figures) -> u64, first_tier: bool| {
            self.rows
                .iter()
                .zip(figures)
                .enumerate()
                .map(|(tier, (row, figures))| {
                    let shown = counted(row) && (first_tier || tier > 0);
                    cell().text_sm().child(if shown {
                        format::count(value(figures))
                    } else {
                        "–".to_owned()
                    })
                })
                .collect::<Vec<_>>()
        };
        let line = |name: Div, cells: Vec<Div>| div().h_flex().gap_2().child(name).children(cells);

        let note = if exp {
            // "4 Adventurer's Experience or 20 Wanderer's Advice do a Hero's
            // Wit's work."
            let top = self.rows.last().map(|row| row.material.name.clone());
            let others: Vec<String> = (self.rows.iter().rev().skip(1))
                .filter_map(|row| Some((largest? / row.material.exp?, &row.material.name)))
                .map(|(n, name)| format!("{n} {name}"))
                .collect();
            format!(
                "Each counts for its EXP: {} do a {}'s work.",
                others.join(" or "),
                top.unwrap_or_default()
            )
        } else {
            "What a tier lacks is crafted from the spare of the tiers below, three for one: \
             what is still needed is what is left to farm."
                .to_owned()
        };

        div()
            .v_flex()
            .gap_3()
            .child(line(div().w(rems(LABEL_REMS)), tiles.collect()))
            .child(line(label("You have"), fields(|row| &row.have)))
            .child(line(label("Add or subtract"), fields(|row| &row.change)))
            .child(line(
                label("The plan needs").text_color(muted),
                numbers(|f| f.needed, true),
            ))
            .when(!exp, |this| {
                this.child(line(
                    label("Crafted").text_color(muted),
                    numbers(|f| f.crafted, false),
                ))
            })
            .child(line(label("Still needed"), numbers(|f| f.missing, true)))
            .child(div().text_sm().text_color(muted).child(note))
    }
}

/// A series' label column and tier columns, in rems.
const LABEL_REMS: f32 = 8.;
const CELL_REMS: f32 = 7.;

impl Render for InventoryForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let allocation = self.allocation(cx);
        let figures: Vec<Figures> = self
            .rows
            .iter()
            .map(|row| figures_of(&row.material.item, &allocation))
            .collect();
        let body = match self.rows.as_slice() {
            [row] => self.render_one(row, figures[0], cx),
            _ => self.render_series(&figures, cx),
        };
        let danger = cx.theme().danger;
        body.when_some(self.error.clone(), |this, error| {
            this.child(div().text_sm().text_color(danger).child(error))
        })
    }
}

/// What the plan needs, crafts and still lacks of `item`.
fn figures_of(item: &CustomItem, allocation: &Allocation) -> Figures {
    match item {
        CustomItem::Material(id) => Figures {
            needed: allocation.needed.materials.get(id).copied().unwrap_or(0),
            crafted: allocation.crafted(*id),
            missing: allocation.missing.materials.get(id).copied().unwrap_or(0),
        },
        CustomItem::Named(name) => Figures {
            needed: allocation
                .needed
                .named
                .get(name.trim())
                .copied()
                .unwrap_or(0),
            crafted: 0,
            missing: allocation
                .missing
                .named
                .get(name.trim())
                .copied()
                .unwrap_or(0),
        },
    }
}

impl PlannerView {
    /// Asks how many of `materials` (one, or a series) the player has.
    pub(super) fn open_inventory(
        &mut self,
        materials: Vec<Material>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let account = self.account(cx);
        let library = self.library.read(cx);
        let plan = Plan {
            planner: library.planner().clone(),
            entries: self.live_plan(cx),
            inventory: library.inventory(account).clone(),
        };
        let width = match materials.len() {
            0 | 1 => 26.,
            tiers => LABEL_REMS + 4. + (CELL_REMS + 0.5) * tiers as f32,
        };
        let form = cx.new(|cx| InventoryForm::new(materials, plan, window, cx));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let (form, view) = (form.clone(), view.clone());
            dialog
                .title("Inventory")
                .w(window.rem_size() * width)
                .child(form.clone())
                .footer(dialog_footer(
                    button("save-inventory").primary().label("Save"),
                ))
                .on_ok(move |_, window, cx| {
                    let Some(changed) = form.update(cx, |form, cx| form.submit(cx)) else {
                        // Keep the dialog open on the error.
                        return false;
                    };
                    if changed.is_empty() {
                        return true;
                    }
                    view.update(cx, |view, cx| {
                        let account = view.account(cx);
                        let task = view.library.update(cx, |library, cx| {
                            library.set_owned_many(account, changed, cx)
                        });
                        report_errors(task, window, cx);
                    })
                    .ok();
                    true
                })
        });
    }
}
