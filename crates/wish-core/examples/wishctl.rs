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
  hoyolab-check [--raw D] Read your HoYoLAB cookie from stdin and check what HoYoLAB tells of your
                          account, writing nothing (D: also save HoYoLAB's answers there, without
                          the cookie)
  hoyolab-fetch [--uid U] Read your HoYoLAB cookie from stdin and show what a sync of your characters
                          and their weapons would change (U: which game account; else the highest AR)
  hoyolab-sync [--uid U]  The same, then write it (into the account of that UID, made if needed)
                          (--from F: read the details hoyolab-check --raw saved, with --uid, instead
                          of asking HoYoLAB)

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

/// Reads the player's HoYoLAB cookie from stdin and prints what HoYoLAB
/// tells of their account: what the HoYoLAB sync needs to know before it is
/// built. Writes nothing to the data folder; never prints the cookie or the
/// authkey. With `raw`, saves HoYoLAB's answers there (they hold the UID and
/// the characters, never the cookie).
fn hoyolab_check(raw: Option<&Path>) -> Result<()> {
    let cookie = read_cookie()?;
    println!("Cookie fields kept: {}", cookie.names().join(", "));
    if !cookie.can_make_authkeys() {
        println!("  (no cookie_token_v2 with account_id_v2: no authkey for the wish history)");
    }

    let transport = hoyolab::HttpTransport::new();
    let client = hoyolab::Client::new(cookie, &transport);
    let accounts = client.game_accounts()?;
    println!("\nGenshin accounts: {}", accounts.len());
    for account in &accounts {
        println!(
            "  {}  {}  AR {}  {}",
            account.uid, account.region, account.level, account.nickname
        );
    }
    let Some(account) = accounts.iter().max_by_key(|a| a.level) else {
        bail!("no Genshin Impact account on this HoYoLAB account");
    };
    let save = |name: &str, value: &serde_json::Value| -> Result<()> {
        if let Some(dir) = raw {
            fs::create_dir_all(dir)?;
            let file = dir.join(name);
            fs::write(&file, serde_json::to_string_pretty(value)?)?;
            println!("  (saved to {})", file.display());
        }
        Ok(())
    };

    let list = client.characters_raw(account)?;
    save("characters.json", &list)?;
    let characters = client.characters(account)?;
    println!("\nCharacters of {}: {}", account.uid, characters.len());
    let ids: Vec<u32> = characters.iter().map(|c| c.id).collect();
    let details_raw = client.character_details_raw(account, &ids)?;
    save("details.json", &details_raw)?;
    let details = client.character_details(account, &ids)?;
    println!("Details: {} characters", details.len());

    // 1. The ascension phase: is it there, under another name?
    if let Some(first) = details_raw["list"].get(0) {
        let keys = |value: &serde_json::Value| -> String {
            value
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_default()
        };
        println!("\n1. Fields of a character's details: {}", keys(first));
        println!("   Fields of its base: {}", keys(&first["base"]));
    }
    let at_a_cap: Vec<String> = details
        .iter()
        .filter(|d| [20, 40, 50, 60, 70, 80].contains(&d.base.level))
        .map(|d| format!("{} (level {})", d.base.name, d.base.level))
        .collect();
    println!(
        "   At a level cap, where the phase is ambiguous: {}",
        if at_a_cap.is_empty() {
            "none".to_owned()
        } else {
            at_a_cap.join(", ")
        }
    );

    // 2. Talents: does a level include the +3 of constellations 3 and 5?
    println!("\n2. Talents of characters at C3 or more (a level above 10 includes the +3):");
    for detail in details.iter().filter(|d| d.base.constellation >= 3) {
        let talents: Vec<String> = detail
            .skills
            .iter()
            .filter(|s| s.skill_type == 1)
            .map(|s| format!("{} {}", s.name, s.level))
            .collect();
        let boosts: Vec<String> = detail
            .constellations
            .iter()
            .filter(|c| c.active && (c.pos == 3 || c.pos == 5))
            .map(|c| {
                let boosted = detail
                    .skills
                    .iter()
                    .find(|s| c.effect.contains(&s.name))
                    .map_or("?", |s| s.name.as_str());
                format!("C{} boosts {boosted}", c.pos)
            })
            .collect();
        println!(
            "   {} C{}: {} ({})",
            detail.base.name,
            detail.base.constellation,
            talents.join(", "),
            boosts.join("; ")
        );
    }

    // 3. How many talents of type 1 each character has (3, or 4 with an
    //    alternate sprint).
    let mut counts: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for detail in &details {
        let n = detail.skills.iter().filter(|s| s.skill_type == 1).count();
        counts.entry(n).or_default().push(&detail.base.name);
    }
    println!("\n3. Talents of type 1 per character:");
    for (n, names) in &counts {
        let shown: Vec<&str> = names.iter().take(8).copied().collect();
        println!(
            "   {n}: {} character(s), such as {}",
            names.len(),
            shown.join(", ")
        );
    }

    // 4. Characters and weapons the game data doesn't know by id.
    let planner = wish_core::planner::PlannerData::embedded();
    let unknown: Vec<String> = details
        .iter()
        .filter(|d| !planner.characters().iter().any(|c| c.id == d.base.id))
        .map(|d| format!("{} ({})", d.base.name, d.base.id))
        .collect();
    let unknown_weapons: Vec<String> = details
        .iter()
        .filter(|d| !planner.weapons().iter().any(|w| w.id == d.weapon.id))
        .map(|d| format!("{} ({})", d.weapon.name, d.weapon.id))
        .collect();
    println!(
        "\n4. Not in the game data by id: characters [{}], weapons [{}]",
        unknown.join(", "),
        unknown_weapons.join(", ")
    );

    // 5. The authkey for the wish history.
    print!("\n5. Authkey for the wish history: ");
    match client.authkey(account) {
        Ok(key) => println!("obtained ({} characters, not shown)", key.len()),
        Err(e) => println!("not obtained: {e}"),
    }
    Ok(())
}

