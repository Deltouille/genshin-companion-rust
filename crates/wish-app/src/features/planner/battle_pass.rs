//! The Battle Pass drawer: the level the account reached, the Gnostic Hymn,
//! the reward plan picked in the game, and whether "Still needed" counts the
//! rewards still to come; what those are, the series picked for each kind of
//! box (or what the plan lacks) and what they become, when the period ends
//! (the level starts again at 0), and a reset.
//!
//! The pass is the [`Library`](crate::app::Library)'s, one per account; each
//! change is saved at once (the level when the slider is released).

use chrono::{DateTime, FixedOffset, Local};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight, ImageSource,
    IntoElement, ParentElement as _, SharedString, Styled as _, Subscription, Window, div, img,
    rems,
};
use wish_core::planner::{
    BATTLE_PASS_LEVELS, BattlePass, BattlePassData, BoxKind, MaterialId, PlannerData, RewardPlan,
};
use wish_ui::{
    ButtonExt as _, Drawer, DrawerState, PickerEvent, PickerItem, PickerState, Segmented, button,
    drawer_slide, format, picker,
};

use super::{PlannerView, report_plan};

/// The drawer's width, in rems.
const WIDTH: f32 = 28.;
/// Its pickers' lists: as wide as the pickers, the drawer less its margins.
const MENU_WIDTH: f32 = WIDTH - 2.5;

/// The drawer's own state in the view.
pub(super) struct PassControls {
    pub drawer: DrawerState,
    pub focus: FocusHandle,
    /// The level, 0 to 50.
    pub level: Entity<SliderState>,
    pub plan: Entity<PickerState<RewardPlan>>,
    /// The series picked for each kind of box ([`BoxKind::ALL`]'s order);
    /// `None`, what the plan lacks.
    pub boxes: Vec<(BoxKind, Entity<PickerState<Option<MaterialId>>>)>,
    _subscriptions: Vec<Subscription>,
}

/// Now, with the offset of the computer's clock: the period's start is a
/// fixed instant, whatever the clock.
pub(super) fn now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}

/// The reward plans, as the game lists the three types each gives.
fn plan_items() -> Vec<PickerItem<RewardPlan>> {
    RewardPlan::ALL
        .iter()
        .map(|plan| {
            let title = match plan {
                RewardPlan::NoArtifactExp => t!("planner-pass-plan-no-artifact-exp"),
                RewardPlan::NoWeaponExp => t!("planner-pass-plan-no-weapon-exp"),
                RewardPlan::NoCharacterExp => t!("planner-pass-plan-no-character-exp"),
                RewardPlan::NoMora => t!("planner-pass-plan-no-mora"),
            };
            PickerItem::new(title, *plan)
        })
        .collect()
}

/// What a box of `kind` can give: what the plan lacks first, then each
/// series in the game's order.
fn box_items(planner: &PlannerData, kind: BoxKind) -> Vec<PickerItem<Option<MaterialId>>> {
    let choices = (planner.box_choices(kind).into_iter()).filter_map(|id| {
        let material = planner.material(id)?;
        Some(PickerItem::new(material.name.to_string(), Some(id)))
    });
    std::iter::once(PickerItem::new(t!("planner-pass-pick-auto"), None))
        .chain(choices)
        .collect()
}

/// A kind of box: "Guides".
fn box_name(kind: BoxKind) -> String {
    match kind {
        BoxKind::Guides => t!("planner-pass-box-guides"),
        BoxKind::Philosophies => t!("planner-pass-box-philosophies"),
        BoxKind::Specialties => t!("planner-pass-box-specialties"),
    }
}

