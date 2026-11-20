# CLAUDE.md

**Genshin Companion** (formerly Genshin Wishes): a Genshin Impact desktop app in Rust + GPUI. It began as a rewrite
of paimon.moe's **wish counter** and now also has a character, weapon and material archive and an ascension
planner. **The point of the project is that the player's data, wish history first, is never lost.**

This file holds what every session needs: the commands, the rules, and a map. **How the code works is in
`docs/architecture/`** (start with `docs/architecture/README.md`, then `overview.md`); read the page for the part
you change before changing it, and fix any sentence of it your change makes wrong, in the same change
(`crates/wish-core/tests/docs.rs` fails on a path these docs name that no longer exists).

- `README.md` is the user-facing doc (features, data folder, building, updating the data, credits). Keep it in step
  when a feature, a command or a data source changes. Its screenshots (`docs/screenshots/`) come from the screenshot
  tool on a synthetic save (`tools/sample-save.mjs`).
- `PLAN.md` is the roadmap, kept local (git-ignored, like `documentation/`, and out of the history): read it before
  starting a milestone, and tick its checkboxes when a step is done. Code and docs never refer to it or to
  `documentation/`'s review tickets: say the reason itself, since other machines don't have them.
- Text-stored enums use `text_enum!` (`crates/wish-core/src/model.rs`, the `TextEnum` trait): one text in JSON and
  SQLite alike. The domain knows no database: one stored in SQLite gets its line in `sql_text!`
  (`crates/wish-core/src/store/sql.rs`), with the other `ToSql`/`FromSql` impls.
- Names: `APP_TITLE` / `APP_ID` (`crates/wish-app/src/lib.rs`), the binary and the data folder
  `genshin-companion`. The crates keep their `wish-*` names; the snapshot format (`genshin-wishes-snapshot`) and the
  mirror file keep the old name on purpose, so existing files still read.

## Environment and commands

- Dev shell: `~/Projects/Rust/flake.nix` (`nix develop ~/Projects/Rust`). It provides GPUI's Linux libraries and
  sets `LD_LIBRARY_PATH`; without it the app compiles but the window fails to open.
  - Agent sessions already run inside it (`IN_NIX_SHELL` is set): run `cargo` directly.
    **Never prefix commands with `nix develop ~/Projects/Rust -c`.** That flake is a plain folder, not a git repo,
    so every call copies all of `~/Projects/Rust`, `target/` folders included, into `/nix/store`, a new multi-GB
    copy after every build. It filled the disk twice.
- `cargo run -p wish-app` (the app), `cargo test --workspace`, `cargo fmt --all`, `bacon`.
- `cargo clippy --workspace --all-targets -- -D warnings`: must stay clean, since CI enforces it (on Linux,
  Windows and macOS). `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` too: a doc link to an item that
  moved fails CI.
- `cargo run -p wish-core --example wishctl -- [--data-dir DIR] <command>`: a data folder from the terminal
  (imports, exports, summary, undo, `check-paimon`, `fetch-game`, HoYoLAB and GOOD checks). Without `--data-dir` it
  uses the real app data folder: point it at a scratch folder when experimenting. Commands:
  `docs/architecture/testing.md`.
- `cargo run -p wish-app --example screenshot -- <data-dir> <out-dir>`: every page offscreen to PNG. **Review UI
  changes with it, never by opening windows on the user's desktop.** `SCREENSHOT_ONLY`, `SCREENSHOT_WIDTH`,
  `SCREENSHOT_HEIGHT`, `SCREENSHOT_LANGUAGE=fr`, `SCREENSHOT_WEAPON`; a synthetic save with
  `node tools/sample-save.mjs <out.json>` (never for engine tests). Details: `docs/architecture/testing.md`.
- `cargo run -p wish-ui --example gallery -- <out-dir>`: every component, light and dark.
- `node tools/extract-paimon-data.mjs` / `node tools/extract-genshin-db.mjs`: regenerate the embedded game data
  from `reference/` / `genshin-db/`; commit the JSON. `docs/architecture/game-data.md`.
