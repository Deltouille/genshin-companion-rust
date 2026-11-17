//! What the app does when it can't start, or panics: release builds on
//! Windows have no console, so a message printed there is lost and a
//! double-click seems to do nothing.
//!
//! Every such failure goes to a file in this user's cache folder (the
//! temporary folder when there is none; [`record_error`]); one before the
//! main window (the data folder, the app already open) is also shown in a
//! small window of its own.

use std::io::Write as _;
use std::path::PathBuf;

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::{
    AppContext as _, Bounds, Context, FontWeight, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};
use wish_ui::button;

use crate::{APP_ID, APP_TITLE, AppAssets};

/// Past this size the error file starts over, the older lines kept once in
/// `…-errors.old.txt`.
const ERROR_FILE_LIMIT: u64 = 256 * 1024;

/// Where startup failures and panics are written: this user's cache folder,
/// not the temporary folder, which on Linux every user of the machine shares.
fn error_file() -> PathBuf {
    let folder = wish_core::paths::user_cache_dir()
        .filter(|folder| std::fs::create_dir_all(folder).is_ok())
        .unwrap_or_else(std::env::temp_dir);
    folder.join(format!("{APP_ID}-errors.txt"))
}

/// Why the data folder couldn't be prepared, for [`show_startup_error`].
pub fn folder_failed_message(error: impl std::fmt::Display) -> String {
    t!("app-folder-failed", error = error.to_string())
}

/// Says another window has the data folder, for [`show_startup_error`].
pub fn already_open_message() -> String {
    t!("app-already-open", app = APP_TITLE)
}

/// Adds `message` to the error file, with the time, and prints it.
pub fn record_error(message: &str) {
    eprintln!("{APP_ID}: {message}");
    let line = format!("{} {message}\n", chrono::Local::now().format("%F %T"));
    let path = error_file();
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > ERROR_FILE_LIMIT) {
        let _ = std::fs::rename(&path, path.with_extension("old.txt"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    // Readable by this user only: it holds paths and error messages.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path);
    // Nowhere else to report a failure to write it.
    if let Ok(mut file) = file {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Writes every panic to the error file too, then reports it as usual.
pub fn record_panics() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record_error(&format!("panic: {info}"));
        report(info);
    }));
}

/// Shows `message` in a small window, and returns once it is closed. For a
/// failure before the main window opens; if this window can't open either,
/// the error file holds the message.
pub fn show_startup_error(message: String) {
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            wish_ui::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(560.), px(260.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some(APP_TITLE.into()),
                    ..Default::default()
                }),
                app_id: Some(APP_ID.into()),
                ..Default::default()
            };
            let message = SharedString::from(message);
            let opened =
                gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| StartupError { message }));
            if opened.is_err() {
                cx.quit();
            }
            cx.activate(true);
        });
}

struct StartupError {
    message: SharedString,
}

impl Render for StartupError {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .v_flex()
            .justify_between()
            .gap_4()
            .p_8()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child(t!("app-cant-start", app = APP_TITLE)),
                    )
                    .child(div().text_base().child(self.message.clone()))
                    .child(div().text_sm().text_color(theme.muted_foreground).child(t!(
                        "app-error-file",
                        file = error_file().display().to_string()
                    ))),
            )
            .child(
                div().h_flex().justify_end().child(
                    button("close")
                        .primary()
                        .label(t!("app-close"))
                        .on_click(|_, _, cx| cx.quit()),
                ),
            )
    }
}
