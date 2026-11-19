//! Settings → Backups: back up now, and restore one (or a file kept elsewhere).

use crate::app::notices::report_outcome;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    Context, ParentElement as _, PathPromptOptions, SharedString, Styled as _, Window, div,
};
use wish_ui::{button, confirm_destructive, format};

use std::path::PathBuf;

use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use wish_core::store::BackupReason;

use super::SettingsPage;

impl SettingsPage {
    pub(super) fn back_up(&mut self, cx: &mut Context<Self>) {
        let task = self.library.update(cx, |library, cx| library.back_up(cx));
        report_outcome(
            task,
            t!("settings-backup-failed"),
            |_| t!("settings-backup-saved"),
            cx,
        );
    }

    /// Lists the backups to restore one, or a backup file kept elsewhere.
    pub(super) fn open_backups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        let files = self.library.read(cx).backups().files.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let muted = cx.theme().muted_foreground;
            let rows = files.iter().map(|file| {
                let (shell, path) = (shell.clone(), file.path.clone());
                let when = format::date_time_seconds(&file.taken_at);
                let id = SharedString::from(format!("restore-{}", file.path.display()));
                div()
                    .h_flex()
                    .gap_3()
                    .py_1()
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .child(div().text_sm().child(when.clone()))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child(backup_reason(file.reason)),
                            ),
                    )
                    .child(button(id).label(t!("settings-restore")).on_click(
                        move |_, window, cx| {
                            window.close_dialog(cx);
                            shell
                                .update(cx, |shell, cx| {
                                    shell.confirm_restore(
                                        path.clone(),
                                        t!("settings-restore-what-backup", when = when.as_str()),
                                        window,
                                        cx,
                                    )
                                })
                                .ok();
                        },
                    ))
            });
            let choose = {
                let shell = shell.clone();
                button("restore-file")
                    .label(t!("settings-restore-choose-file"))
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        shell
                            .update(cx, |shell, cx| shell.choose_backup_file(window, cx))
                            .ok();
                    })
            };
            dialog.title(t!("settings-restore-title")).child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(t!("settings-restore-dialog-help")),
                    )
                    .child(
                        div()
                            .id("backup-list")
                            .v_flex()
                            .max_h_96()
                            .overflow_y_scrollbar()
                            .children(rows),
                    )
                    .child(
                        div().h_flex().justify_between().child(choose).child(
                            button("close-backups")
                                .label(t!("app-close"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        ),
                    ),
            )
        });
    }

    fn choose_backup_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t!("settings-restore-prompt"))),
        });
        let flow = cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut files))) = picked.await
                && let Some(file) = files.pop()
            {
                let name = format::quoted(&crate::app::library::file_name(&file));
                this.update_in(cx, |this, window, cx| {
                    this.confirm_restore(file, name, window, cx)
                })
                .ok();
            }
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }

    fn confirm_restore(
        &mut self,
        file: PathBuf,
        what: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let shell = cx.entity().downgrade();
        confirm_destructive(
            window,
            cx,
            t!("settings-restore-confirm-title", what = what),
            t!("settings-restore-confirm-help"),
            t!("settings-restore-button"),
            move |_, cx| {
                shell
                    .update(cx, |shell, cx| shell.restore_backup(file.clone(), cx))
                    .ok();
            },
        );
    }

    fn restore_backup(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        let task = self
            .library
            .update(cx, |library, cx| library.restore_backup(file, cx));
        report_outcome(
            task,
            t!("settings-restore-failed"),
            |_| t!("settings-restore-done"),
            cx,
        );
    }
}

fn backup_reason(reason: Option<BackupReason>) -> String {
    match reason {
        Some(BackupReason::Daily) => t!("settings-backup-reason-daily"),
        Some(BackupReason::BeforeImport) => t!("settings-backup-reason-import"),
        Some(BackupReason::BeforeMigration) => t!("settings-backup-reason-migration"),
        Some(BackupReason::Manual) => t!("settings-backup-reason-manual"),
        Some(BackupReason::BeforeUndo) => t!("settings-backup-reason-undo"),
        Some(BackupReason::BeforeRestore) => t!("settings-backup-reason-restore"),
        Some(BackupReason::BeforeDelete) => t!("settings-backup-reason-delete"),
        Some(BackupReason::BeforeCounting) => t!("settings-backup-reason-counting"),
        Some(BackupReason::BeforePlanning) => t!("settings-backup-reason-planning"),
        Some(BackupReason::BeforeSync) => t!("settings-backup-reason-sync"),
        Some(BackupReason::BeforeRemovingAccount) => t!("settings-backup-reason-account"),
        None => t!("settings-backup-reason-unknown"),
    }
}
