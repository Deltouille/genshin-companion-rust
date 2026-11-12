//! The overview's Wishes tab: the account's key figures, then one card per
//! banner type (where pity stands, then the type in detail), then wishes and
//! 5★ pity over time.

mod details;
mod monthly_chart;
mod pool_card;
mod totals;

use gpui_kit::base::StyledExt as _;
use gpui_kit::{
    AnyElement, App, Div, IntoElement, ParentElement as _, Styled as _, Window, div, rems,
};
use wish_core::data::GameData;
use wish_core::engine::PoolAnalysis;
use wish_core::model::{Pool, Rarity};

use super::NavigateHandler;
use crate::app::images::Images;
use crate::app::library::AccountSummary;
use crate::shell::{Page, WishPage};
use monthly_chart::monthly_chart;
use pool_card::{POOL_CARD_REMS, pool_card};
pub(super) use totals::figures;
use totals::totals_row;

/// The height of a chart card, in rem: the charts fill it, side by side or
/// stacked.
const CHART_REMS: f32 = 20.;

/// The account's wishes in figures. The cards' History buttons open the
/// banner type's history through `on_navigate`.
pub(super) fn wishes_tab(
    summary: &AccountSummary,
    data: Option<&GameData>,
    images: &Images,
    on_navigate: &NavigateHandler,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let primogem = images.primogem();
    let analysis = &summary.analysis;

    // paimon.moe hides an untouched Beginners' Wish; so do we.
    let pools: Vec<&PoolAnalysis> = analysis
        .pools
        .iter()
        .filter(|p| p.pool != Pool::Beginners || p.counters.total > 0)
        .collect();

    let cards = pools.iter().map(|pool| {
        let page = Page::Wishes(WishPage::History(pool.pool));
        let on_navigate = on_navigate.clone();
        pool_card(
            pool,
            data,
            primogem.clone(),
            move |_, window, cx| on_navigate(page, window, cx),
            cx,
        )
    });

    // The page's width decides the grids: as many banner type cards a
    // row as fit, in rows as even as can be (four as two by two rather
    // than three and one); on a narrow page the charts stack and the key
    // figures go two by two.
    let room = crate::shell::content_width(window) / window.rem_size();
    let fit = ((room + 1.) / (POOL_CARD_REMS + 1.)).floor().max(1.) as usize;
    let card_columns = even_columns(pools.len(), fit) as u16;
    let wide = room > 52.;
    let totals_columns = if room > 46. { 4 } else { 2 };
    // A row of two chart cards, the first taking two thirds of the
    // width, or one under the other. Each has a definite height, which
    // its chart fills.
    let row = |first: AnyElement, second: AnyElement| {
        let chart = || div().h(rems(CHART_REMS));
        if wide {
            div()
                .grid()
                .grid_cols(3)
                .gap_4()
                .child(chart().col_span(2).child(first))
                .child(chart().child(second))
        } else {
            div()
                .v_flex()
                .gap_4()
                .child(chart().child(first))
                .child(chart().child(second))
        }
    };

    div()
        .v_flex()
        .gap_6()
        .child(totals_row(
            analysis.totals(),
            primogem.clone(),
            totals_columns,
            cx,
        ))
        // Cards of a row stretch to the tallest, so the row ends evenly.
        .child(div().grid().grid_cols(card_columns).gap_4().children(cards))
        .child(row(
            monthly_chart(&analysis.monthly, cx).into_any_element(),
            five_star_pity(analysis, cx).into_any_element(),
        ))
}

/// How many columns lay `count` cards out when `fit` fit a row: as few rows
/// as fit allows, then as few columns as keep that many rows, so the rows
/// are even.
pub(super) fn even_columns(count: usize, fit: usize) -> usize {
    let rows = count.max(1).div_ceil(fit.max(1));
    count.max(1).div_ceil(rows)
}

/// Every 5★ of the account, oldest first, on one pity line (the axis goes up
/// to the highest hard pity, 90; a weapon banner's 5★ is named in its tooltip).
fn five_star_pity(analysis: &wish_core::engine::Analysis, cx: &App) -> impl IntoElement {
    let points = analysis
        .pulls_of(Rarity::Five)
        .into_iter()
        .map(|(pool, pull)| {
            let time = pull.wish.time.naive();
            wish_ui::PityPoint {
                tick: time.format("%Y-%m").to_string().into(),
                title: format!(
                    "{} · {} · {}",
                    pull.wish.name,
                    time.format("%-d %b %Y"),
                    pool.display_name()
                )
                .into(),
                pity: pull.pity.unwrap_or(0),
            }
        })
        .collect();
    let top = Pool::ALL
        .iter()
        .map(|pool| pool.hard_pity_five())
        .max()
        .unwrap_or(90);
    wish_ui::pity_chart(points, top, cx)
}

#[cfg(test)]
mod tests {
    use super::even_columns;

    #[test]
    fn cards_fill_even_rows() {
        // (cards, fitting a row) → columns
        for (count, fit, columns) in [
            (3, 3, 3),
            (3, 5, 3),
            (4, 3, 2),
            (4, 4, 4),
            (5, 3, 3),
            (3, 2, 2),
            (2, 1, 1),
            (0, 3, 1),
        ] {
            assert_eq!(
                even_columns(count, fit),
                columns,
                "{count} cards, {fit} fit"
            );
        }
    }
}