/// Reads the player's HoYoLAB cookie from stdin: one line, `name=value`
/// pairs (from the browser's Storage tab, or a request's Cookie header).
fn read_cookie() -> Result<hoyolab::Cookie> {
    use std::io::{BufRead as _, IsTerminal as _};

    if std::io::stdin().is_terminal() {
        eprintln!(
            "Paste your HoYoLAB cookie as one line (ltuid_v2=…; ltoken_v2=…; ltmid_v2=…), then \
             press Enter. It is used for this command only and never saved."
        );
    }
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    // Wiped from memory once read, like the cookie's fields.
    let pasted = hoyolab::Secret::new(line);
    Ok(hoyolab::Cookie::parse(pasted.expose())?)
}

/// Fetches the characters of one game account from HoYoLAB and shows what a
/// sync would change in its collection; with `apply`, writes it (after a
/// backup), into the account of that UID, made if there is none.
fn hoyolab_sync(
    paths: &AppPaths,
    uid: Option<&str>,
    from: Option<&Path>,
    apply: bool,
) -> Result<()> {
    use wish_core::archive::ArchiveData;
    use wish_core::collection::{Change, Collection, SyncNote, plan_sync};
    use wish_core::model::Server;
    use wish_core::planner::{Level, PlannerData};

    let (account, details) = match from {
        Some(file) => saved_details(file, uid)?,
        None => fetch_details(uid)?,
    };
    let account = &account;

    let mut store = Store::open(paths.clone())?;
    let existing = store.account_with_uid(&account.uid)?;
    println!(
        "HoYoLAB: {} ({}, AR {}) → {}",
        account.uid,
        account.nickname,
        account.level,
        existing
            .as_ref()
            .map_or("a new account".to_owned(), |a| format!("“{}”", a.label))
    );
    let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
    let current = match &existing {
        Some(existing) => store.collection(existing.id)?,
        None => Collection::default(),
    };
    let plan = plan_sync(&details, &current, planner, archive);

    let level = |level: Level| {
        const CAPS: [u8; 7] = [20, 40, 50, 60, 70, 80, 90];
        format!("{}/{}", level.level, CAPS[usize::from(level.phase.min(6))])
    };
    let name = |key: &str| {
        planner
            .character(key)
            .map_or(key.to_owned(), |c| c.name.clone())
    };
    let weapon_name = |key: &str| {
        planner
            .weapon(key)
            .map_or(key.to_owned(), |w| w.name.clone())
    };
    let character = |c: &wish_core::collection::OwnedCharacter| {
        let [a, e, q] = c.talents;
        format!(
            "{} {} C{} talents {a}/{e}/{q}",
            name(&c.key),
            level(c.level),
            c.constellation
        )
    };
    let weapon = |w: &wish_core::collection::OwnedWeapon| {
        format!(
            "{} {} R{}{}",
            weapon_name(&w.key),
            level(w.level),
            w.refinement,
            w.holder
                .as_deref()
                .map_or(String::new(), |h| format!(", held by {}", name(h)))
        )
    };
    let count = |changes: &[bool]| changes.iter().filter(|c| **c).count();
    let added = plan
        .characters
        .iter()
        .map(|c| matches!(c, Change::Added(_)));
    let raised = plan
        .characters
        .iter()
        .map(|c| matches!(c, Change::Raised { .. }));
    println!(
        "\nCharacters: {} added, {} raised, {} unchanged",
        count(&added.collect::<Vec<_>>()),
        count(&raised.collect::<Vec<_>>()),
        plan.characters.iter().filter(|c| !c.changes()).count()
    );
    for change in &plan.characters {
        match change {
            Change::Added(c) => println!("  + {}", character(c)),
            Change::Raised { before, after } => {
                println!("  ↑ {}  (was {})", character(after), character(before));
            }
            Change::Unchanged(_) => {}
        }
    }
    let added = plan.weapons.iter().map(|w| matches!(w, Change::Added(_)));
    let raised = plan
        .weapons
        .iter()
        .map(|w| matches!(w, Change::Raised { .. }));
    println!(
        "\nWeapons: {} added, {} raised, {} unchanged",
        count(&added.collect::<Vec<_>>()),
        count(&raised.collect::<Vec<_>>()),
        plan.weapons.iter().filter(|w| !w.changes()).count()
    );
    for change in &plan.weapons {
        match change {
            Change::Added(w) => println!("  + {}", weapon(w)),
            Change::Raised { before, after } => {
                println!("  ↑ {}  (was {})", weapon(after), weapon(before));
            }
            Change::Unchanged(_) => {}
        }
    }
    if !plan.notes.is_empty() {
        println!("\nNotes:");
    }
    for note in &plan.notes {
        let text = match note {
            SyncNote::PhaseUnknown { name, level } => format!(
                "{name} is level {level}: HoYoLAB doesn't say whether they ascended; counted as not yet."
            ),
            SyncNote::AboveMaxLevel { name, level } => {
                format!("{name} is level {level}: counted as 90/90, the highest the app knows.")
            }
            SyncNote::UnknownCharacter { name, id } => {
                format!("{name} ({id}) isn't in the game data: left out.")
            }
            SyncNote::UnknownWeapon { name, id } => {
                format!("The weapon {name} ({id}) isn't in the game data: left out.")
            }
            SyncNote::TalentsUnread { name } => {
                format!("{name}'s talents couldn't be read: left as they were.")
            }
        };
        println!("  ! {text}");
    }

    if !apply {
        println!("\nNothing was written. `hoyolab-sync` writes it, after a backup.");
        return Ok(());
    }
    let target = match existing {
        Some(existing) => existing.id,
        None => {
            store
                .add_account(
                    &account.nickname,
                    Some(&account.uid),
                    Server::from_api_region(&account.region),
                )?
                .id
        }
    };
    let (applied, backup) = store.apply_sync(target, &details, planner, archive)?;
    println!(
        "\nWritten: {} changes. Backup: {}",
        applied.changes(),
        backup.display()
    );
    Ok(())
}

