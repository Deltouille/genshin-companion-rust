//! The wish table: one row per wish, filtered by rarity, sorted by a column
//! (newest first by default), a page at a time. Which rows, in which order,
//! and each cell's text are `model.rs`'s; this is the kit's delegate that
//! draws them.

use std::sync::Arc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme as _, Size};
use gpui_kit::{
    App, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, SharedString, Stateful, Styled as _, Window, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _, rems,
};
use wish_core::data::GameData;
use wish_core::engine::{Analysis, Pull};
use wish_core::model::{Pool, Rarity};

use super::model::{self, Col, Order, PAGE_SIZES, Paging};
use crate::app::images::Images;
use wish_ui::item_tile;
use wish_ui::outcome_badge;
use wish_ui::theme;
use wish_ui::{Pager, Segmented};

/// Table delegate over one pool's pulls.
pub struct PullTable {
    analysis: Option<Arc<Analysis>>,
    pool: Option<Pool>,
    /// Per banner index of the pool: "Raiden Shogun · 4.3".
    banner_labels: Vec<String>,
    /// Per pull: its number on its banner, from 1 (paimon.moe's "#Roll").
    rolls: Vec<Option<u32>>,
    filter: [bool; 3],
    /// The column the rows are sorted by, if not the default (newest first).
    sort: Option<(Col, ColumnSort)>,
    /// Indices into the pool's pulls after filtering and sorting.
    rows: Vec<usize>,
    /// Set by a sort, so the view clears the row selection (see `take_resorted`).
    resorted: bool,
    /// The page shown and how many rows a page holds.
    paging: Paging,
    cols: Vec<Col>,
    images: Arc<Images>,
    /// Columns are sized in rem so they follow the base font size.
    rem: Pixels,
    /// The table's width on screen, once known: what the columns leave of it
    /// goes to the item and banner names.
    available: Pixels,
}

/// The card's content inset (`p_4`) in rem: the first column starts there.
const PAGE_INSET_REMS: f32 = 1.0;
/// A row's height in rem: room for the item's tile and the cell's padding
/// above and below.
pub(super) const ROW_REMS: f32 = 4.0;
/// The header row's height in rem, as the kit's large table has it.
pub(super) const HEADER_REMS: f32 = 2.5;
/// The side of an item's tile in rem, big enough to tell the items apart.
const TILE_REMS: f32 = 3.0;

impl PullTable {
    pub(super) fn new(rem: Pixels) -> Self {
        Self {
            analysis: None,
            pool: None,
            banner_labels: Vec::new(),
            rolls: Vec::new(),
            filter: [true, true, true],
            sort: None,
            resorted: false,
            rows: Vec::new(),
            paging: Paging::first(),
            cols: Vec::new(),
            images: Arc::new(Images::default()),
            rem,
            available: Pixels::ZERO,
        }
    }

    /// Records the table's width on screen; `true` when it changed, so the
    /// columns need sizing again.
    pub(super) fn set_available_width(&mut self, width: Pixels) -> bool {
        let changed = (width - self.available).abs() >= gpui_kit::px(1.);
        if changed {
            self.available = width;
        }
        changed
    }

    /// A column's width in rem before the spare room is shared out. They add
    /// up to 60.5 rem, so the table fits a 1280 px window beside the sidebar.
    fn base_rems(&self, col: Col) -> f32 {
        match col {
            Col::Item => 13.5 + PAGE_INSET_REMS,
            Col::Kind => 9.5,
            Col::Pity => 4.0,
            Col::RateUp => 8.0,
            // Without a rate-up column (Standard), the banner takes its room.
            Col::Banner if !self.cols.contains(&Col::RateUp) => 18.5,
            Col::Banner => 10.5,
            Col::Roll => 3.5,
            Col::Time => 10.5,
        }
    }

    /// Shows `pool`. Keeps the sort, filter and page size; the page goes
    /// back to the first when the pool changes, and stays in range otherwise.
    pub(super) fn set_pool(
        &mut self,
        analysis: Arc<Analysis>,
        pool: Pool,
        data: &GameData,
        images: Arc<Images>,
        filter: [bool; 3],
    ) {
        if self.pool != Some(pool) {
            self.paging.page = 0;
            self.sort = None;
        }
        self.images = images;
        self.banner_labels = model::banner_labels(data, pool);
        self.rolls = analysis
            .pool(pool)
            .map(|pool| pool.banner_rolls())
            .unwrap_or_default();
        self.cols = Col::of(pool);
        self.analysis = Some(analysis);
        self.pool = Some(pool);
        self.filter = filter;
        self.arrange();
    }

