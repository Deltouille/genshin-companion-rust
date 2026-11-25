//! Settings → HoYoLAB: whether the system keyring keeps a HoYoLAB login,
//! which accounts it syncs the chronicle of on its own and when it last did,
//! and the buttons to sync now and to forget it.

use gpui_kit::base::StyledExt as _;
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement as _, Styled as _, div};
use wish_ui::{ButtonExt as _, button, format};

use super::{SettingsPage, group, help, row};
use crate::app::library::KeptLogin;
use crate::app::notices::report;
use crate::features::imports::hoyolab_sync_button;

impl SettingsPage {
    pub(super) fn hoyolab_group(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let library = self.library.read(cx);
        let (kept, syncing) = (library.kept_login(), library.is_syncing_chronicle());
        let link = library.settings(cx).hoyolab.clone();
        let busy = library.is_busy();
        // Whether an account here has one of the login's UIDs: the chronicle
        // goes there only.
        let linked = (library.accounts().iter()).any(|summary| {
            (summary.account.uid.as_ref()).is_some_and(|uid| link.uids.contains(uid))
        });
        let row: AnyElement = match kept {
            KeptLogin::Kept => {
                let uids = if link.uids.is_empty() {
                    t!("settings-hoyolab-no-uid")
                } else {
                    link.uids.join(", ")
                };
                let last = link.synced_at.map_or_else(
                    || t!("settings-hoyolab-never"),
                    |at| format::date_at_time(&at.with_timezone(&chrono::Local)),
                );
                let mut detail = t!("settings-hoyolab-kept-help", uids = uids, last = last);
                if !linked && !link.uids.is_empty() {
                    detail = format!("{detail} {}", t!("settings-hoyolab-unlinked"));
                }
                row(
                    t!("settings-hoyolab-kept"),
                    help(detail, cx),
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            button("sync-chronicle-now")
                                .label(t!("settings-hoyolab-sync-now"))
                                .loading(syncing)
                                .unavailable(syncing || busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let task = this
                                        .library
                                        .update(cx, |library, cx| library.sync_chronicle(true, cx));
                                    // Said by the library, success or failure.
                                    task.detach();
                                })),
                        )
                        .child(
                            button("forget-login")
                                .quiet(cx)
                                .label(t!("settings-hoyolab-forget"))
                                .unavailable(syncing)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let task = this
                                        .library
                                        .update(cx, |library, cx| library.forget_login(cx));
                                    report(task, t!("app-forget-login-failed"), cx);
                                })),
                        ),
                )
            }
            KeptLogin::None => row(
                t!("settings-hoyolab-none"),
                help(t!("settings-hoyolab-none-help"), cx),
                hoyolab_sync_button(),
            ),
            KeptLogin::Unavailable => row(
                t!("settings-hoyolab-none"),
                help(t!("settings-hoyolab-unavailable"), cx),
                div(),
            ),
            KeptLogin::Checking => row(t!("settings-hoyolab-checking"), div(), div()),
        };
        group(
            t!("settings-group-hoyolab"),
            vec![row.into_any_element()],
            cx,
        )
    }
}
