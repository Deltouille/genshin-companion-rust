//! The desktop app, in three layers on top of `wish-core` (the domain) and
//! `wish-ui` (the design system):
//!
//! - [`app`]: state and services, no views: the [`Library`] (store, game
//!   data, analyses, settings), the [`Session`] (selected account, the flow
//!   in progress), the item images, the embedded assets, the notices
//!   (messages about work done in the background, [`app::notify`]);
//! - [`shell`]: the window, its sidebar and the routing between pages;
//! - `features`: one folder per feature (`wishes`: overview, history, import)
//!   and the app's own pages (settings, credits).
//!
//! A library so that examples (such as the offscreen `screenshot` tool) can
//! open the same window as the `genshin-companion` binary.
//!
//! # How data flows
//!
//! Each piece of state has one owner, and the rest of the app reaches it in
//! one of four ways, chosen by what the data is:
//!
//! | What | Owner | How others get it |
//! | --- | --- | --- |
//! | Wish data, settings, backups | [`Library`] | hold `Entity<Library>`, read it, `observe` it to redraw; change it with its methods (writes run in the background) |
//! | Selected account, flow in progress | [`Session`] | hold `Entity<Session>`, read and `observe` it; change it only with its methods |
//! | Which page is shown | [`Shell`] | pages ask with a callback (`on_navigate`) or the shell calls the page (`HistoryView::set_pool`) |
//! | App-wide commands (import) | GPUI actions | buttons and the menu send [`ImportFile`] / [`ImportFromGame`]; the shell forwards them to the import flow |
//!
//! Why these choices:
//!
//! - **Shared state lives in entities that are observed**, not copied into
//!   each page: a page holding `Entity<Session>` always reads the current
//!   account, and `observe` redraws it when that changes. Copies would need
//!   the shell to push every change to every page, and one forgotten push is
//!   a stale screen.
//! - **Changes go through methods**, never through fields: [`Session`]'s
//!   methods keep its rules (the account exists; one flow at a time) and
//!   [`Library`]'s queue the writes. The compiler enforces it: the fields are
//!   private.
//! - **One flow for the whole window** ([`Session::start_flow`]): starting
//!   one cancels the previous before it can open a stale dialog, as before
//!   pages had state of their own. Writes are not at risk: [`Library`] queues
//!   them and they always finish; a cancelled flow only loses its next step.
//! - **Routing stays in the shell**: a page doesn't know the others, so it
//!   asks to open one (`on_navigate`) rather than switching itself.
//! - **Commands are actions**: an action works the same from a button, the
//!   menu or a shortcut, and a button doesn't need to know who carries it out.
//!
//! # Which kind of component
//!
//! - A page **without state of its own** is a `RenderOnce` component built on
//!   each render with what it shows, like a React function component with
//!   props: the overview, the credits. Its data comes from the shell, which
//!   reads the library and the session.
//! - A page **with state that must survive a render** (a table, a running
//!   task, dialogs that call back) is an entity the shell creates once:
//!   `HistoryView`, `SettingsPage`, and
//!   the import flow (a service with no view, owned by the shell because
//!   several pages and the menu start it).
//! - A piece used by two pages or more goes to `wish-ui`; one used by a
//!   single page stays in its feature folder.

pub mod app;
mod features;
pub mod shell;
mod startup_error;

use gpui_kit::{
    App, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

pub use app::{AppAssets, ImageDownloads, Library, Session};
pub use shell::{ArchivePage, Feature, ImportFile, ImportFromGame, Page, Quit, Shell, WishPage};
pub use startup_error::{record_error, record_panics, show_startup_error};

pub const APP_ID: &str = "genshin-companion";
pub const APP_TITLE: &str = "Genshin Companion";

// GPUI's `secondary-` modifier only parses when an OS is supplied, which
// `KeyBinding::new` does not do, so spell the platform modifier out.
#[cfg(target_os = "macos")]
const KEYS: [&str; 2] = ["cmd-o", "cmd-q"];
#[cfg(not(target_os = "macos"))]
const KEYS: [&str; 2] = ["ctrl-o", "ctrl-q"];

/// Initialises GPUI Kit, then the app's key bindings and menus.
pub fn init(cx: &mut App) {
    gpui_kit::init(cx);
    wish_ui::init(cx);

    // Keys first: menus read the keymap when they are set.
    let [import, quit] = KEYS;
    cx.bind_keys([
        KeyBinding::new(import, ImportFile, Some("Shell")),
        KeyBinding::new(quit, Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.set_menus(vec![Menu {
        name: APP_TITLE.into(),
        items: vec![
            MenuItem::action("Import from Game…", ImportFromGame),
            MenuItem::action("Import File…", ImportFile),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ],
        disabled: false,
    }]);
}

pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.), px(820.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(APP_TITLE.into()),
            ..Default::default()
        }),
        app_id: Some(APP_ID.into()),
        window_min_size: Some(size(px(880.), px(560.))),
        ..Default::default()
    }
}
