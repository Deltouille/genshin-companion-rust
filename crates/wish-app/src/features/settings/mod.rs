//! Settings, one card per group: the accounts (`accounts`), the language,
//! your data, its backups and the mirror (`data`, `backups`, `mirror`),
//! recent imports and undoing one (`imports`), exports (`export`), the
//! HoYoLAB login the keyring keeps (`hoyolab`), the image
//! cache (`images`), counting by hand, and the game data. This file holds the
//! page, its small groups, the layout (one centred column, or two on a wide
//! window) and the rows every group is made of.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
    rems,
};

mod accounts;
mod backups;
mod data;
mod export;
mod hoyolab;
mod images;
mod imports;
mod mirror;

use crate::app::notices::report;
use crate::app::{Library, Session};
use wish_core::i18n::Language;
use wish_ui::{PageFrame, PickerEvent, PickerItem, PickerState, card, picker};

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
    /// The interface's language: the system's (`None`) or one chosen.
    language: Entity<PickerState<Option<Language>>>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub fn new(session: Entity<Session>, cx: &mut Context<Self>) -> Self {
        let library = session.read(cx).library().clone();
        let chosen = library.read(cx).settings(cx).language;
        let language =
            cx.new(|cx| PickerState::new(language_options(), Some(language_index(chosen)), cx));
        let subscriptions = vec![
            cx.observe(&library, |this, library, cx| {
                // The options' words, and the choice once the file is read.
                let chosen = library.read(cx).settings(cx).language;
                this.language.update(cx, |picker, cx| {
                    picker.set_items(language_options(), cx);
                    picker.set_selected_index(Some(language_index(chosen)), cx);
                });
                cx.notify()
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(
                &language,
                |this, _, event: &PickerEvent<Option<Language>>, cx| {
                    let PickerEvent::Confirm(language) = event;
                    this.set_language(*language, cx);
                },
            ),
        ];
        Self {
            library,
            session,
            language,
            _subscriptions: subscriptions,
        }
    }

    fn language_group(&self, cx: &App) -> gpui_kit::Div {
        group(
            t!("settings-group-language"),
            vec![row(
                t!("settings-language"),
                help(t!("settings-language-help"), cx),
                picker("language", &self.language)
                    .accessibility_label(t!("settings-language"))
                    .menu_width(rems(14.)),
            )],
            cx,
        )
    }

    /// Turns counting by hand on or off, and saves the setting.
    fn set_manual_counting(&mut self, on: bool, cx: &mut Context<Self>) {
        let preferences = self.library.read(cx).preferences().clone();
        let task = preferences.update(cx, |preferences, cx| {
            preferences.set_manual_counting(on, cx)
        });
        report(task, t!("settings-save-failed"), cx);
    }

    /// Counting wishes by hand, like paimon.moe's manual mode: on or off.
    fn counting_group(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let manual_counting = self.library.read(cx).settings(cx).manual_counting;
        group(
            t!("settings-group-counting"),
            vec![row(
                t!("settings-manual-counting"),
                help(t!("settings-manual-counting-help"), cx),
                Switch::new("manual-counting")
                    .cursor_pointer()
                    .checked(manual_counting)
                    .accessibility_label(t!("settings-manual-counting"))
                    .on_change(
                        cx.listener(|this, on: &bool, _, cx| this.set_manual_counting(*on, cx)),
                    ),
            )],
            cx,
        )
    }

    /// The game data the wish history reads: where it comes from, and what
    /// was wrong with a newer copy in the game-data folder.
    fn game_data_group(&self, cx: &App) -> gpui_kit::Div {
        let library = self.library.read(cx);
        let game_data_warning = library.game_data_warning().cloned();
        let source = library.data().map(|data| {
            data.banners
                .source
                .commit
                .chars()
                .take(8)
                .collect::<String>()
        });
        group(
            t!("settings-group-game-data"),
            vec![row(
                t!("settings-banners-items"),
                help(
                    match source {
                        Some(commit) => t!("settings-banners-items-source", commit = commit),
                        None => t!("settings-banners-items-help"),
                    },
                    cx,
                ),
                div(),
            )]
            .into_iter()
            .chain(game_data_warning.map(|warning| {
                row(
                    t!("settings-game-data-folder"),
                    div().text_sm().text_color(cx.theme().warning).child(t!(
                        "settings-game-data-partial",
                        warning = warning.to_string()
                    )),
                    div(),
                )
            }))
            .collect(),
            cx,
        )
    }

    /// Puts the interface in `language`, or the system's (`None`).
    fn set_language(&mut self, language: Option<Language>, cx: &mut Context<Self>) {
        let preferences = self.library.read(cx).preferences().clone();
        let task = preferences.update(cx, |preferences, cx| preferences.set_language(language, cx));
        report(task, t!("settings-save-failed"), cx);
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_settings(window, cx)
    }
}

impl SettingsPage {
    fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let data = self.data_group(cx);
        let recent = self.imports_group(cx);
        let export = self.export_group(cx);
        let counting = self.counting_group(cx);
        let game_data = self.game_data_group(cx);
        let images = self.images_group(cx);
        let language = self.language_group(cx);
        let accounts = self.accounts_group(cx);
        let hoyolab = self.hoyolab_group(cx);
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
                .child(
                    div()
                        .v_flex()
                        .gap_6()
                        .child(accounts)
                        .child(data)
                        .child(recent),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_6()
                        .child(language)
                        .child(hoyolab)
                        .child(export)
                        .child(images)
                        .child(counting)
                        .child(game_data),
                )
        } else {
            div()
                .v_flex()
                .gap_6()
                .child(language)
                .child(accounts)
                .child(hoyolab)
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
        PageFrame::new(t!("settings-title"), body)
            .max_width(rems(width))
            .into_any_element()
    }
}

/// The languages to choose from: the system's first, then each in itself.
fn language_options() -> Vec<PickerItem<Option<Language>>> {
    let system = PickerItem::new(
        t!(
            "settings-language-system",
            language = Language::system().name()
        ),
        None,
    );
    std::iter::once(system)
        .chain(Language::ALL.map(|language| PickerItem::new(language.name(), Some(language))))
        .collect()
}

/// Where `chosen` is among [`language_options`].
fn language_index(chosen: Option<Language>) -> usize {
    chosen.map_or(0, |language| {
        1 + Language::ALL
            .iter()
            .position(|l| *l == language)
            .unwrap_or_default()
    })
}

/// A titled group of settings on one card, its rows split by hairlines.
fn group(title: String, rows: Vec<AnyElement>, cx: &App) -> gpui_kit::Div {
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
