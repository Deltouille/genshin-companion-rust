//! The Spiral Abyss's page: the period's figures, its stars against every
//! period kept, its records, then each floor's chambers with the teams that
//! fought them.

use super::*;
use wish_core::chronicle::AbyssRecord;
use wish_core::planner::PlannerData;

use super::model::{AbyssPeriod, Floor, Highlight};

/// Where two cards sit side by side, in rems of room.
const TWO_COLUMNS_REMS: f32 = 52.;

pub(super) fn page(
    all: &[AbyssRecord],
    record: &AbyssRecord,
    planner: &PlannerData,
    images: &Images,
    room: f32,
    cx: &App,
) -> AnyElement {
    let period = model::abyss_period(record, planner);
    let AbyssPeriod {
        range: _,
        stars,
        max_stars,
        deepest,
        deepest_detail,
        battles,
        won,
        highlights,
        floors,
        kept,
    } = period;
    let theme = cx.theme();
    let count = |n: u32| wish_ui::format::count(n);
    let trend = trend(
        "abyss-stars",
        t!("chronicle-stars-per-period"),
        model::abyss_trend(all),
        Some(u64::from(max_stars)),
        theme::rarity_chart(wish_core::model::Rarity::Five, cx),
        cx,
    );
    let records = records(&highlights, planner, images, cx);
    let two = room >= TWO_COLUMNS_REMS;
    div()
        .v_flex()
        .gap_4()
        .child(figures(
            vec![
                (
                    t!("chronicle-stars"),
                    count(stars),
                    t!("chronicle-stars-of", max = max_stars),
                ),
                (t!("chronicle-deepest"), deepest, deepest_detail),
                (
                    t!("chronicle-battles"),
                    count(battles),
                    t!("chronicle-battles-won", won = won),
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
                .child(records),
        )
        .children(
            floors
                .iter()
                .map(|floor| self::floor(floor, planner, images, cx)),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(kept),
        )
        .into_any_element()
}

/// The period's records: each first place, its character and its figure.
fn records(
    highlights: &[Highlight],
    planner: &PlannerData,
    images: &Images,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    section(t!("chronicle-records"), cx)
        .child(
            div()
                .v_flex()
                .gap_2()
                .children(highlights.iter().enumerate().map(|(ix, highlight)| {
                    div()
                        .h_flex()
                        .gap_3()
                        .child(member_tile(
                            SharedString::from(format!("abyss-record-{ix}")),
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
                })),
        )
        .into_any_element()
}

/// One floor: its stars, then a row per chamber with each half's team.
fn floor(floor: &Floor, planner: &PlannerData, images: &Images, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let floor_id = floor.title.clone();
    card(cx)
        .p_5()
        .gap_4()
        .child(card_title(floor.title.clone(), cx).child(
            div().text_sm().text_color(theme.muted_foreground).child(t!(
                "chronicle-floor-stars",
                stars = floor.stars,
                max = floor.max_stars
            )),
        ))
        .child(
            div()
                .v_flex()
                .gap_3()
                .children(floor.chambers.iter().enumerate().map(|(ix, chamber)| {
                    div()
                        .h_flex()
                        .flex_wrap()
                        .items_start()
                        .gap_4()
                        .py_2()
                        .when(ix > 0, |this| this.border_t_1().border_color(theme.border))
                        .child(
                            div()
                                .v_flex()
                                .w(rems(14.))
                                .gap_0p5()
                                .child(
                                    div()
                                        .h_flex()
                                        .gap_2()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(chamber.title.clone()),
                                        )
                                        .child(stars(chamber.stars, chamber.max_stars, cx)),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(chamber.when.clone()),
                                ),
                        )
                        .children(
                            chamber
                                .halves
                                .iter()
                                .enumerate()
                                .map(|(half, (label, team))| {
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(label.clone()),
                                        )
                                        .child(div().h_flex().gap_2().children(
                                            team.iter().enumerate().map(|(n, member)| {
                                                member_tile(
                                                    SharedString::from(format!(
                                                        "abyss-{floor_id}-{ix}-{half}-{n}"
                                                    )),
                                                    member,
                                                    planner,
                                                    images,
                                                    cx,
                                                )
                                            }),
                                        ))
                                }),
                        )
                })),
        )
        .into_any_element()
}