impl PlannerView {
    /// The drawer's controls, closed.
    pub(super) fn pass_controls(window: &mut Window, cx: &mut Context<Self>) -> PassControls {
        let level = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(f32::from(BATTLE_PASS_LEVELS))
                .step(1.0)
                .default_value(0.0)
        });
        let plan = cx.new(|cx| PickerState::new(plan_items(), Some(0), cx));
        let mut subscriptions = vec![
            // The rewards follow the slider while it is dragged; released, it
            // saves.
            cx.subscribe_in(&level, window, |this, _, event: &SliderEvent, _, cx| {
                if let SliderEvent::Release(_) = event {
                    let level = this.pass_level(cx);
                    let now = now();
                    this.change_pass(
                        move |pass| pass.at_level(level, BattlePassData::embedded(), now),
                        cx,
                    );
                }
                this.rebuild(cx);
                cx.notify();
            }),
            cx.subscribe(&plan, |this, _, event: &PickerEvent<RewardPlan>, cx| {
                let PickerEvent::Confirm(plan) = event;
                let plan = *plan;
                this.change_pass(move |pass| BattlePass { plan, ..pass }, cx);
            }),
        ];
        // The series lists are filled when the drawer opens, in the language
        // of the moment.
        let mut boxes = Vec::new();
        for kind in BoxKind::ALL {
            let state = cx.new(|cx| PickerState::searchable(Vec::new(), None, window, cx));
            subscriptions.push(cx.subscribe(
                &state,
                move |this, _, event: &PickerEvent<Option<MaterialId>>, cx| {
                    let PickerEvent::Confirm(pick) = event;
                    let pick = *pick;
                    this.change_pass(
                        move |pass| BattlePass {
                            picks: pass.picks.with(kind, pick),
                            ..pass
                        },
                        cx,
                    );
                },
            ));
            boxes.push((kind, state));
        }
        PassControls {
            drawer: DrawerState::Closed,
            focus: cx.focus_handle(),
            level,
            plan,
            boxes,
            _subscriptions: subscriptions,
        }
    }

    /// The account's pass in the current period: as entered, or at level 0
    /// once a later period started; a new one when it was never entered.
    pub(super) fn current_pass(&self, cx: &App) -> BattlePass {
        let (data, now) = (BattlePassData::embedded(), now());
        let account = self.account(cx);
        match self.library.read(cx).battle_pass(account) {
            Some(pass) => pass.at_level(pass.level_at(data, now), data, now),
            None => BattlePass::new(data, now),
        }
    }

    /// The level shown: the slider's while the drawer is open, else the
    /// account's.
    pub(super) fn pass_level(&self, cx: &App) -> u8 {
        if self.pass.drawer == DrawerState::Closed {
            return self.current_pass(cx).level;
        }
        match self.pass.level.read(cx).value() {
            // Whole steps from 0 to 50.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            SliderValue::Single(level) => level.round().clamp(0., 50.) as u8,
            SliderValue::Range(_, to) => to.round().clamp(0., 50.) as u8,
        }
    }

    /// Opens the drawer on the account's pass, with the keyboard in it.
    pub(crate) fn open_pass(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pass = self.current_pass(cx);
        self.pass.level.update(cx, |slider, cx| {
            slider.set_value(f32::from(pass.level), window, cx);
        });
        self.pass.plan.update(cx, |picker, cx| {
            picker.set_items(plan_items(), cx);
            picker.set_selected_value(&pass.plan, cx);
        });
        let planner = self.library.read(cx).planner().clone();
        for (kind, state) in &self.pass.boxes {
            state.update(cx, |picker, cx| {
                picker.set_items(box_items(&planner, *kind), cx);
                picker.set_selected_value(&pass.picks.of(*kind), cx);
            });
        }
        self.pass.drawer = DrawerState::Open;
        self.pass.focus.focus(window, cx);
        self.rebuild(cx);
        cx.notify();
    }

    /// Slides the drawer out, then forgets it.
    pub(crate) fn close_pass(&mut self, cx: &mut Context<Self>) {
        if self.pass.drawer != DrawerState::Open {
            return;
        }
        self.pass.drawer = DrawerState::Closing;
        cx.notify();
        let slide = drawer_slide(cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(slide).await;
            this.update(cx, |this, cx| {
                if this.pass.drawer == DrawerState::Closing {
                    this.pass.drawer = DrawerState::Closed;
                    this.rebuild(cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Opens or closes the reward plan's list, as a click on it does (the
    /// screenshot tool uses it).
    pub(crate) fn show_pass_plans(&mut self, open: bool, window: &mut Window, cx: &mut App) {
        self.pass
            .plan
            .update(cx, |picker, cx| picker.show(open, window, cx));
    }

    /// Saves the account's pass, as `change` makes it from the current one.
    fn change_pass(
        &mut self,
        change: impl FnOnce(BattlePass) -> BattlePass,
        cx: &mut Context<Self>,
    ) {
        let pass = change(self.current_pass(cx));
        let account = self.account(cx);
        let task = self
            .library
            .update(cx, |library, cx| library.set_battle_pass(account, pass, cx));
        report_plan(task, cx);
    }

    /// The drawer, when it shows.
    pub(super) fn pass_drawer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.pass.drawer == DrawerState::Closed {
            return None;
        }
        let theme = cx.theme();
        let (data, now) = (BattlePassData::embedded(), now());
        let pass = self.current_pass(cx);
        let level = self.pass_level(cx);
        let model = &self.model.pass;
        let library = self.library.read(cx);
        let (planner, images) = (library.planner().clone(), library.images(cx).clone());
        let muted = theme.muted_foreground;

        let label = |text: String| {
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(text)
        };
        let help = |text: String| div().text_sm().text_color(muted).child(text);
        let section = |title: String| div().v_flex().gap_2().child(label(title));

        // A material on its tile, its name and how many.
        let material_row = |id: MaterialId, count: u64| {
            planner.material(id).map(|material| {
                div()
                    .h_flex()
                    .gap_3()
                    .text_sm()
                    .child(small_tile(images.material(material), material.rarity, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(material.name.to_string()),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format::count(count)),
                    )
            })
        };

        let view = cx.entity().downgrade();
        let close = {
            let view = view.clone();
            std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.close_pass(cx)).ok();
            }) as wish_ui::CloseHandler
        };

        let level_section = section(t!("planner-pass-level"))
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .child(
                        Slider::new(&self.pass.level)
                            .flex_1()
                            .cursor_pointer()
                            .accessibility_label(t!("planner-pass-level")),
                    )
                    .child(
                        div()
                            .w(rems(3.5))
                            .text_right()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{level} / {BATTLE_PASS_LEVELS}")),
                    ),
            )
            .child(help(t!(
                "planner-pass-period",
                end = format::date(&data.next_period(now).with_timezone(&Local).date_naive()),
                days = (data.next_period(now) - now).num_days()
            )));

        // Two segments at their own width, not the drawer's.
        let premium = {
            let view = view.clone();
            section(t!("planner-pass-track")).child(
                div().h_flex().child(
                    Segmented::new("pass-track")
                        .option(t!("planner-pass-free"))
                        .option(t!("planner-pass-premium"))
                        .selected(usize::from(pass.premium))
                        .on_select(move |ix: &usize, _, cx| {
                            let premium = *ix == 1;
                            view.update(cx, |this, cx| {
                                this.change_pass(move |pass| BattlePass { premium, ..pass }, cx);
                            })
                            .ok();
                        }),
                ),
            )
        };

        let plan = section(t!("planner-pass-plan"))
            .child(
                picker("pass-plan", &self.pass.plan)
                    .menu_width(rems(MENU_WIDTH))
                    .accessibility_label(t!("planner-pass-plan")),
            )
            .child(help(t!("planner-pass-plan-help")));

        let counted = {
            let view = view.clone();
            div()
                .h_flex()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .v_flex()
                        .gap_1()
                        .child(label(t!("planner-pass-counted")))
                        .child(help(t!("planner-pass-counted-help"))),
                )
                .child(
                    Switch::new("pass-counted")
                        .cursor_pointer()
                        .checked(pass.counted)
                        .accessibility_label(t!("planner-pass-counted"))
                        .on_change(move |on: &bool, _, cx| {
                            let counted = *on;
                            view.update(cx, |this, cx| {
                                this.change_pass(move |pass| BattlePass { counted, ..pass }, cx);
                            })
                            .ok();
                        }),
                )
        };

        let to_come = if model.rewards.is_empty() && model.boxes.is_empty() {
            section(t!("planner-pass-to-come-none"))
        } else {
            section(t!(
                "planner-pass-to-come",
                from = model.from_level,
                to = BATTLE_PASS_LEVELS
            ))
            .children((model.rewards.iter()).filter_map(|&(id, count)| material_row(id, count)))
        };

        // Each kind of box still to come: its series picker, and what it
        // becomes (or how many the plan has no use for).
        let boxes = (!model.boxes.is_empty()).then(|| {
            let sizes = data.boxes;
            let kinds = model.boxes.iter().filter_map(|&(kind, count)| {
                let (_, state) = self.pass.boxes.iter().find(|(k, _)| *k == kind)?;
                let picked = (model.picked.iter())
                    .filter(|(k, _, _)| *k == kind)
                    .filter_map(|&(_, id, count)| material_row(id, count));
                let left = (model.left.iter())
                    .find(|(k, _)| *k == kind)
                    .map(|&(_, count)| help(t!("planner-pass-left", count = count)));
                Some(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(
                            div()
                                .h_flex()
                                .gap_3()
                                .child(div().flex_1().child(label(box_name(kind))))
                                .child(div().text_sm().text_color(muted).child(t!(
                                    "planner-pass-box-count",
                                    count = count,
                                    size = sizes.of(kind)
                                ))),
                        )
                        .child(
                            picker(SharedString::from(format!("pass-box-{kind:?}")), state)
                                .menu_width(rems(MENU_WIDTH))
                                .accessibility_label(box_name(kind)),
                        )
                        .children(picked)
                        .children(left),
                )
            });
            div()
                .v_flex()
                .gap_4()
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(label(t!("planner-pass-boxes-title")))
                        .child(help(t!("planner-pass-boxes-help"))),
                )
                .children(kinds)
        });

        let reset = {
            let view = view.clone();
            button("pass-reset")
                .quiet(cx)
                .label(t!("planner-pass-reset"))
                .tooltip(t!("planner-pass-reset-help"))
                .on_click(move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        let now = self::now();
                        this.change_pass(
                            move |pass| pass.at_level(0, BattlePassData::embedded(), now),
                            cx,
                        );
                    })
                    .ok();
                })
        };
        let done = {
            let close = close.clone();
            button("pass-done")
                .label(t!("planner-pass-done"))
                .on_click(move |_, window, cx| close(window, cx))
        };

        Some(
            Drawer::new(
                "battle-pass",
                self.pass.drawer,
                &self.pass.focus,
                t!("planner-pass"),
                t!("planner-pass-close"),
                close,
            )
            .width(WIDTH)
            .child(level_section)
            .child(premium)
            .child(plan)
            .child(counted)
            .child(to_come)
            .when_some(boxes, Drawer::child)
            .footer_child(reset)
            .footer_child(div().flex_1())
            .footer_child(done)
            .into_any_element(),
        )
    }
}

/// A material's icon on its rarity tile, small.
fn small_tile(icon: Option<ImageSource>, stars: u8, cx: &App) -> impl IntoElement {
    div()
        .size_8()
        .flex_none()
        .rounded(cx.theme().radius)
        .bg(wish_ui::theme::stars_tile(stars, cx))
        .children(icon.map(|icon| img(icon).size_full()))
}
