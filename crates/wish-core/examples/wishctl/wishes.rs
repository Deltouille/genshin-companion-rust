//! The wish history's commands: imports (paimon.moe, snapshots, the game),
//! the export, the check against paimon.moe, and the summary.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};
use wish_core::data::GameData;
use wish_core::engine::{self, PoolAnalysis, RateUpOutcome};
use wish_core::import::api::{self, GachaLink};
use wish_core::import::{ImportBatch, ImportReport, api_plan, paimon, paimon_check};
use wish_core::model::{PRIMOGEMS_PER_WISH, Pool, Rarity};
use wish_core::service::{self, export::ExportSummary};
use wish_core::snapshot::Snapshot;
use wish_core::stats::{RarityStats, RateStats};
use wish_core::{AppPaths, Store};

use super::{game_data, read};

pub(crate) fn game(paths: &AppPaths, link: &str, apply: bool) -> Result<()> {
    let link = GachaLink::parse(link)?;
    let data = game_data(paths);
    let store = Mutex::new(Store::open(paths.clone())?);
    let mut progress = |p: api::FetchProgress| {
        eprint!(
            "\r{:<16} page {:>3}, {} new wishes so far",
            p.gacha_type.pool().display_name(),
            p.page,
            p.fetched
        );
    };
    // As the app does it: the same service.
    let plan = service::game::fetch(
        &link,
        &api::HttpTransport::new(),
        api::Pacing::default(),
        &store,
        &data,
        &mut progress,
        &AtomicBool::new(false),
    )?;
    eprintln!();
    let mut store = service::lock(&store);

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

pub(crate) fn import_paimon(paths: &AppPaths, file: &Path) -> Result<()> {
    let (name, bytes) = read(file)?;
    let data = game_data(paths);
    let batch = paimon::parse(&bytes, &data)
        .with_context(|| format!("reading {} as a paimon.moe save", file.display()))?
        .with_original(name, bytes);
    apply(paths, batch)
}

pub(crate) fn import_snapshot(paths: &AppPaths, file: &Path) -> Result<()> {
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

pub(crate) fn export(paths: &AppPaths, file: &Path) -> Result<()> {
    let store = Store::open(paths.clone())?;
    let exported = service::export::snapshot(&store)?;
    wish_core::paths::write_atomic(file, &exported.bytes)?;
    if let ExportSummary::Snapshot { wishes, accounts } = exported.summary {
        println!(
            "Wrote {accounts} account(s), {wishes} wishes to {}",
            file.display()
        );
    }
    Ok(())
}

pub(crate) fn check_paimon(paths: &AppPaths, file: &Path) -> Result<()> {
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
pub(crate) fn summary(paths: &AppPaths) -> Result<()> {
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
