//! Settings: where the data lives, its backups, and the mirror folder.

use crate::app::notices::{Notice, notify};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, rems,
};

mod backups;
mod export;
mod images;
mod imports;
mod mirror;

use crate::app::library::{ExportKind, MirrorStatus};
use crate::app::{Library, Session};
use imports::{import_origin, local_time};
use wish_core::store::{KEEP_BACKUPS, KEEP_HOURLY_BACKUPS};
use wish_ui::format;
use wish_ui::{ButtonExt as _, PageFrame, button, card};

/// A column of settings, in rems.
const COLUMN_REMS: f32 = 48.;
/// The room a page needs for two columns of settings side by side, in rems.
const TWO_COLUMNS_REMS: f32 = 84.;

/// The Settings page.
///
/// An entity rather than a plain component (like the overview) because it
/// starts flows (export, backup, restore, undo, mirror folder) whose tasks
/// outlive a render, and opens dialogs that call back into it. It holds:
///
/// - the library, to list the backups, imports and settings, and to run
///   the changes;
/// - the session, for the selected account (spreadsheet export) and to run
///   its flows as the window's one flow.
///
/// It redraws whenever either of them changes.
pub struct SettingsPage {
    library: Entity<Library>,
    session: Entity<Session>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub fn new(session: Entity<Session>, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let subscriptions = vec![
            cx.observe(&library, |_, _, cx| cx.notify()),
            cx.observe(&session, |_, _, cx| cx.notify()),
        ];
        Self {
            library,
            session,
            _subscriptions: subscriptions,
        }
    }

    /// Turns counting by hand on or off, and saves the setting.
    fn set_manual_counting(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let task = self
            .library
            .update(cx, |library, cx| library.set_manual_counting(on, cx));
        cx.spawn_in(window, async move |this, cx| {
            if let Err(err) = task.await {
                this.update_in(cx, |_, _, cx| {
                    let message = format!("Couldn’t save the setting: {err:#}");
                    notify(Notice::failed(message), cx);
                })
                .ok();
            }
        })
        .detach();
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_settings(window, cx)
    }
}

impl SettingsPage {
    fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let library = self.library.read(cx);
        let theme = cx.theme();
        let busy = library.is_busy();
        let root = library.paths().root().to_path_buf();
        let backups = library.backups();
        let mirror_folder = library.settings().mirror_folder.clone();
        let manual_counting = library.settings().manual_counting;
        let imports = library.imports().to_vec();

        let backup_line = match backups.newest {
            Some(newest) => format!(
                "{} {}, the latest on {}. Taken before every import, undo or restore and once \
                 a day (the {KEEP_BACKUPS} most recent are kept), and once an hour while counting \
                 by hand or editing the plan (the {KEEP_HOURLY_BACKUPS} most recent).",
                backups.count,
                format::plural(backups.count as u32, "backup", "backups"),
                newest.format("%-d %b %Y at %H:%M")
            ),
            None => "No backups yet. One is taken before every import or undo, and once a day."
                .to_owned(),
        };
        let mirror_status = match (&mirror_folder, library.mirror()) {
            (None, _) => None,
            (Some(_), Some(MirrorStatus::Written(at))) => {
                Some((format!("Last written at {}.", at.format("%H:%M")), false))
            }
            (Some(_), Some(MirrorStatus::Failed(error))) => {
                Some((format!("Couldn’t write the last copy: {error}"), true))
            }
            (Some(_), None) => Some(("Written after the next change.".to_owned(), false)),
        };
        let game_data_warning = library.game_data_warning().cloned();
        let settings_warning = library.settings_warning().cloned();
        let source = library.data().map(|data| {
            data.banners
                .source
                .commit
                .chars()
                .take(8)
                .collect::<String>()
        });

        let muted = theme.muted_foreground;
        let danger = theme.danger;
        let mono = theme.mono_font_family.clone();
        let path_text = |path: String| {
            div()
                .text_sm()
                .font_family(mono.clone())
                .truncate()
                .child(path)
        };