    pub(super) fn set_filter(&mut self, filter: [bool; 3]) {
        self.filter = filter;
        self.paging.page = 0;
        self.arrange();
    }

    /// Filters and sorts the rows, and keeps the page in range.
    fn arrange(&mut self) {
        let sort = match self.sort {
            Some((col, ColumnSort::Ascending)) => Some((col, Order::Ascending)),
            Some((col, ColumnSort::Descending)) => Some((col, Order::Descending)),
            _ => None,
        };
        self.rows = model::arrange(self.pulls(), &self.rolls, self.filter, sort);
        self.paging = self.paging.at(self.paging.page, self.rows.len());
    }

    /// How many pages the rows fill (at least one, so an empty table has a page).
    pub(super) fn pages(&self) -> usize {
        self.paging.pages(self.rows.len())
    }

    pub(super) fn paging(&self) -> Paging {
        self.paging
    }

    /// All the rows that pass the filter, on every page.
    pub(super) fn row_total(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn set_page(&mut self, page: usize) {
        self.paging = self.paging.at(page, self.rows.len());
    }

    /// Changes the page size, keeping the first row shown on screen.
    pub(super) fn set_page_size(&mut self, page_size: usize) {
        self.paging = self.paging.resized(page_size);
    }

    /// Whether the rows were sorted since the last call: the selected row
    /// index then points at another wish.
    pub(super) fn take_resorted(&mut self) -> bool {
        std::mem::take(&mut self.resorted)
    }

    /// The rows of the current page.
    fn visible(&self) -> &[usize] {
        &self.rows[self.paging.range(self.rows.len())]
    }

    fn pulls(&self) -> &[Pull] {
        let pool = self
            .pool
            .and_then(|pool| self.analysis.as_ref()?.pool(pool));
        pool.map_or(&[], |pool| pool.pulls.as_slice())
    }

    /// The pull on row `row_ix` of the current page.
    pub(super) fn pull(&self, row_ix: usize) -> Option<&Pull> {
        self.pulls().get(*self.visible().get(row_ix)?)
    }

    fn col(&self, col_ix: usize) -> Col {
        self.cols.get(col_ix).copied().unwrap_or(Col::Banner)
    }

    fn text(&self, row_ix: usize, col_ix: usize) -> String {
        let Some(pull) = self.pull(row_ix) else {
            return String::new();
        };
        let roll =
            (self.visible().get(row_ix)).and_then(|ix| self.rolls.get(*ix).copied().flatten());
        model::cell(pull, self.col(col_ix), &self.banner_labels, roll)
    }
}

impl TableDelegate for PullTable {
    fn columns_count(&self, _: &App) -> usize {
        self.cols.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.visible().len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        let col = self.col(col_ix);
        // Every column sorts; the header shows the current sort, kept here so
        // it survives `TableState::refresh`, which reads the columns again.
        let sort = match self.sort {
            Some((sorted, sort)) if sorted == col => sort,
            _ => ColumnSort::Default,
        };
        let column = Column::new(col.name(), col.name()).sort(sort);
        // On a wide window, the item and banner names share the spare room
        // rather than leaving it empty on the right.
        let base: f32 = self.cols.iter().map(|col| self.base_rems(*col)).sum();
        // Less a margin for the vertical scrollbar, so no horizontal one appears.
        let spare = (self.available - self.rem * (base + 1.0)).max(Pixels::ZERO);
        let width = match col {
            Col::Item | Col::Banner => self.rem * self.base_rems(col) + spare / 2.,
            _ => self.rem * self.base_rems(col),
        };
        // The rows are taller than any of the kit's sizes, whose padding is
        // then the narrow default: every column keeps the large one.
        let mut paddings = Size::Large.table_cell_padding();
        if col == Col::Item {
            // The first column starts its text on the card's inset, like the
            // cards' content elsewhere.
            paddings.left = self.rem * PAGE_INSET_REMS;
        }
        let column = column.width(width).paddings(paddings);
        match col {
            Col::Pity | Col::Roll => column.text_right(),
            _ => column,
        }
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        self.sort = Some((self.col(col_ix), sort));
        self.resorted = true;
        self.paging.page = 0;
        self.arrange();
    }

