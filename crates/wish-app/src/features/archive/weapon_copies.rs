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
    AnyElement, App, AppContext as _, Context, Div, Entity, IntoElement, ParentElement as _,
    SharedString, Styled as _, Subscription, Task, Window, div, rems,
};
use wish_core::collection::{MAX_REFINEMENT, OwnedWeapon};
use wish_core::model::{AccountId, Rarity};
use wish_core::planner::{Level, Weapon};

use super::level_options;
use super::weapons::WeaponsView;
use crate::app::notices::{Notice, notify};
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
        cx.spawn(async move |_, cx| {
            let notice = match task.await {
                Ok(_) => Notice::done(format!(
                    "A copy of {name} is in your weapons. Set its level, refinement and holder \
                     on its page."
                )),
                Err(err) => Notice::failed(format!("{name} wasn’t added: {err:#}")),
            };
            cx.update(|cx| notify(notice, cx));
        })
        .detach();
    }

    /// Saves what changed on a copy.
    pub(super) fn save_copy(&mut self, weapon: OwnedWeapon, cx: &mut Context<Self>) {
        let Some(account) = self.account(cx) else {
            return;
        };
        let task = self.library.update(cx, |library, cx| {
            library.update_owned_weapon(account, weapon, cx)
        });
        report(task, cx);
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
        let title = SharedString::from(format!("Remove this copy of {name}?"));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let library = library.clone();
            alert
                .title(title.clone())
                .description(
                    "Its level, refinement and holder are forgotten. A backup is taken first.",
                )
                .footer(dialog_footer(button("confirm").danger().label("Remove")))
                .on_ok(move |_, _, cx| {
                    library
                        .update(cx, |library, cx| {
                            library.remove_owned_weapon(account, id, cx).detach()
                        })
                        .ok();
                    true
                })
        });
    }

    /// Makes the pickers of the open weapon's copies follow what is stored:
    /// one pair per copy, their choices set from it, those of removed copies
    /// dropped.
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
        self.copies
            .retain(|controls| copies.iter().any(|c| c.id == controls.id));
        for copy in copies {
            let level = steps.iter().position(|l| *l == copy.level).unwrap_or(0);
            let holder = SharedString::from(copy.holder.clone().unwrap_or_default());
            // The owned characters who wield this type, and whoever holds it.
            let mut holders = vec![PickerItem::new("Nobody", SharedString::default())];
            holders.extend(
                planner
                    .characters()
                    .iter()
                    .filter(|c| {
                        (c.weapon == weapon.weapon && owned.character(&c.key).is_some())
                            || copy.holder.as_deref() == Some(c.key.as_str())
                    })
                    .map(|c| PickerItem::new(c.name.clone(), SharedString::from(c.key.clone()))),
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

    /// The panel of what the account has of `weapon`: a row per copy, and
    /// the way to add one.
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
                "I have it"
            } else {
                "Add a copy"
            })
            .on_click(cx.listener({
                let (key, name) = (weapon.key.clone(), weapon.name.clone());
                move |this, _, _, cx| this.add_copy(key.clone(), name.clone(), cx)
            }));
        let rows = copies.iter().enumerate().filter_map(|(ix, copy)| {
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
            let favourite = {
                let mut owned = copy.clone();
                owned.favourite = !copy.favourite;
                let (icon, label) = if copy.favourite {
                    (IconName::StarFill, "Remove from favourites")
                } else {
                    (IconName::Star, "Add to favourites")
                };
                icon_button(format!("favourite-copy-{}", copy.id), icon, label)
                    .ghost()
                    .when(copy.favourite, |this| this.text_color(gold))
                    .on_click(cx.listener(move |this, _, _, cx| this.save_copy(owned.clone(), cx)))
            };
            let remove = button(SharedString::from(format!("remove-copy-{}", copy.id)))
                .ghost()
                .label("Remove")
                .on_click(cx.listener({
                    let (id, name) = (copy.id, weapon.name.clone());
                    move |this, _, window, cx| {
                        this.confirm_remove_copy(id, name.clone(), window, cx)
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
                    .child(field(
                        "Level",
                        div().w(rems(9.)).child(
                            picker(
                                SharedString::from(format!("copy-level-{}", copy.id)),
                                &controls.level,
                            )
                            .accessibility_label("Level")
                            .menu_width(rems(9.)),
                        ),
                        cx,
                    ))
                    .child(field("Refinement", refinement, cx))
                    .child(field(
                        "Held by",
                        div().w(rems(13.)).child(
                            picker(
                                SharedString::from(format!("copy-holder-{}", copy.id)),
                                &controls.holder,
                            )
                            .accessibility_label("Held by")
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
            .child(card_title("My copies", cx).child(add))
            .when(rows.is_empty(), |this| {
                this.child(div().text_sm().text_color(muted).child(
                    "You don’t have it. Add a copy to note its level, refinement and who holds it.",
                ))
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

/// Shows a notification if a write of the weapons fails.
fn report<T: 'static>(task: Task<anyhow::Result<T>>, cx: &mut Context<WeaponsView>) {
    cx.spawn(async move |_, cx| {
        if let Err(err) = task.await {
            cx.update(|cx| {
                notify(
                    Notice::failed(format!("Your weapons were not saved: {err:#}")),
                    cx,
                );
            });
        }
    })
    .detach();
}
