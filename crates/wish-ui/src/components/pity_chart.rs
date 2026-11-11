use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::chart::LineChart;
use gpui_kit::{App, Div, ParentElement as _, SharedString, Styled as _, div};
use wish_core::model::Rarity;

use super::{card, card_title};
use crate::format;

/// One 5★ on [`pity_chart`].
#[derive(Clone)]
pub struct PityPoint {
    /// Under the line: when it came (`2024-07`).
    pub tick: SharedString,
    /// The tooltip's title: the item, and when and where it came.
    pub title: SharedString,
    pub pity: u32,
}

/// How lucky the 5★ were, one after the other: a dot per 5★ at the pity it
/// came at, oldest on the left (dates below), on an axis up to `top` (the
/// hard pity), with the average as a dashed line. Fewer than two 5★ are said in words.
pub fn pity_chart(points: Vec<PityPoint>, top: u32, cx: &App) -> Div {
    let theme = cx.theme();
    let average = (!points.is_empty())
        .then(|| points.iter().map(|p| f64::from(p.pity)).sum::<f64>() / points.len() as f64);
    let title = card_title("5★ pity", cx).child(
        div()
            .flex_none()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(match average {
                Some(average) if points.len() > 1 => {
                    format!("average {}", format::number(average))
                }
                _ => String::new(),
            }),
    );
    let frame = card(cx).h_full().gap_3().p_4().child(title);
    if let [only] = points.as_slice() {
        return frame.child(
            div()
                .text_color(theme.muted_foreground)
                .child(format!("One 5★ so far, at pity {}.", only.pity)),
        );
    }
    if points.is_empty() {
        return frame.child(div().text_color(theme.muted_foreground).child("No 5★ yet."));
    }

    // The 5★ series colour, validated against both card surfaces.
    let gold = crate::theme::rarity_chart(Rarity::Five, cx);
    // Three dates (the first, the middle, the last) fit a third of the page.
    let ticks = points.len().min(3);
    let mut chart = LineChart::new(points)
        .appear(false)
        .linear()
        .dot()
        .x(|p: &PityPoint| p.tick.clone())
        .x_tick_count(ticks)
        .y(|p: &PityPoint| f64::from(p.pity))
        .stroke(gold)
        .y_axis(true)
        // The whole range of pity, so a dot's height reads against the hard
        // pity; no headroom, so the ticks stay on whole steps.
        .y_domain(0.0, f64::from(top))
        .y_padding(0.0, 0.0)
        .y_tick_count(crate::axis::whole_ticks(top, 5))
        .y_tick_format(|v| format!("{v:.0}"))
        .name("Pity")
        .tooltip_title(|p: &PityPoint| p.title.clone())
        .tooltip_value(|p: &PityPoint, _| p.pity.to_string().into());
    if let Some(average) = average {
        chart = chart.reference_line(average);
    }
    frame.child(div().flex_1().min_h_0().child(chart))
}
