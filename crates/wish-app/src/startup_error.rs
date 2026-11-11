//! What the app does when it can't start, or panics: release builds on
//! Windows have no console, so a message printed there is lost and a
//! double-click seems to do nothing.
//!
//! Every such failure goes to a file in the temporary folder
//! ([`record_error`]); one before the main window (the data folder, the app
//! already open) is also shown in a small window of its own.

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

/// Where startup failures and panics are written.
fn error_file() -> PathBuf {
    std::env::temp_dir().join(format!("{APP_ID}-errors.txt"))
}

/// Adds `message` to the error file, with the time, and prints it.
pub fn record_error(message: &str) {
    eprintln!("{APP_ID}: {message}");
    let line = format!("{} {message}\n", chrono::Local::now().format("%F %T"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(error_file());
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
                            .child(format!("{APP_TITLE} can’t start")),
                    )
                    .child(div().text_base().child(self.message.clone()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("Also written to {}.", error_file().display())),
                    ),
            )
            .child(
                div().h_flex().justify_end().child(
                    button("close")
                        .primary()
                        .label("Close")
                        .on_click(|_, _, cx| cx.quit()),
                ),
            )
    }
}
