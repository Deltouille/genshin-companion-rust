# CI, licences, releases

## The workflows

| Workflow | When | What |
| --- | --- | --- |
| `.github/workflows/ci.yml` | every push to `main` and every pull request; called by the release | `fmt` (the rustfmt check), `licences` (the third-party licences file is current), `build` on Linux, Windows and macOS (clippy `-D warnings`, the tests, a release build; on Linux also rustdoc with warnings as errors, so a doc link to an item that moved fails), all `--locked` |
| `.github/workflows/audit.yml` | every push, every Monday, before each release | cargo-deny (`deny.toml`): known vulnerabilities from RustSec, crates.io as the only source |
| `.github/workflows/release.yml` | a `v*` tag | runs `ci.yml` and `audit.yml`, builds each target without a cache, packages it, publishes the release |

- **Every action is pinned to a commit**, its tag in a comment (Dependabot proposes updates), and checkouts don't keep
  the token (`persist-credentials: false`). Keep it that way when adding a step.
- **The release profile** (the workspace's `Cargo.toml`) uses thin LTO and one codegen unit: about a fifth
  smaller a binary for a longer build. Never `panic = "abort"`: a write that panics becomes an error
  (`Library::write`), which needs panics to unwind.
- **Rust is pinned**: `rust-toolchain.toml` and each workflow's `toolchain:` (1.100.0), so two builds of a commit
  are the same. Dependabot doesn't raise it: raise both together. The Nix dev shell brings its own Rust.
- **The audit**: an unmaintained crate fails it only when it is a direct dependency (those under GPUI Kit wait for
  its updates); an advisory to live with goes in `deny.toml`'s `ignore`, with a reason. Locally:
  `nix shell nixpkgs#cargo-deny -c cargo-deny --locked check advisories sources`.

## Licences

The app is GPL-3.0-or-later (`LICENSE`, `license.workspace` in every crate). GPLv2 is not an option: many
dependencies, GPUI Kit among them, are Apache-2.0 only. **Check that a new dependency's licence is
GPLv3-compatible.**

`tools/third-party-licenses.sh` (`nix shell nixpkgs#cargo-about -c tools/third-party-licenses.sh`) writes
`crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt`: every crate's licence (cargo-about; `about.toml` lists the
accepted, GPLv3-compatible ones, `about.hbs` the layout), then the sections for what isn't a crate (Lucide's icons,
Inter). **Run it after any change to `Cargo.lock` and commit the file**: CI regenerates it and fails when
it differs, or when a crate's licence isn't accepted. Release archives hold it, and Credits shows it. The
workspace's crates are `publish = false`, so cargo-about leaves them out.

Credits (`crates/wish-app/src/features/credits/`) keeps paimon.moe's, genshin-db's and genshin.py's MIT notices
(`crates/wish-app/licenses/`), names Project Amber, and states that the game's art and names are HoYoverse's.
Credit any new third-party asset there and in the licences file.

## Making a release

1. Set the version in the workspace's `Cargo.toml` (and let `Cargo.lock` follow).
2. Write `docs/releases/v<version>.md`: what's new, for players. GitHub shows a release's newlines as line breaks,
   so write each paragraph and list item on one line. There is no GitHub login here to edit a release afterwards.
3. Commit, then push a tag of that version: `git tag v<version> && git push origin v<version>`.

The release workflow checks the tag matches the version (a tag that doesn't publishes nothing), runs CI and the
audit, builds each target (Linux on Ubuntu 22.04 for older glibc, Windows, macOS Apple Silicon and Intel) without a
cache, packages each with `tools/package-release.sh`, and only the last job, which compiles nothing, writes to the
repository: it publishes the release with the notes file, then GitHub's generated list of changes, the archives and
`SHA256SUMS.txt`.

The README's Download section names the archives and the first launch of unsigned builds: keep it in step.

If a runner is unavailable (the macOS Apple Silicon one has been), the release job is skipped and nothing is
published: re-run the failed jobs, or release the next patch version.
