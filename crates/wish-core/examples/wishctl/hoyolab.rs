//! HoYoLAB's commands: the cookie read from stdin with the terminal's echo
//! off, what HoYoLAB tells of an account, and a sync of its characters. The
//! cookie is never printed nor stored; it is dropped once its requests end.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};
use wish_core::{AppPaths, Store, service};

use super::collection::{print_notes, print_sync};

/// Reads the player's HoYoLAB cookie from stdin and prints what HoYoLAB
/// tells of their account: what the HoYoLAB sync needs to know before it is
/// built. Writes nothing to the data folder; never prints the cookie or the
/// authkey. With `raw`, saves HoYoLAB's answers there (they hold the UID and
/// the characters, never the cookie).
pub(crate) fn hoyolab_check(raw: Option<&Path>) -> Result<()> {
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

    let terminal = std::io::stdin().is_terminal();
    let mut line = String::new();
    {
        // A terminal shows what is pasted, and keeps it in its scrollback.
        let hidden = terminal.then(HiddenInput::start).flatten();
        if terminal {
            eprintln!(
                "Paste your HoYoLAB cookie as one line (ltuid_v2=…; ltoken_v2=…; ltmid_v2=…), \
                 then press Enter. It is used for this command only and never saved."
            );
            if hidden.is_some() {
                eprintln!("It won't show on the screen as you paste it.");
            } else {
                eprintln!("It will show on the screen: clear the terminal afterwards.");
            }
        }
        std::io::stdin().lock().read_line(&mut line)?;
    }
    if terminal {
        eprintln!();
    }
    // Wiped from memory once read, like the cookie's fields.
    let pasted = hoyolab::Secret::new(line);
    Ok(hoyolab::Cookie::parse(pasted.expose())?)
}

/// The terminal's echo off until dropped, so a pasted cookie neither shows
/// nor stays in the scrollback. `None` where it can't be turned off (no
/// `stty`, or not Unix).
struct HiddenInput;

impl HiddenInput {
    fn start() -> Option<Self> {
        stty("-echo").then_some(HiddenInput)
    }
}

impl Drop for HiddenInput {
    fn drop(&mut self) {
        stty("echo");
    }
}

/// Runs `stty` on the terminal (its standard input, inherited).
#[cfg(unix)]
fn stty(setting: &str) -> bool {
    std::process::Command::new("stty")
        .arg(setting)
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(unix))]
fn stty(_: &str) -> bool {
    false
}

/// Fetches the characters of one game account from HoYoLAB and shows what a
/// sync would change in its collection; with `apply`, writes it (after a
/// backup), into the account of that UID, made if there is none.
pub(crate) fn hoyolab_sync(
    paths: &AppPaths,
    uid: Option<&str>,
    from: Option<&Path>,
    apply: bool,
) -> Result<()> {
    use wish_core::archive::ArchiveData;
    use wish_core::planner::PlannerData;

    let (account, details) = match from {
        Some(file) => saved_details(file, uid)?,
        None => fetch_details(uid)?,
    };

    let mut store = Store::open(paths.clone())?;
    let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
    // Worked out as the app does, then into the suggested account, as
    // `import-game`: the one with the UID, else the only account without a
    // UID, else a new one.
    let preview = service::hoyolab::preview(account, details, &store, None, planner, archive)?;
    let account = &preview.account;
    let choice = &preview.choices[preview.suggested];
    let existing = choice.target.clone();
    let others = preview.choices.len() - 1;
    println!(
        "HoYoLAB: {} ({}, AR {}) → {}{}",
        account.uid,
        account.nickname,
        account.level,
        existing
            .as_ref()
            .map_or("a new account".to_owned(), |a| format!("“{}”", a.label)),
        if others > 0 {
            format!(" (suggested; {others} other choices in the app)")
        } else {
            String::new()
        }
    );
    let plan = &choice.plan;
    print_sync(plan, planner);
    print_notes(&plan.notes);

    if !apply {
        println!("\nNothing was written. `hoyolab-sync` writes it, after a backup.");
        return Ok(());
    }
    let target = existing.map(|existing| existing.id);
    let (_, applied, backup) =
        store.apply_sync(target, account, &preview.details, planner, archive)?;
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
    // As the app does it: every account behind the cookie.
    let fetched = service::hoyolab::fetch(
        cookie,
        &hoyolab::HttpTransport::new(),
        &AtomicBool::new(false),
    )?;
    let index = match uid {
        Some(uid) => fetched.iter().position(|(a, _)| a.uid == uid),
        None => (0..fetched.len()).max_by_key(|&i| fetched[i].0.level),
    };
    let Some(index) = index else {
        let known: Vec<&str> = fetched.iter().map(|(a, _)| a.uid.as_str()).collect();
        bail!(
            "no such Genshin account on this HoYoLAB account (it has: {})",
            known.join(", ")
        );
    };
    Ok(fetched.into_iter().nth(index).expect("found above"))
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
