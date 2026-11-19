//! Settings → Accounts: each account with its UID, server and wishes; add
//! one by hand (a player who imports no wishes, a second account), rename
//! one, give one without a UID its UID or its server, and remove one that
//! has no wishes.
//!
//! A UID, once an account has it, stays: the game import and HoYoLAB find
//! the account by it (`Store::identify_account`). The server follows the
//! UID; without one, the player may say it, so that the banners' times and
//! the planner's game day are the server's.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};
use wish_core::model::{Account, AccountId, Server};
use wish_ui::{
    ButtonExt as _, PickerItem, PickerState, button, confirm_destructive, dialog_footer,
    form_dialog, format, input, picker,
};

use super::{SettingsPage, group, help, row};
use crate::app::library::AccountSummary;
use crate::app::notices::{Notice, notify, report, report_outcome};

/// The form's width, in rems.
const FORM_REMS: f32 = 30.;

impl SettingsPage {
    pub(super) fn accounts_group(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let library = self.library.read(cx);
        let busy = library.is_busy();
        let accounts: Vec<(AccountSummary, usize)> = (library.accounts().iter())
            .map(|summary| {
                let characters = library.owned(Some(summary.account.id)).characters.len();
                (summary.clone(), characters)
            })
            .collect();
        let mut rows: Vec<AnyElement> = (accounts.into_iter())
            .map(|(summary, characters)| {
                let account = summary.account.clone();
                let wishes = summary.analysis.totals().wishes;
                let detail = [
                    match &account.uid {
                        Some(uid) => t!("settings-account-uid", uid = uid.as_str()),
                        None => t!("settings-account-no-uid"),
                    },
                    (account.server)
                        .map_or_else(|| t!("settings-account-no-server"), format::server),
                    t!("settings-account-wishes", count = wishes),
                    t!("settings-account-characters", count = characters),
                ]
                .join(" · ");
                let id = account.id.0;
                let edited = account.clone();
                let removed = account.clone();
                row(
                    account.label,
                    help(detail, cx),
                    div()
                        .h_flex()
                        .gap_2()
                        // Only an account without wishes can go.
                        .when(wishes == 0, |this| {
                            this.child(
                                button(SharedString::from(format!("remove-account-{id}")))
                                    .ghost()
                                    .label(t!("settings-account-remove"))
                                    .unavailable(busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.confirm_remove_account(removed.clone(), window, cx)
                                    })),
                            )
                        })
                        .child(
                            button(SharedString::from(format!("edit-account-{id}")))
                                .label(t!("settings-account-edit"))
                                .unavailable(busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_account(Some(edited.clone()), window, cx)
                                })),
                        ),
                )
            })
            .collect();
        rows.push(row(
            t!("settings-account-add-title"),
            help(t!("settings-account-add-help"), cx),
            button("add-account")
                .label(t!("settings-account-add"))
                .unavailable(busy)
                .on_click(cx.listener(|this, _, window, cx| this.open_account(None, window, cx))),
        ));
        group(t!("settings-group-accounts"), rows, cx)
    }

    /// Opens the form for a new account, or to edit `account`.
    pub(crate) fn open_account(
        &mut self,
        account: Option<Account>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let others: Vec<(String, String)> = (self.library.read(cx).accounts().iter())
            .filter(|summary| Some(summary.account.id) != account.as_ref().map(|a| a.id))
            .filter_map(|summary| {
                let account = &summary.account;
                Some((account.uid.clone()?, account.label.clone()))
            })
            .collect();
        let title = match &account {
            Some(_) => t!("settings-account-edit-title"),
            None => t!("settings-account-add-title"),
        };
        let editing = account.is_some();
        let form = cx.new(|cx| AccountForm::new(account, others, window, cx));
        let page = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let (form, page) = (form.clone(), page.clone());
            form_dialog(dialog)
                .title(title.clone())
                .w(window.rem_size() * FORM_REMS)
                .child(form.clone())
                .footer(dialog_footer(button("save-account").primary().label(
                    if editing {
                        t!("settings-account-save")
                    } else {
                        t!("settings-account-add-button")
                    },
                )))
                .on_ok(move |_, _, cx| {
                    let Some(change) = form.update(cx, |form, cx| form.submit(cx)) else {
                        // Keep the form open on its error.
                        return false;
                    };
                    page.update(cx, |page, cx| page.save_account(change, cx))
                        .ok();
                    true
                })
        });
    }

    fn save_account(&mut self, change: AccountChange, cx: &mut Context<Self>) {
        match change {
            AccountChange::Add { label, uid, server } => {
                let session = self.session.clone();
                let task = self.library.update(cx, |library, cx| {
                    library.add_account(label.clone(), uid, server, cx)
                });
                cx.spawn(async move |_, cx| match task.await {
                    // The new account is shown, as an import's is.
                    Ok(account) => {
                        session.update(cx, |session, cx| session.select_account(account.id, cx));
                        cx.update(|cx| {
                            notify(
                                Notice::done(t!(
                                    "settings-account-added",
                                    account = format::quoted(&label)
                                )),
                                cx,
                            )
                        });
                    }
                    Err(error) => {
                        cx.update(|cx| {
                            notify(
                                Notice::failure(t!("settings-account-add-failed"), &error),
                                cx,
                            )
                        });
                    }
                })
                .detach();
            }
            AccountChange::Edit {
                account,
                label,
                identity,
            } => {
                let task = self.library.update(cx, |library, cx| {
                    library.edit_account(account, label, identity, cx)
                });
                report(task, t!("settings-account-edit-failed"), cx);
            }
        }
    }

    fn confirm_remove_account(
        &mut self,
        account: Account,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.entity().downgrade();
        let name = format::quoted(&account.label);
        let title = t!("settings-account-remove-title", account = name.as_str());
        confirm_destructive(
            window,
            cx,
            title,
            t!("settings-account-remove-help"),
            t!("settings-account-remove-button"),
            move |_, cx| {
                let name = name.clone();
                page.update(cx, |page, cx| {
                    let task = (page.library)
                        .update(cx, |library, cx| library.remove_account(account.id, cx));
                    report_outcome(
                        task,
                        t!("settings-account-remove-failed"),
                        move |()| t!("settings-account-removed", account = name.as_str()),
                        cx,
                    );
                })
                .ok();
            },
        );
    }
}

