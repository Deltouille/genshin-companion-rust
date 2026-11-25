//! The chronicle: what HoYoLAB shows of the account's play for a while
//! only, kept here for good, a page each: the Spiral Abyss, the Imaginarium
//! Theater and the Traveler's Diary. Each page picks a period (a month for
//! the Diary) and shows it, with a chart over every period kept.
//!
//! An entity: the period picked on each page is its own state. What it shows
//! is `model.rs`'s, worked out from the [`Library`]'s chronicle of the
//! account the [`Session`] has selected. A sync from HoYoLAB fills it (the
//! shell's `SyncFromHoyolab`).

mod abyss;
mod diary;
mod model;
mod theater;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, rems,
};
use wish_ui::{
    PageFrame, PickerEvent, PickerItem, PickerState, card, card_title, item_tile, picker, theme,
};

use wish_core::planner::PlannerData;

use crate::app::images::Images;
use crate::app::{Library, Session};
use crate::features::imports::hoyolab_sync_button;
use crate::shell::ChroniclePage;
use model::{Member, Point};

/// A team member's tile, in rems.
const MEMBER_REMS: f32 = 3.;

/// The chronicle's three pages, each with the period it shows.
pub struct ChronicleView {
    session: Entity<Session>,
    library: Entity<Library>,
    page: ChroniclePage,
    /// The period picked on each page, by its place among those kept
    /// (newest first).
    abyss: Entity<PickerState<usize>>,
    theater: Entity<PickerState<usize>>,
    diary: Entity<PickerState<usize>>,
    _subscriptions: Vec<Subscription>,
}

impl ChronicleView {
    pub fn new(session: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let new_picker =
            |cx: &mut Context<Self>| cx.new(|cx| PickerState::new(Vec::new(), None, cx));
        let (abyss, theater, diary) = (new_picker(cx), new_picker(cx), new_picker(cx));
        let mut subscriptions = vec![
            // New periods kept, or another language: the pickers list them
            // again.
            cx.observe_in(&library, window, |this, _, _, cx| {
                this.list_periods(false, cx)
            }),
            // Another account: its newest periods.
            cx.observe_in(&session, window, |this, _, _, cx| {
                this.list_periods(true, cx)
            }),
        ];
        for picker in [&abyss, &theater, &diary] {
            subscriptions
                .push(cx.subscribe(picker, |_, _, _: &PickerEvent<usize>, cx| cx.notify()));
        }
        let mut view = Self {
            session,
            library,
            page: ChroniclePage::Abyss,
            abyss,
            theater,
            diary,
            _subscriptions: subscriptions,
        };
        view.list_periods(true, cx);
        view
    }

    /// Shows `page`.
    pub fn set_page(&mut self, page: ChroniclePage, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    /// Lists each page's periods in its picker; `newest` picks the newest
    /// one, else the one picked stays (by its place) while it exists.
    fn list_periods(&mut self, newest: bool, cx: &mut Context<Self>) {
        let account = self.session.read(cx).account();
        let chronicle = self.library.read(cx).chronicle(account).clone();
        let lists = [
            (&self.abyss, model::abyss_periods(&chronicle.abyss)),
            (&self.theater, model::theater_periods(&chronicle.theater)),
            (&self.diary, model::diary_months(&chronicle.diary)),
        ];
        for (picker, labels) in lists {
            picker.update(cx, |picker, cx| {
                let count = labels.len();
                let kept = picker.selected_value().copied().filter(|ix| *ix < count);
                let items = (labels.into_iter().enumerate())
                    .map(|(ix, label)| PickerItem::new(label, ix))
                    .collect();
                picker.set_items(items, cx);
                let selected = if newest { None } else { kept };
                picker.set_selected_index(selected.or((count > 0).then_some(0)), cx);
            });
        }
        cx.notify();
    }

    /// The period picked on a page.
    fn picked(picker: &Entity<PickerState<usize>>, cx: &App) -> usize {
        picker.read(cx).selected_value().copied().unwrap_or(0)
    }
}

impl Render for ChronicleView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let account = self.session.read(cx).account();
        let library = self.library.read(cx);
        let chronicle = library.chronicle(account).clone();
        let planner = library.planner().clone();
        let images = library.images(cx).clone();
        let room = crate::shell::content_width(window) / window.rem_size();
        let (title, picker_state, empty, body) = match self.page {
            ChroniclePage::Abyss => {
                let ix = Self::picked(&self.abyss, cx);
                (
                    t!("chronicle-abyss"),
                    &self.abyss,
                    t!("chronicle-empty-abyss"),
                    (chronicle.abyss.get(ix)).map(|record| {
                        abyss::page(&chronicle.abyss, record, &planner, &images, room, cx)
                    }),
                )
            }
            ChroniclePage::Theater => {
                let ix = Self::picked(&self.theater, cx);
                (
                    t!("chronicle-theater"),
                    &self.theater,
                    t!("chronicle-empty-theater"),
                    (chronicle.theater.get(ix)).map(|record| {
                        theater::page(&chronicle.theater, record, &planner, &images, room, cx)
                    }),
                )
            }
            ChroniclePage::Diary => {
                let ix = Self::picked(&self.diary, cx);
                (
                    t!("chronicle-diary"),
                    &self.diary,
                    t!("chronicle-empty-diary"),
                    (chronicle.diary.get(ix))
                        .map(|record| diary::page(&chronicle.diary, record, room, cx)),
                )
            }
        };
        let label = match self.page {
            ChroniclePage::Diary => t!("chronicle-month"),
            _ => t!("chronicle-period"),
        };
        let has_periods = body.is_some();
        let unreadable = chronicle.unreadable;
        let body = match body {
            Some(body) => body,
            None => empty_state(empty, cx),
        };
        let body = div()
            .v_flex()
            .gap_4()
            .child(body)
            .when(unreadable > 0, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("chronicle-unreadable", count = unreadable)),
                )
            });
        PageFrame::new(title, body).when(has_periods, |frame| {
            frame
                .action(
                    div().w(rems(16.)).child(
                        picker("chronicle-period", picker_state)
                            .accessibility_label(label)
                            .menu_width(rems(16.)),
                    ),
                )
                .action(hoyolab_sync_button())
        })
    }
}

