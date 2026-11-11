# CLAUDE.md

**Genshin Companion** (formerly Genshin Wishes): a Genshin Impact desktop app in Rust + GPUI. It began as a rewrite
of paimon.moe's **wish counter** and now also has a character archive and an ascension planner.
The point of the project is that the player's data, wish history first, is never lost.

- Name: `APP_TITLE` / `APP_ID` (`crates/wish-app/src/lib.rs`), the binary `genshin-companion` and the data folder
  `genshin-companion`. `AppPaths::from_os` copies a former `genshin-wishes` folder on first start and leaves it as it
  was (`paths::copy_data_folder`). The crates keep their `wish-*` names. The snapshot format
  (`genshin-wishes-snapshot`) and the mirror file's name keep the old name on purpose, so existing files still read.
- Licence: GPL-3.0-or-later (`LICENSE`, `license.workspace` in every crate). GPLv2 is not an option: many
  dependencies (GPUI Kit among them) are Apache-2.0 only. Check that a new dependency's licence is GPLv3-compatible.
- Third-party licences: `tools/third-party-licenses.sh` (`nix shell nixpkgs#cargo-about -c tools/third-party-licenses.sh`)
  writes `crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt`: every crate's licence (cargo-about; `about.toml` lists
  the accepted, GPLv3-compatible ones and `about.hbs` the layout), then Lucide's and Google Sans Flex's. **Run it after any
  change to `Cargo.lock` and commit the file**: CI regenerates it and fails when it differs, or when a crate's
  licence isn't accepted. Release archives hold it, and the Credits page shows it. The crates are `publish = false`
  (cargo-about leaves them out). Something embedded that isn't a crate (a font, icons) gets its own section there.
- The game's names, numbers and texts (descriptions of characters, talents, constellations, weapons) are embedded
  (`crates/wish-core/data/`): short, needed offline, and what the wikis and genshin-db publish; the user chose to
  keep them (review LEG-2, option 1). Its images are not (below).
- `README.md` is the user-facing doc (features, data folder, building, updating the data, credits). Keep it in step
  when a feature, a command or a data source changes. Its screenshots (`docs/screenshots/`) come from the
  screenshot tool on a synthetic save (`tools/sample-save.mjs`).
`PLAN.md` is the roadmap: read it before starting a milestone, and tick its checkboxes when a step is done.

## Environment and commands

- Dev shell: `~/Projects/Rust/flake.nix` (`nix develop ~/Projects/Rust`).
  It provides GPUI's Linux libraries and sets `LD_LIBRARY_PATH`. Without it the app compiles,
  but the window fails to open, because Vulkan/Wayland/X11 are loaded at runtime.
  - Agent sessions already run inside it (`IN_NIX_SHELL` is set): run `cargo` directly.
    **Never prefix commands with `nix develop ~/Projects/Rust -c`.** That flake is a plain folder,
    not a git repo, so every call copies all of `~/Projects/Rust`, `target/` folders included,
    into `/nix/store`, a new multi-GB copy after every build. It filled the disk twice.
- `cargo run -p wish-app`: run the app (binary `genshin-companion`).
- `cargo test --workspace`
- `cargo run -p wish-core --example wishctl -- [--data-dir DIR] <command>`: command-line import, export and summary.
  Commands: `import-paimon`, `check-paimon`, `import-snapshot`, `find-link`, `fetch-game`, `import-game`, `undo`,
  `export`, `summary`, `backup`.
  - `check-paimon <file>` compares the engine with a paimon.moe save without touching any data.
  - `fetch-game <link>` fetches and plans a game import without writing anything.
  - `summary` lists import ids for `undo`.
  Without `--data-dir` it uses the real app data folder, so point it at a scratch folder when experimenting.
- `node tools/extract-paimon-data.mjs`: regenerates `crates/wish-core/data/*.json` from `reference/`.
  Run it after `git -C reference pull` and commit the JSON. It fails on unknown featured keys;
  add real typos to `KEY_FIXES` in the script.
- `node tools/extract-genshin-db.mjs`: regenerates `crates/wish-core/data/planner.json` (the planner's characters,
  weapons, materials and costs) and `archive.json` (the character pages: profile, stats and growth curves, talents
  with their values at levels 1–15, constellations, art and icon names) from `genshin-db/`, a sparse clone of https://github.com/theBowja/genshin-db
  (`git clone --depth 1 --filter=blob:none --sparse …`, then `git sparse-checkout set src/data/English/characters
  src/data/English/talents src/data/English/weapons src/data/English/materials src/data/English/constellations
  src/data/image src/data/stats src/data/curve src/data/version`). The EXP tables,
  which genshin-db lacks, come from the game's data (AnimeGameData); icon names genshin-db lacks yet (the newest
  characters), from Project Amber's API. Run it after `git -C genshin-db pull` and commit the JSON. It fails on a
  cost naming an unknown material. Characters without talent data (the Manekins) are skipped, and listed.
