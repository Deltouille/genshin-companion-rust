//! The Imaginarium Theater's page: the period's figures, its furthest act
//! against every period kept, its highlights, then each act with its cast.

use super::*;
use wish_core::chronicle::TheaterRecord;
use wish_core::planner::PlannerData;

use super::model::TheaterPeriod;

/// Where two cards sit side by side, in rems of room.
const TWO_COLUMNS_REMS: f32 = 52.;
/// The narrowest an act's card may be, in rems: four tiles.
const ACT_REMS: f32 = 16.;

pub(super) fn page(
    all: &[TheaterRecord],
    record: &TheaterRecord,
    planner: &PlannerData,
    images: &Images,
    room: f32,
    cx: &App,
) -> AnyElement {
    let TheaterPeriod {
        range: _,
        difficulty,
        furthest,
        medals,
        flowers,
        support,
        minutes,
        highlights,
        acts,
        kept,
    } = model::theater_period(record, planner);
    let theme = cx.theme();
    let count = |n: u32| wish_ui::format::count(n);
    let trend = trend(
        "theater-acts",
        t!("chronicle-furthest-per-period"),
        model::theater_trend(all),
        None,
        theme::rarity_chart(wish_core::model::Rarity::Four, cx),
        cx,
    );
    let highlights =
        section(t!("chronicle-highlights"), cx).child(div().v_flex().gap_2().children(
            highlights.iter().enumerate().map(|(ix, highlight)| {
                div()
                    .h_flex()
                    .gap_3()
                    .child(member_tile(
                        SharedString::from(format!("theater-highlight-{ix}")),
                        &highlight.member,
                        planner,
                        images,
                        cx,
                    ))
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().truncate().child(highlight.title.clone()))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(highlight.member.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(highlight.value.clone()),
                    )
            }),
        ));
    let two = room >= TWO_COLUMNS_REMS;
    let columns = ((room + 0.75) / (ACT_REMS + 0.75)).floor().max(1.) as u16;
    let gold = theme::rarity(wish_core::model::Rarity::Five, cx);
    div()
        .v_flex()
        .gap_4()
        .child(figures(
            vec![
                (
                    t!("chronicle-furthest-act"),
                    count(u32::from(furthest)),
                    difficulty,
                ),
                (
                    t!("chronicle-medals"),
                    count(medals),
                    t!("chronicle-medals-detail"),
                ),
                (
                    t!("chronicle-flowers"),
                    count(flowers),
                    t!("chronicle-support-count", count = support),
                ),
                (
                    t!("chronicle-time"),
                    minutes
                        .map_or_else(|| "–".to_owned(), |m| t!("chronicle-minutes", minutes = m)),
                    String::new(),
                ),
            ],
            room,
            cx,
        ))
        .child(
            div()
                .grid()
                .grid_cols(if two { 2 } else { 1 })
                .gap_4()
                .child(trend)
                .child(highlights),
        )
        .when(!acts.is_empty(), |this| {
            this.child(
                section(t!("chronicle-acts"), cx).child(
                    div()
                        .grid()
                        .grid_cols(columns)
                        .gap_3()
                        .children(acts.iter().enumerate().map(|(ix, act)| {
                            div()
                                .v_flex()
                                .gap_2()
                                .p_3()
                                .rounded(theme.radius_lg)
                                .bg(theme.muted)
                                .child(
                                    div()
                                        .h_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex_1()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(act.title.clone()),
                                        )
                                        .when(act.medal, |this| {
                                            this.child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "act-medal-{ix}"
                                                    )))
                                                    .text_color(gold)
                                                    .child("★")
                                                    .tooltip(|window, cx| {
                                                        Tooltip::new(t!("chronicle-medal"))
                                                            .build(window, cx)
                                                    }),
                                            )
                                        }),
                                )
                                .child(div().h_flex().gap_2().children(
                                    act.cast.iter().enumerate().map(|(n, member)| {
                                        member_tile(
                                            SharedString::from(format!("act-{ix}-{n}")),
                                            member,
                                            planner,
                                            images,
                                            cx,
                                        )
                                    }),
                                ))
                        })),
                ),
            )
        })
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(kept),
        )
        .into_any_element()
}