        let data =
            group(
                "Your data",
                vec![
                row(
                    "Data folder",
                    div()
                        .v_flex()
                        .gap_0p5()
                        .child(path_text(root.display().to_string()))
                        .child(help(
                            "Your wish database, its backups and a copy of every imported file.",
                            cx,
                        ))
                        .when_some(settings_warning, |this, warning| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().warning)
                                    .child(format!("{warning}.")),
                            )
                        }),
                    button("reveal-data-folder")
                        .label("Show in file manager")
                        .on_click(move |_, _, cx| cx.reveal_path(&root)),
                ),
                row(
                    "Backups",
                    help(backup_line, cx),
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            button("back-up-now")
                                .label("Back up now")
                                .loading(busy)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.back_up(window, cx)),
                                ),
                        )
                        .child(
                            button("restore-backup")
                                .label("Restore…")
                                .unavailable(busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_backups(window, cx)
                                })),
                        ),
                ),
                row(
                    "Mirror folder",
                    div()
                        .v_flex()
                        .gap_0p5()
                        .when_some(mirror_folder.clone(), |this, folder| {
                            this.child(path_text(folder.display().to_string()))
                        })
                        .child(help(
                            "A copy of all your wishes, updated after every change. Choose a \
                             folder synced by Syncthing, Nextcloud, Dropbox or similar to keep \
                             a copy off this computer.",
                            cx,
                        ))
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
                                    .ghost()
                                    .label("Stop mirroring")
                                    .unavailable(busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.set_mirror_folder(None, window, cx)
                                    })),
                            )
                        })
                        .child(
                            button("choose-mirror-folder")
                                .label("Choose folder…")
                                .unavailable(busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.choose_mirror_folder(window, cx)
                                })),
                        ),
                ),
            ],
                cx,
            );

        let recent_rows: Vec<AnyElement> = if imports.is_empty() {
            vec![row("No imports yet", div(), div())]
        } else {
            imports
                .into_iter()
                .map(|record| {
                    let undoable = record.undone_at.is_none() && record.added > 0;
                    let status = if record.undone_at.is_some() {
                        "undone".to_owned()
                    } else {
                        format!("{} added", format::count(record.added))
                    };
                    let id = SharedString::from(format!("undo-import-{}", record.id));
                    row(
                        format!("{} · {}", import_origin(&record), record.account_label),
                        help(
                            format!("{} · {status}", local_time(&record.imported_at)),
                            cx,
                        ),
                        div().when(undoable, |this| {
                            this.child(button(id).label("Undo…").unavailable(busy).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.confirm_undo(record.clone(), window, cx)
                                }),
                            ))
                        }),
                    )
                })
                .collect()
        };
        let recent = group("Recent imports", recent_rows, cx);

        let has_account = self.session.read(cx).account().is_some();
        let export_row =
            |kind: ExportKind, title: &'static str, description: &'static str, enabled: bool| {
                row(
                    title,
                    help(description, cx),
                    button(SharedString::from(format!("export-{kind:?}")))
                        .label("Export…")
                        .unavailable(!enabled)
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.export(kind, window, cx)),
                        ),
                )
            };
        let export = group(
            "Export",
            vec![
                export_row(
                    ExportKind::Snapshot,
                    "Snapshot",
                    "Every account, with nothing left out. Import it back here, on this or \
                     another computer.",
                    true,
                ),
                export_row(
                    ExportKind::Uigf,
                    "UIGF",
                    "For other trackers such as Snap Hutao or Starward. Accounts need a UID.",
                    true,
                ),
                export_row(
                    ExportKind::Spreadsheet,
                    "Spreadsheet",
                    "The account on screen, in paimon.moe’s Excel layout.",
                    has_account,
                ),
            ],
            cx,
        );

        let counting = group(
            "Counting",
            vec![row(
                "Count wishes by hand",
                help(
                    "Adds +1, +10 and “Add 4★ or 5★…” to each banner page, like paimon.moe’s \
                     manual mode. Each entry shows in Recent imports and can be undone there.",
                    cx,
                ),
                Switch::new("manual-counting")
                    .cursor_pointer()
                    .checked(manual_counting)
                    .accessibility_label("Count wishes by hand")
                    .on_change(cx.listener(|this, on: &bool, window, cx| {
                        this.set_manual_counting(*on, window, cx)
                    })),
            )],
            cx,
        );

        let game_data = group(
            "Game data",
            vec![row(
                "Banners and items",
                help(
                    match source {
                        Some(commit) => format!(
                            "Used to work out banners and rate-up results. From paimon.moe, \
                             commit {commit}."
                        ),
                        None => "Used to work out banners and rate-up results.".to_owned(),
                    },
                    cx,
                ),
                div(),
            )]
            .into_iter()
            .chain(game_data_warning.map(|warning| {
                row(
                    "Your game-data folder",
                    div()
                        .text_sm()
                        .text_color(cx.theme().warning)
                        .child(format!("Not all of it is used: {warning}.")),
                    div(),
                )
            }))
            .collect(),
            cx,
        );

        let images = self.images_group(cx);
        // A centred column; on a wide window, two: the data and its imports
        // on the left, the rest on the right.
        let room = crate::shell::content_width(window) / window.rem_size();
        let wide = room >= TWO_COLUMNS_REMS;
        let body = if wide {
            div()
                .grid()
                .grid_cols(2)
                .gap_6()
                .items_start()
                .child(div().v_flex().gap_6().child(data).child(recent))
                .child(
                    div()
                        .v_flex()
                        .gap_6()
                        .child(export)
                        .child(images)
                        .child(counting)
                        .child(game_data),
                )
        } else {
            div()
                .v_flex()
                .gap_6()
                .child(data)
                .child(recent)
                .child(export)
                .child(counting)
                .child(game_data)
                .child(images)
        };
        let width = if wide {
            2. * COLUMN_REMS + 1.5
        } else {
            COLUMN_REMS
        };
        PageFrame::new("Settings", body)
            .max_width(rems(width))
            .into_any_element()
    }
}

/// A titled group of settings on one card, its rows split by hairlines.
fn group(title: &'static str, rows: Vec<AnyElement>, cx: &App) -> gpui_kit::Div {
    let border = cx.theme().border;
    div()
        .v_flex()
        .gap_2()
        .child(
            div()
                .px_1()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(
            card(cx).children(rows.into_iter().enumerate().map(|(ix, row)| {
                div()
                    .when(ix > 0, |this| this.border_t_1().border_color(border))
                    .child(row)
            })),
        )
}

/// One setting: its name and help on the leading side, its controls on the
/// trailing side.
fn row(
    label: impl Into<SharedString>,
    detail: impl IntoElement,
    controls: impl IntoElement,
) -> AnyElement {
    div()
        .h_flex()
        .items_center()
        .gap_4()
        .px_4()
        .py_3()
        .child(
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().truncate().child(label.into()))
                .child(detail),
        )
        .child(div().flex_shrink_0().child(controls))
        .into_any_element()
}

/// Muted help text under a setting's name.
fn help(text: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