- **The game's images are never in the repository or the builds** (they are HoYoverse's; the user chose not to
  redistribute them). `wish_core::images::Remote` names each image's cache file and its sources: paimon.moe's site
  for the wish icons, banner art and primogem (`paimon/…`), Project Amber (`gi.yatta.moe/assets/UI/<game file>.png`)
  for character, weapon, material, element, talent and constellation icons and wish art (`amber/…`). The app
  downloads each the first time a page shows it, into `<data dir>/cache/images` (`AppPaths::image_cache`).
  Never add game images (or a tool that packs them) back. README screenshots of the app are fine.
- `cargo clippy --workspace --all-targets -- -D warnings`: must stay clean, since CI enforces it.
- `cargo fmt --all`
- `bacon`: continuous check.
- CI (`.github/workflows/ci.yml`): fmt, clippy, test and release build on Linux, Windows and macOS, all `--locked`.
  Releases (`release.yml`, on a `v*` tag): run `ci.yml` first, then build each target without a cache and package
  it (`tools/package-release.sh`); only the last job, which compiles nothing, may write to the repository. Every
  action is pinned to a commit with its tag in a comment (Dependabot proposes updates); checkouts don't keep the
  token (`persist-credentials: false`). Keep it that way when adding a step.
  `audit.yml` runs cargo-deny (`deny.toml`: known vulnerabilities from RustSec, crates.io as the only source) on
  every push, every Monday and before each release. An unmaintained crate fails it only when it is a direct
  dependency (those under GPUI Kit wait for its updates); an advisory to live with goes in `ignore`, with a reason.
  Locally: `nix shell nixpkgs#cargo-deny -c cargo-deny --locked check advisories sources`.

## Layout

The three crates mirror a front-end's core / shared / features split, and Cargo enforces the direction:
`wish-app` → `wish-ui` → `wish-core`, never the reverse.

