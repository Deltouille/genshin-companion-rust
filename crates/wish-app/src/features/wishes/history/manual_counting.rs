//! Counting wishes by hand: the history page's `+1`, `+10`, "Add 4★ or 5★…"
//! and "Delete…" controls, carried out here.
//!
//! Each entry is an import of its own (undoable from Settings); only wishes
//! entered by hand can be deleted, after a backup.

use std::cell::Cell;
use std::sync::Arc;

use crate::app::notices::{Notice, notify};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::input::InputState;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, div, prelude::FluentBuilder as _, rems,
};
use wish_core::data::{GameData, Item};
use wish_core::engine::DEFAULT_SERVER;
use wish_core::import::{ImportBatch, manual};
use wish_core::model::{Account, AccountId, ItemType, Pool, Rarity, Wish, WishId, WishTime};
use wish_ui::{PickerItem, PickerState, button, dialog_footer, input, picker};

use super::{HistoryView, ManualAction};

impl HistoryView {
    pub(super) fn on_manual(
        &mut self,
        action: &ManualAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            ManualAction::AddThreeStars(pool, count) => {
                let account = self.counting_account(cx);
                let time = now_for(account.as_ref());
                let batch = manual::three_stars(account.as_ref(), *pool, *count, time);
                self.save_counted(batch, window, cx);
            }
            ManualAction::AddItem(pool) => self.open_add_item(*pool, window, cx),
            ManualAction::Delete(wish) => self.confirm_delete(wish.clone(), window, cx),
        }
    }

    /// The account the entries go to: the one on screen, or none yet (the
    /// first entry then creates one).
    fn counting_account(&self, cx: &Context<Self>) -> Option<Account> {
        self.session
            .read(cx)
            .summary(cx)
            .map(|summary| summary.account.clone())
    }

    /// Writes one entry. Success shows in the table, so only a failure is
    /// reported. Detached rather than kept in `flow`: a second click must not
    /// cancel the report of the first.
    fn save_counted(&mut self, batch: ImportBatch, window: &mut Window, cx: &mut Context<Self>) {
        let task = self
            .library
            .update(cx, |library, cx| library.apply_import(batch, cx));
        cx.spawn_in(window, async move |this, cx| {
            if let Err(err) = task.await {
                this.update_in(cx, |_, _, cx| {
                    let message = format!("Couldn’t save the wish: {err:#}");
                    notify(Notice::failed(message), cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Opens "Add a 4★ or 5★" for `pool`.
    pub fn open_add_item(&mut self, pool: Pool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = self.library.read(cx).data().cloned() else {
            return;
        };
        let form = cx.new(|cx| AddItemForm::new(pool, data, window, cx));
        let view = cx.entity().downgrade();
        let title = SharedString::from(format!("Add a 4★ or 5★ to {}", pool.display_name()));
        let item = form.read(cx).item.clone();
        let focus_once = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                item.focus_handle(cx).focus(window, cx);
            }
            let (form, view) = (form.clone(), view.clone());
            dialog
                .title(title.clone())
                .child(form.clone())
                .footer(dialog_footer(
                    button("confirm-add-item").primary().label("Add"),
                ))
                .on_ok(move |_, window, cx| {
                    let Some((item, pity)) = form.update(cx, |form, cx| form.submit(cx)) else {
                        // Keep the dialog open on the error.
                        return false;
                    };
                    view.update(cx, |view, cx| {
                        let account = view.counting_account(cx);
                        let time = now_for(account.as_ref());
                        let batch = manual::item(
                            account.as_ref(),
                            pool.default_gacha_type(),
                            &item,
                            pity,
                            time,
                        );
                        view.save_counted(batch, window, cx);
                    })
                    .ok();
                    true
                })
        });
    }

    fn confirm_delete(&mut self, wish: Wish, window: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.session.read(cx).account() else {
            return;
        };
        let view = cx.entity().downgrade();
        let what = match wish.item_key {
            Some(_) => wish.name.clone(),
            None => format!("the {}★ wish", wish.rarity.stars()),
        };
        let title = SharedString::from(format!(
            "Delete {what} of {}?",
            wish.time.naive().format("%-d %b %Y, %H:%M")
        ));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (view, id) = (view.clone(), wish.id.clone());
            alert
                .title(title.clone())
                .description("It was entered by hand. A backup is taken first.")
                .footer(dialog_footer(
                    button("confirm").danger().label("Delete wish"),
                ))
                .on_ok(move |_, window, cx| {
                    let id = id.clone();
                    view.update(cx, |view, cx| view.delete_manual(account, id, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn delete_manual(
        &mut self,
        account: AccountId,
        id: WishId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = self.library.update(cx, |library, cx| {
            library.delete_manual(account, vec![id], cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |_, _, cx| {
                let note = match result {
                    Ok(_) => Notice::done("Wish deleted."),
                    Err(err) => Notice::failed(format!("Couldn’t delete the wish: {err:#}")),
                };
                notify(note, cx);
            })
            .ok();
        })
        .detach();
    }
}

/// Now, on the account's server clock (the game's times are server-local).
fn now_for(account: Option<&Account>) -> WishTime {
    WishTime::now_on(account.and_then(|a| a.server).unwrap_or(DEFAULT_SERVER))
}

/// The "Add a 4★ or 5★" form: which item, and its pity when known.
struct AddItemForm {
    pool: Pool,
    data: Arc<GameData>,
    item: Entity<PickerState<String>>,
    pity: Entity<InputState>,
    error: Option<SharedString>,
}

impl AddItemForm {
    fn new(pool: Pool, data: Arc<GameData>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut items: Vec<&Item> = data
            .items
            .iter()
            .filter(|item| item.rarity != Rarity::Three)
            .collect();
        items.sort_by(|a, b| b.rarity.cmp(&a.rarity).then_with(|| a.name.cmp(&b.name)));
        let options: Vec<PickerItem<String>> = items.into_iter().map(item_option).collect();
        Self {
            pool,
            item: cx.new(|cx| PickerState::searchable(options, None, window, cx)),
            pity: cx
                .new(|cx| InputState::new(window, cx).placeholder("Counted from earlier wishes")),
            data,
            error: None,
        }
    }

    /// The chosen item and its pity, or `None` after showing what is wrong.
    fn submit(&mut self, cx: &mut Context<Self>) -> Option<(Item, Option<u32>)> {
        let result = self.read(cx);
        self.error = result.as_ref().err().cloned();
        cx.notify();
        result.ok()
    }

    fn read(&self, cx: &Context<Self>) -> Result<(Item, Option<u32>), SharedString> {
        let Some(item) = self
            .item
            .read(cx)
            .selected_value()
            .and_then(|key| self.data.items.by_key(key))
        else {
            return Err("Choose the item you got.".into());
        };
        let text = self.pity.read(cx).value();
        let text = text.trim();
        if text.is_empty() {
            return Ok((item.clone(), None));
        }
        let most = match item.rarity {
            Rarity::Five => self.pool.hard_pity_five(),
            _ => self.pool.hard_pity_four(),
        };
        match text.parse::<u32>() {
            Ok(pity) if (1..=most).contains(&pity) => Ok((item.clone(), Some(pity))),
            _ => Err(format!(
                "Pity is a whole number from 1 to {most}. Leave it empty to count it."
            )
            .into()),
        }
    }
}

impl Render for AddItemForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let danger = cx.theme().danger;
        div()
            .v_flex()
            .gap_3()
            .child(
                v_form()
                    .child(
                        field().label("Item").child(
                            picker("add-item", &self.item)
                                .placeholder("Choose an item")
                                .accessibility_label("Item")
                                .menu_width(rems(24.)),
                        ),
                    )
                    .child(
                        field()
                            .label("Pity")
                            .description("Leave empty to count it from the wishes before it.")
                            .child(input(&self.pity, cx)),
                    ),
            )
            .when_some(self.error.clone(), |this, error| {
                this.child(div().text_sm().text_color(danger).child(error))
            })
    }
}

/// An entry of the item picker: "Name · 5★ character", standing for its key.
fn item_option(item: &Item) -> PickerItem<String> {
    PickerItem::new(
        format!(
            "{} · {}★ {}",
            item.name,
            item.rarity.stars(),
            match item.item_type {
                ItemType::Character => "character",
                ItemType::Weapon => "weapon",
                ItemType::Unknown => "item",
            }
        ),
        item.key.clone(),
    )
}