/// What a page says before anything is kept: what HoYoLAB forgets, and the
/// way to keep it.
fn empty_state(title: String, cx: &App) -> AnyElement {
    div()
        .w_full()
        .h_flex()
        .justify_center()
        .py_16()
        .child(
            div()
                .v_flex()
                .items_center()
                .gap_4()
                .max_w(rems(30.))
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("chronicle-empty-help")),
                )
                .child(hoyolab_sync_button().primary()),
        )
        .into_any_element()
}

/// A card with a title over `content`.
fn section(title: impl Into<SharedString>, cx: &App) -> Div {
    card(cx).p_5().gap_4().child(card_title(title, cx))
}

/// A row of figures, each a small label over a large value and a muted
/// detail, as many a row as `room` holds.
fn figures(figures: Vec<(String, String, String)>, room: f32, cx: &App) -> Div {
    let columns = ((room + 0.75) / (11. + 0.75))
        .floor()
        .clamp(1., figures.len() as f32) as u16;
    let theme = cx.theme();
    div()
        .grid()
        .grid_cols(columns)
        .gap_3()
        .children(figures.into_iter().map(|(label, value, detail)| {
            card(cx)
                .px_4()
                .py_3()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(div().text_2xl().font_weight(FontWeight::BOLD).child(value))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(detail),
                )
        }))
}

/// A character's tile, its level under it and its name in a tooltip.
fn member_tile(
    id: impl Into<SharedString>,
    member: &Member,
    planner: &PlannerData,
    images: &Images,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let icon = (member.key.as_deref())
        .and_then(|key| planner.character(key))
        .and_then(|character| images.character(character));
    let name: SharedString = match &member.tag {
        Some(tag) => format!("{} ({tag})", member.name).into(),
        None => member.name.clone().into(),
    };
    let caption = match (&member.tag, member.level) {
        (Some(tag), _) => tag.clone(),
        (None, Some(level)) => t!("chronicle-level", level = level),
        (None, None) => String::new(),
    };
    div()
        .id(id.into())
        .v_flex()
        .items_center()
        .gap_0p5()
        .child(item_tile(member.rarity, icon, rems(MEMBER_REMS), cx))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(caption),
        )
        .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
        .into_any_element()
}

/// A chart of one figure over the periods kept, a dot each, or the one
/// period in words; `top` is the axis's top, when the figure has one.
fn trend(
    id: &'static str,
    title: String,
    points: Vec<Point>,
    top: Option<u64>,
    color: Hsla,
    cx: &App,
) -> AnyElement {
    let frame = (card(cx).h_full().min_h(rems(16.)))
        .gap_3()
        .p_4()
        .child(card_title(title, cx));
    if points.len() < 2 {
        return frame
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("chronicle-one-period")),
            )
            .into_any_element();
    }
    let most = points.iter().map(|p| p.value).max().unwrap_or(0);
    let top = top.unwrap_or(most).max(1);
    let (top, ticks) = wish_ui::axis::round_axis(u32::try_from(top).unwrap_or(u32::MAX), 5);
    let x_ticks = points.len().min(6);
    let chart = LineChart::new(points)
        .appear(false)
        .linear()
        .dot()
        .x(|p: &Point| SharedString::from(p.tick.clone()))
        .x_tick_count(x_ticks)
        .y(|p: &Point| p.value as f64)
        .stroke(color)
        .y_axis(true)
        .y_domain(0.0, f64::from(top))
        .y_padding(0.0, 0.0)
        .y_tick_count(ticks)
        .y_tick_format(|v| format!("{v:.0}"))
        .name(id)
        .tooltip_title(|p: &Point| p.title.clone().into())
        .tooltip_value(|p: &Point, _| wish_ui::format::count(p.value).into());
    frame
        .child(div().flex_1().min_h_0().child(chart))
        .into_any_element()
}

/// A star rating, "★★☆", in the 5★ colour.
fn stars(stars: u8, max: u8, cx: &App) -> Div {
    let gold = theme::rarity(wish_core::model::Rarity::Five, cx);
    let muted = cx.theme().muted_foreground.opacity(0.5);
    div().h_flex().children((0..max).map(|n| {
        div()
            .text_color(if n < stars { gold } else { muted })
            .child("★")
    }))
}
