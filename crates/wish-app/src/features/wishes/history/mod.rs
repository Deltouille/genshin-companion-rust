//! History of one banner type: every wish newest first, or banner by banner.

mod banners;
mod charts;
mod manual_counting;
mod model;
mod table;

use std::sync::Arc;

use gpui_kit::base::{ElementExt as _, StyledExt as _};
use gpui_kit::component::table::{DataTable, TableEvent, TableState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, Size};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};
use wish_core::data::GameData;
use wish_core::engine::Analysis;
use wish_core::model::{Pool, Rarity, Source, Wish};

use crate::app::images::Images;
use crate::app::{Library, Session};
use model::{rarity_ix, subtitle};
use table::PullTable;
use wish_ui::{PageFrame, Segmented, card, filter_chip};

/// The height of the charts above the table, in rem.
const CHARTS_REMS: f32 = 12.0;
/// The window height, in rem, below which the charts give their room to the
/// table (which then keeps about seven rows at 800 px).
const CHARTS_MIN_WINDOW_REMS: f32 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Every wish, newest first.
    Wishes,
    /// One entry per banner, with its art and numbers.
    Banners,
}

/// What the page shows, kept for the banner view.
struct Shown {
    analysis: Arc<Analysis>,
    pool: Pool,
    data: Arc<GameData>,
    images: Arc<Images>,
}

/// What the counting controls ask for, carried out in `manual_counting.rs`.
enum ManualAction {
    /// Add this many 3★ wishes without an item, made now.
    AddThreeStars(Pool, u32),
    /// Ask which 4★ or 5★ to add.
    AddItem(Pool),
    /// Delete a wish entered by hand.
    Delete(Wish),
}

/// The history page of one banner type: the wish table or the banner view,
/// and the controls to count wishes by hand.
///
/// An entity: it keeps its own state (the table, the view and filters, the
/// selected row) between renders. It holds the library and the session and
/// observes both, so it reloads itself when the data or the selected account
/// changes. The shell only tells it which banner type to show
/// ([`HistoryView::set_pool`]), since routing is the shell's job.
pub struct HistoryView {
    library: Entity<Library>,
    session: Entity<Session>,
    /// The banner type to show, chosen by the shell.
    pool: Option<Pool>,
    table: Entity<TableState<PullTable>>,
    shown: Option<Shown>,
    mode: Mode,
    /// Which rarities are listed: 5★, 4★, 3★.
    rarities: [bool; 3],
    subtitle: SharedString,
    /// Show the controls to count wishes by hand.
    manual_counting: bool,
    /// The selected row, when it is a wish entered by hand.
    selected_manual: Option<Wish>,
    _subscriptions: Vec<Subscription>,
}

