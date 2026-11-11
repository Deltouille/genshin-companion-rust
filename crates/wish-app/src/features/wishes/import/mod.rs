//! Import flow: pick a file, read and check it in the background, preview
//! it, and write it only once the user confirms.

mod game;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::app::notices::{Notice, notify};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, IconName, WindowExt as _};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, FontWeight, IntoElement,
    ParentElement as _, PathPromptOptions, SharedString, Styled as _, Subscription, Window, div,
};
use wish_core::import::{ImportBatch, ImportReport, PreparedImport};
use wish_core::model::Pool;

use crate::app::library::{LoadState, file_name};
use crate::app::{Library, Session};
use crate::shell::{ImportFile, ImportFromGame};
use game::{GameImport, GameImportEvent};
use wish_ui::{button, dialog_close_footer, dialog_footer, format};

/// The import flow, started by the Import buttons, the menu and Ctrl/Cmd+O
/// (the [`ImportFile`] and [`ImportFromGame`] actions, which the shell
/// forwards here).
///
/// A service rather than a page: it has no view of its own and opens its
/// dialogs over whatever page is shown, so the shell owns it, not a page.
/// It holds the library (to read and write) and the session (the account a
/// spreadsheet without a UID goes to, the account to show once imported,
/// and the one flow in progress).
pub struct ImportFlow {
    library: Entity<Library>,
    session: Entity<Session>,
    /// Events of the open "Import from the game" dialog.
    game_import_events: Option<Subscription>,
}

impl ImportFlow {
    pub fn new(session: Entity<Session>, cx: &App) -> Self {
        Self {
            library: session.read(cx).library().clone(),
            session,
            game_import_events: None,
        }
    }

    /// Asks for a file, reads and checks it in the background, then previews it.
    pub fn import_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let library = self.library.read(cx);
        if !matches!(library.state(), LoadState::Ready) || library.is_busy() {
            return;
        }
        let target = self.session.read(cx).account();
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from("Import")),
        });
        let flow = cx.spawn_in(window, async move |this, cx| {
            let path = match picked.await {
                Ok(Ok(Some(mut paths))) => paths.pop(),
                Ok(Err(err)) => {
                    this.update_in(cx, |_, _, cx| {
                        let message = format!("Couldn’t open the file picker: {err:#}");
                        notify(Notice::failed(message), cx);
                    })
                    .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            let Some(path) = path else { return };
            let name = file_name(&path);

            let Ok(prepare) = this.update(cx, |this, cx| {
                this.library
                    .update(cx, |library, cx| library.prepare_import(path, target, cx))
            }) else {
                return;
            };
            let prepared = prepare.await;
            this.update_in(cx, |this, window, cx| match prepared {
                Ok(prepared) => this.open_preview(name, prepared, window, cx),
                Err(err) => open_message(
                    format!("Couldn’t read “{name}”"),
                    format!("{err:#}"),
                    window,
                    cx,
                ),
            })
            .ok();
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }

    fn open_preview(
        &mut self,
        name: String,
        prepared: PreparedImport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let import = cx.entity().downgrade();
        let prepared = Rc::new(prepared);
        let title = SharedString::from(format!("Import “{name}”?"));
        window.open_dialog(cx, move |dialog, _, cx| {
            let on_ok = {
                let (import, prepared) = (import.clone(), prepared.clone());
                move |_: &_, window: &mut Window, cx: &mut App| {
                    let batch = prepared.batch.clone();
                    import
                        .update(cx, |import, cx| import.apply_import(batch, window, cx))
                        .ok();
                    true
                }
            };
            dialog
                .title(title.clone())
                .child(preview(&prepared, cx))
                .footer(dialog_footer(
                    button("confirm-import").primary().label("Import"),
                ))
                .on_ok(on_ok)
        });
    }

    /// Opens "Import from the game" (find or paste the link, fetch, preview).
    pub fn import_from_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.library.read(cx).state(), LoadState::Ready) {
            return;
        }
        let view = cx.new(|cx| GameImport::new(self.library.clone(), window, cx));
        self.game_import_events =
            Some(
                cx.subscribe_in(&view, window, |this, _, event, window, cx| {
                    let GameImportEvent::Imported(report) = event;
                    this.show_report("from the game", report, window, cx);
                }),
            );

        let input = view.read(cx).input().clone();
        let focus_once = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if focus_once.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            dialog.title("Import from the game").child(view.clone())
        });
    }

    fn apply_import(&mut self, batch: ImportBatch, window: &mut Window, cx: &mut Context<Self>) {
        let name = batch.detail.clone().unwrap_or_default();
        let task = self
            .library
            .update(cx, |library, cx| library.apply_import(batch, cx));
        let flow = cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(report) => this.show_report(&format!("in “{name}”"), &report, window, cx),
                Err(err) => open_message(
                    format!("Couldn’t import “{name}”"),
                    format!("{err:#}\n\nNothing was changed."),
                    window,
                    cx,
                ),
            })
            .ok();
        });
        self.session
            .update(cx, |session, _| session.start_flow(flow));
    }

    /// Reports a finished import. `source` completes "every wish … was
    /// already stored", e.g. `in “save.json”` or `from the game`.
    fn show_report(
        &mut self,
        source: &str,
        report: &ImportReport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Show what was just imported.
        if let Some(first) = report.accounts.first() {
            let id = first.account.id;
            self.session
                .update(cx, |session, cx| session.select_account(id, cx));
        }

        let added = report.added();
        let linked: u32 = report.accounts.iter().map(|a| a.linked).sum();
        let headline = if added == 0 && linked == 0 {
            format!("Nothing new: every wish {source} was already stored.")
        } else if added == 0 {
            format!(
                "No new wishes. {} stored {} matched the game’s records.",
                format::count(linked),
                format::plural(linked, "wish", "wishes")
            )
        } else {
            format!(
                "Added {} new {}.",
                format::count(added),
                format::plural(added, "wish", "wishes")
            )
        };
        if report.warnings.is_empty() {
            notify(Notice::done(headline), cx);
        } else {
            open_message(headline, report.warnings.join("\n\n"), window, cx);
        }
    }
}

