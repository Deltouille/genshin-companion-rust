//! The row of key figures at the top of the Wishes tab: wishes, 5★, 4★ and
//! the 50/50 record, over every banner type. A narrow page lays them out two
//! by two.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{App, Div, ImageSource, IntoElement, ParentElement as _, Styled as _, div, img};
use wish_core::model::Rarity;
use wish_core::stats::{RarityStats, Totals};
use wish_ui::{big_number, card, format, stat, theme};

pub(super) fn totals_row(
    totals: Totals,
    primogem: Option<ImageSource>,
    columns: usize,
    cx: &App,
) -> impl IntoElement {
    let colors = cx.theme();
    let average = |stats: RarityStats| match stats.average_pity() {
        // In the precision of the banner tables and the pity chart below.
        Some(pity) => format!("Average pity {}", format::number(pity)),
        None => "None yet".to_owned(),
    };
    let rate = match totals.win_rate() {
        Some(rate) => format::whole_percent(rate),
        None => "–".to_owned(),
    };
    let cells = [
        stat(
            "Wishes",
            big_number(format::count(totals.wishes), colors.foreground),
            div()
                .h_flex()
                .gap_1p5()
                .children(primogem.map(|primogem| img(primogem).size_4()))
                .child(format!("{} primogems", format::primogems(totals.wishes))),
            cx,
        ),
        stat(
            "5★",
            big_number(
                format::count(totals.five.count),
                theme::rarity(Rarity::Five, cx),
            ),
            average(totals.five),
            cx,
        ),
        stat(
            "4★",
            big_number(
                format::count(totals.four.count),
                theme::rarity(Rarity::Four, cx),
            ),
            average(totals.four),
            cx,
        ),
        stat(
            "50/50 won",
            big_number(rate, colors.foreground),
            format!("{} won · {} lost", totals.five_won, totals.five_lost),
            cx,
        ),
    ];
    figures(cells, columns, cx)
}

/// A card of key figures (`wish_ui::stat`s), `columns` a row, with hairlines
/// between them: before each but a row's first, and above the rows after the
/// first.
pub(in crate::features::overview) fn figures(
    cells: impl IntoIterator<Item = Div>,
    columns: usize,
    cx: &App,
) -> Div {
    let colors = cx.theme();
    card(cx)
        .grid()
        .grid_cols(columns as u16)
        .children(cells.into_iter().enumerate().map(|(ix, cell)| {
            div()
                .min_w_0()
                .px_5()
                .py_4()
                .border_color(colors.border)
                .when(ix % columns > 0, |this| this.border_l_1())
                .when(ix >= columns, |this| this.border_t_1())
                .child(cell)
        }))
}
