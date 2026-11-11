//! Command-line access to the wish database: migrate paimon.moe data and
//! inspect it before the app has screens for it.
//!
//!     cargo run -p wish-core --example wishctl -- [--data-dir DIR] <command>

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};
use wish_core::data::GameData;
use wish_core::engine::{self, PoolAnalysis, RateUpOutcome};
use wish_core::import::api::{self, GachaLink};
use wish_core::import::{ImportBatch, ImportReport, api_plan, paimon, paimon_check};
use wish_core::model::{PRIMOGEMS_PER_WISH, Pool, Rarity};
use wish_core::snapshot::Snapshot;
use wish_core::stats::{RarityStats, RateStats};
use wish_core::store::BackupReason;
use wish_core::{AppPaths, Store};

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

DIR defaults to the app's data folder.";

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let paths = match take_option(&mut args, "--data-dir")? {
        Some(dir) => AppPaths::at(dir),
        None => AppPaths::from_os()?,
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

fn game(paths: &AppPaths, link: &str, apply: bool) -> Result<()> {
    let link = GachaLink::parse(link)?;
    let data = game_data(paths);
    let mut store = Store::open(paths.clone())?;
    let known = store.known_api_ids()?;
    let is_known = |uid: &str, id: &str| known.get(uid).is_some_and(|ids| ids.contains(id));
    let mut progress = |p: api::FetchProgress| {
        eprint!(
            "\r{:<16} page {:>3}, {} new wishes so far",
            p.gacha_type.pool().display_name(),
            p.page,
            p.fetched
        );
    };
    let fetched = api::fetch(
        &link,
        &api::HttpTransport::new(),
        api::Pacing::default(),
        &is_known,
        &mut progress,
        &AtomicBool::new(false),
    )?;
    eprintln!();

    let plan = api_plan::plan(fetched, &store, &data)?;
    let Some(target) = plan.targets.get(plan.suggested) else {
        println!("The game returned no wishes.");
        return Ok(());
    };
    let destination = match &target.target {
        api_plan::Target::New => "a new account".to_owned(),
        api_plan::Target::Existing(account) => format!("“{}”", account.label),
    };
    println!(
        "UID {}: {} new, {} matched with migrated wishes, {} held back → {destination}",
        plan.uid.as_deref().unwrap_or("?"),
        target.new_total(),
        target.linked,
        target.held_back_total(),
    );
    for warning in &target.batch.warnings {
        println!("Warning: {warning}");
    }
    if apply {
        let report = store.apply_import(target.batch.clone())?;
        print_report(&store, &report)?;
    }
    Ok(())
}

/// The game data, as the app would load it, with what was wrong with the
/// copies in the game-data folder on stderr.
fn game_data(paths: &AppPaths) -> GameData {
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

fn read(file: &Path) -> Result<(String, Vec<u8>)> {
    let bytes = fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let name = file
        .file_name()
        .map_or_else(|| "import".to_owned(), |n| n.to_string_lossy().into_owned());
    Ok((name, bytes))
}

fn import_paimon(paths: &AppPaths, file: &Path) -> Result<()> {
    let (name, bytes) = read(file)?;
    let data = game_data(paths);
    let batch = paimon::parse(&bytes, &data)
        .with_context(|| format!("reading {} as a paimon.moe save", file.display()))?
        .with_original(name, bytes);
    apply(paths, batch)
}

fn import_snapshot(paths: &AppPaths, file: &Path) -> Result<()> {
    let (name, bytes) = read(file)?;
    let batch = Snapshot::from_json(&bytes)
        .with_context(|| format!("reading {} as a snapshot", file.display()))?
        .into_batch()
        .with_original(name, bytes);
    apply(paths, batch)
}

fn apply(paths: &AppPaths, batch: ImportBatch) -> Result<()> {
    let detail = batch.detail.clone().unwrap_or_default();
    let in_file = batch.wish_count();
    let mut store = Store::open(paths.clone())?;
    let report = store.apply_import(batch)?;

    println!("{detail}: {in_file} wishes read, {} new", report.added());
    print_report(&store, &report)
}

fn print_report(store: &Store, report: &ImportReport) -> Result<()> {
    for account in &report.accounts {
        let uid = account.account.uid.as_deref().unwrap_or("no UID");
        let server = account
            .account
            .server
            .map_or("unknown server", |s| s.as_str());
        let status = if account.created { ", new account" } else { "" };
        println!("  {} ({uid}, {server}{status})", account.account.label);

        let stored = count_by_pool(store, account.account.id)?;
        for (pool, counts) in &account.pools {
            println!(
                "    {:<16} {:>5} new, {:>5} already stored, {:>5} in database",
                pool.display_name(),
                counts.added,
                counts.already_present,
                stored.get(pool).copied().unwrap_or(0),
            );
        }
    }
    if let Some(backup) = &report.backup {
        println!("Backup before import: {}", backup.display());
    }
    if let Some(archived) = &report.archived {
        println!("Original file kept at: {}", archived.display());
    }
    for warning in &report.warnings {
        println!("Warning: {warning}");
    }
    Ok(())
}

fn count_by_pool(
    store: &Store,
    account: wish_core::model::AccountId,
) -> Result<BTreeMap<Pool, usize>> {
    let mut counts = BTreeMap::new();
    for wish in store.wishes(account)? {
        *counts.entry(wish.pool()).or_default() += 1;
    }
    Ok(counts)
}

fn export(paths: &AppPaths, file: &Path) -> Result<()> {
    let store = Store::open(paths.clone())?;
    let snapshot = Snapshot::from_store(&store)?;
    snapshot.write_to(file)?;
    let wishes: usize = snapshot.accounts.iter().map(|a| a.wishes.len()).sum();
    println!(
        "Wrote {} account(s), {wishes} wishes to {}",
        snapshot.accounts.len(),
        file.display()
    );
    Ok(())
}

fn check_paimon(paths: &AppPaths, file: &Path) -> Result<()> {
    let (_, bytes) = read(file)?;
    let data = game_data(paths);
    let comparison = paimon_check::compare(&bytes, &data)
        .with_context(|| format!("reading {} as a paimon.moe save", file.display()))?;
    if comparison.matches() {
        println!(
            "All {} values paimon.moe stored (pity, 50/50, counters) match.",
            comparison.checked
        );
        return Ok(());
    }
    for difference in &comparison.differences {
        println!("{difference}");
    }
    bail!(
        "{} of {} values differ from paimon.moe's",
        comparison.differences.len(),
        comparison.checked
    )
}

/// The numbers paimon.moe's wish counter page shows, per account.
fn summary(paths: &AppPaths) -> Result<()> {
    let store = Store::open(paths.clone())?;
    let data = game_data(paths);
    println!("Database: {}", paths.database().display());

    let accounts = store.accounts()?;
    if accounts.is_empty() {
        println!("No accounts yet. Import a paimon.moe save with `import-paimon <file>`.");
    }
    for account in accounts {
        let analysis = engine::analyze(
            store.wishes(account.id)?,
            account.server,
            &store.manual_counters(account.id)?,
            &data,
        );
        let total = analysis.total_wishes();
        println!(
            "\n{} ({}, {}): {total} {}, {} primogems",
            account.label,
            account.uid.as_deref().unwrap_or("no UID"),
            account.server.map_or("server not set", |s| s.as_str()),
            plural(total, "wish", "wishes"),
            u64::from(total) * u64::from(PRIMOGEMS_PER_WISH),
        );
        for pool in &analysis.pools {
            print_pool(pool, &data);
        }
        let months: Vec<_> = analysis
            .monthly
            .iter()
            .map(|m| format!("{} {}", m.label(), m.total))
            .collect();
        println!("  Wishes per month: {}", months.join(", "));
    }

    let imports = store.recent_imports(10)?;
    if !imports.is_empty() {
        println!("\nRecent imports (undo one with `undo <id>`):");
        for record in imports {
            println!(
                "  #{:<4} {}  {:<11} {:<30} {:<12} {} added{}",
                record.id,
                record.imported_at,
                record.source,
                record.detail.as_deref().unwrap_or(""),
                record.account_label,
                record.added,
                if record.undone_at.is_some() {
                    ", undone"
                } else {
                    ""
                },
            );
        }
    }

    let backups = store.backups()?;
    match backups.first() {
        Some(newest) => println!(
            "\n{} backup(s), newest {} ({})",
            backups.len(),
            newest.taken_at,
            newest.path.display()
        ),
        None => println!("\nNo backups yet."),
    }
    Ok(())
}

fn print_pool(pool: &PoolAnalysis, data: &GameData) {
    let counters = pool.counters;
    let guaranteed = |yes: bool| if yes { " (next is rate-up)" } else { "" };
    let (g5, g4) = pool
        .guarantee
        .map_or(("", ""), |g| (guaranteed(g.five), guaranteed(g.four)));
    let manual = if pool.manual_counters {
        ", typed by hand"
    } else {
        ""
    };
    println!(
        "  {}: {} {}{manual}. 5★ pity {}/{}{g5}, 4★ pity {}/{}{g4}",
        pool.pool.display_name(),
        counters.total,
        plural(counters.total, "wish", "wishes"),
        counters.pity5,
        pool.pool.hard_pity_five(),
        counters.pity4,
        pool.pool.hard_pity_four(),
    );

    let stats = &pool.stats;
    let rarity_line = |name: &str, r: &RarityStats, rate: Option<RateStats>| {
        let average = r
            .average_pity()
            .map_or_else(|| "-".to_owned(), |a| format!("{a:.1}"));
        let rate = rate
            .and_then(|rate| {
                rate.win_rate().map(|w| {
                    format!(
                        ", rate-up won {} of {} ({:.0}%)",
                        rate.won,
                        rate.won + rate.lost,
                        w * 100.0
                    )
                })
            })
            .unwrap_or_default();
        println!(
            "    {name}: {} ({:.2}%), average pity {average}{rate}",
            r.count,
            r.share(stats.total) * 100.0,
        );
    };
    rarity_line("5★", &stats.five, stats.rate_five);
    rarity_line("4★", &stats.four, stats.rate_four);
    println!(
        "        {} character(s), {} weapon(s)",
        stats.four_character.count, stats.four_weapon.count
    );

    let fives: Vec<_> = pool
        .pulls
        .iter()
        .filter(|p| p.wish.rarity == Rarity::Five)
        .map(|p| {
            let outcome = match p.rate_up.map(|r| r.outcome()) {
                Some(RateUpOutcome::Won) => ", won 50/50",
                Some(RateUpOutcome::Lost) => ", lost 50/50",
                Some(RateUpOutcome::Guaranteed) => ", guaranteed",
                Some(RateUpOutcome::LostWhileGuaranteed) => ", off-banner while guaranteed?",
                None => "",
            };
            format!("{} ({}{outcome})", p.wish.name, p.pity.unwrap_or(0))
        })
        .collect();
    if !fives.is_empty() {
        println!("    5★ wishes: {}", fives.join(", "));
    }

    if pool.outside_banners > 0 {
        let latest = data
            .banners
            .for_pool(pool.pool)
            .last()
            .map(|b| b.end.to_string());
        println!(
            "    Note: {} wish(es) fall outside every known banner (data ends {}); \
             update the game data if they are recent",
            pool.outside_banners,
            latest.unwrap_or_default(),
        );
    }
    if pool.unknown_items > 0 {
        println!(
            "    Note: {} 4★/5★ wish(es) have an unknown item",
            pool.unknown_items
        );
    }
}

fn plural(count: u32, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}
