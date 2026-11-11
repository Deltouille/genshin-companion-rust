//! Overview: the account's key figures, then one card per banner type (where
//! pity stands, then the type in detail), then wishes and 5★ pity over time.

mod details;
mod monthly_chart;
mod pool_card;
mod totals;

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, Styled as _, Window, div, rems,
};
use wish_core::data::GameData;
use wish_core::engine::PoolAnalysis;
use wish_core::model::{Pool, Rarity};

use crate::app::images::Images;
use crate::app::library::AccountSummary;
use crate::features::wishes::import::{import_file_button, import_game_button};
use crate::shell::{Page, WishPage};
use monthly_chart::monthly_chart;
use pool_card::{POOL_CARD_REMS, pool_card};
use totals::totals_row;
use wish_ui::PageFrame;

/// The height of a chart card, in rem: the charts fill it, side by side or
/// stacked.
const CHART_REMS: f32 = 20.;

/// Called with the page to open, e.g. a banner type's history.
type NavigateHandler = Rc<dyn Fn(Page, &mut Window, &mut App)>;

/// The overview of one account.
///
/// It has no state of its own, so it is a plain component (`RenderOnce`)
/// that the shell builds each time with what it shows: the account's
/// summary (the session's selected account), the game data and the
/// primogem icon (the library), and whether a write is running (to show the
/// import button busy). Opening a banner's history is routing, the shell's
/// job, so the cards' History buttons call back through `on_navigate`.
#[derive(IntoElement)]
pub struct Overview {
    summary: AccountSummary,
    data: Option<Arc<GameData>>,
    images: Arc<Images>,
    busy: bool,
    on_navigate: NavigateHandler,
}

impl Overview {
    pub fn new(
        summary: AccountSummary,
        data: Option<Arc<GameData>>,
        images: Arc<Images>,
        busy: bool,
        on_navigate: impl Fn(Page, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            summary,
            data,
            images,
            busy,
            on_navigate: Rc::new(on_navigate),
        }
    }
}

impl RenderOnce for Overview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (summary, data) = (&self.summary, &self.data);
        let primogem = self.images.primogem();
        let analysis = &summary.analysis;
        let account = &summary.account;

        // paimon.moe hides an untouched Beginners' Wish; so do we.
        let pools: Vec<&PoolAnalysis> = analysis
            .pools
            .iter()
            .filter(|p| p.pool != Pool::Beginners || p.counters.total > 0)
            .collect();

        let cards = pools.iter().map(|pool| {
            let page = Page::Wishes(WishPage::History(pool.pool));
            let on_navigate = self.on_navigate.clone();
            pool_card(
                pool,
                data.as_deref(),
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

        let body = div()
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
            ));

        let frame = PageFrame::new(account.label.clone(), body);
        let frame = match &account.uid {
            Some(uid) => frame.subtitle(format!("UID {uid}")),
            None => frame,
        };
        frame
            .action(import_game_button())
            .action(import_file_button(self.busy))
    }
}

/// How many columns lay `count` cards out when `fit` fit a row: as few rows
/// as fit allows, then as few columns as keep that many rows, so the rows
/// are even.
fn even_columns(count: usize, fit: usize) -> usize {
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
