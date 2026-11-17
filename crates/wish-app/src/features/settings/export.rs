//! Settings → Export: snapshot, UIGF and spreadsheet files.

use crate::app::notices::report_outcome;

use gpui_kit::{Context, Window};

use super::SettingsPage;
use crate::app::library::ExportKind;

impl SettingsPage {
    pub(super) fn export(&mut self, kind: ExportKind, window: &mut Window, cx: &mut Context<Self>) {
        let account = self.session.read(cx).account();
        let library = self.library.read(cx);
        let date = chrono::Local::now().format("%Y-%m-%d");
        let name = match kind {
            ExportKind::Snapshot => format!("genshin-companion-{date}.json"),
            ExportKind::Uigf => format!("genshin-companion-uigf-{date}.json"),
            ExportKind::Spreadsheet => {
                let label = account
                    .and_then(|id| library.account(id))
                    .map(|a| a.account.label.replace(['/', '\\', ':'], "-"))
                    .unwrap_or_default();
                format!("genshin-companion-{label}-{date}.xlsx")
            }
        };
        let folder = std::env::home_dir().unwrap_or_else(|| library.paths().root().to_path_buf());
        let picked = cx.prompt_for_new_path(&folder, Some(&name));
        let flow = cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            // Once the file is chosen, the export is reported whatever the
            // window does next.
            this.update(cx, |this, cx| {
                let task = (this.library)
                    .update(cx, |library, cx| library.export(kind, path, account, cx));
                report_outcome(
                    task,
                    t!("settings-export-failed"),
                    |summary| summary.to_string(),
                    cx,
                );
            })
            .ok();
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }
}
