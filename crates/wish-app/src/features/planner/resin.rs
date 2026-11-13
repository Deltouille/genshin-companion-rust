//! The resin estimate, after genshin-center's: how much Original Resin, and
//! how many days of it, farming what is still lacking takes on average
//! (`wish_core::planner::resin`). One line on each card, its runs by source
//! in a tooltip; the whole plan's under "Still needed", by source; and the
//! dialog that sets how it counts (world level, resin a day, the sources).

use std::collections::BTreeSet;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::InputState;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Rems, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, img, prelude::FluentBuilder as _, rems,
};
use wish_core::planner::{MAX_WORLD_LEVEL, ResinEstimate, ResinOptions, ResinSource};

use super::PlannerView;
use crate::app::images::Images;
use crate::app::notices::{Notice, notify};
use wish_ui::{Segmented, button, dialog_footer, filter_chip, format, input};

/// The most resin a day the options take: the game's hard cap on what can
/// be held.
const MAX_RESIN_PER_DAY: u32 = 2_000;

/// Original Resin's icon `size` square, or the room for it until it is
/// downloaded.
fn resin_icon(images: &Images, size: Rems) -> AnyElement {
    match images.resin() {
        Some(icon) => img(icon).size(size).flex_shrink_0().into_any_element(),
        None => div().size(size).flex_shrink_0().into_any_element(),
    }
}

/// "6.9 days".
fn days(estimate: &ResinEstimate, options: &ResinOptions) -> String {
    let days = estimate.days(options.resin_per_day);
    let unit = if days > 0. && days <= 1. {
        "day"
    } else {
        "days"
    };
    format!("{days:.1} {unit}")
}

/// "61 runs".
fn runs(runs: u64) -> String {
    format!(
        "{} {}",
        format::count(runs),
        format::plural(u32::try_from(runs).unwrap_or(u32::MAX), "run", "runs")
    )
}

/// One card's estimate: the resin and the days on one line, the runs by
/// source in its tooltip. Nothing when it lacks nothing farmed with resin.
pub(super) fn entry_resin(
    id: i64,
    estimate: &ResinEstimate,
    options: &ResinOptions,
    images: &Images,
    cx: &App,
) -> Option<AnyElement> {
    if estimate.is_empty() {
        return None;
    }
    let tip: Vec<String> = (estimate.sources.iter())
        .map(|source| {
            format!(
                "{}: {}, {} resin",
                source.source.label(),
                runs(source.runs),
                format::count(source.resin())
            )
        })
        .collect();
    let tip = SharedString::from(tip.join("\n"));
    Some(
        div()
            .id(SharedString::from(format!("resin-{id}")))
            .h_flex()
            .gap_1p5()
            .text_sm()
            .child(resin_icon(images, rems(1.5)))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("~{} resin", format::count(estimate.resin()))),
            )
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("· ~{}", days(estimate, options))),
            )
            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            .into_any_element(),
    )
}

/// The whole plan's estimate: the resin and the days, then each source's
/// runs and resin, and the way to the options.
pub(super) fn plan_resin(
    estimate: &ResinEstimate,
    options: &ResinOptions,
    images: &Images,
    cx: &Context<PlannerView>,
) -> Div {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let options_button = button("resin-options")
        .icon(gpui_kit::assets::IconName::Settings)
        .label("Resin options…")
        .on_click(cx.listener(|this, _, window, cx| this.open_resin_options(window, cx)));
    let counted = ResinSource::ALL.iter().any(|&s| options.counts(s));
    let headline = if estimate.is_empty() {
        div().text_color(muted).child(if counted {
            "Nothing still lacking is farmed with resin."
        } else {
            "No source is counted: choose them in the options."
        })
    } else {
        div()
            .v_flex()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("~{} resin", format::count(estimate.resin()))),
            )
            .child(div().text_sm().text_color(muted).child(format!(
                "About {} at {} resin a day, world level {}",
                days(estimate, options),
                format::count(u64::from(options.resin_per_day)),
                options.world_level
            )))
    };
    // Where each is farmed, in its tooltip.
    let sources = estimate.sources.iter().map(|source| {
        let place = SharedString::from(source.source.place());
        div()
            .id(SharedString::from(format!(
                "resin-source-{:?}",
                source.source
            )))
            .v_flex()
            .gap_0p5()
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(source.source.label()),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(runs(source.runs)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("{} resin", format::count(source.resin()))),
            )
            .tooltip(move |window, cx| Tooltip::new(place.clone()).build(window, cx))
    });
    div()
        .v_flex()
        .gap_4()
        .pt_4()
        .border_t_1()
        .border_color(theme.border)
        .child(
            div()
                .h_flex()
                .gap_3()
                .child(resin_icon(images, rems(2.5)))
                .child(headline.flex_1().min_w_0())
                .child(options_button),
        )
        .when(!estimate.is_empty(), |this| {
            this.child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap_x_10()
                    .gap_y_3()
                    .children(sources),
            )
        })
}

