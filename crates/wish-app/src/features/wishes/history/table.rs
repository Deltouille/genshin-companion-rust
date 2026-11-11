//! The wish table: one row per wish, filtered by rarity, sorted by a column
//! (newest first by default), a page at a time.

use std::cmp::Ordering;
use std::sync::Arc;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme as _, Size};
use gpui_kit::{
    App, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, SharedString, Stateful, Styled as _, Window, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _,
};
use wish_core::data::GameData;
use wish_core::engine::{Analysis, Pull, RateUpOutcome};
use wish_core::model::{ItemType, Pool, Rarity};

use super::rarity_ix;
use crate::app::images::Images;
use wish_ui::item_tile;
use wish_ui::outcome_badge;
use wish_ui::{format, theme};

/// The page sizes offered under the table.
pub(super) const PAGE_SIZES: [usize; 3] = [25, 50, 100];

/// Table delegate over one pool's pulls.
pub struct PullTable {
    analysis: Option<Arc<Analysis>>,
    pool: Option<Pool>,
    /// Per banner index of the pool: "Raiden Shogun · 4.3".
    banner_labels: Vec<SharedString>,
    /// Per pull: its number on its banner, from 1 (paimon.moe's "#Roll").
    rolls: Vec<Option<u32>>,
    filter: [bool; 3],
    /// The column the rows are sorted by, if not the default (newest first).
    sort: Option<(Col, ColumnSort)>,
    /// Indices into the pool's pulls after filtering and sorting.
    rows: Vec<usize>,
    /// Set by a sort, so the view clears the row selection (see `take_resorted`).
    resorted: bool,
    /// The page shown (from 0) and how many rows a page holds.
    page: usize,
    page_size: usize,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Item,
    /// Rarity and character or weapon: "5★ Character". One column for both,
    /// so the table fits a 1280 px window.
    Kind,
    Pity,
    /// Only on pools with rate-up items.
    RateUp,
    Banner,
    /// The wish's number on its banner.
    Roll,
    Time,
}