- `nix shell nixpkgs#cargo-about -c tools/third-party-licenses.sh`: after any change to `Cargo.lock`, and commit
  `crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt` (CI fails when it differs).
- `nix shell nixpkgs#cargo-deny -c cargo-deny --locked check advisories sources`: the dependency audit.
- Try the real app on scratch data: `GENSHIN_COMPANION_DATA_DIR=<dir>`.

## Map

The three crates mirror a front end's core / shared / features split, and Cargo enforces the direction:
`wish-app` → `wish-ui` → `wish-core` → `hoyolab`, never the reverse.

| Where | What | Read |
| --- | --- | --- |
| `crates/hoyolab` | a small HoYoLAB client (after genshin.py): game accounts, characters, authkey; the cookie as `Secret`s | `imports.md` |
| `crates/wish-core/src/{model,engine,stats}.rs`, `data.rs` | wishes, pools, pity, banners, 50/50; items and banners | `wishes.md` |
| `crates/wish-core/src/planner/` | the planner's data, costs, plan, custom entries, allocation, resin, what to farm | `planner.md` |
| `crates/wish-core/src/collection/`, `archive.rs` | what an account owns, HoYoLAB syncs, GOOD files; the archive's pages | `collection-archive.md` |
| `crates/wish-core/src/store/`, `snapshot.rs`, `paths.rs`, `settings.rs` | SQLite, migrations, backups, accounts; snapshots and the mirror; the data folder | `storage.md` |
| `crates/wish-core/src/import/`, `uigf.rs`, `excel.rs`, `service/` | every import, the game's API, use cases shared with `wishctl` | `imports.md` |
| `crates/wish-core/src/i18n/`, `locales/` | Fluent texts, the game's texts in every language | `i18n.md` |
| `crates/wish-core/src/images.rs`, `crates/wish-app/src/app/images.rs` | where each game image comes from; the cache and downloader | `images.md` |
| `crates/wish-ui` | the theme (`theme.json`), components, formatting, charts | `ui.md` |
| `crates/wish-app/src/lib.rs`, `app/` | **the data flow's rules** (top of `lib.rs`); `Library`, `Session`, `Preferences`, the write queue, notices | `app-state.md` |
| `crates/wish-app/src/shell/`, `features/` | the window, routing, sidebar; one folder per feature | `pages.md` |
| `.github/workflows/`, `tools/`, `docs/releases/` | CI, audit, licences, releases | `ci-release.md` |

`docs/architecture/recipes.md` walks through common changes (a text, a setting, a migration, an import source, a
page, a feature, a component, a dependency). `docs/architecture/gpui.md` lists GPUI's pitfalls met here.

`reference/` (paimon-moe) and `genshin-db/` are git-ignored, read-only clones the extraction scripts read. **Never
edit them.** In `reference/`, the wish logic is `src/functions/wish.js` and `src/routes/wish/import.svelte`
(`processWishes`, and `getLog` for the API), the counter and summary `src/routes/wish/_counter.svelte`,
`_summary.svelte`, `[id].svelte`, the data `src/data/banners.js`, `bannersDual.js`, `characters.js`,
`weaponList.js`, and the storage this app replaces `src/stores/saveManager.js`.

## Rules

### Data safety (non-negotiable)

- Imports only add rows: `INSERT … ON CONFLICT (account_id, id) DO NOTHING`. Never use `INSERT OR IGNORE`: it also
  silently skips rows that fail a `CHECK`. Never delete or overwrite existing wishes as a side effect of an import.
  Report a stored wish whose content differs from the incoming one; don't replace it.
- What the game tells us about stored wishes is recorded beside them, never written into them: the game id of a
  migrated wish goes in `wish_links`.
- Every row an import adds carries its `import_id` (wishes, links, manual counters), so `Store::undo_import` can
  remove it. A snapshot's characters, weapons, plan and inventory don't (they are merged, not just added): undoing
  its import removes its wishes, and the `before-import` backup undoes the rest. Undo takes a backup first and marks
  the import undone rather than deleting the log entry.
- Never hold the store lock during network requests.
- Importers never guess. An unknown item or an unparseable time fails the import with a clear message, because a
  wrong rarity corrupts every pity count after it.
