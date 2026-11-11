//! Settings → Recent imports: undo one after confirming.

use crate::app::notices::{Notice, notify};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::{Context, SharedString, Window};

use wish_core::import::{ImportRecord, ImportSource};

use super::SettingsPage;
use wish_ui::{button, dialog_footer, format};

impl SettingsPage {
    pub(super) fn confirm_undo(
        &mut self,
        record: ImportRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let shell = cx.entity().downgrade();
        let title = SharedString::from(match record.source {
            ImportSource::Api => format!(
                "Undo the game import of {}?",
                local_time(&record.imported_at)
            ),
            ImportSource::Manual => format!(
                "Undo {}, counted by hand?",
                record.detail.as_deref().unwrap_or("the wishes")
            ),
            _ => format!("Undo importing {}?", import_origin(&record)),
        });
        let description = SharedString::from(format!(
            "This removes the {} {} it added to {}. A backup is taken first.",
            format::count(record.added),
            format::plural(record.added, "wish", "wishes"),
            record.account_label,
        ));
        let import_id = record.id;
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            alert
                .title(title.clone())
                .description(description.clone())
                .footer(dialog_footer(
                    button("confirm").danger().label("Undo import"),
                ))
                .on_ok(move |_, window, cx| {
                    shell
                        .update(cx, |shell, cx| shell.undo_import(import_id, window, cx))
                        .ok();
                    true
                })
        });
    }

    pub(super) fn undo_import(
        &mut self,
        import_id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = self
            .library
            .update(cx, |library, cx| library.undo_import(import_id, cx));
        let flow = cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |_, _, cx| {
                let note = match result {
                    Ok(report) => Notice::done(format!(
                        "Removed {} {}.",
                        format::count(report.removed_wishes),
                        format::plural(report.removed_wishes, "wish", "wishes")
                    )),
                    Err(err) => Notice::failed(format!("Couldn’t undo the import: {err:#}")),
                };
                notify(note, cx);
            })
            .ok();
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }
}

/// What an import came from: the file's name, or the game.
pub(super) fn import_origin(record: &ImportRecord) -> String {
    match (record.source, &record.detail) {
        (ImportSource::Api, _) => "The game".to_owned(),
        (ImportSource::Manual, Some(what)) => format!("Counted by hand: {what}"),
        (ImportSource::Manual, None) => "Counted by hand".to_owned(),
        (_, Some(file)) => format!("“{file}”"),
        (_, None) => "A file".to_owned(),
    }
}

/// `2026-11-09T12:20:00+01:00` → `9 Nov 2026, 12:20`.
pub(super) fn local_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|time| time.format("%-d %b %Y, %H:%M").to_string())
        .unwrap_or_else(|_| rfc3339.to_owned())
}