/// The details of one game account's characters, from HoYoLAB: `uid`'s, or
/// the account with the highest Adventure Rank. The cookie is dropped once
/// they are in.
fn fetch_details(
    uid: Option<&str>,
) -> Result<(hoyolab::GameAccount, Vec<hoyolab::CharacterDetail>)> {
    let cookie = read_cookie()?;
    let transport = hoyolab::HttpTransport::new();
    let client = hoyolab::Client::new(cookie, &transport);
    let accounts = client.game_accounts()?;
    let account = match uid {
        Some(uid) => accounts.iter().find(|a| a.uid == uid),
        None => accounts.iter().max_by_key(|a| a.level),
    };
    let Some(account) = account.cloned() else {
        let known: Vec<&str> = accounts.iter().map(|a| a.uid.as_str()).collect();
        bail!(
            "no such Genshin account on this HoYoLAB account (it has: {})",
            known.join(", ")
        );
    };
    let ids: Vec<u32> = client.characters(&account)?.iter().map(|c| c.id).collect();
    let details = client.character_details(&account, &ids)?;
    Ok((account, details))
}

/// The details `hoyolab-check --raw` saved in `file`, for the account `uid`.
fn saved_details(
    file: &Path,
    uid: Option<&str>,
) -> Result<(hoyolab::GameAccount, Vec<hoyolab::CharacterDetail>)> {
    #[derive(serde::Deserialize)]
    struct Saved {
        list: Vec<hoyolab::CharacterDetail>,
    }
    let Some(uid) = uid else {
        bail!("--from needs --uid: the file doesn't say whose characters they are");
    };
    let text = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let saved: Saved = serde_json::from_str(&text)?;
    let account = hoyolab::GameAccount {
        uid: uid.to_owned(),
        region: hoyolab::genshin_server(uid).unwrap_or_default().to_owned(),
        nickname: uid.to_owned(),
        level: 0,
        game_biz: "hk4e_global".to_owned(),
    };
    Ok((account, saved.list))
}
