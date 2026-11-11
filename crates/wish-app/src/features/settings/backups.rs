//! Settings → Backups: back up now, and restore one (or a file kept elsewhere).

use crate::app::notices::{Notice, notify};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    Context, ParentElement as _, PathPromptOptions, SharedString, Styled as _, Window, div,
};
use wish_ui::{button, dialog_footer};

use std::path::PathBuf;

use gpui_kit::InteractiveElement as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use wish_core::store::BackupReason;

use super::SettingsPage;

impl SettingsPage {
    pub(super) fn back_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = self.library.update(cx, |library, cx| library.back_up(cx));
        let flow = cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |_, _, cx| {
                let note = match result {
                    Ok(_) => Notice::done("Backup saved."),
                    Err(err) => Notice::failed(format!("Couldn’t back up: {err:#}")),
                };
                notify(note, cx);
            })
            .ok();
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }

    /// Lists the backups to restore one, or a backup file kept elsewhere.
    pub(super) fn open_backups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        let files = self.library.read(cx).backups().files.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let muted = cx.theme().muted_foreground;
            let rows = files.iter().map(|file| {
                let (shell, path) = (shell.clone(), file.path.clone());
                let when = file.taken_at.format("%-d %b %Y, %H:%M:%S").to_string();
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
                    .child(button(id).label("Restore…").on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        shell
                            .update(cx, |shell, cx| {
                                shell.confirm_restore(
                                    path.clone(),
                                    format!("the backup from {when}"),
                                    window,
                                    cx,
                                )
                            })
                            .ok();
                    }))
            });
            let choose = {
                let shell = shell.clone();
                button("restore-file")
                    .label("Choose file…")
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        shell
                            .update(cx, |shell, cx| shell.choose_backup_file(window, cx))
                            .ok();
                    })
            };
            dialog.title("Restore a backup").child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(div().text_sm().text_color(muted).child(
                        "Restoring replaces everything with what the backup holds: your \
                             wishes, and the planner's plan and inventory. The current state is \
                             backed up first.",
                    ))
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
                                .label("Close")
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
            prompt: Some(SharedString::from("Restore")),
        });
        let flow = cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(mut files))) = picked.await
                && let Some(file) = files.pop()
            {
                let name = format!("“{}”", crate::app::library::file_name(&file));
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
        let title = SharedString::from(format!("Restore {what}?"));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, file) = (shell.clone(), file.clone());
            alert
                .title(title.clone())
                .description(
                    "Everything is replaced by what the backup holds: your wishes, and the \
                     planner's plan and inventory too. The current state is backed up first, so \
                     you can restore it if needed.",
                )
                .footer(dialog_footer(button("confirm").danger().label("Restore")))
                .on_ok(move |_, window, cx| {
                    shell
                        .update(cx, |shell, cx| {
                            shell.restore_backup(file.clone(), window, cx)
                        })
                        .ok();
                    true
                })
        });
    }

    fn restore_backup(&mut self, file: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let task = self
            .library
            .update(cx, |library, cx| library.restore_backup(file, cx));
        let flow = cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |_, _, cx| {
                let note = match result {
                    Ok(_) => Notice::done("Backup restored. The previous state was backed up."),
                    Err(err) => Notice::failed(format!("Couldn’t restore: {err:#}")),
                };
                notify(note, cx);
            })
            .ok();
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }
}

fn backup_reason(reason: Option<BackupReason>) -> &'static str {
    match reason {
        Some(BackupReason::Daily) => "Daily backup",
        Some(BackupReason::BeforeImport) => "Before an import",
        Some(BackupReason::BeforeMigration) => "Before an app update",
        Some(BackupReason::Manual) => "Backed up by hand",
        Some(BackupReason::BeforeUndo) => "Before undoing an import",
        Some(BackupReason::BeforeRestore) => "Before restoring a backup",
        Some(BackupReason::BeforeDelete) => "Before deleting wishes",
        Some(BackupReason::BeforeCounting) => "Before counting wishes",
        Some(BackupReason::BeforePlanning) => "Before changing the plan",
        None => "Backup",
    }
}
