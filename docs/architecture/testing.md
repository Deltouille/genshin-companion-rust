# Tests and tools

```bash
cargo test --workspace                                   # every test
cargo clippy --workspace --all-targets -- -D warnings    # must stay clean: CI enforces it
cargo fmt --all
bacon                                                    # continuous check
```

## Where the tests are

| Tests | What they check |
| --- | --- |
| unit tests (`#[cfg(test)] mod tests` at the end of a file) | that file's logic; most of `wish-core`, every page model (`model.rs`), the `hoyolab` crate on a fake `Transport` |
| `crates/wish-core/tests/engine.rs` | the engine against paimon.moe's own stored numbers (golden: `paimon-sample.json`, `paimon-engine.json`) |
| `crates/wish-core/tests/import.rs` | paimon.moe save → store → snapshot, end to end; also every real export in `fixtures/private/` |
| `crates/wish-core/tests/undo.rs` | undoing imports, game ids for migrated wishes, upgrading an old database |
| `crates/wish-core/tests/planner.rs` | costs against totals the game is known for |
| `crates/wish-core/tests/collection.rs` | accounts by hand, plans, inventories and collections per account, syncs |
| `crates/wish-core/tests/good.rs` | GOOD imports into the store; also real exports in `fixtures/private/good/` |
| `crates/wish-core/tests/formats.rs` | the frozen witnesses still read, and the writer writes the current version |
| `crates/wish-core/tests/layers.rs` | the domain modules import no database, network or file |
| `crates/wish-app/tests/messages.rs` | no error carries English words of the app's code (`.context(format!(…))`) |
| `crates/wish-app/tests/buttons.rs` | no `.ghost()` button, which presses with the filled buttons' colour (`ButtonExt::quiet` instead) |
| `crates/wish-core/tests/docs.rs` | every path the docs and `CLAUDE.md` name exists |
| `…_texts_are_complete` (one per crate) | the Fluent texts: every key, every language, French typography ([i18n.md](i18n.md)) |
| `crates/wish-app/src/app/library/tests.rs` | the write queue's promises, end-to-end flows (a HoYoLAB sync, an import from the game) with fakes |
| `crates/wish-app/tests/shell.rs` | the window's model and navigation, headless (nothing drawn; downloads off) |

## Fixtures

- `crates/wish-core/tests/fixtures/paimon-sample.json`, `paimon-engine.json`: small paimon.moe saves whose stored
  values follow paimon.moe's rules. Keep them that way when editing.
- `crates/wish-core/tests/fixtures/witness/`: files written by released versions (a version 1 snapshot by 0.5.0,
  version 2 by 0.6.0, a `settings.json` of 0.6). **Never edit or regenerate one**: a format change gets a new version
  and a new witness written by the app that introduces it.
- `crates/wish-core/tests/fixtures/private/`: git-ignored. Real exports (they hold UIDs) that the tests also run on
  when present; GOOD exports in its `good/` folder.

## Rules the tests keep

- **Never the network.** The game's API through `api::tests::FakeApi`, HoYoLAB through a fake `Transport` with
  recorded, anonymised answers, the app through `Services::offline()` (every request fails, nothing downloads) with
  fakes put in for a flow.
- **One language per test**: `with_language` (thread-local), never `set_language`; tests run side by side.
- **The test scheduler's random order** for the write queue: `SEED=n` tries another.
- Headless GPUI tests give `HeadlessAppContext` a `NoopTextSystem`: building the macOS platform off the main
  thread panics ([gpui.md](gpui.md)).

## Seeing the UI without a desktop

```bash
node tools/sample-save.mjs <save.json>        # a synthetic save, ~600 wishes (never for engine tests)
cargo run -p wish-core --example wishctl -- --data-dir <dir> import-paimon <save.json>
cargo run -p wish-app --example screenshot -- <dir> <out-dir>
```

The screenshot tool (`crates/wish-app/examples/screenshot.rs`) opens the real window offscreen and saves each page
and dialog, light and dark, as PNG. It downloads the images a page needs and waits for them.

| Variable | Effect |
| --- | --- |
| `SCREENSHOT_ONLY=overview,banners` | only the captures whose names start with these |
| `SCREENSHOT_WIDTH=2000`, `SCREENSHOT_HEIGHT=1500` | a wider or taller window (a whole page, the two-column layouts) |
| `SCREENSHOT_LANGUAGE=fr` | the pages in French (English otherwise, whatever the data folder says) |
| `SCREENSHOT_WEAPON=coolsteel` | another weapon's page than Amos' Bow |

Put `"manual_counting": true` in the data folder's `settings.json` to shoot the counting controls too. The HoYoLAB
preview comes from a sample HoYoLAB in the tool (`SampleHoyolab`), the GOOD preview from `SAMPLE_GOOD`, a synthetic
file: never the real ones. The README's screenshots (`docs/screenshots/`) come from this tool, in the dark theme, on the synthetic save
with a few owned characters and a plan of real characters added through a snapshot (`wishctl export`, edit the
`characters` and `plan` of the JSON, `import-snapshot` into a fresh folder with the image cache copied over):
`SCREENSHOT_HEIGHT=1000`, and 1400 for the planner so its cards show.

To check a refactor changed nothing on screen: shoot every page with the old and the new build on two copies of
the same data folder, and compare the PNGs. Only the times differ (the latest backup's, a mirror's), and a backup
taken at open shifts them by a minute: use a folder that already has today's backup.

To try the real app on scratch data: `GENSHIN_COMPANION_DATA_DIR=<dir> cargo run -p wish-app`.

## `wishctl`

`cargo run -p wish-core --example wishctl -- [--data-dir DIR] <command>` (`crates/wish-core/examples/wishctl/`).
Without `--data-dir` it uses the app's real data folder: point it at a scratch folder when experimenting. Its
writing commands take the folder's lock, so they stop while the app is open.

| Command | What it does |
| --- | --- |
| `import-paimon <file>`, `check-paimon <file>` | import a paimon.moe save; compare the engine with the numbers it stored, writing nothing |
| `import-snapshot <file>`, `export <file>` | merge a snapshot; write one |
| `find-link`, `fetch-game <link>`, `import-game <link>` | find the history link on this computer; show what an import from the game would add; do it |
| `undo <id>`, `summary`, `backup` | undo an import (`summary` lists the ids); the counters, pity and 50/50 per account; a backup now |
| `hoyolab-check [--raw DIR]` | read a cookie from stdin (echo off) and print what HoYoLAB tells of the account (and whether the chronicle's answers read), writing nothing, never printing the cookie or the authkey; `--raw` saves HoYoLAB's answers |
| `hoyolab-fetch [--uid U]`, `hoyolab-sync [--uid U]` | what a sync would change (the collection, the chronicle); do it (`--from FILE --uid U` reads saved answers, offline) |
| `check-good <file>`, `import-good <file>` | what a GOOD import would change; do it (`--uid U` or `--account NAME`, else the only account, else a new "Main") |

The HoYoLAB cookie never goes in a command line or the shell's history: these commands read it from stdin.
