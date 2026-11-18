//! Each entry's sliders: a level range (a character's levels, or those of a
//! weapon of its rarity) and a character's three talent ranges. They are the
//! view's own state: while one is dragged the costs follow it ([`PlannerView::live_plan`]),
//! and released it saves its entry.

use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::{App, AppContext as _, Context, Entity, Subscription, Window};
use wish_core::planner::{CharacterGoal, Level, PlanEntry, PlanTarget, WeaponGoal};

use super::{PlannerView, report_plan};

/// An entry's sliders: its level, from and to (as indexes into `steps`), and
/// a character's three talents.
pub(super) struct Controls {
    pub(super) level: Entity<SliderState>,
    /// The levels the slider goes through: a character's, or those of a
    /// weapon of its rarity.
    pub(super) steps: Vec<Level>,
    pub(super) talents: Vec<Entity<SliderState>>,
    _subscriptions: Vec<Subscription>,
}

impl PlannerView {
    /// Makes sliders for new character and weapon entries and drops those of
    /// removed ones. Existing sliders keep their values: the plan they would
    /// be set from is what they saved.
    pub(super) fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let library = self.library.read(cx);
        let (plan, planner) = (library.plan(account).to_vec(), library.planner().clone());
        self.controls
            .retain(|id, _| plan.iter().any(|entry| entry.id == *id));
        for entry in plan {
            if self.controls.contains_key(&entry.id) {
                continue;
            }
            let controls = match &entry.target {
                PlanTarget::CustomWeapon { stars, goal, .. } => {
                    let steps = Level::weapon_steps(*stars);
                    self.make_controls(entry.id, steps, (goal.from, goal.to), None, window, cx)
                }
                PlanTarget::Weapon { key, goal } => {
                    let rarity = planner.weapon(key).map_or(5, |weapon| weapon.rarity);
                    let steps = Level::weapon_steps(rarity);
                    self.make_controls(entry.id, steps, (goal.from, goal.to), None, window, cx)
                }
                target => match target.goal() {
                    Some(goal) => self.make_controls(
                        entry.id,
                        Level::steps(),
                        (goal.from, goal.to),
                        Some(goal.talents),
                        window,
                        cx,
                    ),
                    None => continue,
                },
            };
            self.controls.insert(entry.id, controls);
        }
    }

    pub(super) fn make_controls(
        &mut self,
        id: i64,
        steps: Vec<Level>,
        (from, to): (Level, Level),
        talents: Option<[(u8, u8); 3]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Controls {
        let step = |level: Level| steps.iter().position(|s| *s == level).unwrap_or(0) as f32;
        let last = (steps.len() - 1) as f32;
        let (from, to) = (step(from), step(to).min(last));
        let level = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(last)
                .step(1.0)
                .default_value((from, to))
        });
        let talents: Vec<_> = talents
            .into_iter()
            .flatten()
            .map(|(from, to)| {
                cx.new(|_| {
                    SliderState::new()
                        .min(1.0)
                        .max(10.0)
                        .step(1.0)
                        .default_value((f32::from(from), f32::from(to)))
                })
            })
            .collect();
        // The costs follow a slider while it is dragged; released, it saves.
        let on_slide = move |this: &mut Self, event: &SliderEvent, cx: &mut Context<Self>| {
            if let SliderEvent::Release(_) = event {
                this.save(id, cx);
            }
            this.rebuild(cx);
            cx.notify();
        };
        let mut subscriptions =
            vec![
                cx.subscribe_in(&level, window, move |this, _, event, _, cx| {
                    on_slide(this, event, cx)
                }),
            ];
        for talent in &talents {
            subscriptions.push(
                cx.subscribe_in(talent, window, move |this, _, event, _, cx| {
                    on_slide(this, event, cx)
                }),
            );
        }
        Controls {
            level,
            steps,
            talents,
            _subscriptions: subscriptions,
        }
    }

    /// The level range an entry's slider shows now.
    pub(super) fn levels(&self, id: i64, cx: &App) -> Option<(Level, Level)> {
        let controls = self.controls.get(&id)?;
        let (from, to) = slider_range(&controls.level, cx);
        let last = controls.steps.len() - 1;
        Some((controls.steps[from.min(last)], controls.steps[to.min(last)]))
    }

    /// The goal a character entry's sliders show now.
    pub(super) fn goal(&self, id: i64, cx: &App) -> Option<CharacterGoal> {
        let (from, to) = self.levels(id, cx)?;
        let controls = self.controls.get(&id)?;
        let talents: Vec<(u8, u8)> = (controls.talents.iter())
            .map(|talent| {
                let (from, to) = slider_range(talent, cx);
                (from as u8, to as u8)
            })
            .collect();
        Some(CharacterGoal {
            from,
            to,
            talents: talents.try_into().ok()?,
        })
    }

    /// The goal a weapon entry's slider shows now.
    pub(super) fn weapon_goal(&self, id: i64, cx: &App) -> Option<WeaponGoal> {
        let (from, to) = self.levels(id, cx)?;
        Some(WeaponGoal { from, to })
    }

    /// Saves an entry's goal from its sliders.
    pub(super) fn save(&mut self, id: i64, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let Some(mut entry) = self
            .library
            .read(cx)
            .plan(account)
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            return;
        };
        let changed = match (self.goal(id, cx), self.weapon_goal(id, cx)) {
            (Some(goal), _) if entry.target.goal().is_some() => {
                let saved = entry.target.goal_mut().expect("a character's goal");
                std::mem::replace(saved, goal) != goal
            }
            (_, Some(goal)) => match entry.target.weapon_goal_mut() {
                Some(saved) => std::mem::replace(saved, goal) != goal,
                None => false,
            },
            _ => false,
        };
        if changed {
            let task = (self.library).update(cx, |library, cx| {
                library.update_plan_entry(account, entry, cx)
            });
            report_plan(task, cx);
        }
    }

    /// The plan with each entry's goal as its sliders show it.
    pub(super) fn live_plan(&self, cx: &App) -> Vec<PlanEntry> {
        let mut plan = self.library.read(cx).plan(self.account(cx)).to_vec();
        for entry in &mut plan {
            if let Some(goal) = entry.target.goal_mut()
                && let Some(live) = self.goal(entry.id, cx)
            {
                *goal = live;
            }
            if let Some(goal) = entry.target.weapon_goal_mut()
                && let Some(live) = self.weapon_goal(entry.id, cx)
            {
                *goal = live;
            }
        }
        plan
    }
}

/// A range slider's ends, as whole steps.
fn slider_range(state: &Entity<SliderState>, cx: &App) -> (usize, usize) {
    match state.read(cx).value() {
        SliderValue::Range(from, to) => (from.round() as usize, to.round() as usize),
        SliderValue::Single(to) => (0, to.round() as usize),
    }
}
