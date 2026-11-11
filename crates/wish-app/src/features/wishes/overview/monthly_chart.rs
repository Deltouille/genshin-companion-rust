//! The chart of wishes per month, all banner types together.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::chart::AreaChart;
use gpui_kit::{App, IntoElement, ParentElement as _, SharedString, Styled as _, div};
use wish_core::model::Rarity;
use wish_core::stats::MonthStats;

use wish_ui::{card, card_title, format, legend, theme};

/// Wishes per month as paimon.moe draws them: the wishes, the 4★ and the 5★
/// as overlaid areas on one axis (they count the same thing, so the 5★ hug
/// the bottom, which is the truth), every month from the first wish to the
/// last (a month without wishes drops to zero, so the time axis stays honest).
pub(super) fn monthly_chart(months: &[MonthStats], cx: &App) -> impl IntoElement {
    let (Some(first), Some(last)) = (months.first(), months.last()) else {
        return div().into_any_element();
    };
    let by_month: std::collections::HashMap<(i32, u32), MonthStats> =
        months.iter().map(|m| ((m.year, m.month), *m)).collect();
    let mut series = Vec::new();
    let (mut year, mut month) = (first.year, first.month);
    while (year, month) <= (last.year, last.month) {
        series.push(by_month.get(&(year, month)).copied().unwrap_or(MonthStats {
            year,
            month,
            total: 0,
            five: 0,
            four: 0,
        }));
        (year, month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
    }

    // An area needs two points: a single month is said in words.
    if let [only] = series.as_slice() {
        let date = chrono::NaiveDate::from_ymd_opt(only.year, only.month, 1)
            .map(|d| d.format("%B %Y").to_string())
            .unwrap_or_else(|| only.label());
        return card(cx)
            .h_full()
            .gap_3()
            .p_4()
            .child(card_title("Wishes per month", cx))
            .child(div().text_color(cx.theme().muted_foreground).child(format!(
                "All {} wishes so far were made in {date}: {} × 4★, {} × 5★.",
                format::count(only.total),
                only.four,
                only.five
            )))
            .into_any_element();
    }

    // `rarity_chart`'s colours pass the dataviz validator together, against
    // the card surface of both themes. Fills are the stroke at low opacity,
    // so the areas behind stay visible.
    let wishes = theme::rarity_chart(Rarity::Three, cx);
    let four = theme::rarity_chart(Rarity::Four, cx);
    let five = theme::rarity_chart(Rarity::Five, cx);
    // Month labels need about 5 rem each; at most 7 keep them apart.
    let ticks = series.len().min(7);
    let most = series.iter().map(|m| m.total).max().unwrap_or(0);
    let (top, y_ticks) = wish_ui::axis::round_axis(most, 6);
    let chart = AreaChart::new(series)
        .appear(false)
        .linear()
        .x(|m: &MonthStats| SharedString::from(m.label()))
        .x_tick_count(ticks)
        .y_axis(true)
        .y_domain(0.0, f64::from(top))
        // No headroom above the top, or the ticks would leave the round steps.
        .y_padding(0.0, 0.0)
        .y_tick_count(y_ticks)
        .y_tick_format(|v| format!("{v:.0}"))
        .y(|m: &MonthStats| f64::from(m.total))
        .stroke(wishes)
        .fill(wishes.opacity(0.3))
        .name("Wishes")
        .y(|m: &MonthStats| f64::from(m.four))
        .stroke(four)
        .fill(four.opacity(0.35))
        .name("4★")
        .y(|m: &MonthStats| f64::from(m.five))
        .stroke(five)
        .fill(five.opacity(0.4))
        .name("5★")
        .tooltip_title(|m: &MonthStats| {
            chrono::NaiveDate::from_ymd_opt(m.year, m.month, 1)
                .map(|d| d.format("%B %Y").to_string())
                .unwrap_or_else(|| m.label())
                .into()
        })
        .tooltip_value(|m: &MonthStats, series, value| {
            let count = format::count(value as u64);
            if series == 0 {
                format!("{count} ({} primogems)", format::primogems(m.total)).into()
            } else {
                count.into()
            }
        });

    card(cx)
        .h_full()
        .gap_3()
        .p_4()
        .child(card_title("Wishes per month", cx).child(legend(
            [
                (wishes, "Wishes".into()),
                (four, "4★".into()),
                (five, "5★".into()),
            ],
            cx,
        )))
        .child(div().flex_1().min_h_0().child(chart))
        .into_any_element()
}
