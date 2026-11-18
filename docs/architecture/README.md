# How Genshin Companion works

This folder explains the code: what each part is for, where it lives, how data moves between the parts, and the
rules each part keeps. It is written for whoever changes the code next, a person or an agent. `CLAUDE.md` at the
root holds the rules and commands every session needs; these pages hold the detail, to read when a task touches
their subject.

Read [overview.md](overview.md) first. Then the page for the part you are changing:

| Page | What it covers |
| --- | --- |
| [overview.md](overview.md) | The crates and their layers, where each thing is, how one feature spreads over the crates |
| [app-state.md](app-state.md) | `wish-app`: who owns which state, how pages read and change it, the write queue, flows, notices |
| [pages.md](pages.md) | `wish-app`'s pages: the shell and routing, the shape of a page, each feature's files |
| [storage.md](storage.md) | The data folder, SQLite (tables, migrations, backups), snapshots and the mirror, `settings.json` |
| [imports.md](imports.md) | Every way data comes in (files, the game, counting by hand, HoYoLAB, GOOD) and how undo works |
| [wishes.md](wishes.md) | The wish counter's engine: pools, pity, banners and time zones, 50/50, the summary numbers |
| [planner.md](planner.md) | The ascension planner: game data, costs, custom entries, allocation and crafting, resin, farming |
| [collection-archive.md](collection-archive.md) | What an account owns, how syncs and GOOD files change it, and the archive's pages |
| [game-data.md](game-data.md) | The embedded game data, where it comes from, the extraction scripts, overriding it |
| [images.md](images.md) | The game's images: never shipped, where each comes from, the cache, the downloader |
| [i18n.md](i18n.md) | Texts in every language (Fluent), the game's own texts, the checks |
| [ui.md](ui.md) | `wish-ui`, the design system: the look and the user's choices, components, charts |
| [gpui.md](gpui.md) | GPUI and GPUI Kit as this project uses them, and their pitfalls |
| [testing.md](testing.md) | The tests, their fixtures, the headless window tests, the screenshot tool, `wishctl` |
| [ci-release.md](ci-release.md) | CI, the dependency audit, licences, making a release |
| [recipes.md](recipes.md) | Step-by-step changes: a page, a feature, a migration, a setting, a text, an import source… |

## Keeping these pages true

A page that says something the code no longer does is worse than no page. When a change makes a sentence here
wrong, fix the sentence in the same change. `crates/wish-core/tests/docs.rs` checks that every repository path
these pages and `CLAUDE.md` name in backticks (`crates/…`, `tools/…`, `docs/…`) still exists, so a moved or
renamed file fails the tests until the docs follow.

Each fact has one home. The code's own comments (`//!` at the top of each file, `///` on items) say what that
file or item does; these pages say how the parts fit together and link to the files. When both would say the same
thing, the code says it and the page links to it.

Diagrams are [Mermaid](https://mermaid.js.org), which GitHub draws.
