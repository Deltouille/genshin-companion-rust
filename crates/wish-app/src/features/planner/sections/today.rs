//! The "Farm today" card, above "Still needed": the domains open on the
//! game's day that give what the plan still lacks, each with its tiers still
//! lacking (they open the inventory dialog, as everywhere on the page) and
//! who needs them.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, rems,
};
use wish_ui::{card, card_title, format};

use super::super::model::TodayModel;
use super::card::Picture;
use super::materials::grid;
use super::{Actions, Look};

/// Each domain's block is at least this wide, in rems: three tiles and the
/// domain's name.
const BLOCK_REMS: f32 = 15.;

#[derive(IntoElement)]
pub(in super::super) struct FarmToday {
    pub today: TodayModel,
    pub look: Look,
    pub actions: Actions,
}

impl RenderOnce for FarmToday {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let day = format::weekday(self.today.day.day);
        let (when, help) = match self.today.day.server {
            Some(server) => (
                t!(
                    "planner-today-day",
                    day = day,
                    server = format::server(server)
                ),
                t!("planner-today-help"),
            ),
            None => (
                t!("planner-today-day-local", day = day),
                t!("planner-today-help-local"),
            ),
        };
        let body = if self.today.farms.is_empty() {
            let next = (self.today.next).map(format::weekday).unwrap_or_default();
            div().text_sm().child(t!("planner-today-none", day = next))
        } else {
            let blocks = (self.today.farms.iter().enumerate()).map(|(index, farm)| {
                let people = (farm.entries.iter().enumerate()).map(|(at, (portrait, name))| {
                    let tip = SharedString::from(name.clone());
                    div()
                        .id(SharedString::from(format!("today-{index}-for-{at}")))
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tip.clone())
                                .build(window, cx)
                        })
                        .child(Picture::of(portrait, &self.look).render(cx).size_8())
                });
                div()
                    .v_flex()
                    .gap_2()
                    .min_w(rems(BLOCK_REMS))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(farm.domain.clone()),
                    )
                    .child(grid(
                        &format!("today-{index}"),
                        &farm.materials,
                        &self.look,
                        &self.actions,
                        cx,
                    ))
                    .child(
                        div()
                            .h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                div()
                                    .mr_1()
                                    .text_sm()
                                    .text_color(muted)
                                    .child(t!("planner-today-for")),
                            )
                            .children(people),
                    )
            });
            div()
                .h_flex()
                .flex_wrap()
                .items_start()
                .gap_x_8()
                .gap_y_4()
                .children(blocks)
        };
        card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title(t!("planner-today"), cx)
                    .child(div().text_sm().text_color(muted).child(when)),
            )
            .child(div().text_sm().text_color(muted).child(help))
            .child(body)
    }
}