- `crates/wish-core`: model, banner/item data, SQLite storage, importers, pity / 50-50 engine, stats, planner costs.
  **Never depends on GPUI.** All logic and tests go here.
  - `model.rs`: domain types (`Pool`, `GachaType`, `Wish`, `WishTime`…). Text-stored enums use `text_enum!`.
  - `data.rs`: embedded `items.json` / `banners.json`, overridable from `<data dir>/game-data/`.
    `GameData::load` never fails: a copy that can't be read, or older than the embedded one (an item missing,
    banners ending earlier), is left out with a warning (Settings and the archive show it), so the wishes always open.
  - `images.rs`: where each of the game's images comes from (`Remote`: cache file and URLs) and `download` (a few
    checks: only images, at most 16 MB, never outside the cache (`in_cache`, also used for the blur), https only and
    no redirects; paimon.moe answers a missing file with its HTML page, which counts as missing). No GPUI; the app
    calls it on its background executor.
  - `archive.rs`: `ArchiveData`, the embedded `archive.json` (overridable like the rest): `CharacterPage` (profile,
    talents, constellations), `ArchiveData::stats` (base × growth curve at the level + the ascension phase's bonus),
    `Talent::values` (rows formatted as the game does: `P`, `F1P`, `F1`… see `format_value`). Keyed like the planner.
    `WeaponPage` (description, version, wish art name, `WeaponStats`: ATK and the secondary stat with their curves and
    the phases' ATK bonus, `Passive` with its text at refinements 1–5, or just one for unrefinable ones) and
    `ArchiveData::weapon_stats`.
  - `planner/`: the ascension planner. `data.rs`: `PlannerData`, the embedded `planner.json` (overridable like
    the wish data; a cost naming an unknown material is refused). `mod.rs`: `Level` (level and ascension phase:
    80 is phase 5 before ascending, 6 after), `CharacterGoal` / `WeaponGoal` (from → to, talents 1–10),
    `PlannerData::character_cost` / `weapon_cost` → `Requirement` (materials by game id, Mora included, plus the
    EXP), `Requirement::materials_with_exp` (EXP as books / ores, and its Mora). Game rules the data lacks (level
    caps, what books and ores give, 1 Mora per 5 character EXP or 10 weapon EXP) are constants there.
    `tests/planner.rs` checks totals the game is known for (90/10/10/10: 168 local specialties, 46 boss drops,
    36/96/129 common drops, 9/63/114 books, 18 weekly drops, 3 crowns; 8,362,650 EXP to 90).
    `plan.rs`: the player's plan (`PlanEntry`: a character with a `CharacterGoal`, a weapon with a `WeaponGoal`
    (`WeaponGoal::full(rarity)`, levels over `Level::weapon_steps(rarity)`: 70 at most for 1★ and 2★), a custom
    character, or a custom entry of earlier versions, a name and `CustomCost`s, only shown and removable now; a `CustomItem` is a known
    material or a typed name), `PlannerData::plan_totals` / `entry_totals` (EXP added up before it becomes books),
    `material_named` (a typed resource that names a known material counts as it).
    `custom.rs`: custom characters and weapons, like genshin-center's. `Role::WEAPON` (domain material, elite drop,
    common drop; an elite drop's lowest tier is 2★) and `PlannerData::weapon_roles`; `custom_weapon_cost` copies the
    costs of the first regular weapon of the same rarity (`weapon_templates`), materials swapped. Every character
    but the Traveler uses six `Role`s (`Role::CHARACTER`)
    (common, local specialty, boss, gem, talent book, weekly boss; told apart by the material's `kind`, and boss
    from weekly by ascension vs talents) in the same amounts. `PlannerData::roles` reads a character's,
    `families(role)` lists the series the characters use (picker options), and `custom_character_cost` costs the
    first regular character's goal with the custom materials in place of its own (a typed name for a tiered role
    becomes "Name (2★)"…). `inventory.rs`: `Inventory`, what the player has, by material id or name.
    `allocate.rs`: `PlannerData::allocate` shares the inventory out over the plan in its order, like
    genshin-center: each `EntryShare` has what the entry needs and what it still lacks after the entries before
    it; `Allocation::missing` is the plan's. A tier short is crafted 3 for 1 from the tiers below (after the
    entry's own needs of those tiers; crafting Mora not counted); EXP books and ores count for their EXP.
    `PlannerData::craft_families` / `craft_family`: the crafted series, grouped by (`kind`, `sort`) with
    consecutive rarities (gems, talent books, enemy drops, weapon materials).
  - `store/plan.rs` + migrations `003_planner.sql`, `004_inventory_and_custom_characters.sql` and
    `005_planner_weapons.sql` (a weapon entry has `weapon_key`, and its key in `name` too, which 003's CHECK wants),
    `006_planner_custom_weapons.sql` (`custom_weapon` tells a custom weapon from a custom character): `plan_entries`
    (`stars` set only for a custom character), `plan_custom_costs`, `plan_custom_materials` (a custom character's
    material per role); `Store::plan`, `add_plan_entry`, `update_plan_entry`, `remove_plan_entry`.
    `store/inventory.rs`: `inventory` (one row per material owned) with `Store::inventory` and `set_owned` (0
    removes the row). Each write is one transaction after at most one `before-planning` backup an hour. Costs are
    never stored. The plan and the inventory are app-wide (not per account) for now, and not yet in the snapshot
    export or the mirror.
  - `store/`: SQLite (`Store`), migrations in `store/schema/`, backups.
  - `import/`: parsers that build an in-memory `ImportBatch`; only `Store::apply_import` writes.
  - `snapshot.rs`: the app's lossless JSON export. `uigf.rs`: UIGF v4 export and parsing.
    `excel.rs`: paimon.moe's v3 spreadsheet, export and import.
  - `import/manual.rs`: batches for counting by hand (`+1`/`+10`, one 4★/5★), each an `ImportSource::Manual` import.
  - `engine.rs`: banners, pity, 50/50, guarantees (`engine::analyze`). `stats.rs`: summary numbers and monthly counts.
  - `import/paimon_check.rs`: compares the engine with the values paimon.moe stored in a save.
  - `import/api.rs` (link parsing, fetching behind a `Transport` trait), `import/game_cache.rs`
    (finding the link on disk), `import/api_plan.rs` (new wishes vs links vs held back).
    API tests use `api::tests::FakeApi`; never call the real API in tests.
    This is the acceptance test for any engine change.
  - `tests/engine.rs`: golden tests on `paimon-sample.json` and `paimon-engine.json` (an America account, with a pull
    between two banners). Their stored values follow paimon's rules; keep them that way when editing.
  - `tests/import.rs`: end-to-end data tests on `tests/fixtures/paimon-sample.json`.
    They also run on real exports placed in `tests/fixtures/private/`.
- `crates/wish-ui`: the design system (shared layer, like shadcn's `ui/`). Depends on `gpui-kit` and `wish-core`'s
  types only, never on app state. Components take plain data and render it.
  - `theme.json` + `theme.rs`: the look, **Material 3** (the user's choice, after the Material 3 tokens of their
    blog, les-archives-de-teyvat.vercel.app). Colours: surface `#F9F9FF` / `#111318`, cards on surface-container-low
    `#F3F3FA` / `#191C20`, inner tiles on surface-container-high, primary `#3E5F90` / `#A7C8FF` (also the current
    segment of a `Segmented`, the user's choice: M3's tonal fill and tick made it change width), the secondary
    container (`#D9E3F8` / `#3D4758`) for the drawer's active indicator. `accent` is
    the hover state layer (on-surface at 8%), drawn over a surface rather than in place of it. Default buttons are
    M3's outlined buttons (`button.*` tokens), `.primary()` the filled ones, `.ghost()` the text ones. Shapes:
    `radius` 20 (pills at 40–44 px), `radius.lg` 28 (cards, dialogs, fields). The layout and the rarity colours
    (5★ `#FFB13F`, 4★ `#D28FD6`) stay paimon.moe's. Raw colours live only in `theme.json`. `theme.rs` registers
    Google Sans Flex (`fonts/`, SIL OFL: five static instances of the variable font, made with fonttools'
    `varLib.instancer` at opsz 18, wdth 100, ROND 0) and the themes, and gives rarity colours and icon tiles.
    Earlier looks the user turned down: a muted navy-and-gold "designed" one (AI-generic), paimon.moe's purple-navy,
    then the game-menu "Teyvat" theme for M3. Keep the contrast high.
  - `components/`: `PageFrame`, `NavItem` (M3 navigation drawer entry, a 56 px pill; `nested` for a feature's pages),
    `NavGroup` (a feature in the sidebar, folding), `card`, `card_title`, `MaterialTile` (a material and its count on its
    rarity tile; `.done` greys it out, `bare` drops the count, no icon shows "?"), `rich_text` (game text whose `**…**` terms turn semibold), `stat` (title, value,
    description), `stat_tile`, `big_number`, `stat_label`, `badge`, `outcome_badge`, `item_tile`,
    and the controls (`controls.rs`, `segmented.rs`, `pager.rs`): `button`, `icon_button`, `select`, `input`,
    `filter_chip` (M3 filter chip), `Segmented` (M3 segmented button, for a view or value choice; the current one
    filled with the primary colour, no tick), `Pager`, and `dialog_footer` / `dialog_close_footer`; `snackbar`
    (M3 snackbar: inverse colours, 4 px corners, a close button; shown through the app's `notify`). GPUI Kit's controls are 32 px with 14 px text, which looks like another
    app next to the app's proportions: **never use `Button::new`, `Select::new`, `Input::new` or the kit's `Pagination`,
    `ButtonGroup` and `Toggle` directly in the app; use these** (`button_style` sizes a button the kit builds, such
    as a dialog's close button). Alert dialogs get `.footer(dialog_footer(button(…).danger()…))`, since their
    built-in OK and Cancel can't be sized.
    **Everything clickable shows the pointing hand** (`cursor_pointer`): these helpers do it, since the kit gives
    every button but a link the arrow. A disabled button takes `.unavailable(bool)` (`wish_ui::ButtonExt`), not the
    kit's `.disabled(…)`, so it gets the arrow back; a button that does nothing (the current tab or page) gets
    `.cursor_default()`. Anything else clickable (a `div` with `on_click`, a kit `Button`, `Slider` or `Switch`)
    takes `.cursor_pointer()` itself.
    A new piece used by two pages or more goes here; one used by a single page stays in its feature.
  - `assets.rs`: `UiAssets` (component icons plus the Lucide icons listed in `icon_assets!`) and `pool_icon`.
  - `format.rs`: number and label text.
  - Charts: the user wants as few bar charts as possible; prefer areas, lines with dots, donuts. Give a chart
    card a definite height and its chart `flex_1().min_h_0()`, never a `min_h` larger than its row.
  - `axis.rs`: tick counts and axis tops that keep a counting chart's ticks on whole numbers (the kit spaces
    ticks evenly from zero to the maximum, so a maximum of 5 labels 1.25 as "1"). `legend` and `pity_chart` (in `components/`)
    names a chart's series. Series colours come from `theme::rarity_chart` (`chart.2`/`.3`/`.4` in `theme.json`,
    validated with the dataviz script against both card surfaces), not from the text colours of `theme::rarity`.
  - `examples/gallery.rs`: renders every component offscreen, light and dark:
    `cargo run -p wish-ui --example gallery -- <out-dir>`.
- `crates/wish-app`: the window. Keep it thin: it calls `wish-core`, renders with `wish-ui`.
  It is a library (so examples and tests can open the real window) plus the `genshin-companion` binary.
  **How data flows, and why, is documented at the top of `src/lib.rs`; follow it and update it when it changes.**
  In short:
  - each piece of state has one owner: data and settings → `Library`; selected account and the flow in progress
    → `Session`; the page shown → `Shell`; app-wide commands → GPUI actions (`ImportFile`, `ImportFromGame`);
  - others hold an `Entity<…>`, read it and `observe` it; they change it only through its methods (fields stay
    private). Don't copy shared state into pages;
  - one flow for the whole window: start async flows with `Session::start_flow`, which cancels the previous one;
  - a page without state of its own is a `RenderOnce` component with props (`Overview`, `Credits`); a page with
    state that must survive a render is an entity created once by the shell (`HistoryView`, `SettingsPage`,
    and `ImportFlow`, a service with no view). Pages don't know each other: they ask the shell to navigate
    (`on_navigate`).
  - `app/`: state and services, no views.
    - `library.rs`: the `Library` entity, which owns the store, game data, analyses and settings.
      Every database write goes through `Library::write`, which runs it on the background executor. Writes queue
      on the store lock and run detached, so they finish even if the caller drops the returned task. The state is
      reloaded after a failed write too, which undoes what a page showed at once (the planner's sliders, order and
      inventory); a panic in a write becomes its error (the lock it poisoned is taken back: each write is one
      transaction); quitting waits up to 5 s for the writes under way (`on_app_quit`). Report a write's error to the
      user (the planner's `report_errors`); never `.detach()` it silently. Settings change through
      `change_settings` only: at once in memory, saved in order (an older save never overwrites a newer one). A
      `settings.json` that doesn't parse is renamed `settings.json.damaged` and Settings says so
      (`Settings::load_or_set_aside`).
    - `session.rs`: the `Session` entity: selected account (kept valid when the data changes) and the one flow.
    - `images.rs`: `Images`, the game's images by what they show (`item`, `banner`, `character`, `material`,
      `skill`, `splash`…). It answers from the files it knows are in the cache and queues the others; the
      `Library`'s downloader (`download_images`, 6 at a time on the background executor) fetches them and redraws.
      Until then, or offline, the page draws without the image. A failed download is retried at the next launch;
      one the sites lack (404) leaves a `<file>.missing` marker, trusted for a week (`images::MISSING_FOR`).
      `Library::download_all_images(Collection::Icons | Everything)` downloads the whole `images::catalog` for
      offline use (Settings → Images, with progress and Stop); `Images::claim` keeps it and the pages from
      downloading one file twice. Downloads are opt-in beyond what pages show: don't add eager prefetching, it
      would load the two volunteer-run sites.
      `Library::open(paths, ImageDownloads::Off, cx)` turns downloads off: tests must never touch the network.
      Banner files are named by `Banner::slug()`, never by the raw name (a `:` would make GPUI take the path for a URL).
      The queue holds `Job`s: downloads, and `Job::Blur`, which makes the blurred backdrop of a downloaded wish art
      (`Images::backdrop`, `blur/<file>` in the cache: shrunk to 160 px and blurred with the `image` crate, which
      GPUI already builds; GPUI cannot blur an image itself).
    - `assets.rs`: `AppAssets`, the design system's icons (`wish_ui::UiAssets`).
    - `notices.rs`: messages about work done in the background, as M3 snackbars centred under the page:
      `notify(Notice::done(…) | Notice::failed(…), cx)` from anywhere. Done goes after 4 s (longer for a long
      message, 10 s at most); Failed stays until closed. At most 3 show, newest lowest. `Notices` is created and
      drawn by the shell (a global points `notify` at it), deferred at `NOTICES_PRIORITY` so it stays above
      dialogs. **Never use the kit's `window.push_notification`**: its toasts sit top right over the pages' buttons,
      and their hover close button can't be recoloured for the snackbar's inverse surface.
  - `shell/`: `Shell` (sidebar, routing, the page entities, forwarding actions) and `sidebar.rs`.
    `shell::content_width(window)` is a page's content width (window less `SIDEBAR_REMS` and `PAGE_PADDING_REMS`),
    for layouts that size from it (the planner's columns, the character hero).
    Routing is two levels: `Page::Wishes(WishPage::…)` for a feature's pages, `Page::Settings` / `Page::Credits`
    for the app's own. Each feature is a `Feature` with a `home()` page and a `NavGroup` in the sidebar (its pages
    as `NavItem::nested` under it); the header opens the feature, or folds its group when already there.
    The app will grow beyond wishes (an ascension planner like genshin-center's is planned): a new feature gets a
    `Feature` variant, a `Page` variant with its own page enum, a group in `sidebar.rs` and a folder in `features/`.
  - `features/`: one folder per feature, plus the app's own pages.
    - `wishes/`: the wish counter.
      - `overview/`: `Overview` (`RenderOnce`), top to bottom: `totals.rs` (key figures; two by two on a narrow
        page), one card per banner type (`pool_card.rs`: its latest banner and a link to its history, 5★ and 4★ pity
        with the next one's odds, its wishes, then `details.rs`, paimon.moe's per-type summary: 5★ and 4★ count,
        share and average pity, 50/50 record, 4★ characters vs weapons, then every 5★ as a pill coloured by pity),
        then the charts: `monthly_chart.rs` (wishes, 4★ and 5★ per month as overlaid areas, like paimon.moe) and the
        pity of each 5★ in time order (`wish_ui::pity_chart`, a line with the average dashed, shared with the
        history page). Everything about a banner type stays in its one card. The cards go as many a row as fit
        (`POOL_CARD_REMS` each) in even rows (four as two by two, `even_columns`) and stretch to the row's tallest;
        the charts sit side by side (two thirds, one third) or stack, at a fixed height (`CHART_REMS`). Totals and
        the 5★ in order come from `wish-core` (`Analysis::totals`, `pulls_of`), the per-type numbers from
        `PoolAnalysis::stats`.
      - `history/`: `HistoryView` (entity: observes `Library` and `Session`; the shell only calls `set_pool`),
        `table.rs` (`DataTable` delegate: large rows, sort by any column, pages of 25/50/100; the sort lives in the
        delegate so `TableState::refresh` keeps it; a "Type" column with rarity and character/weapon and a "#"
        column with the wish's number on its banner, `PoolAnalysis::banner_rolls`; 4★/5★ rows tinted from
        `render_tr`; the widths add up to 60.5 rem so it fits a 1280 px window; on a wider one the card measures
        itself with `on_prepaint` and the item and banner columns share the spare room), `charts.rs` (rarity donut,
        wishes per banner as an area, each 5★'s pity as a line; the strip hides on windows under 50 rem high, since
        the table scrolls on its own and must keep its room), `banners.rs` (banner view: the 5★ history, then a grid of banner cards, art on top, as many columns as fit), `manual_counting.rs`
        (+1/+10, add-item dialog, delete confirmation, all handled by the view itself).
      - `import/`: `ImportFlow` (entity, no view): pick a file, preview dialog, apply; `game.rs`: the
        "Import from the game" dialog; `import_file_button` / `import_game_button` send the actions.
    - `planner/`: the ascension planner, like genshin-center's (`Page::Planner`, group "Planner"). `PlannerView`
      (entity): a searchable picker adds a character (from 1 to 90/90, talents to 10: `CharacterGoal::FULL`); each
      character entry has range sliders (level over `Level::steps`, the three talents) that save on release, and
      the costs follow them while dragged. The "Still needed" card is on top (`Allocation::missing`), then the
      character cards in a grid of up to 4 columns (each card at least `CARD_REMS` wide, worked out from the
      window's width); a card's tiles show what the entry still lacks after the inventory share-out, greyed at 0 once
      covered. A card is dragged by its header (grip, picture, name: `DraggedEntry`) and dropped on another,
      whose place it takes (`Library::move_plan_entry`, `Store::reorder_plan`, one transaction): the order decides
      who gets the inventory first. While dragged, a copy of the card (`CardPreview`, drawn by `card_copy` at the
      card's width, ranges as filled bars) follows the pointer and the card itself is dimmed (`PlannerView::dragging`;
      never read the view's entity inside its own render, pass the view). Clicking a material opens `inventory.rs`: one material
      (how many the player has, add or subtract, a ley line's Mora), or a crafted series side by side, saved in one
      write (`Library::set_owned_many`). `custom.rs`: the custom character or weapon dialog (Character | Weapon when new, name, stars, one material per
      role; clicking a role swaps
      the form for a searchable grid of its series, or a typed name). Custom characters get the same sliders. The
      plan and inventory are the `Library`'s (`plan`, `add_plan_entry`, `update_plan_entry`, `remove_plan_entry`,
      `inventory`, `set_owned`); removing asks first. A character's archive page has "Add to the planner".
      `Shell::open_custom_entry` / `open_inventory` open the dialogs (the screenshot tool uses them).
    - `archive/`: the character archive, like Project Amber's (`Page::Archive(ArchivePage::Characters)`,
      group "Archive"). `CharactersView` (entity: filters, open character, tab, level sliders; the shell calls
      `show_list`, `show_character`, `set_tab`). `list.rs`: search, element / weapon / rarity filters, sort, the card
      grid (128 px icons on the rarity tile, element badge, "New" for the latest version). `detail.rs`: a hero banner (the
      blurred backdrop faded into the card under the text, the art drawn 1.3× the banner's height and cropped, the
      stars, description and facts), then tabs: Profile (stats at the slider's level and phase, and what reaching it costs, from
      `PlannerData::character_cost`), Talents (values at the talent-level slider, cost of one talent to it),
      Constellations.
    - `archive/weapons.rs`: `WeaponsView` (entity, `Page::Archive(ArchivePage::Weapons)`, the shell calls
      `show_weapon`): the weapon list (search, type and rarity filters, sort) and a weapon's page (the same
      `detail::hero_banner` with its upright wish art, stats at the level slider and their cost, the passive by
      refinement, "Add to the planner"). The planner has an "Add a weapon…" picker; a weapon card has a level
      range only (`Controls::steps` holds its rarity's levels).
    - `settings/`: `SettingsPage` (entity) and its rows; `backups.rs`, `imports.rs` (undo), `export.rs`, `mirror.rs`,
      `images.rs` (the image cache: size, empty, download all for offline use).
    - `credits/`: `Credits` (`RenderOnce`). It must keep paimon.moe's and genshin-db's MIT notices
      (`licenses/paimon-moe-MIT.txt`, `licenses/genshin-db-MIT.txt`)
      and state that the game's art and names are HoYoverse's. Credit any new third-party asset there.
      "Open-source components" shows `THIRD-PARTY-LICENSES.txt` in a dialog (`open_licenses`, a uniform list of its
      lines wrapped at 96 characters; `Shell::open_licenses` for the screenshot tool).
  - `tests/shell.rs`: headless tests of the window model (downloads off). `examples/screenshot.rs`: renders pages
    offscreen to PNG; it downloads the images a page needs (network) and waits for them before each capture.
- `genshin-db/`: git-ignored, read-only sparse clone of genshin-db, the planner's data source. Never edit it.
  The app reads only `planner.json`, its own format, so a change of source only changes the extraction script.
- `reference/`: git-ignored, read-only clone of paimon-moe, the original Svelte app, used as a reference.
  Never edit it. The relevant files:
  - `src/functions/wish.js`: banner matching, pity, 50/50
  - `src/routes/wish/import.svelte`: API fetching (`getLog`), merge (`processWishes`)
  - `src/routes/wish/_counter.svelte`, `_summary.svelte`, `[id].svelte`: UI and stats
  - `src/data/banners.js`, `bannersDual.js`, `characters.js`, `weaponList.js`: game data
  - `src/stores/saveManager.js`: the old storage (what we are replacing)

## GPUI

- Depend on `gpui-kit` only (pinned `=0.9.1` in the workspace `Cargo.toml`). Never add `gpui` directly:
  `gpui-kit` pins its own GPUI snapshot (`gpui-pre`), and two GPUI versions in one build do not interoperate.
- `use gpui_kit::*;` is GPUI. Styled components are under `gpui_kit::component::…`.
  Startup is `gpui_kit::application()` → `gpui_kit::init(cx)` → `gpui_kit::open_window(...)`,
  which mounts the `Root` that dialogs and notifications need.
- This GPUI differs from older docs and examples. Check the real API in the sources rather than from memory:
  `~/.cargo/registry/src/index.crates.io-*/{gpui-kit-0.9.1,gpui-component-0.9.1,gpui-base-0.9.1,gpui-pre-0.3.12}`.
- Do blocking work (HTTP with `ureq`, file I/O, SQLite) on the background executor, never on the UI thread.
- Before UI work, read the GPUI Kit skills' guides (`gpui-kit-design-guides`, and the Coding Guides in `gpui-kit`).
  Colours come from `cx.theme()` tokens; spacing and sizes from rem helpers.
  Table column widths are computed from `window.rem_size()`.
- Review UI changes with the screenshot tool rather than by opening windows on the user's desktop:
  `cargo run -p wish-app --example screenshot -- <data-dir> <out-dir>`
  (fill `<data-dir>` with `wishctl --data-dir <data-dir> import-paimon <fixture>`).
  For realistic volume, `node tools/sample-save.mjs <out.json>` writes a synthetic ~600-wish paimon.moe save to
  import into a scratch folder (never use it for engine tests).
  `SCREENSHOT_HEIGHT=1500` renders a taller window to see a whole page, `SCREENSHOT_WIDTH=2000` a wider one,
  `SCREENSHOT_ONLY=overview,banners` only the captures whose names start with these; put `"manual_counting": true` in the
  data dir's `settings.json` to also shoot the counting controls and the add-item dialog.
  To try the real app on scratch data, set `GENSHIN_COMPANION_DATA_DIR=<dir>` (the former
  `GENSHIN_WISHES_DATA_DIR` still works).
- Known gotchas in this GPUI:
  - `KeyBinding::new("secondary-…")` panics, because `secondary` needs an OS it isn't given. Spell out `cmd-`/`ctrl-` per platform.
  - `Button::disabled` needs `Disableable` in scope; `v_flex()`/`h_flex()` on `div()` need `gpui_kit::base::StyledExt`.
  - `Sidebar` children must all be one type; render a lone `SidebarMenuItem` with `SidebarItem::render`.
  - Enabling `gpui-kit/test-support` pulls the weak `gpui-fast` crates into resolution with exact pins
    (`taffy =0.15.0`, …). If the lockfile conflicts, regenerate it rather than adding pins.
  - Headless captures: dialogs and images need real sleeps, `advance_clock` and `window.refresh()` before the shot
    (see `examples/screenshot.rs`); charts need `.appear(false)` or they draw empty.
  - Tests run off the main thread, where building the macOS platform (`platform::current_platform`) panics:
    headless tests give `HeadlessAppContext` a `NoopTextSystem` (`tests/shell.rs`). Examples run on the main thread,
    so they can use the platform's text system. Code compiled only for some platforms must not leave dead code on
    the others (CI runs clippy with `-D warnings` on Windows and macOS too).
  - Deferred layers paint by priority: GPUI Kit's dialogs at 10 and up, popups (a select's list, menus) at 100,
    tooltips at 200. An overlay of the app's own that must cover dialogs goes between (the notices, 50).
  - Only the icons in `gpui-kit-assets`' `default-icons.txt` are bundled (no `trash`; `delete` is a backspace glyph).
    Add others to `icon_assets!` in `assets.rs` and use them as `gpui_kit::assets::IconName::…`.
  - Chart colour: `theme.chart_2` (`#4E7CFF`) passes the `dataviz` validator on both card surfaces.
  - `img` sets its own aspect ratio when none is given, which overrides an explicit height: pass `.aspect_ratio(…)`
    with `ObjectFit::Cover`. Absolute children with `h_full()` don't resolve; give them explicit sizes.
  - Clipping (`overflow_hidden`) is rectangular: round the child itself to keep a card's corner.
  - The segmented `TabBar` paints its active tab with the page `background`, and `Toggle`'s checked state uses
    `accent` (also the hover colour); hence `wish_ui::Segmented` and `filter_chip`.
  - Without the bundled font, bold and black weights don't render (the default font lacks them). GPUI doesn't vary
    a variable font's weight: bundle static instances.
  - A kit `Button`'s hover state sets its background (and border) over the caller's: to keep a fill under the
    state layer, draw it on a wrapper (`NavItem`), and draw dividers as elements of their own (`Segmented`). Its
    selected state replays the caller's style, and has no hover: `.selected(true).bg(…)` keeps a fill (the
    current segment). In a column, an `img`'s own height wins over `aspect_ratio`: give it a height too (the banner
    cards size their art from the card's width).
  - `Button::custom(ButtonCustomVariant)` fades its colour to 20% at rest; for a filled state use `.primary()`.
    A caller's style on a kit `Button`, `Select` or `Input` (`.h_11()`, `.px_4()`, `.bg()`) is applied after the kit's
    own size, so it wins; their inner text size comes from `Size` (`.large()` gives 16 px).
    GPUI Kit's `SidebarMenuItem` is fixed at 28 px, hence `wish_ui::NavItem` (a `Button` with M3's proportions).
  - A centred window is clamped to the display; the screenshot tool opens tall windows at the origin instead.

## Data-safety rules (non-negotiable)

- Imports only add rows: `INSERT … ON CONFLICT (account_id, id) DO NOTHING`.
  Never use `INSERT OR IGNORE`: it also silently skips rows that fail a `CHECK`.
  Never delete or overwrite existing wishes as a side effect of an import.
  Report a stored wish whose content differs from the incoming one; don't replace it.
- What the game tells us about stored wishes is recorded beside them, never written into them.
  The game id of a migrated wish goes in `wish_links`.
- Every row an import adds carries its `import_id` (wishes, links, manual counters), so `Store::undo_import` can remove it.
  Undo takes a backup first and marks the import undone rather than deleting the log entry.
- Never hold the store lock during network requests.
- Importers never guess. An unknown item or an unparseable time fails the import with a clear message,
  because a wrong rarity corrupts every pity count after it.
- Take a SQLite backup before any import, migration, restore or bulk edit.
  The exceptions are counting by hand and editing the plan: at most one `before-counting` / `before-planning`
  backup an hour each (`Store::backup_for_counting` / `backup_for_planning`; any other backup starts the hour
  again). These hourly backups are kept apart (the newest `KEEP_HOURLY_BACKUPS`, 10) from the others (the newest
  `KEEP_BACKUPS`, 30), so a long session can't push the backups of real imports out. Backup names carry UTC times
  (`…-123Z-reason.db`); the older local-time names still read, and backups sort by the instant, not the name.
- A restored file may hold no trigger or view (the app makes none): it would act on the next import.
- One process at a time uses a data folder (`AppPaths::lock`, `<data dir>/.lock`); a second start says the app
  is already open. A failure before the main window (and every panic) goes to `genshin-companion-errors.txt` in the
  temporary folder, and before the window also to a small window (`startup_error.rs`): release builds on Windows
  have no console.
- Every write that changes several rows happens in a single transaction.
  A fetch is staged in memory and written only after the user confirms.
- Never persist computed values (pity, banner, 50/50, guarantee). Recompute them from raw rows.
- Schema changes go through migrations only: add a new `store/schema/NNN_*.sql` and append it to `MIGRATIONS`.
  Never edit a migration that has shipped. A database with a newer `user_version` must be refused untouched.
- Exports and mirror files are written atomically: temp file, fsync, rename.
- Tests that use real exports (they contain UIDs) go in `crates/wish-core/tests/fixtures/private/`, which is git-ignored.

## Domain rules

- `gacha_type`: 100 Beginners, 200 Standard, 301 and 400 Character Event (**shared pity**), 302 Weapon, 500 Chronicled.
  5★ pity is 90 (80 for weapons); 4★ pity is 10. Primogems per pull: 160.
- API times are server-local wall clock with no timezone. Banner times in paimon's data are UTC+8.
  For `timezoneDependent` banners, subtract `8 - server_offset` hours.
  Offsets: Asia/TW +8, China +8, America −5, Europe +1.
- Fetch with `lang=en-us` so names match the item data. Rarity comes from the API's `rank_type`.
  An unknown item must never fail an import.
- API retcodes: 0 ok, −101 link expired, −100 link cut or altered, −110 too frequent (retry with backoff).
  A page shorter than 20 wishes is the last one.
- When porting from paimon.moe, keep its numbers identical first (that's the acceptance test), then improve.
  Any deliberate difference gets a comment at the spot and a line in `PLAN.md`'s engine rules.
- Pity counts the wishes since the previous item of the same rarity, this one included.
  A 5★ does not reset the 4★ count.
- 50/50 only applies to Character and Weapon Event, using the banner's `featured` / `featured_rare` item keys.
