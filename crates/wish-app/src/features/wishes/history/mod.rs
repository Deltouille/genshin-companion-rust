//! History of one banner type: every wish newest first, or banner by banner.

mod banners;
mod charts;
mod manual_counting;
mod table;

use std::sync::Arc;

use gpui_kit::base::{ElementExt as _, StyledExt as _};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{DataTable, TableEvent, TableState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, rems,
};
use wish_core::data::GameData;
use wish_core::engine::{Analysis, PoolAnalysis};
use wish_core::model::{Pool, Rarity, Source, Wish};

use crate::app::images::Images;
use crate::app::{Library, Session};
use banners::{BANNER_CARD_REMS, BANNER_COLUMNS, banner_card, five_star_history};
use table::{PAGE_SIZES, PullTable};
use wish_ui::format;
use wish_ui::pool_icon;
use wish_ui::{ButtonExt as _, PageFrame, Pager, Segmented, button, card, filter_chip};

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
        let manual_counting = library.settings().manual_counting;
        let images = library.images().clone();
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

    /// `+1`, `+10`, add a 4★ or 5★, and delete the selected hand-entered
    /// wish, carried out in `manual_counting.rs`.
    fn render_manual_controls(&self, pool: Pool, cx: &mut Context<Self>) -> impl IntoElement {
        let add = |count: u32, cx: &mut Context<Self>| {
            button(SharedString::from(format!("manual-add-{count}")))
                .label(format!("+{count}"))
                .tooltip(format!(
                    "Add {} 3★ {} made now",
                    format::count(count),
                    format::plural(count, "wish", "wishes")
                ))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.on_manual(&ManualAction::AddThreeStars(pool, count), window, cx)
                }))
        };
        let selected = self.selected_manual.clone();
        div()
            .h_flex()
            .gap_2()
            .ml_auto()
            .child(add(1, cx))
            .child(add(10, cx))
            .child(
                button("manual-add-item")
                    .icon(IconName::Plus)
                    .label("Add 4★ or 5★…")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.on_manual(&ManualAction::AddItem(pool), window, cx)
                    })),
            )
            .when(self.mode == Mode::Wishes, |this| {
                this.child(
                    button("manual-delete")
                        .ghost()
                        .label("Delete…")
                        .tooltip("Delete the selected wish you entered by hand")
                        .unavailable(selected.is_none())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(wish) = selected.clone() {
                                this.on_manual(&ManualAction::Delete(wish), window, cx);
                            }
                        })),
                )
            })
    }

    /// Sizes the table's columns to its width on screen.
    fn set_table_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.table.update(cx, |table, cx| {
            if table.delegate_mut().set_available_width(width) {
                table.refresh(cx);
            }
        });
    }

    /// Shows page `page` (from 0) of the wish table.
    fn set_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.selected_manual = None;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_page(page);
            table.clear_selection(cx);
            table.scroll_to_row(0, cx);
            table.refresh(cx);
        });
        cx.notify();
    }

    fn set_page_size(&mut self, page_size: usize, cx: &mut Context<Self>) {
        self.selected_manual = None;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_page_size(page_size);
            table.clear_selection(cx);
            table.refresh(cx);
        });
        cx.notify();
    }

    /// Under the table: the page size, which rows are shown, and the pages.
    fn render_pager(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let table = self.table.read(cx).delegate();
        let (page, pages, size, total) = (
            table.page(),
            table.pages(),
            table.page_size(),
            table.row_total(),
        );
        let shown = if total == 0 {
            "No wishes".to_owned()
        } else {
            format!(
                "{}–{} of {}",
                format::count((page * size + 1) as u64),
                format::count(((page + 1) * size).min(total) as u64),
                format::count(total as u64)
            )
        };
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

    fn toggle(&mut self, rarity: Rarity, shown: bool, cx: &mut Context<Self>) {
        self.rarities[rarity_ix(rarity)] = shown;
        let filter = self.rarities;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_filter(filter);
            table.refresh(cx);
        });
        cx.notify();
    }

    /// The rarity donut, the wishes per banner and the pity of each 5★, side
    /// by side above the table.
    fn render_charts(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let shown = self.shown.as_ref()?;
        let pool = shown.analysis.pool(shown.pool)?;
        if pool.pulls.is_empty() {
            return None;
        }
        Some(
            div()
                .h_flex()
                .items_stretch()
                .gap_4()
                .flex_none()
                .h(rems(CHARTS_REMS))
                .child(div().flex_none().child(charts::rarity_donut(pool, cx)))
                // Wishes per banner, when there were several (the Standard banner
                // never changes), and the pity of each 5★, when there is one.
                .children(
                    charts::banner_chart(pool, &shown.data, cx)
                        .map(|chart| div().flex_1().min_w_0().child(chart)),
                )
                .when(pool.stats.five.count > 0, |this| {
                    this.child(div().flex_1().min_w_0().child(charts::five_star_pity(
                        pool,
                        &shown.data,
                        cx,
                    )))
                }),
        )
    }

    fn render_banners(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(shown) = &self.shown else {
            return div().into_any_element();
        };
        let Some(pool) = shown.analysis.pool(shown.pool) else {
            return div().into_any_element();
        };
        let banners = shown.data.banners.for_pool(shown.pool);
        // As many cards a row as fit, a rem apart, as in the planner.
        let content = crate::shell::content_width(window);
        let room = content / window.rem_size();
        let columns =
            (((room + 1.) / (BANNER_CARD_REMS + 1.)).floor() as u16).clamp(1, BANNER_COLUMNS);
        let gaps = window.rem_size() * (f32::from(columns) - 1.);
        let card_width = (content - gaps) / f32::from(columns);
        let cards: Vec<AnyElement> = pool
            .banners
            .iter()
            .rev()
            .map(|range| {
                banner_card(
                    &banners[range.banner],
                    &pool.pulls[range.pulls.clone()],
                    &shown.images,
                    card_width,
                    window,
                    cx,
                )
            })
            .collect();
        div()
            .id("banner-list")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_6()
                    .py_5()
                    .children(five_star_history(pool, &shown.images, cx))
                    .child(div().grid().grid_cols(columns).gap_4().children(cards)),
            )
            .into_any_element()
    }
}

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .shown
            .as_ref()
            .map_or("History", |s| s.pool.display_name());
        let mode = self.mode;
        let switch = Segmented::new("history-mode")
            .option("Wishes")
            .option("Banners")
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
                    .tooltip(format!(
                        "{} the {}★ wishes",
                        if on { "Hide" } else { "Show" },
                        rarity.stars()
                    ))
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
                                    div().flex_1().min_h_0().child(
                                        DataTable::new(&self.table)
                                            .large()
                                            .bordered(false)
                                            .accessibility_label(format!("{title} wishes")),
                                    ),
                                )
                                .child(self.render_pager(cx)),
                        ),
                )
                .into_any_element(),
            Mode::Banners => self.render_banners(window, cx),
        };

        let mut frame = PageFrame::new(title, body);
        if let Some(shown) = &self.shown {
            frame = frame.icon(Icon::new(pool_icon(shown.pool)));
        }
        frame
            .subtitle(self.subtitle.clone())
            .toolbar(toolbar)
            .body_scrolls()
    }
}

fn rarity_ix(rarity: Rarity) -> usize {
    match rarity {
        Rarity::Five => 0,
        Rarity::Four => 1,
        Rarity::Three => 2,
    }
}

fn subtitle(pool: &PoolAnalysis) -> String {
    let counters = pool.counters;
    let mut parts = vec![
        format!(
            "{} {}",
            format::count(counters.total),
            format::plural(counters.total, "wish", "wishes")
        ),
        format!(
            "5★ pity {} / {}",
            counters.pity5,
            pool.pool.hard_pity_five()
        ),
        format!(
            "4★ pity {} / {}",
            counters.pity4,
            pool.pool.hard_pity_four()
        ),
    ];
    if let Some(guarantee) = pool.guarantee {
        parts.push(format!(
            "next 5★: {}",
            format::next_rate_up(pool.pool, guarantee.five)
        ));
    }
    parts.join(" · ")
}
