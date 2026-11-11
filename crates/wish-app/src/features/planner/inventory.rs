//! The inventory dialog, after genshin-center's: clicking a material of the
//! plan asks how many the player has, with a second field to add or subtract
//! (what a domain run just gave), and for Mora a ley line's worth. A material
//! crafted from lower tiers (gems, talent books, enemy drops) shows its whole
//! series side by side, since the plan crafts them up.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AppContext as _, Context, Div, Entity, FontWeight, ImageSource, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _, rems,
};
use wish_core::planner::{CustomItem, MORA};

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
    /// What the whole plan needs.
    pub needed: u64,
}

/// One material's fields.
struct Row {
    material: Material,
    owned: u64,
    have: Entity<InputState>,
    change: Entity<InputState>,
}

pub(super) struct InventoryForm {
    rows: Vec<Row>,
    error: Option<SharedString>,
}

impl InventoryForm {
    /// `materials`: one, or a series lowest tier first; with what the
    /// player has of each.
    fn new(materials: Vec<(Material, u64)>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // A series' fields are narrow.
        let hint = if materials.len() > 1 {
            "±"
        } else {
            "+60,000 or -20"
        };
        let rows: Vec<Row> = materials
            .into_iter()
            .map(|(material, owned)| {
                let have = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("0")
                        .default_value(format::count(owned))
                });
                let change = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
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
        Self { rows, error: None }
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

    /// The counts that changed, or `None` with the reason shown.
    fn submit(&mut self, cx: &mut Context<Self>) -> Option<Vec<(CustomItem, u64)>> {
        let read = |state: &Entity<InputState>| {
            let text = state.read(cx).value();
            if text.trim().is_empty() {
                Some(0)
            } else {
                format::parse_number(&text)
            }
        };
        let mut changed = Vec::new();
        let mut problem = None;
        for row in &self.rows {
            match (read(&row.have), read(&row.change)) {
                (Some(have), _) if have < 0 => {
                    problem.get_or_insert("You can't have fewer than none.");
                }
                (Some(have), Some(change)) => {
                    let count = have.saturating_add(change).max(0) as u64;
                    if count != row.owned {
                        changed.push((row.material.item.clone(), count));
                    }
                }
                _ => {
                    problem.get_or_insert("Type whole numbers, such as 360,000 or -20.");
                }
            }
        }
        self.error = problem.map(SharedString::from);
        cx.notify();
        problem.is_none().then_some(changed)
    }

    /// One material: its tile and name, then its two fields.
    fn render_one(&self, row: &Row, cx: &Context<Self>) -> Div {
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
                                format!("The plan needs {}", format::count(material.needed)),
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

    /// A series: a column per tier, rows for the tiles, the two fields and
    /// what the plan needs.
    fn render_series(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let label = |text: &'static str| {
            div()
                .h_full()
                .flex()
                .items_center()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        let cell = || div().w(rems(7.)).flex().justify_center();
        let tiles = self.rows.iter().map(|row| {
            let name = row.material.name.clone();
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
                            .w(rems(7.))
                            .text_xs()
                            .text_center()
                            .text_color(theme.muted_foreground)
                            .line_clamp(2)
                            .child(name),
                    ),
            )
        });
        let fields = |pick: fn(&Row) -> &Entity<InputState>| {
            self.rows
                .iter()
                .map(move |row| cell().child(input(pick(row), cx)))
                .collect::<Vec<_>>()
        };
        let needs = self
            .rows
            .iter()
            .map(|row| cell().text_sm().child(format::count(row.material.needed)));
        let line = |name: Div, cells: Vec<Div>| div().h_flex().gap_2().child(name).children(cells);
        div()
            .v_flex()
            .gap_3()
            .child(line(div().w(rems(8.)), tiles.collect()))
            .child(line(label("You have").w(rems(8.)), fields(|row| &row.have)))
            .child(line(
                label("Add or subtract").w(rems(8.)),
                fields(|row| &row.change),
            ))
            .child(line(
                label("The plan needs")
                    .w(rems(8.))
                    .text_color(theme.muted_foreground),
                needs.collect(),
            ))
            .child(div().text_sm().text_color(theme.muted_foreground).child(
                "The plan crafts what it lacks of a tier from the tiers below, three for one.",
            ))
    }
}

impl Render for InventoryForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.rows.as_slice() {
            [row] => self.render_one(row, cx),
            _ => self.render_series(cx),
        };
        let danger = cx.theme().danger;
        body.when_some(self.error.clone(), |this, error| {
            this.child(div().text_sm().text_color(danger).child(error))
        })
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
        let inventory = self.library.read(cx).inventory();
        let materials: Vec<(Material, u64)> = materials
            .into_iter()
            .map(|material| {
                let owned = inventory.owned(&material.item);
                (material, owned)
            })
            .collect();
        let width = match materials.len() {
            0 | 1 => 26.,
            tiers => 12. + 7.5 * tiers as f32,
        };
        let form = cx.new(|cx| InventoryForm::new(materials, window, cx));
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
                        let task = view
                            .library
                            .update(cx, |library, cx| library.set_owned_many(changed, cx));
                        report_errors(task, window, cx);
                    })
                    .ok();
                    true
                })
        });
    }
}
