//! What the selected account owns, on the weapon pages: the copies of each
//! weapon (one row per copy: level, refinement, the character holding it,
//! favourite), adding one, and removing one after asking. Every change saves
//! at once; a character holds one weapon, so giving it a copy takes it from
//! the copy it held.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, IntoElement,
    ParentElement as _, SharedString, Styled as _, Subscription, Task, Window, div, rems,
};
use wish_core::collection::{MAX_REFINEMENT, OwnedWeapon};
use wish_core::model::{AccountId, Rarity};
use wish_core::planner::{Level, Weapon};

use super::super::level_options;
use super::WeaponsView;
use super::model::copy_groups;
use crate::app::notices::{report, report_outcome};
use wish_ui::{
    PickerEvent, PickerItem, PickerState, Segmented, button, card, card_title, dialog_footer,
    icon_button, picker,
};

/// The pickers of one copy on the open weapon's page.
pub(super) struct CopyControls {
    id: i64,
    level: Entity<PickerState<usize>>,
    /// A character's key, or "" for nobody.
    holder: Entity<PickerState<SharedString>>,
    _subscriptions: Vec<Subscription>,
}

impl WeaponsView {
    /// The account whose weapons are shown: the sidebar's.
    pub(super) fn account(&self, cx: &App) -> Option<AccountId> {
        self.session.read(cx).account()
    }

    /// The account's copies of weapon `key`, in the order they were added.
    pub(super) fn copies_of(&self, key: &str, cx: &App) -> Vec<OwnedWeapon> {
        let library = self.library.read(cx);
        library
            .owned(self.account(cx))
            .copies(key)
            .cloned()
            .collect()
    }

    /// Adds a copy of `key`, at level 1, held by nobody.
    pub(super) fn add_copy(&mut self, key: String, name: String, cx: &mut Context<Self>) {
        let account = self.account(cx);
        let task = self.library.update(cx, |library, cx| {
            library.add_owned_weapon(account, OwnedWeapon::new(key), cx)
        });
        report_outcome(
            task,
            t!("archive-not-added", name = name.as_str()),
            move |_| t!("archive-copy-added", name = name.as_str()),
            cx,
        );
    }

    /// Saves what changed on a copy.
    pub(super) fn save_copy(&mut self, weapon: OwnedWeapon, cx: &mut Context<Self>) {
        let Some(account) = self.account(cx) else {
            return;
        };
        let task = self.library.update(cx, |library, cx| {
            library.update_owned_weapon(account, weapon, cx)
        });
        report_weapons(task, cx);
    }

    /// Marks every copy of `key` as a favourite, or none if one was.
    pub(super) fn toggle_weapon_favourite(&mut self, key: &str, cx: &mut Context<Self>) {
        let copies = self.copies_of(key, cx);
        let favourite = !copies.iter().any(|c| c.favourite);
        for mut copy in copies {
            copy.favourite = favourite;
            self.save_copy(copy, cx);
        }
    }