/// What the form asks the library to do.
enum AccountChange {
    Add {
        label: String,
        uid: Option<String>,
        server: Option<Server>,
    },
    Edit {
        account: AccountId,
        label: String,
        /// The UID and server, for an account without a UID.
        identity: Option<(Option<String>, Option<Server>)>,
    },
}

/// The form: a name, and the UID and server while the account has no UID.
pub(crate) struct AccountForm {
    /// The account edited; `None` for a new one.
    editing: Option<Account>,
    name: Entity<InputState>,
    uid: Entity<InputState>,
    server: Entity<PickerState<Option<Server>>>,
    /// The other accounts' UIDs and names: a UID one has is refused here,
    /// before the store would.
    others: Vec<(String, String)>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl AccountForm {
    fn new(
        editing: Option<Account>,
        others: Vec<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let label = editing
            .as_ref()
            .map(|a| a.label.clone())
            .unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(label));
        let uid = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("settings-account-uid-placeholder"))
        });
        let chosen = editing.as_ref().and_then(|a| a.server);
        let server = cx.new(|cx| {
            let mut state = PickerState::new(server_options(), Some(0), cx);
            state.set_selected_value(&chosen, cx);
            state
        });
        // A UID says its server.
        let subscriptions = vec![cx.subscribe(&uid, |this, uid, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            if let Some(server) = Server::from_uid(uid.read(cx).value().trim()) {
                (this.server).update(cx, |state, cx| state.set_selected_value(&Some(server), cx));
            }
        })];
        name.update(cx, |state, cx| state.focus(window, cx));
        Self {
            editing,
            name,
            uid,
            server,
            others,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// Whether the UID and server can still be given.
    fn open_identity(&self) -> bool {
        self.editing
            .as_ref()
            .is_none_or(|account| account.uid.is_none())
    }

    /// The change asked for, or `None` with the reason shown.
    fn submit(&mut self, cx: &mut Context<Self>) -> Option<AccountChange> {
        let label = self.name.read(cx).value().trim().to_owned();
        let uid = self.uid.read(cx).value().trim().to_owned();
        let uid = (!uid.is_empty()).then_some(uid);
        let server = self.server.read(cx).selected_value().copied().flatten();
        let problem = if label.is_empty() {
            Some(wish_core::Error::EmptyAccountName.to_string())
        } else if let Some(uid) = uid.as_ref().filter(|_| self.open_identity()) {
            let valid = (9..=10).contains(&uid.len()) && uid.bytes().all(|b| b.is_ascii_digit());
            if !valid {
                Some(wish_core::Error::InvalidUid(uid.clone()).to_string())
            } else {
                (self.others.iter())
                    .find(|(taken, _)| taken == uid)
                    .map(|(_, account)| {
                        wish_core::Error::UidTaken {
                            uid: uid.clone(),
                            account: account.clone(),
                        }
                        .to_string()
                    })
            }
        } else {
            None
        };
        if let Some(problem) = problem {
            self.error = Some(format::capitalize(&problem).into());
            cx.notify();
            return None;
        }
        Some(match &self.editing {
            None => AccountChange::Add { label, uid, server },
            Some(account) => AccountChange::Edit {
                account: account.id,
                label,
                identity: (account.uid.is_none()).then_some((uid, server)),
            },
        })
    }
}

impl Render for AccountForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let field = |label: String, control: AnyElement| {
            div()
                .v_flex()
                .gap_1p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(control)
        };
        let identity: AnyElement = match self.editing.as_ref().and_then(|a| a.uid.clone()) {
            // It has its UID: shown, not edited.
            Some(uid) => {
                let server = (self.editing.as_ref().and_then(|a| a.server))
                    .map_or_else(|| t!("settings-account-no-server"), format::server);
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_sm().child(format!(
                        "{} · {server}",
                        t!("settings-account-uid", uid = uid.as_str())
                    )))
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(t!("settings-account-uid-kept")),
                    )
                    .into_any_element()
            }
            None => div()
                .v_flex()
                .gap_4()
                .child(field(
                    t!("settings-account-uid-label"),
                    input(&self.uid, cx).into_any_element(),
                ))
                .child(field(
                    t!("settings-account-server"),
                    picker("account-server", &self.server)
                        .accessibility_label(t!("settings-account-server"))
                        .into_any_element(),
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(t!("settings-account-identity-help")),
                )
                .into_any_element(),
        };
        let danger = theme.danger;
        div()
            .v_flex()
            .gap_4()
            .child(field(
                t!("settings-account-name"),
                input(&self.name, cx).into_any_element(),
            ))
            .child(identity)
            .when_some(self.error.clone(), |this, error| {
                this.child(div().text_sm().text_color(danger).child(error))
            })
    }
}

/// The servers to choose from: none said first, then the game's.
fn server_options() -> Vec<PickerItem<Option<Server>>> {
    std::iter::once(PickerItem::new(t!("settings-account-server-unknown"), None))
        .chain(
            [
                Server::America,
                Server::Europe,
                Server::Asia,
                Server::TwHkMo,
                Server::China,
            ]
            .map(|server| PickerItem::new(format::server(server), Some(server))),
        )
        .collect()
}
