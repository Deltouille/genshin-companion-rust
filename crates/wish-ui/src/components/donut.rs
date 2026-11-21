//! A donut chart on its card: the slices round a ring, their total in its
//! hole, and beside it each slice's count and share, so the numbers carry a
//! slice too thin to see.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::chart::PieChart;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, Hsla, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, rems,
};

use super::{card, card_title};
use crate::format;

/// One slice of a [`Donut`].
#[derive(Clone, Debug)]
pub struct DonutSlice {
    /// What it counts: "5★", "C2", "80–89".
    pub label: SharedString,
    /// The label's colour, when it carries one (a rarity's); else the text's.
    pub label_color: Option<Hsla>,
    pub count: u64,
    /// The slice's and its swatch's colour.
    pub color: Hsla,
}

/// A donut on a card as high as its parent: `title`, the ring with the total
/// and `caption` ("characters") in its hole, the legend beside it.
#[derive(IntoElement)]
pub struct Donut {
    id: ElementId,
    title: SharedString,
    slices: Vec<DonutSlice>,
    caption: SharedString,
    label_rems: f32,
    footer: Option<AnyElement>,
}

impl Donut {
    /// `id` tells the donuts of a page apart (each keeps its hover).
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        slices: Vec<DonutSlice>,
        caption: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            slices,
            caption: caption.into(),
            label_rems: 2.,
            footer: None,
        }
    }

    /// The width of the legend's label column, in rems: room for the
    /// longest label, so the counts line up (2 by default, for "5★").
    pub fn label_rems(mut self, rems: f32) -> Self {
        self.label_rems = rems;
        self
    }

    /// A line under the ring and its legend.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

impl RenderOnce for Donut {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let total: u64 = self.slices.iter().map(|slice| slice.count).sum();
        let share = move |count: u64| {
            if total == 0 {
                0.0
            } else {
                count as f64 / total as f64
            }
        };

        // A small gap between the slices keeps them apart.
        let chart = PieChart::new(self.slices.clone())
            .id(self.id)
            .appear(false)
            .inner_radius_ratio(0.68)
            .pad_angle(0.02)
            .value(|slice: &DonutSlice| slice.count as f32)
            .color(|slice: &DonutSlice| slice.color)
            .tooltip_name(|slice: &DonutSlice| slice.label.clone())
            .tooltip_value(move |slice: &DonutSlice, _, _| {
                format!(
                    "{} ({})",
                    format::count(slice.count),
                    format::share(share(slice.count))
                )
                .into()
            });

        let legend = self.slices.iter().map(|slice| {
            div()
                .h_flex()
                .gap_2()
                .child(div().flex_none().size_2p5().rounded_sm().bg(slice.color))
                .child(
                    div()
                        .w(rems(self.label_rems))
                        .whitespace_nowrap()
                        .font_weight(FontWeight::SEMIBOLD)
                        .when_some(slice.label_color, |this, color| this.text_color(color))
                        .child(slice.label.clone()),
                )
                .child(
                    div()
                        .w_12()
                        .text_right()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format::count(slice.count)),
                )
                .child(
                    div()
                        .w_16()
                        .text_right()
                        .text_color(theme.muted_foreground)
                        .child(format::share(share(slice.count))),
                )
        });

        card(cx)
            .h_full()
            .p_4()
            .gap_2()
            .child(card_title(self.title, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .h_flex()
                    .gap_4()
                    .child(
                        // A square as high as the room under the title, so the
                        // ring (80% of it) never leaves the card.
                        div()
                            .relative()
                            .h_full()
                            .aspect_ratio(1.)
                            .flex_none()
                            .child(chart)
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .v_flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BLACK)
                                            .child(format::count(total)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(self.caption),
                                    ),
                            ),
                    )
                    .items_center()
                    .child(div().v_flex().gap_1p5().text_sm().children(legend)),
            )
            .children(self.footer.map(|footer| {
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(footer)
            }))
    }
}
