// Release builds open no console window on Windows; debug builds keep it for
// their output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::AppContext as _;
use wish_app::{Library, Services, Shell};
use wish_core::AppPaths;
use wish_core::settings::Settings;

/// Points the app at another data folder, e.g. a scratch copy for testing.
const DATA_DIR_VAR: &str = "GENSHIN_COMPANION_DATA_DIR";
/// Its name when the app was called Genshin Wishes, still understood.
const FORMER_DATA_DIR_VAR: &str = "GENSHIN_WISHES_DATA_DIR";

fn main() {
    wish_app::record_panics();
    let dir = std::env::var_os(DATA_DIR_VAR).or_else(|| std::env::var_os(FORMER_DATA_DIR_VAR));
    let paths = match dir {
        Some(dir) => Ok(AppPaths::at(dir)),
        None => AppPaths::from_os(),
    };
    let paths = match paths {
        Ok(paths) => paths,
        Err(err) => {
            wish_core::i18n::set_language(wish_core::i18n::Language::system());
            fail(wish_app::folder_failed_message(err))
        }
    };
    // Before any message or window, so they are in the player's language.
    wish_core::i18n::set_language(Settings::read_language(&paths));
    // Two windows on one folder would each show a stale state, and write the
    // mirror and the backups in turn.
    let _lock = match paths.lock() {
        Ok(Some(lock)) => Some(lock),
        Ok(None) => fail(wish_app::already_open_message()),
        // A folder that can't be locked (some network drives) still opens.
        Err(err) => {
            wish_app::record_error(&format!("not locking the data folder: {err}"));
            None
        }
    };

    gpui_kit::application()
        .with_assets(wish_app::AppAssets)
        .run(move |cx| {
            wish_app::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let library = cx.new(|cx| Library::open(paths, Services::network(), cx));
            let opened = gpui_kit::open_window(wish_app::window_options(cx), cx, |window, cx| {
                cx.new(|cx| Shell::new(library, window, cx))
            });
            if let Err(err) = opened {
                // No window, so nowhere to say it but the error file.
                wish_app::record_error(&format!("the main window could not open: {err:#}"));
                cx.quit();
            }
            cx.activate(true);
        });
}

/// Says why the app can't start, in a window of its own (release builds on
/// Windows have no console), and stops.
fn fail(message: String) -> ! {
    wish_app::record_error(&message);
    wish_app::show_startup_error(message);
    std::process::exit(1)
}