    /// Asks before forgetting copy `id` of weapon `name`.
    fn confirm_remove_copy(&mut self, id: i64, name: String, window: &mut Window, cx: &mut App) {
        let Some(account) = self.account(cx) else {
            return;
        };
        let library = self.library.downgrade();
        let title = SharedString::from(t!("archive-remove-copy-title", name = name.as_str()));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let library = library.clone();
            alert
                .title(title.clone())
                .description(t!("archive-remove-copy-help"))
                .footer(dialog_footer(
                    button("confirm").danger().label(t!("archive-remove")),
                ))
                .on_ok(move |_, _, cx| {
                    let task = library.update(cx, |library, cx| {
                        library.remove_owned_weapon(account, id, cx)
                    });
                    if let Ok(task) = task {
                        // A failed removal brings the copy back: say why.
                        report(task, t!("archive-copy-not-removed"), cx);
                    }
                    true
                })
        });
    }

    /// Makes the pickers of the open weapon's copies follow what is stored:
    /// one pair per row (a group of copies alike, edited through its first),
    /// their choices set from it, those of rows gone dropped.
    pub(super) fn sync_copies(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.open.clone() else {
            self.copies.clear();
            return;
        };
        let library = self.library.read(cx);
        let (planner, account) = (library.planner().clone(), self.account(cx));
        let owned = library.owned(account).clone();
        let Some(weapon) = planner.weapon(&key) else {
            self.copies.clear();
            return;
        };
        let steps = Level::weapon_steps(weapon.rarity);
        let copies: Vec<OwnedWeapon> = owned.copies(&key).cloned().collect();
        let rows: Vec<OwnedWeapon> = (copy_groups(&copies).into_iter())
            .map(|group| group.first)
            .collect();
        self.copies
            .retain(|controls| rows.iter().any(|c| c.id == controls.id));
        for copy in rows {
            let level = steps.iter().position(|l| *l == copy.level).unwrap_or(0);
            let holder = SharedString::from(copy.holder.clone().unwrap_or_default());
            // The owned characters who wield this type, and whoever holds it.
            let mut holders = vec![PickerItem::new(
                t!("archive-nobody"),
                SharedString::default(),
            )];
            holders.extend(
                planner
                    .characters()
                    .iter()
                    .filter(|c| {
                        (c.weapon == weapon.weapon && owned.character(&c.key).is_some())
                            || copy.holder.as_deref() == Some(c.key.as_str())
                    })
                    .map(|c| {
                        PickerItem::new(c.name.to_string(), SharedString::from(c.key.clone()))
                    }),
            );
            match self.copies.iter().find(|c| c.id == copy.id) {
                Some(controls) => {
                    controls.level.update(cx, |picker, cx| {
                        if picker.selected_value() != Some(&level) {
                            picker.set_selected_value(&level, cx);
                        }
                    });
                    controls.holder.update(cx, |picker, cx| {
                        picker.set_items(holders, cx);
                        picker.set_selected_value(&holder, cx);
                    });
                }
                None => {
                    let level_picker =
                        cx.new(|cx| PickerState::new(level_options(&steps), Some(level), cx));
                    let at = holders.iter().position(|h| h.value == holder);
                    let holder_picker = cx.new(|cx| PickerState::new(holders, at, cx));
                    let id = copy.id;
                    let subscriptions = vec![
                        cx.subscribe(&level_picker, {
                            let steps = steps.clone();
                            move |this, _, event: &PickerEvent<usize>, cx| {
                                let PickerEvent::Confirm(index) = event;
                                this.change_copy(id, cx, |copy| {
                                    copy.level = steps[(*index).min(steps.len() - 1)];
                                });
                            }
                        }),
                        cx.subscribe(
                            &holder_picker,
                            move |this, _, event: &PickerEvent<SharedString>, cx| {
                                let PickerEvent::Confirm(key) = event;
                                let holder = (!key.is_empty()).then(|| key.to_string());
                                this.change_copy(id, cx, |copy| copy.holder = holder);
                            },
                        ),
                    ];
                    self.copies.push(CopyControls {
                        id,
                        level: level_picker,
                        holder: holder_picker,
                        _subscriptions: subscriptions,
                    });
                }
            }
        }
    }

    /// Changes copy `id` with `change`, then saves it if it changed.
    fn change_copy(
        &mut self,
        id: i64,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut OwnedWeapon),
    ) {
        let library = self.library.read(cx);
        let Some(copy) = (library.owned(self.account(cx)).weapons.iter())
            .find(|w| w.id == id)
            .cloned()
        else {
            return;
        };
        let mut changed = copy.clone();
        change(&mut changed);
        if changed != copy {
            self.save_copy(changed, cx);
        }
    }

    /// The panel of what the account has of `weapon`: how many copies, a row
    /// per group of copies alike, and the way to add one.
    pub(super) fn my_copies(
        &self,
        weapon: &Weapon,
        refinable: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let gold = wish_ui::theme::rarity(Rarity::Five, cx);
        let copies = self.copies_of(&weapon.key, cx);
        let add = button("add-copy")
            .icon(IconName::Plus)
            .label(if copies.is_empty() {
                t!("archive-i-have-it")
            } else {
                t!("archive-add-copy")
            })
            .on_click(cx.listener({
                let (key, name) = (weapon.key.clone(), weapon.name.clone());
                move |this, _, _, cx| this.add_copy(key.clone(), name.to_string(), cx)
            }));
        let groups = copy_groups(&copies);
        let rows = groups.iter().enumerate().filter_map(|(ix, group)| {
            let copy = &group.first;
            let alike = group.ids.len();
            let controls = self.copies.iter().find(|c| c.id == copy.id)?;
            let refinement: AnyElement = if refinable {
                let owned = copy.clone();
                (1..=MAX_REFINEMENT)
                    .fold(
                        Segmented::new(SharedString::from(format!("refinement-{}", copy.id))),
                        |seg, r| seg.option(format!("R{r}")),
                    )
                    .selected(usize::from(copy.refinement.max(1) - 1))
                    .on_select(cx.listener(move |this, ix: &usize, _, cx| {
                        let mut owned = owned.clone();
                        owned.refinement = *ix as u8 + 1;
                        this.save_copy(owned, cx);
                    }))
                    .into_any_element()
            } else {
                div().text_sm().child("R1").into_any_element()
            };
            // A favourite is every copy of the row's.
            let favourite = {
                let marked: Vec<OwnedWeapon> = (copies.iter())
                    .filter(|c| group.ids.contains(&c.id))
                    .map(|c| OwnedWeapon {
                        favourite: !copy.favourite,
                        ..c.clone()
                    })
                    .collect();
                let (icon, label) = if copy.favourite {
                    (IconName::StarFill, t!("archive-remove-favourite"))
                } else {
                    (IconName::Star, t!("archive-add-favourite"))
                };
                icon_button(format!("favourite-copy-{}", copy.id), icon, label)
                    .ghost()
                    .when(copy.favourite, |this| this.text_color(gold))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        for copy in &marked {
                            this.save_copy(copy.clone(), cx);
                        }
                    }))
            };
            // Removing takes one copy of the row.
            let remove = button(SharedString::from(format!("remove-copy-{}", copy.id)))
                .ghost()
                .label(if alike > 1 {
                    t!("archive-remove-one")
                } else {
                    t!("archive-remove")
                })
                .on_click(cx.listener({
                    let (id, name) = (copy.id, weapon.name.clone());
                    move |this, _, window, cx| {
                        this.confirm_remove_copy(id, name.to_string(), window, cx)
                    }
                }));
            Some(
                div()
                    .h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap_x_6()
                    .gap_y_3()
                    .when(ix > 0, |this| this.pt_4().border_t_1().border_color(border))
                    .when(alike > 1, |this| {
                        this.child(field(
                            &t!("archive-alike"),
                            div()
                                .h(rems(2.75))
                                .h_flex()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("×{alike}")),
                            cx,
                        ))
                    })
                    .child(field(
                        &t!("archive-level"),
                        div().w(rems(9.)).child(
                            picker(
                                SharedString::from(format!("copy-level-{}", copy.id)),
                                &controls.level,
                            )
                            .accessibility_label(t!("archive-level"))
                            .menu_width(rems(9.)),
                        ),
                        cx,
                    ))
                    .child(field(&t!("archive-refinement"), refinement, cx))
                    .child(field(
                        &t!("archive-held-by"),
                        div().w(rems(13.)).child(
                            picker(
                                SharedString::from(format!("copy-holder-{}", copy.id)),
                                &controls.holder,
                            )
                            .accessibility_label(t!("archive-held-by"))
                            .menu_width(rems(13.)),
                        ),
                        cx,
                    ))
                    .child(div().flex_1())
                    .child(div().h_flex().gap_2().child(favourite).child(remove)),
            )
        });
        let rows: Vec<Div> = rows.collect();
        card(cx)
            .p_4()
            .gap_4()
            .child(
                card_title(
                    match copies.len() {
                        0 | 1 => t!("archive-my-copies"),
                        count => t!("archive-my-copies-count", count = count),
                    },
                    cx,
                )
                .child(add),
            )
            .when(rows.is_empty(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(t!("archive-no-copies")),
                )
            })
            .children(rows)
            .into_any_element()
    }
}

/// A control with its label above.
fn field(label: &str, control: impl IntoElement, cx: &App) -> Div {
    div()
        .v_flex()
        .gap_1()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_owned()),
        )
        .child(control)
}

/// Shows a notice if a write of the weapons fails.
fn report_weapons<T: 'static>(task: Task<anyhow::Result<T>>, cx: &mut App) {
    report(task, t!("archive-weapons-not-saved"), cx);
}