impl HistoryView {
    pub fn new(session: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let delegate = PullTable::new(window.rem_size());
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .sortable(true)
                .col_movable(false)
        });
        let subscriptions = vec![
            cx.observe(&library, |this, _, cx| this.refresh(cx)),
            cx.observe(&session, |this, _, cx| this.refresh(cx)),
            // A sort moved the rows under the selection: clear it.
            cx.observe(&table, |this, table, cx| {
                if table.update(cx, |table, _| table.delegate_mut().take_resorted()) {
                    table.update(cx, |table, cx| table.clear_selection(cx));
                    this.selected_manual = None;
                }
            }),
            cx.subscribe(&table, |this, table, event: &TableEvent, cx| {
                if let TableEvent::SelectRow(row) = event {
                    this.selected_manual = table
                        .read(cx)
                        .delegate()
                        .pull(*row)
                        .filter(|pull| pull.wish.source == Source::Manual)
                        .map(|pull| pull.wish.clone());
                    cx.notify();
                }
            }),
        ];
        Self {
            library,
            session,
            pool: None,
            table,
            shown: None,
            mode: Mode::Wishes,
            rarities: [true, true, true],
            subtitle: SharedString::default(),
            manual_counting: false,
            selected_manual: None,
            _subscriptions: subscriptions,
        }
    }

    /// Shows `pool`, the banner type the shell routed to.
    pub fn set_pool(&mut self, pool: Pool, cx: &mut Context<Self>) {
        self.pool = Some(pool);
        self.refresh(cx);
    }

    /// Reloads what the page shows from the library and the session, after
    /// either of them changed or the banner type did.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(pool) = self.pool else {
            return;
        };
        let library = self.library.read(cx);
        let Some(data) = library.data().cloned() else {
            return;
        };
        let manual_counting = library.settings(cx).manual_counting;
        let images = library.images(cx).clone();
        let analysis = match self.session.read(cx).summary(cx) {
            Some(summary) => summary.analysis.clone(),
            // Counting by hand from scratch: an empty page with the controls.
            None if manual_counting => Arc::new(Analysis {
                pools: Vec::new(),
                monthly: Vec::new(),
            }),
            None => return,
        };
        self.set_manual_counting(manual_counting, cx);
        self.show(analysis, pool, data, images, cx);
    }

    /// Shows `pool` of `analysis`. Keeps the filter and view when only the
    /// data changed, resets them when the page now shows another pool.
    fn show(
        &mut self,
        analysis: Arc<Analysis>,
        pool: Pool,
        data: Arc<GameData>,
        images: Arc<Images>,
        cx: &mut Context<Self>,
    ) {
        if self.shown.as_ref().map(|s| s.pool) != Some(pool) {
            self.rarities = [true, true, true];
            self.mode = Mode::Wishes;
        }
        // Row indexes now point at other wishes.
        self.selected_manual = None;
        self.subtitle = analysis.pool(pool).map(subtitle).unwrap_or_default().into();
        let rarities = self.rarities;
        let (table_analysis, table_data, table_images) =
            (analysis.clone(), data.clone(), images.clone());
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_pool(
                table_analysis,
                pool,
                &table_data,
                table_images,
                rarities,
            );
            table.clear_selection(cx);
            table.refresh(cx);
        });
        self.shown = Some(Shown {
            analysis,
            pool,
            data,
            images,
        });
        cx.notify();
    }

    fn set_manual_counting(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.manual_counting != on {
            self.manual_counting = on;
            cx.notify();
        }
    }

    /// Switches between the wish list and the banner view.
    pub fn show_banners(&mut self, banners: bool, cx: &mut Context<Self>) {
        self.mode = if banners { Mode::Banners } else { Mode::Wishes };
        cx.notify();
    }

    fn toggle(&mut self, rarity: Rarity, shown: bool, cx: &mut Context<Self>) {
        self.rarities[rarity_ix(rarity)] = shown;
        let filter = self.rarities;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_filter(filter);
            table.refresh(cx);
        });
        cx.notify();
    }
}

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .shown
            .as_ref()
            .map_or_else(|| t!("history-title"), |s| s.pool.display_name());
        let mode = self.mode;
        let switch = Segmented::new("history-mode")
            .option(t!("history-mode-wishes"))
            .option(t!("history-mode-banners"))
            .selected(match mode {
                Mode::Wishes => 0,
                Mode::Banners => 1,
            })
            .on_select(cx.listener(|this, ix: &usize, _, cx| {
                this.mode = if *ix == 1 {
                    Mode::Banners
                } else {
                    Mode::Wishes
                };
                cx.notify();
            }));
        let theme = cx.theme();
        let filters =
            div()
                .h_flex()
                .gap_2()
                .children([Rarity::Five, Rarity::Four, Rarity::Three].map(|rarity| {
                    let on = self.rarities[rarity_ix(rarity)];
                    let color = match rarity {
                        Rarity::Three => theme.blue,
                        rarity => wish_ui::theme::rarity(rarity, cx),
                    };
                    filter_chip(
                        SharedString::from(format!("show-{}", rarity.stars())),
                        format!("{}★", rarity.stars()),
                        color,
                        on,
                        cx,
                    )
                    .tooltip(if on {
                        t!("history-hide-rarity", stars = rarity.stars())
                    } else {
                        t!("history-show-rarity", stars = rarity.stars())
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(rarity, !on, cx)))
                }));
        let manual = self
            .shown
            .as_ref()
            .filter(|_| self.manual_counting)
            .map(|shown| self.render_manual_controls(shown.pool, cx));
        let toolbar = div()
            .h_flex()
            .flex_wrap()
            .gap_4()
            .child(switch)
            .when(mode == Mode::Wishes, |this| this.child(filters))
            .children(manual);

        let body = match mode {
            // The charts, then the table filling the rest of the page. The table
            // scrolls on its own, so the page does not: on a window too short to
            // leave the table room, the charts step aside.
            Mode::Wishes => div()
                .size_full()
                .child(
                    div()
                        .v_flex()
                        .size_full()
                        .gap_4()
                        .px_6()
                        .pt_2()
                        .pb_6()
                        .when(
                            window.viewport_size().height
                                >= window.rem_size() * CHARTS_MIN_WINDOW_REMS,
                            |this| this.children(self.render_charts(cx)),
                        )
                        .child(
                            card(cx)
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                // The table's width decides its columns' (see `PullTable`);
                                // known once laid out, applied on the next frame.
                                .on_prepaint({
                                    let view = cx.entity().downgrade();
                                    move |bounds, _, cx| {
                                        cx.defer(move |cx| {
                                            view.update(cx, |this, cx| {
                                                this.set_table_width(bounds.size.width, cx)
                                            })
                                            .ok();
                                        })
                                    }
                                })
                                .child(
                                    // Taller rows than the kit's sizes, for
                                    // icons big enough to tell apart; the
                                    // header keeps the large height.
                                    div().flex_1().min_h_0().text_base().child(
                                        DataTable::new(&self.table)
                                            .with_size(Size::Size(
                                                window.rem_size() * table::ROW_REMS,
                                            ))
                                            .header_row_height(
                                                window.rem_size() * table::HEADER_REMS,
                                            )
                                            .bordered(false)
                                            .accessibility_label(t!(
                                                "history-table-label",
                                                title = title.as_str()
                                            )),
                                    ),
                                )
                                .child(self.render_pager(cx)),
                        ),
                )
                .into_any_element(),
            Mode::Banners => self.render_banners(window, cx),
        };

        PageFrame::new(title, body)
            .subtitle(self.subtitle.clone())
            .toolbar(toolbar)
            .body_scrolls()
    }
}
