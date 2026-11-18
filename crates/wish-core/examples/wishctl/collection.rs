//! GOOD files, and how the commands print what a HoYoLAB sync or a GOOD
//! import would change in a collection.

use std::path::Path;

use anyhow::{Context, Result, bail};
use wish_core::service;
use wish_core::{AppPaths, Store};

/// Shows what importing the GOOD file `file` would change, into the account
/// with UID `uid` or named `label`, else the only account (a new "Main" when
/// there is none); with `apply`, writes it after a backup.
pub(crate) fn good(
    paths: &AppPaths,
    file: &Path,
    uid: Option<&str>,
    label: Option<&str>,
    apply: bool,
) -> Result<()> {
    use wish_core::planner::PlannerData;

    let good = service::good::read(file)?;
    let mut store = Store::open(paths.clone())?;
    let accounts = store.accounts()?;
    let target = match (uid, label) {
        (Some(uid), _) => Some(
            (accounts.iter())
                .find(|a| a.uid.as_deref() == Some(uid))
                .with_context(|| format!("no account has the UID {uid}"))?
                .id,
        ),
        (None, Some(label)) => Some(
            (accounts.iter())
                .find(|a| a.label == label)
                .with_context(|| format!("no account is named “{label}”"))?
                .id,
        ),
        (None, None) => match &accounts[..] {
            [] => None,
            [only] => Some(only.id),
            _ => bail!(
                "this data folder has several accounts: say which with --uid or --account ({})",
                (accounts.iter())
                    .map(|a| format!("“{}”", a.label))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
    };
    let planner = PlannerData::embedded();
    let source = good.source.clone();
    let preview = service::good::preview(good, &store, target, planner)?;
    println!(
        "GOOD file from {} → {}",
        if source.is_empty() {
            "an unnamed app"
        } else {
            &source
        },
        (preview.account.as_ref()).map_or("a new account, “Main”".to_owned(), |a| format!(
            "“{}”",
            a.label
        ))
    );
    let plan = &preview.plan;
    print_sync(&plan.collection, planner);
    println!("\nMaterials: {} changed", plan.materials.len());
    for change in &plan.materials {
        let name = (planner.material(change.id))
            .map_or_else(|| change.id.to_string(), |m| m.name.to_string());
        println!("  {name}: {} → {}", change.before, change.after);
    }
    print_notes(&plan.collection.notes);

    if !apply {
        println!("\nNothing was written. `import-good` writes it, after a backup.");
        return Ok(());
    }
    let (_, applied, backup) = store.apply_good(target, &preview.file, planner)?;
    println!(
        "\nWritten: {} changes. Backup: {}",
        applied.changes(),
        backup.display()
    );
    Ok(())
}

pub(crate) fn print_notes(notes: &[wish_core::collection::SyncNote]) {
    if !notes.is_empty() {
        println!("\nNotes:");
    }
    for note in notes {
        println!("  ! {note}");
    }
}

/// What a sync or a GOOD import adds and raises, identical lines once with
/// their count (a GOOD file's fodder comes by the hundred).
pub(crate) fn print_sync(
    plan: &wish_core::collection::SyncPlan,
    planner: &wish_core::planner::PlannerData,
) {
    use wish_core::collection::Change;
    use wish_core::planner::Level;

    let level = |level: Level| {
        const CAPS: [u8; 7] = [20, 40, 50, 60, 70, 80, 90];
        format!("{}/{}", level.level, CAPS[usize::from(level.phase.min(6))])
    };
    let name = |key: &str| {
        planner
            .character(key)
            .map_or(key.to_owned(), |c| c.name.to_string())
    };
    let weapon_name = |key: &str| {
        planner
            .weapon(key)
            .map_or(key.to_owned(), |w| w.name.to_string())
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
    fn tally<T>(changes: &[Change<T>]) -> (usize, usize, usize) {
        changes
            .iter()
            .fold((0, 0, 0), |(added, raised, kept), change| match change {
                Change::Added(_) => (added + 1, raised, kept),
                Change::Raised { .. } => (added, raised + 1, kept),
                Change::Unchanged(_) => (added, raised, kept + 1),
            })
    }
    /// Each line once, in the order first seen, with how many times it came.
    fn grouped(lines: impl Iterator<Item = String>) -> Vec<(String, usize)> {
        let mut grouped: Vec<(String, usize)> = Vec::new();
        for line in lines {
            match grouped.iter_mut().find(|(seen, _)| *seen == line) {
                Some((_, count)) => *count += 1,
                None => grouped.push((line, 1)),
            }
        }
        grouped
    }
    let show = |lines: Vec<(String, usize)>| {
        for (line, count) in lines {
            match count {
                1 => println!("  {line}"),
                _ => println!("  {line}  ×{count}"),
            }
        }
    };

    let (added, raised, kept) = tally(&plan.characters);
    println!("\nCharacters: {added} added, {raised} raised, {kept} unchanged");
    show(grouped(plan.characters.iter().filter_map(
        |change| match change {
            Change::Added(c) => Some(format!("+ {}", character(c))),
            Change::Raised { before, after } => Some(format!(
                "↑ {}  (was {})",
                character(after),
                character(before)
            )),
            Change::Unchanged(_) => None,
        },
    )));
    let (added, raised, kept) = tally(&plan.weapons);
    println!("\nWeapons: {added} added, {raised} raised, {kept} unchanged");
    show(grouped(plan.weapons.iter().filter_map(
        |change| match change {
            Change::Added(w) => Some(format!("+ {}", weapon(w))),
            Change::Raised { before, after } => {
                Some(format!("↑ {}  (was {})", weapon(after), weapon(before)))
            }
            Change::Unchanged(_) => None,
        },
    )));
}