    /// Rows are keyed by wish id, which stays stable when filters change.
    /// A 4★ or 5★ row is tinted with its rarity from the left, fading out,
    /// as on paimon.moe, so they stand out in a run of 3★.
    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let Some(pull) = self.pull(row_ix) else {
            return div().id(("row", row_ix));
        };
        let rarity = pull.wish.rarity;
        let strength = if cx.theme().is_dark() { 0.24 } else { 0.16 };
        div().id(SharedString::from(pull.wish.id.to_string())).when(
            rarity != Rarity::Three,
            |this| {
                let color = theme::rarity(rarity, cx);
                this.bg(linear_gradient(
                    90.,
                    linear_color_stop(color.opacity(strength), 0.),
                    linear_color_stop(color.opacity(0.), 1.),
                ))
            },
        )
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let text = self.text(row_ix, col_ix);
        let Some(pull) = self.pull(row_ix) else {
            return div();
        };
        let rarity = pull.wish.rarity;
        let muted = cx.theme().muted_foreground;
        let content = match self.col(col_ix) {
            // The icon has its own slot so names stay aligned without one.
            Col::Item => div()
                .h_flex()
                .gap_3()
                .child(item_tile(
                    pull.wish.rarity,
                    self.images.item(&pull.wish),
                    rems(TILE_REMS),
                    cx,
                ))
                .child(
                    div()
                        .truncate()
                        .when(rarity != Rarity::Three, |this| {
                            this.text_color(theme::rarity(rarity, cx))
                                .font_weight(FontWeight::SEMIBOLD)
                        })
                        .child(text),
                ),
            Col::Kind => div()
                .h_flex()
                .gap_1p5()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::rarity(rarity, cx))
                        .child(format!("{}★", rarity.stars())),
                )
                .child(
                    div()
                        .truncate()
                        .text_color(muted)
                        .child(model::kind(pull.wish.item_type)),
                ),
            Col::RateUp => match pull.rate_up.map(|r| r.outcome()) {
                Some(outcome) => div().h_flex().child(outcome_badge(outcome, cx)),
                None => div(),
            },
            Col::Banner | Col::Roll | Col::Time => div().truncate().text_color(muted).child(text),
            Col::Pity => div().child(text),
        };
        // Centred in the row's height; full width, so a right-aligned
        // column's text reaches the right.
        div()
            .size_full()
            .flex()
            .items_center()
            .child(content.flex_1().min_w_0())
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        Some(self.text(row_ix, col_ix).into())
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.text(row_ix, col_ix)
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("No wishes match the selected rarities")
    }
}

impl super::HistoryView {
    /// Sizes the table's columns to its width on screen.
    pub(super) fn set_table_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.table.update(cx, |table, cx| {
            if table.delegate_mut().set_available_width(width) {
                table.refresh(cx);
            }
        });
    }

    /// Shows page `page` (from 0) of the wish table.
    pub(super) fn set_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.selected_manual = None;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_page(page);
            table.clear_selection(cx);
            table.scroll_to_row(0, cx);
            table.refresh(cx);
        });
        cx.notify();
    }

    pub(super) fn set_page_size(&mut self, page_size: usize, cx: &mut Context<Self>) {
        self.selected_manual = None;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_page_size(page_size);
            table.clear_selection(cx);
            table.refresh(cx);
        });
        cx.notify();
    }

    /// Under the table: the page size, which rows are shown, and the pages.
    pub(super) fn render_pager(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let table = self.table.read(cx).delegate();
        let (paging, pages, total) = (table.paging(), table.pages(), table.row_total());
        let (page, size) = (paging.page, paging.size);
        let shown = paging.shown(total);
        let sizes = PAGE_SIZES
            .iter()
            .fold(Segmented::new("page-size"), |sizes, n| {
                sizes.option(n.to_string())
            })
            .selected(PAGE_SIZES.iter().position(|n| *n == size).unwrap_or(0))
            .on_select(cx.listener(|this, ix: &usize, _, cx| {
                if let Some(size) = PAGE_SIZES.get(*ix) {
                    this.set_page_size(*size, cx);
                }
            }));
        div()
            .h_flex()
            .items_center()
            .gap_4()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .text_color(theme.muted_foreground)
                    .child("Rows per page")
                    .child(sizes),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(theme.muted_foreground)
                    .child(shown),
            )
            // One page needs no navigation.
            .when(pages > 1, |this| {
                this.child(
                    Pager::new("history-pages")
                        .page(page + 1)
                        .pages(pages)
                        .on_change(cx.listener(|this, page: &usize, _, cx| {
                            this.set_page(page.saturating_sub(1), cx)
                        })),
                )
            })
    }
}
