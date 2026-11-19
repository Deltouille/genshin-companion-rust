# GPUI and GPUI Kit here

- Depend on `gpui-kit` only (pinned `=0.9.1` in the workspace `Cargo.toml`). Never add `gpui` directly: `gpui-kit`
  pins its own GPUI snapshot (`gpui-pre`), and two GPUI versions in one build don't interoperate.
- `use gpui_kit::*;` is GPUI. Styled components are under `gpui_kit::component::…`. Startup is
  `gpui_kit::application()` → `gpui_kit::init(cx)` → `gpui_kit::open_window(…)`, which mounts the `Root` that
  dialogs and notifications need (`crates/wish-app/src/main.rs`, `wish_app::init`).
- This GPUI differs from older docs and examples. Check the real API in the sources rather than from memory:
  `~/.cargo/registry/src/index.crates.io-*/{gpui-kit-0.9.1,gpui-component-0.9.1,gpui-base-0.9.1,gpui-pre-0.3.12}`.
- Do blocking work (HTTP with `ureq`, file I/O, SQLite) on the background executor, never on the UI thread.
- Colours come from `cx.theme()` tokens (and `wish_ui::theme::palette`), spacing and sizes from rem helpers. Table
  column widths are computed from `window.rem_size()`.
- Entities: hold `Entity<T>`, read with `.read(cx)`, change with `.update(cx, …)`, hear changes with
  `cx.observe`/`observe_in`, events with `cx.subscribe`. Async work: `cx.spawn(async move |this, cx| …)` on the app
  thread, `cx.background_spawn` off it; a `Task` dropped is cancelled, `.detach()` lets it run. Timers:
  `cx.background_executor().timer(d)`.

## Pitfalls met in this project

- `KeyBinding::new("secondary-…")` panics: `secondary` needs an OS it isn't given. Spell out `cmd-`/`ctrl-` per
  platform (`crates/wish-app/src/lib.rs`).
- `Button::disabled` needs `Disableable` in scope; `v_flex()`/`h_flex()` on `div()` need
  `gpui_kit::base::StyledExt`.
- `Sidebar` children must all be one type; render a lone `SidebarMenuItem` with `SidebarItem::render`.
- Enabling `gpui-kit/test-support` pulls the `gpui-fast` crates into resolution with exact pins (`taffy =0.15.0`…).
  If the lockfile conflicts, regenerate it rather than adding pins.
- Headless captures: dialogs and images need real sleeps, `advance_clock` and `window.refresh()` before the shot
  (see `crates/wish-app/examples/screenshot.rs`); charts need `.appear(false)` or they draw empty.
- Tests run off the main thread, where building the macOS platform (`platform::current_platform`) panics: headless
  tests give `HeadlessAppContext` a `NoopTextSystem` (`crates/wish-app/tests/shell.rs`). Examples run on the main
  thread and can use the platform's text system.
- Code compiled only on some platforms must not leave dead code on the others: CI runs clippy with `-D warnings` on
  Windows and macOS too.
- Deferred layers paint by priority: GPUI Kit's dialogs at 10 and up, popups (a select's list, menus) at 100,
  tooltips at 200. An overlay of the app's own that must cover dialogs goes between (the notices, 50).
- `img` sets its own aspect ratio when none is given, which overrides an explicit height: pass `.aspect_ratio(…)`
  with `ObjectFit::Cover`. In a column, an `img`'s own height wins over `aspect_ratio`: give it a height too.
  Absolute children with `h_full()` don't resolve; give them explicit sizes.
- Clipping (`overflow_hidden`) is rectangular: round the child itself to keep a card's corner.
- The segmented `TabBar` paints its active tab with the page `background`, and `Toggle`'s checked state uses
  `accent` (also the hover colour): hence `wish_ui::Segmented` and `filter_chip`.
- Without the bundled font, bold and black weights don't render (the default font lacks them). GPUI doesn't vary a
  variable font's weight: bundle static instances.
- A kit `Button`'s hover state sets its background (and border) over the caller's: to keep a fill under the state
  layer, draw it on a wrapper (`NavItem`), and draw dividers as elements of their own (`Segmented`). Its selected
  state replays the caller's style and has no hover: `.selected(true).bg(…)` keeps a fill (the current segment).
- `Button::custom(ButtonCustomVariant)` fades its colour to 20% at rest; for a filled state use `.primary()`. A
  caller's style on a kit `Button`, `Select` or `Input` (`.h_11()`, `.px_4()`, `.bg()`) applies after the kit's own
  size, so it wins; their inner text size comes from `Size` (`.large()` gives 16 px).
- GPUI Kit's `SidebarMenuItem` is fixed at 28 px, hence `wish_ui::NavItem`.
- GPUI Kit binds Enter to a dialog's `Confirm`, which runs `on_ok`: an alert whose action is in `on_ok` runs it
  on Enter, whatever has the focus. `wish_ui::confirm_destructive` puts the action in its button's `on_click`
  instead, and focuses Cancel.
- GPUI Kit's dialogs close on a click outside them by default (`overlay_closable`), which loses a half-filled form:
  `wish_ui::form_dialog` turns it off. Alert dialogs never close from the backdrop. Every dialog gives the focus
  back to what opened it.
- `cx.reduce_motion()` says whether the system asks for less motion (GPUI Kit reads it at `init`); the kit's
  transitions follow it, a raw `with_animation` doesn't.
- A centred window is clamped to the display; the screenshot tool opens tall windows at the origin instead.
- In grids, `row_span` overrides `row_start`: place items with `col_start`/`col_end`/`row_start`/`row_end` (lines
  from 1). Grid items stretch by default. A div without listeners, hover style or cursor inserts no hitbox, so
  content laid over a clickable surface doesn't steal its hover or click (the custom entry dialog's role tiles).
- Never read an entity inside its own render or update (it is borrowed): pass the view, or a weak handle.
- Banner image files are named by a slug: a `:` in a path makes GPUI take it for a URL.
