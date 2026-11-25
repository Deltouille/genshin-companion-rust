//! The Traveler's Diary's page: the month's primogems and Mora, the
//! primogems of every month kept, where the month's came from, its days, and
//! every reward it logged.

use gpui_kit::component::chart::AreaChart;
use gpui_kit::component::scroll::ScrollableElement as _;
use wish_core::chronicle::DiaryRecord;
use wish_ui::{Donut, DonutSlice};

use super::model::DiaryMonth;
use super::*;

/// Where two cards sit side by side, in rems of room.
const TWO_COLUMNS_REMS: f32 = 52.;

pub(super) fn page(all: &[DiaryRecord], record: &DiaryRecord, room: f32, cx: &App) -> AnyElement {
    let DiaryMonth {
        title: _,
        primogems,
        mora,
        sources,
        days,
        log,
        kept,
    } = model::diary_month(record);
    let theme = cx.theme();
    let count = |n: u64| wish_ui::format::count(n);
    let blue = theme::rarity_chart(wish_core::model::Rarity::Three, cx);
    let trend = trend(
        "diary-months",
        t!("chronicle-primogems-per-month"),
        model::diary_trend(all),
        None,
        blue,
        cx,
    );
    // The sources, in the 5★ ramp, the most the deepest.
    let ramp = theme::rarity_ramp(wish_core::model::Rarity::Five, sources.len().max(1), cx);
    let slices: Vec<DonutSlice> = (sources.iter().zip(ramp.into_iter().rev()))
        .map(|(source, color)| DonutSlice {
            label: source.name.clone().into(),
            label_color: None,
            count: source.primogems,
            color,
        })
        .collect();
    // The legend needs room for a source's name in French ("Activités
    // quotidiennes"): three fifths of the row, the chart the rest.
    let two = room >= TWO_COLUMNS_REMS;
    let donut = div().h(rems(16.)).when(two, |this| this.col_span(3)).child(
        Donut::new(
            "diary-sources",
            t!("chronicle-sources"),
            slices,
            t!("chronicle-primogems"),
        )
        .label_rems(10.5),
    );
    let trend = div().when(two, |this| this.col_span(2)).child(trend);
    let per_day = per_day(days, blue, cx);
    let log_count = log.len();
    div()
        .v_flex()
        .gap_4()
        .child(figures(
            vec![
                (t!("chronicle-primogems"), count(primogems), String::new()),
                (t!("chronicle-mora"), count(mora), String::new()),
            ],
            room,
            cx,
        ))
        .child(
            div()
                .grid()
                .grid_cols(if two { 5 } else { 1 })
                .gap_4()
                .child(trend)
                .child(donut),
        )
        .child(per_day)
        .when(!log.is_empty(), |this| {
            this.child(
                card(cx)
                    .p_5()
                    .gap_3()
                    .child(
                        card_title(t!("chronicle-log"), cx).child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(t!("chronicle-log-count", count = log_count)),
                        ),
                    )
                    .child(
                        div()
                            .id("diary-log")
                            .max_h(rems(24.))
                            .overflow_y_scrollbar()
                            .child(div().v_flex().children(log.into_iter().enumerate().map(
                                |(ix, (when, action, num))| {
                                    div()
                                        .h_flex()
                                        .gap_3()
                                        .py_1p5()
                                        .text_sm()
                                        .when(ix > 0, |this| {
                                            this.border_t_1().border_color(theme.border)
                                        })
                                        .child(
                                            div()
                                                .w(rems(11.))
                                                .flex_none()
                                                .text_color(theme.muted_foreground)
                                                .child(when),
                                        )
                                        .child(div().flex_1().min_w_0().truncate().child(action))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(format!("+{}", wish_ui::format::count(num))),
                                        )
                                },
                            ))),
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

/// The month's primogems day by day, as an area.
fn per_day(days: Vec<model::Point>, color: Hsla, cx: &App) -> AnyElement {
    let frame = card(cx)
        .h(rems(14.))
        .gap_3()
        .p_4()
        .child(card_title(t!("chronicle-per-day"), cx));
    if days.len() < 2 {
        return frame.into_any_element();
    }
    let most = days.iter().map(|d| d.value).max().unwrap_or(0).max(1);
    let (top, ticks) = wish_ui::axis::round_axis(u32::try_from(most).unwrap_or(u32::MAX), 5);
    let x_ticks = days.len().min(8);
    let chart = AreaChart::new(days)
        .appear(false)
        .linear()
        .x(|d: &model::Point| SharedString::from(d.tick.clone()))
        .x_tick_count(x_ticks)
        .y_axis(true)
        .y_domain(0.0, f64::from(top))
        .y_padding(0.0, 0.0)
        .y_tick_count(ticks)
        .y_tick_format(|v| format!("{v:.0}"))
        .y(|d: &model::Point| d.value as f64)
        .stroke(color)
        .fill(color.opacity(0.3))
        .name(t!("chronicle-primogems"))
        .tooltip_title(|d: &model::Point| d.title.clone().into())
        .tooltip_value(|d: &model::Point, _, _| wish_ui::format::count(d.value).into());
    frame
        .child(div().flex_1().min_h_0().child(chart))
        .into_any_element()
}
