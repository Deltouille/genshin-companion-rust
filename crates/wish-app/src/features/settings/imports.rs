//! Settings → Recent imports: the latest imports, and undoing one after
//! confirming.

use crate::app::notices::report_outcome;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, Context, Div, ParentElement as _, SharedString, Window, div};

use wish_core::import::{ImportRecord, ImportSource};

use super::{SettingsPage, group, help, row};
use wish_ui::{ButtonExt as _, button, dialog_footer, format};

impl SettingsPage {
    /// The latest imports, newest first, each with Undo while it has
    /// something to take out.
    pub(super) fn imports_group(&self, cx: &mut Context<Self>) -> Div {
        let library = self.library.read(cx);
        let busy = library.is_busy();
        let imports = library.imports().to_vec();
        let rows: Vec<AnyElement> = if imports.is_empty() {
            vec![row(t!("settings-imports-none"), div(), div())]
        } else {
            imports
                .into_iter()
                .map(|record| {
                    let undoable = record.undone_at.is_none() && record.added > 0;
                    let status = if record.undone_at.is_some() {
                        t!("settings-import-undone")
                    } else {
                        t!("settings-import-added", count = record.added)
                    };
                    let id = SharedString::from(format!("undo-import-{}", record.id));
                    row(
                        format!("{} · {}", import_origin(&record), record.account_label),
                        help(
                            format!("{} · {status}", local_time(&record.imported_at)),
                            cx,
                        ),
                        div().when(undoable, |this| {
                            this.child(
                                button(id)
                                    .label(t!("settings-undo"))
                                    .unavailable(busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.confirm_undo(record.clone(), window, cx)
                                    })),
                            )
                        }),
                    )
                })
                .collect()
        };
        group(t!("settings-group-imports"), rows, cx)
    }

    pub(super) fn confirm_undo(
        &mut self,
        record: ImportRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let shell = cx.entity().downgrade();
        let title = SharedString::from(match record.source {
            ImportSource::Api => t!(
                "settings-undo-game-title",
                time = local_time(&record.imported_at)
            ),
            ImportSource::Manual => t!(
                "settings-undo-manual-title",
                what = record
                    .detail
                    .clone()
                    .unwrap_or_else(|| t!("settings-undo-manual-wishes"))
            ),
            _ => t!("settings-undo-file-title", origin = import_origin(&record)),
        });
        let description = SharedString::from(t!(
            "settings-undo-help",
            count = record.added,
            account = record.account_label.as_str(),
        ));
        let import_id = record.id;
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            alert
                .title(title.clone())
                .description(description.clone())
                .footer(dialog_footer(
                    button("confirm").danger().label(t!("settings-undo-button")),
                ))
                .on_ok(move |_, _, cx| {
                    shell
                        .update(cx, |shell, cx| shell.undo_import(import_id, cx))
                        .ok();
                    true
                })
        });
    }

    pub(super) fn undo_import(&mut self, import_id: i64, cx: &mut Context<Self>) {
        let task = self
            .library
            .update(cx, |library, cx| library.undo_import(import_id, cx));
        report_outcome(
            task,
            t!("settings-undo-failed"),
            |report| t!("settings-undo-done", count = report.removed_wishes),
            cx,
        );
    }
}

/// What an import came from: the file's name, or the game.
pub(super) fn import_origin(record: &ImportRecord) -> String {
    match (record.source, &record.detail) {
        (ImportSource::Api, _) => t!("settings-origin-game"),
        (ImportSource::Manual, Some(what)) => {
            t!("settings-origin-manual-what", what = what.as_str())
        }
        (ImportSource::Manual, None) => t!("settings-origin-manual"),
        (_, Some(file)) => format::quoted(file),
        (_, None) => t!("settings-origin-file"),
    }
}

/// `2026-11-09T12:20:00+01:00` → `9 Nov 2026, 12:20`.
pub(super) fn local_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|time| format::date_time(&time))
        .unwrap_or_else(|_| rfc3339.to_owned())
}