impl Col {
    fn name(self) -> &'static str {
        match self {
            Col::Item => "Item",
            Col::Kind => "Type",
            Col::Pity => "Pity",
            Col::RateUp => "Rate-up",
            Col::Banner => "Banner",
            Col::Roll => "#",
            Col::Time => "Time",
        }
    }
}

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
            page: 0,
            page_size: PAGE_SIZES[0],
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
            self.page = 0;
            self.sort = None;
        }
        self.images = images;
        self.banner_labels = data
            .banners
            .for_pool(pool)
            .iter()
            .map(|banner| match &banner.version {
                Some(version) if !banner.short_name.is_empty() => {
                    format!("{} · {version}", banner.short_name).into()
                }
                _ => banner.name.clone().into(),
            })
            .collect();
        self.rolls = analysis
            .pool(pool)
            .map(|pool| pool.banner_rolls())
            .unwrap_or_default();
        self.cols = [
            Col::Item,
            Col::Kind,
            Col::Pity,
            Col::RateUp,
            Col::Banner,
            Col::Roll,
            Col::Time,
        ]
        .into_iter()
        .filter(|col| *col != Col::RateUp || pool.has_rate_up())
        .collect();
        self.analysis = Some(analysis);
        self.pool = Some(pool);
        self.filter = filter;
        self.arrange();
    }

    pub(super) fn set_filter(&mut self, filter: [bool; 3]) {
        self.filter = filter;
        self.page = 0;
        self.arrange();
    }

    /// Filters and sorts the rows, and keeps the page in range.
    fn arrange(&mut self) {
        let pulls = self.pulls();
        let mut rows: Vec<usize> = (0..pulls.len())
            .filter(|&ix| self.filter[rarity_ix(pulls[ix].wish.rarity)])
            .collect();
        match self.sort {
            Some((col, sort @ (ColumnSort::Ascending | ColumnSort::Descending))) => {
                // Equal keys keep wish order, so a 10-pull stays in sequence.
                rows.sort_by(|&a, &b| self.compare(col, a, b).then(a.cmp(&b)));
                if sort == ColumnSort::Descending {
                    rows.reverse();
                }
            }
            _ => rows.reverse(),
        }
        self.rows = rows;
        self.page = self.page.min(self.pages().saturating_sub(1));
    }

    fn compare(&self, col: Col, a: usize, b: usize) -> Ordering {
        let pulls = self.pulls();
        let roll = |ix: usize| self.rolls.get(ix).copied().flatten();
        let (roll_a, roll_b) = (roll(a), roll(b));
        let (a, b) = (&pulls[a], &pulls[b]);
        let outcome = |pull: &Pull| {
            pull.rate_up.map(|r| match r.outcome() {
                RateUpOutcome::Won => 0,
                RateUpOutcome::Guaranteed => 1,
                RateUpOutcome::Lost => 2,
                RateUpOutcome::LostWhileGuaranteed => 3,
            })
        };
        match col {
            Col::Item => a.wish.name.cmp(&b.wish.name),
            Col::Kind => (a.wish.rarity, kind(a.wish.item_type))
                .cmp(&(b.wish.rarity, kind(b.wish.item_type))),
            Col::Pity => a.pity.cmp(&b.pity),
            Col::RateUp => outcome(a).cmp(&outcome(b)),
            Col::Banner => a.banner.cmp(&b.banner),
            Col::Roll => roll_a.cmp(&roll_b),
            // Wish order is time order, with ties broken by sequence.
            Col::Time => Ordering::Equal,
        }
    }

    /// How many pages the rows fill (at least one, so an empty table has a page).
    pub(super) fn pages(&self) -> usize {
        self.rows.len().div_ceil(self.page_size).max(1)
    }

    pub(super) fn page(&self) -> usize {
        self.page
    }

    pub(super) fn page_size(&self) -> usize {
        self.page_size
    }

    /// All the rows that pass the filter, on every page.
    pub(super) fn row_total(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn set_page(&mut self, page: usize) {
        self.page = page.min(self.pages().saturating_sub(1));
    }

    /// Changes the page size, keeping the first row shown on screen.
    pub(super) fn set_page_size(&mut self, page_size: usize) {
        let first = self.page * self.page_size;
        self.page_size = page_size;
        self.page = first / page_size;
    }

    /// Whether the rows were sorted since the last call: the selected row
    /// index then points at another wish.
    pub(super) fn take_resorted(&mut self) -> bool {
        std::mem::take(&mut self.resorted)
    }

    /// The rows of the current page.
    fn visible(&self) -> &[usize] {
        let start = (self.page * self.page_size).min(self.rows.len());
        let end = (start + self.page_size).min(self.rows.len());
        &self.rows[start..end]
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
        match self.col(col_ix) {
            Col::Time => pull.wish.time.naive().format("%Y-%m-%d %H:%M").to_string(),
            Col::Item => pull.wish.name.clone(),
            Col::Kind => match kind(pull.wish.item_type) {
                "" => format!("{}★", pull.wish.rarity.stars()),
                kind => format!("{}★ {kind}", pull.wish.rarity.stars()),
            },
            Col::Pity => pull.pity.map(|p| p.to_string()).unwrap_or_default(),
            Col::RateUp => pull
                .rate_up
                .map(|r| format::outcome(r.outcome()).to_owned())
                .unwrap_or_default(),
            Col::Banner => pull
                .banner
                .and_then(|b| self.banner_labels.get(b))
                .map(ToString::to_string)
                .unwrap_or_default(),
            Col::Roll => self
                .visible()
                .get(row_ix)
                .and_then(|ix| self.rolls.get(*ix).copied().flatten())
                .map(|roll| roll.to_string())
                .unwrap_or_default(),
        }
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
        let column = column.width(width);
        match col {
            // The first column starts its text on the card's inset, like the
            // cards' content elsewhere.
            Col::Item => {
                let mut paddings = Size::Large.table_cell_padding();
                paddings.left = self.rem * PAGE_INSET_REMS;
                column.paddings(paddings)
            }
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
        self.page = 0;
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
        match self.col(col_ix) {
            // The icon has its own slot so names stay aligned without one.
            Col::Item => div()
                .h_flex()
                .gap_2()
                .child(item_tile(
                    pull.wish.rarity,
                    self.images.item(&pull.wish),
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
                        .child(kind(pull.wish.item_type)),
                ),
            Col::RateUp => match pull.rate_up.map(|r| r.outcome()) {
                Some(outcome) => div()
                    .h_full()
                    .h_flex()
                    .items_center()
                    .child(outcome_badge(outcome, cx)),
                None => div(),
            },
            Col::Banner | Col::Roll | Col::Time => div().truncate().text_color(muted).child(text),
            Col::Pity => div().child(text),
        }
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

/// "Character" or "Weapon"; nothing for a 3★ counted by hand, whose item is
/// not known.
fn kind(item_type: ItemType) -> &'static str {
    match item_type {
        ItemType::Character => "Character",
        ItemType::Weapon => "Weapon",
        ItemType::Unknown => "",
    }
}
