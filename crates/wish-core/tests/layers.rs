//! The domain's modules, the game's rules (`model`, `engine`, `stats`,
//! `planner`, `collection`, `archive`, `chronicle`), touch neither the database, nor the
//! network, nor files: they test alone, and any shell can use them. Only
//! `store`, `import`, `images`, `paths` and `settings` reach outside.
//!
//! This test reads their sources and fails on an import that crosses the
//! line, so the rule is checked rather than hoped for. Test modules (from the
//! `#[cfg(test)]` that ends a file) and comments are not read.

use std::fs;
use std::path::{Path, PathBuf};

/// The domain's files and folders, under `src/`.
const DOMAIN: [&str; 7] = [
    "model.rs",
    "engine.rs",
    "stats.rs",
    "planner",
    "collection",
    "archive.rs",
    "chronicle.rs",
];

/// What the domain must not use, and what it would reach.
const FORBIDDEN: [(&str, &str); 9] = [
    ("rusqlite", "the database"),
    ("crate::store", "the database"),
    ("ureq", "the network"),
    ("HttpTransport", "the network"),
    ("crate::import::api", "the game's API"),
    ("crate::images", "image downloads"),
    ("std::fs", "files"),
    ("crate::paths", "the data folder"),
    ("crate::settings", "the settings file"),
];

/// Crossings already there, each with what will remove it. One that is gone
/// fails the test too, so this list doesn't outlive them.
const KNOWN: [(&str, &str, &str); 2] = [
    (
        "planner/data.rs",
        "std::fs",
        "PlannerData::load reads the game-data folder's copy",
    ),
    (
        "archive.rs",
        "std::fs",
        "ArchiveData::load reads the game-data folder's copy",
    ),
];

fn sources(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            sources(&entry.unwrap().path(), files);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        files.push(path.to_owned());
    }
}

/// Whether `line` uses `what`: the path itself, or `fs` named in a
/// `use std::{…}` group.
fn uses(line: &str, what: &str) -> bool {
    line.contains(what)
        || (what == "std::fs"
            && line.contains("std::{")
            && line
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == "fs"))
}

#[test]
fn the_domain_reaches_no_database_network_or_file() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for part in DOMAIN {
        sources(&src.join(part), &mut files);
    }
    assert!(files.len() > 10, "the domain's files are found");

    let mut crossings = Vec::new();
    let mut seen = Vec::new();
    for file in &files {
        let name = file.strip_prefix(&src).unwrap();
        let name = name.to_string_lossy().replace('\\', "/");
        let text = fs::read_to_string(file).unwrap();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.starts_with("#[cfg(test)]") {
                break;
            }
            if line.starts_with("//") {
                continue;
            }
            for (what, reaches) in FORBIDDEN {
                if !uses(line, what) {
                    continue;
                }
                if KNOWN.iter().any(|(f, w, _)| *f == name && *w == what) {
                    seen.push((name.clone(), what));
                } else {
                    crossings.push(format!(
                        "{name}:{}: `{what}` reaches {reaches}: {line}",
                        number + 1
                    ));
                }
            }
        }
    }
    assert!(
        crossings.is_empty(),
        "the domain must stay pure; move this outside it (store/, import/, images.rs…):\n{}",
        crossings.join("\n")
    );
    let gone: Vec<String> = (KNOWN.iter())
        .filter(|(file, what, _)| !seen.iter().any(|(f, w)| f == file && w == what))
        .map(|(file, what, why)| format!("{file}: `{what}` ({why})"))
        .collect();
    assert!(
        gone.is_empty(),
        "no longer there; remove it from KNOWN:\n{}",
        gone.join("\n")
    );
}
