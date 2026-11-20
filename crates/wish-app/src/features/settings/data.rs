//! Settings → Your data: the data folder, its backups, and the mirror folder,
//! with what was wrong with `settings.json` when it didn't read.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{App, Context, Div, ParentElement as _, Styled as _, div};
use wish_core::store::{KEEP_BACKUPS, KEEP_HOURLY_BACKUPS};
use wish_ui::{ButtonExt as _, button, format};

use super::{SettingsPage, group, help, row};
use crate::app::library::MirrorStatus;

impl SettingsPage {
    pub(super) fn data_group(&self, cx: &mut Context<Self>) -> Div {
        let library = self.library.read(cx);
        let busy = library.is_busy();
        let root = library.paths().root().to_path_buf();
        let backups = library.backups();
        let mirror_folder = library.settings(cx).mirror_folder.clone();
        let settings_warning = library.settings_warning(cx).cloned();
        let backup_line = match backups.newest {
            Some(newest) => t!(
                "settings-backups-summary",
                count = backups.count,
                latest = format::date_at_time(&newest),
                kept = KEEP_BACKUPS,
                kept_hourly = KEEP_HOURLY_BACKUPS,
            ),
            None => t!("settings-backups-none"),
        };
        let mirror_status = match (&mirror_folder, library.mirror()) {
            (None, _) => None,
            (Some(_), Some(MirrorStatus::Written(at))) => Some((
                t!("settings-mirror-written", time = format::time(at)),
                false,
            )),
            (Some(_), Some(MirrorStatus::Failed(error))) => Some((
                t!("settings-mirror-failed", error = error.to_string()),
                true,
            )),
            (Some(_), None) => Some((t!("settings-mirror-pending"), false)),
        };
        let (muted, danger) = (cx.theme().muted_foreground, cx.theme().danger);

        group(
            t!("settings-group-data"),
            vec![
                row(
                    t!("settings-data-folder"),
                    div()
                        .v_flex()
                        .gap_0p5()
                        .child(path_text(root.display().to_string(), cx))
                        .child(help(t!("settings-data-folder-help"), cx))
                        .when_some(settings_warning, |this, warning| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().warning)
                                    .child(format!("{warning}.")),
                            )
                        }),
                    button("reveal-data-folder")
                        .label(t!("settings-reveal"))
                        .on_click(move |_, _, cx| cx.reveal_path(&root)),
                ),
                row(
                    t!("settings-backups"),
                    help(backup_line, cx),
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            button("back-up-now")
                                .label(t!("settings-back-up-now"))
                                .loading(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.back_up(cx))),
                        )
                        .child(
                            button("restore-backup")
                                .label(t!("settings-restore"))
                                .unavailable(busy)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.open_backups(window, cx)
                                    }),
                                ),
                        ),
                ),
                row(
                    t!("settings-mirror-folder"),
                    div()
                        .v_flex()
                        .gap_0p5()
                        .when_some(mirror_folder.clone(), |this, folder| {
                            this.child(path_text(folder.display().to_string(), cx))
                        })
                        .child(help(t!("settings-mirror-help"), cx))
                        .when_some(mirror_status, |this, (line, failed)| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(if failed { danger } else { muted })
                                    .child(line),
                            )
                        }),
                    div()
                        .h_flex()
                        .gap_2()
                        .when(mirror_folder.is_some(), |this| {
                            this.child(
                                button("stop-mirroring")
                                    .quiet(cx)
                                    .label(t!("settings-mirror-stop"))
                                    .unavailable(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_mirror_folder(None, cx)
                                    })),
                            )
                        })
                        .child(
                            button("choose-mirror-folder")
                                .label(t!("settings-mirror-choose"))
                                .unavailable(busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.choose_mirror_folder(window, cx)
                                })),
                        ),
                ),
            ],
            cx,
        )
    }
}

/// A path on its own line, in the monospace font, cut short when too long.
fn path_text(path: String, cx: &App) -> Div {
    div()
        .text_sm()
        .font_family(cx.theme().mono_font_family.clone())
        .truncate()
        .child(path)
}