- Take a SQLite backup before any import, migration, restore or bulk edit. The exceptions are counting by hand and
  editing the plan or the collection: at most one `before-counting` / `before-planning` backup an hour each, kept
  apart from the others (`KEEP_HOURLY_BACKUPS` vs `KEEP_BACKUPS`) so a long session can't push out the backups of
  real imports.
- A restored file may hold no trigger or view (the app makes none): it would act on the next import.
- One process at a time uses a data folder (`AppPaths::lock`).
- Every write that changes several rows happens in a single transaction. A fetch is staged in memory and written
  only after the user confirms.
- Never persist computed values (pity, banner, 50/50, guarantee, planner costs). Recompute them from raw rows.
- Schema changes go through migrations only: add `store/schema/NNN_*.sql` and append it to `MIGRATIONS`. Never edit
  a migration that has shipped. A database with a newer `user_version`, or a negative one (no app writes it, and
  it would skip every migration), must be refused untouched.
- Read numbers from SQLite and files with checked types (`Option<u32>`, `try_from`): a value out of range is an
  error, never another item or a 0. `wish-core` turns clippy's lossy-cast lints on (`[lints.clippy]` in its
  `Cargo.toml`): convert with `try_from`, or allow a cast at its spot with the reason.
- A write that changes an account's wishes, manual counters or server calls `Store::changed`, or the app keeps a
  stale analysis.
- Exports and mirror files are written atomically: temp file, fsync, rename.
- Files written outside the database must keep reading: `crates/wish-core/tests/formats.rs` checks the frozen
  witnesses in `crates/wish-core/tests/fixtures/witness/`. **Never edit or regenerate a witness.** A format change
  gets a new version and a new witness, written by the app that introduces it.
- The write queue's promises (order, newest state, a write finishing when its caller lets go, reload after a
  failure or a panic, quitting waiting) are tested in `crates/wish-app/src/app/library/tests.rs`; keep them.
- Real exports (they hold UIDs) go in `crates/wish-core/tests/fixtures/private/` (GOOD ones in its `good/`), which
  is git-ignored.

### Privacy and others' property

- **The HoYoLAB cookie is never stored or printed**: in memory only, as `Secret`s, used for its requests, sent only
  to HoYoverse's hosts over https without redirects. Never ask the user to paste it here or run a cookie command
  through this session: they run those in their own terminal. Never change the user's HoYoLAB account settings.
- **The game's images are never in the repository or the builds** (they are HoYoverse's). The app downloads each
  on first use (`docs/architecture/images.md`). Never add game images or a tool that packs them. Downloads are
  opt-in beyond what pages show: no eager prefetching of the two volunteer-run sites.
- The game's names, numbers and texts are embedded (the user's choice).
- Credits keeps paimon.moe's, genshin-db's and genshin.py's MIT notices and says the game's art and names are
  HoYoverse's. Credit any new third-party asset there.
- Licence: GPL-3.0-or-later. **A new dependency's licence must be GPLv3-compatible** (GPLv2 is not an option: GPUI
  Kit is Apache-2.0 only).

### Code

- `wish-core` **never depends on GPUI**. All logic and its tests go there. Its domain (`model`, `engine`, `stats`,
  `planner`, `collection`, `archive`) reaches no database, network or file: `crates/wish-core/tests/layers.rs`
  checks it.
- `wish-app` stays thin: it calls `wish-core` and renders with `wish-ui`. **Follow the data flow's rules at the top
  of `crates/wish-app/src/lib.rs`**: one owner per piece of state; others hold an `Entity`, read it and `observe`
  it, and change it through its methods; one flow for the window (`Session::start_flow`); writes through
  `Library::write`, their errors reported with `notices::report` (never `.detach()`ed silently, never a flow).
- A big page is an entity, a `model.rs` of pure functions tested without GPUI, and `sections/` that draw it.
- **Every text a player reads is a Fluent message** (`t!("key", name = value)`), in every language, French
  typography included; never a literal in a view, never `format!` of words; numbers passed as numbers. Tests use
  `with_language`, never `set_language`. Match what the game or a file says against `Text::english()`.
  `docs/architecture/i18n.md`.