/// The options dialog's fields.
pub(super) struct ResinForm {
    world_level: usize,
    per_day: Entity<InputState>,
    ignored: BTreeSet<ResinSource>,
    error: Option<SharedString>,
}

impl ResinForm {
    fn new(options: &ResinOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let per_day = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("180")
                .default_value(options.resin_per_day.to_string())
        });
        Self {
            world_level: usize::from(options.world_level.min(MAX_WORLD_LEVEL)),
            per_day,
            ignored: options.ignored.clone(),
            error: None,
        }
    }

    /// The options as entered, or `None` (and why, shown) when the resin a
    /// day doesn't read.
    fn submit(&mut self, cx: &mut Context<Self>) -> Option<ResinOptions> {
        let text = self.per_day.read(cx).value();
        let per_day = format::parse_number(&text)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| (1..=MAX_RESIN_PER_DAY).contains(n));
        let Some(resin_per_day) = per_day else {
            self.error = Some(
                format!(
                    "Resin a day is a whole number from 1 to {}.",
                    format::count(u64::from(MAX_RESIN_PER_DAY))
                )
                .into(),
            );
            cx.notify();
            return None;
        };
        Some(ResinOptions {
            world_level: self.world_level as u8,
            resin_per_day,
            ignored: self.ignored.clone(),
        })
    }
}

/// A field: its label, the control, a line of help under it.
fn field(label: &str, control: impl IntoElement, help: &str, cx: &App) -> Div {
    let muted = cx.theme().muted_foreground;
    div()
        .v_flex()
        .gap_2()
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(label.to_owned()),
        )
        .child(control)
        .child(div().text_sm().text_color(muted).child(help.to_owned()))
}

impl Render for ResinForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let form = cx.entity().downgrade();
        let levels = (0..=MAX_WORLD_LEVEL)
            .fold(Segmented::new("world-level"), |seg, level| {
                seg.option(level.to_string())
            })
            .selected(self.world_level)
            .on_select(move |ix: &usize, _, cx| {
                form.update(cx, |form, cx| {
                    form.world_level = *ix;
                    cx.notify();
                })
                .ok();
            });
        let chips = ResinSource::ALL.iter().map(|&source| {
            let on = !self.ignored.contains(&source);
            filter_chip(
                SharedString::from(format!("count-{source:?}")),
                source.label(),
                cx.theme().foreground,
                on,
                cx,
            )
            .tooltip(source.place())
            .on_click(cx.listener(move |form, _, _, cx| {
                if !form.ignored.remove(&source) {
                    form.ignored.insert(source);
                }
                cx.notify();
            }))
        });
        div()
            .v_flex()
            .gap_5()
            .child(field(
                "World level",
                div().h_flex().child(levels),
                "Bosses give more materials as it rises; ley lines give their most from 6.",
                cx,
            ))
            .child(field(
                "Resin a day",
                div().w(rems(10.)).child(input(&self.per_day, cx)),
                "180 regenerate in a day; add what you refill or take from Fragile Resin.",
                cx,
            ))
            .child(field(
                "Count",
                div().h_flex().flex_wrap().gap_2().children(chips),
                "Domains count at their highest level. Weekly bosses, gems, ores, enemy \
                 drops and local specialties take no resin counted here.",
                cx,
            ))
            .when_some(self.error.clone(), |this, error| {
                this.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
    }
}

impl PlannerView {
    /// Opens the options of the resin estimate.
    pub(crate) fn open_resin_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options = self.library.read(cx).settings().resin.clone();
        let form = cx.new(|cx| ResinForm::new(&options, window, cx));
        let library = self.library.downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let (form, library) = (form.clone(), library.clone());
            dialog
                .title("Resin estimate")
                .w(window.rem_size() * 36.)
                .child(form.clone())
                .footer(dialog_footer(button("save-resin").primary().label("Save")))
                .on_ok(move |_, window, cx| {
                    let Some(options) = form.update(cx, |form, cx| form.submit(cx)) else {
                        // Keep the dialog open on the error.
                        return false;
                    };
                    let Ok(task) =
                        library.update(cx, |library, cx| library.set_resin_options(options, cx))
                    else {
                        return true;
                    };
                    window
                        .spawn(cx, async move |cx| {
                            if let Err(err) = task.await {
                                cx.update(|_, cx| {
                                    notify(
                                        Notice::failed(format!(
                                            "The resin options were not saved: {err:#}"
                                        )),
                                        cx,
                                    )
                                })
                                .ok();
                            }
                        })
                        .detach();
                    true
                })
        });
    }
}
