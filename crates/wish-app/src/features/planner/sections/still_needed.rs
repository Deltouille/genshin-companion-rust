//! The "Still needed" card, on top of the plan: what the whole plan lacks
//! once the inventory is shared out, and the resin farming it takes.

use std::rc::Rc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, IntoElement, ParentElement as _, RenderOnce, Styled as _, Window, div};
use wish_ui::{card, card_title, format};

use super::super::model::PlannerModel;
use super::super::resin;
use super::materials::grid;
use super::{Actions, Look};

#[derive(IntoElement)]
pub(in super::super) struct StillNeeded {
    pub model: Rc<PlannerModel>,
    pub look: Look,
    pub actions: Actions,
}

impl RenderOnce for StillNeeded {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let entries = self.model.entries.len();
        card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title("Still needed", cx).child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{entries} {}",
                            format::plural(entries as u32, "entry", "entries")
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
            .child(grid(
                "total",
                &self.model.total,
                &self.look,
                &self.actions,
                cx,
            ))
            .child(resin::plan_resin(
                &self.model.resin,
                &self.look.resin,
                &self.look.images,
                self.actions.resin_options.clone(),
                cx,
            ))
    }
}
