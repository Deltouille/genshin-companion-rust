//! `wishctl`: a data folder from the terminal, without the window. It
//! imports and exports, prints a summary paimon.moe's counter page would show,
//! undoes an import, and checks what an import from the game, a HoYoLAB sync or
//! a GOOD file would change before writing it. Each command goes through the
//! same `service` and `Store` calls as the app, so it is also how to try an
//! importer on a real file, or work on the data where no window can open.
//!
//!     cargo run -p wish-core --example wishctl -- [--data-dir DIR] <command>
//!
//! Without `--data-dir` it uses the app's real data folder: point it at a
//! scratch folder when experimenting. The commands, one file each: `wishes`
//! (the wish history), `hoyolab` (the cookie and HoYoLAB), `collection` (GOOD
//! files, and what a sync or a GOOD import changes).

mod collection;
mod hoyolab;
mod wishes;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use wish_core::data::GameData;
use wish_core::store::BackupReason;
use wish_core::{AppPaths, Store};

use collection::good;
use hoyolab::{hoyolab_check, hoyolab_sync};
use wishes::{check_paimon, export, game, import_paimon, import_snapshot, summary};

const USAGE: &str = "\
Usage: wishctl [--data-dir DIR] <command>

Commands:
  import-paimon <file>    Import a paimon.moe save (Settings → Export, or the Google Drive file)
  check-paimon <file>     Compare our pity and 50/50 results with those stored in a paimon.moe save
  import-snapshot <file>  Merge a snapshot written by `export`
  find-link               Look for the wish history link in the game's cache on this computer
  fetch-game <link>       Fetch the game's wish history and show what an import would add
  import-game <link>      Fetch the game's wish history and import it (into the suggested account)
  undo <import-id>        Remove what one import added (ids are listed by `summary`)
  export <file>           Write every account to a snapshot file
  summary                 Show counters, pity, 50/50 and backups for every account
  backup                  Take a backup now
  hoyolab-check [--raw D] Read your HoYoLAB cookie from stdin and check what HoYoLAB tells of your
                          account, writing nothing (D: also save HoYoLAB's answers there, without
                          the cookie)
  hoyolab-fetch [--uid U] Read your HoYoLAB cookie from stdin and show what a sync of your characters
                          and their weapons would change (U: which game account; else the highest AR)
  hoyolab-sync [--uid U]  The same, then write it (into the account of that UID; else the only one
                          without a UID, which takes it; else a new one)
                          (--from F: read the details hoyolab-check --raw saved, with --uid, instead
                          of asking HoYoLAB)
  check-good <file>       Show what importing a GOOD file (Irminsul, Inventory Kamera, Genshin
                          Optimizer) would change, writing nothing: into the account of --uid U or
                          --account NAME, else the only one (a new “Main” when there is none)
  import-good <file>      The same, then write it, after a backup

DIR defaults to the app's data folder.";

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let paths = match take_option(&mut args, "--data-dir")? {
        Some(dir) => AppPaths::at(dir),
        None => AppPaths::from_os()?,
    };
    let raw = take_option(&mut args, "--raw")?;
    let uid = take_option(&mut args, "--uid")?;
    let from = take_option(&mut args, "--from")?;
    let label = take_option(&mut args, "--account")?;

    // One process at a time writes to a data folder: a command that writes
    // takes its lock, as the app does, and stops while the app has it.
    let writes = matches!(
        args.first().map(String::as_str),
        Some(
            "import-paimon"
                | "import-snapshot"
                | "import-game"
                | "undo"
                | "backup"
                | "hoyolab-sync"
                | "import-good"
        )
    );
    let _lock = if writes {
        let lock = paths.lock()?;
        Some(lock.with_context(|| {
            format!(
                "the app is open on this data folder ({}): close it first",
                paths.root().display()
            )
        })?)
    } else {
        None
    };

    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["import-paimon", file] => import_paimon(&paths, Path::new(file)),
        ["check-paimon", file] => check_paimon(&paths, Path::new(file)),
        ["import-snapshot", file] => import_snapshot(&paths, Path::new(file)),
        ["find-link"] => {
            match wish_core::import::game_cache::find_link() {
                Some(found) => println!("{}\n(from {})", found.url, found.cache_file.display()),
                None => println!("No link found. Open the wish history in the game first."),
            }
            Ok(())
        }
        ["fetch-game", link] => game(&paths, link, false),
        ["import-game", link] => game(&paths, link, true),
        ["undo", id] => {
            let mut store = Store::open(paths)?;
            let report = store.undo_import(id.parse().context("an import id is a number")?)?;
            println!(
                "Removed {} wishes, {} game ids and {} manual counters. Backup: {}",
                report.removed_wishes,
                report.removed_links,
                report.removed_counters,
                report.backup.display()
            );
            Ok(())
        }
        ["export", file] => export(&paths, Path::new(file)),
        ["summary"] => summary(&paths),
        ["hoyolab-check"] => hoyolab_check(raw.as_deref().map(Path::new)),
        ["hoyolab-fetch"] => hoyolab_sync(
            &paths,
            uid.as_deref(),
            from.as_deref().map(Path::new),
            false,
        ),
        ["hoyolab-sync"] => {
            hoyolab_sync(&paths, uid.as_deref(), from.as_deref().map(Path::new), true)
        }
        ["check-good", file] => good(
            &paths,
            Path::new(file),
            uid.as_deref(),
            label.as_deref(),
            false,
        ),
        ["import-good", file] => good(
            &paths,
            Path::new(file),
            uid.as_deref(),
            label.as_deref(),
            true,
        ),
        ["backup"] => {
            let store = Store::open(paths)?;
            println!(
                "Backup written to {}",
                store.backup(BackupReason::Manual)?.display()
            );
            Ok(())
        }
        [] | ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => bail!("unknown command\n\n{USAGE}"),
    }
}

/// The game data, as the app would load it, with what was wrong with the
/// copies in the game-data folder on stderr.
pub(crate) fn game_data(paths: &AppPaths) -> GameData {
    let (data, warnings) = GameData::load(&paths.game_data());
    for warning in warnings {
        eprintln!("game-data: {warning}");
    }
    data
}

fn take_option(args: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    let Some(at) = args.iter().position(|a| a == name) else {
        return Ok(None);
    };
    if at + 1 >= args.len() {
        bail!("{name} needs a value");
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Ok(Some(value))
}

pub(crate) fn read(file: &Path) -> Result<(String, Vec<u8>)> {
    let bytes = fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let name = file
        .file_name()
        .map_or_else(|| "import".to_owned(), |n| n.to_string_lossy().into_owned());
    Ok((name, bytes))
}