- `wish_core::Error` messages reach the player: what happened and what to do, never a developer's step. The app
  never adds words of its own to an error (`.context(format!(…))`): a file's failure is a `wish_core::Error::Io`
  (`crates/wish-app/tests/messages.rs` checks).
- A mutex is locked with `wish_core::service::lock`, which takes a poisoned lock back. Never `panic = "abort"`:
  `Library::write` turns a write that panics into its error.
- Domain rules (`docs/architecture/wishes.md`): gacha types 100 Beginners, 200 Standard, 301 and 400 Character
  Event (**shared pity**), 302 Weapon, 500 Chronicled; 5★ pity 90 (80 for weapons), 4★ pity 10; pity counts this
  wish, and a 5★ doesn't reset the 4★ count; 50/50 on Character and Weapon Event only. API times are server-local
  wall clock; banner times are UTC+8; for `timezoneDependent` banners subtract `8 − offset` hours (Asia/TW/China
  +8, America −5, Europe +1). Fetch with `lang=en-us` so names match the item data. Rarity comes from the API's
  `rank_type`, so an item the data doesn't know yet never fails an import from the game (a file without rarities
  does fail on one: importers never guess).
- When porting from paimon.moe, keep its numbers identical first (`crates/wish-core/tests/engine.rs` is the
  acceptance test, and its stored values follow paimon's rules), then improve; a deliberate difference gets a
  comment at the spot.
- Tests never touch the network: `api::tests::FakeApi`, a fake HoYoLAB `Transport`, `Services::offline()`.

### UI

- **The look is the user's choice** (after the game's menus; `docs/architecture/ui.md` says which alternatives
  were turned down). Ask which point to change before reworking it, and show screenshots.
- Before UI work, read the GPUI Kit skills' guides (`gpui-kit-design-guides`, and the Coding Guides in `gpui-kit`).
- **Never use `Button::new`, `Select::new`, `Input::new`, or the kit's `Pagination`, `ButtonGroup` and `Toggle`
  directly in the app**: use `wish_ui`'s `button`, `picker`, `input`, `Pager`, `Segmented`, `filter_chip`. A button
  without fill is `.quiet(cx)`, never the kit's `.ghost()`, which flashes parchment when pressed
  (`crates/wish-app/tests/buttons.rs` checks).
- **Everything clickable shows the pointing hand**; a disabled button takes `.unavailable(bool)`.
- **Everything a click does, the keyboard does too** (build it on a kit `Button`, not a `div` with `on_click`). A
  dialog the player types in is a `form_dialog`; asking before removing or replacing data is `confirm_destructive`
  (Enter keeps the data). The rules: `docs/architecture/ui.md`, "Interaction and accessibility".
- Raw colours live only in `crates/wish-ui/src/theme.json`. Keep text contrast at 4.5:1.
- As few bar charts as possible: areas, lines with dots, donuts.
- Never the kit's `window.push_notification`: notices go through `notify`.
- Depend on `gpui-kit` only (pinned `=0.9.1`), never `gpui` directly. This GPUI differs from older docs: check the
  real API in `~/.cargo/registry/src/index.crates.io-*/{gpui-kit-0.9.1,gpui-component-0.9.1,gpui-base-0.9.1,gpui-pre-0.3.12}`.
  Blocking work (HTTP, files, SQLite) runs on the background executor. Pitfalls: `docs/architecture/gpui.md`.

### CI and releases

- CI runs fmt, clippy, the tests, rustdoc and a release build on Linux, Windows and macOS, all `--locked`. Every
  action is pinned to a commit with its tag in a comment, and checkouts don't keep the token: keep it that way.
  Rust is pinned in `rust-toolchain.toml` and each workflow's `toolchain:`: raise both together.
- A release: the version in `Cargo.toml`, the notes in `docs/releases/v<version>.md` (each paragraph and list item
  on one line: GitHub shows newlines as line breaks; there is no login here to edit a release afterwards), a
  commit, then the tag. The README's Download section names the archives: keep it in step.
  `docs/architecture/ci-release.md`.