/// What the file holds, before anything is written.
fn preview(prepared: &PreparedImport, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let accounts = prepared.batch.accounts.iter().map(|account| {
        let mut per_pool: BTreeMap<Pool, u32> = BTreeMap::new();
        for wish in &account.wishes {
            *per_pool.entry(wish.pool()).or_default() += 1;
        }
        let pools = per_pool
            .iter()
            .map(|(pool, count)| format!("{} {}", pool.display_name(), format::count(*count)))
            .collect::<Vec<_>>()
            .join(" · ");
        let mut heading = account.label.clone();
        if let Some(uid) = &account.uid {
            heading = format!("{heading} · UID {uid}");
        }
        div()
            .v_flex()
            .gap_1()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(heading))
            .child(div().text_sm().child(pools))
    });

    let check = prepared.paimon_check.as_ref().map(|check| {
        let (text, color) = if check.matches() {
            (
                format!(
                    "Pity and 50/50 results match paimon.moe for all {} values it stored.",
                    format::count(check.checked)
                ),
                theme.muted_foreground,
            )
        } else {
            (
                format!(
                    "{} of {} values differ from what paimon.moe stored. This happens when \
                     paimon.moe’s banner data changed after your last import there. \
                     Your wishes are imported unchanged either way.",
                    format::count(check.differences.len() as u32),
                    format::count(check.checked)
                ),
                theme.warning,
            )
        };
        div().text_sm().text_color(color).child(text)
    });

    div()
        .v_flex()
        .gap_4()
        .children(accounts)
        .children(check)
        .children(prepared.batch.warnings.iter().map(|warning| {
            div()
                .text_sm()
                .text_color(theme.warning)
                .child(warning.clone())
        }))
        .child(
            div().text_sm().text_color(theme.muted_foreground).child(
                "Wishes that are already stored stay unchanged, and a backup is taken first.",
            ),
        )
}

/// A dialog that reports something and only needs acknowledging.
fn open_message(title: String, message: String, window: &mut Window, cx: &mut App) {
    let (title, message) = (SharedString::from(title), SharedString::from(message));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(title.clone())
            .child(div().text_sm().child(message.clone()))
            .footer(dialog_close_footer("OK"))
    });
}

/// "Import file…": sends the [`ImportFile`] action, so it works the same
/// from any page and from the menu.
pub fn import_file_button(busy: bool) -> Button {
    button("import")
        .label("Import file…")
        .icon(IconName::FolderOpen)
        .loading(busy)
        .on_click(|_, window, cx| window.dispatch_action(Box::new(ImportFile), cx))
}

/// "Import from game…": sends the [`ImportFromGame`] action.
pub fn import_game_button() -> Button {
    button("import-game")
        .label("Import from game…")
        .icon(IconName::RefreshCw)
        .on_click(|_, window, cx| window.dispatch_action(Box::new(ImportFromGame), cx))
}
