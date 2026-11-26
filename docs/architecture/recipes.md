# Recipes

Step-by-step changes that touch several places. Each ends the same way: `cargo fmt --all`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and for a visible change the
screenshot tool in both themes and both languages ([testing.md](testing.md)). Then the README (features), and these
pages or `CLAUDE.md` if a sentence there became wrong.

## A text a player reads

1. Add the key to the crate's `.ftl` of the right part, **in every language** (`crates/<crate>/locales/en/…`,
   `fr/…`), with the same variables. French: no-break spaces before `? ! : ;`, curly ’, "vous".
2. Use it with `t!("key", name = value)`; numbers as numbers.
3. The crate's `…_texts_are_complete` test says what's missing or unused.

## A setting

1. A field on `Settings` (`crates/wish-core/src/settings.rs`) with `#[serde(default)]`, so older files read.
2. A `Preferences::set_…` (`crates/wish-app/src/app/preferences.rs`) that goes through `change`.
3. A row in Settings: a group method in the right file of `crates/wish-app/src/features/settings/`, built from
   `group` and `row`.
4. Pages that show something different read `library.settings(cx)`; the library already notifies them.

## A database change

1. A new `crates/wish-core/src/store/schema/NNN_<what>.sql`, appended to `MIGRATIONS`. Never edit one that shipped.
   `STRICT` tables, `CHECK`s for what must hold.
2. A new text enum to store gets its line in `sql_text!` (`crates/wish-core/src/store/sql.rs`); read numbers with
   checked types (`Option<u32>`), never `as`.
3. The `Store` methods, in the file of their subject; a write that changes several rows is one transaction, after
   a backup (or the hourly one for plan and collection edits). A write that changes an account's wishes, counters or
   server calls `Store::changed`.
4. If the data must survive an export: the snapshot (`crates/wish-core/src/snapshot.rs`) gets it as a record of its
   own, a new format version, a new witness written by the app that introduces it, and the merge on import.
5. Tests in `crates/wish-core/tests/`, including opening a database of the previous version.

## An import source

1. A parser in `crates/wish-core/src/import/` (or beside the format) building an `ImportBatch` in memory. It never
   guesses: an unknown item or time fails with a clear `Error`.
2. Recognise it in `import::prepare` if it is a file the player picks.
3. Writing stays `Store::apply_import`: rows are only added, each carrying its `import_id`.
4. The preview shows what would change before anything is written.
5. Tests on a fixture; never the network.

## A page in an existing feature

1. A `Page` variant (or one in the feature's page enum) in `crates/wish-app/src/shell/page.rs`.
2. The view: a `RenderOnce` with props if it has no state of its own, else an entity the shell creates once in
   `Shell::new` and renders in `render_page`. A big page: entity + `model.rs` + `sections/` ([pages.md](pages.md)).
3. A `NavItem::nested` under the feature's group in `crates/wish-app/src/shell/sidebar.rs`.
4. Its texts in the feature's `.ftl`.
5. A capture in `crates/wish-app/examples/screenshot.rs`, and a headless test in `crates/wish-app/tests/shell.rs`
   if it navigates.

## A feature

1. Its rules in `wish-core` (a module of the domain if it is game logic: no database, network or file, which
   `tests/layers.rs` checks), its storage in `store/`, its use cases in `service/` if `wishctl` needs them too.
2. A `Feature` variant, a `Page` variant with its own page enum, a group in `sidebar.rs`, a
   folder in `crates/wish-app/src/features/` and a `.ftl` part (listed in `texts!` in `crates/wish-app/src/lib.rs`).
3. Library methods for its writes (`crates/wish-app/src/app/library/`, a file for the subject), reported with
   `notices::report`.
4. A section in these pages: [overview.md](overview.md)'s "One feature across the crates", and a page of its own if
   it is large.

## A component

1. In `crates/wish-ui/src/components/` if two pages or more use it, else in its feature.
2. It takes plain data and the theme; it never reads app state.
3. Colours from tokens (`cx.theme()`, `theme::palette`), sizes from rems, controls with the theme's `radius`, the pointing hand on
   what is clickable.
4. Exported from `components/mod.rs` and `lib.rs`; shown in `crates/wish-ui/examples/gallery.rs`.

## An icon

Only the kit's `default-icons.txt` icons are bundled. Add a Lucide icon to `icon_assets!` in
`crates/wish-ui/src/assets.rs` and use it as `gpui_kit::assets::IconName::…`.

## A dependency

One used by several crates goes in the workspace's `[workspace.dependencies]` (one version for all), each crate
adding its features with `{ workspace = true, features = [...] }`.

1. Its licence must be GPLv3-compatible ([ci-release.md](ci-release.md)).
2. Never `gpui` itself: `gpui-kit` brings its own.
3. Run `tools/third-party-licenses.sh` and commit `crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt`.

## Game data

New banners or characters: run `tools/update-game-data.sh`, review what changed, run the tests, commit the JSON
([game-data.md](game-data.md)). A field the app needs from genshin-db: the script writes it, the Rust type reads it
with `#[serde(default)]` if older copies in a game-data folder lack it.

## A release

[ci-release.md](ci-release.md): the version, the notes in `docs/releases/`, a commit, a tag.
