//! The docs name files, and files move. This reads `CLAUDE.md` and every page
//! of `docs/architecture/`, and fails on a path in backticks (`crates/…`,
//! `tools/…`, `docs/…`, `.github/…`) that doesn't exist, or a link to a page
//! that doesn't: a file moved or renamed fails the tests until the docs
//! follow it.

use std::fs;
use std::path::{Path, PathBuf};

/// Where the paths the docs name start.
const ROOTS: [&str; 4] = ["crates/", "tools/", "docs/", ".github/"];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The documents to check: `CLAUDE.md` and the architecture pages.
fn documents(root: &Path) -> Vec<PathBuf> {
    let mut pages: Vec<PathBuf> = fs::read_dir(root.join("docs/architecture"))
        .expect("docs/architecture/ exists")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .collect();
    pages.sort();
    pages.insert(0, root.join("CLAUDE.md"));
    pages
}

/// Every text between backticks on a line, inline code only.
fn code_spans(text: &str) -> Vec<&str> {
    let mut spans = Vec::new();
    let mut in_block = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            continue;
        }
        let mut parts = line.split('`');
        parts.next();
        while let (Some(code), Some(_)) = (parts.next(), parts.next()) {
            spans.push(code);
        }
    }
    spans
}

/// Whether `span` names a path of the repository rather than a pattern: a
/// placeholder (`<code>`, `…`, `*`, `{a,b}`), a command or a git-ignored
/// folder that is only there on the user's computer is left alone.
fn as_path(span: &str) -> Option<&str> {
    let span = span.trim_end_matches(['/', ',', '.', ':']);
    let is_path = ROOTS.iter().any(|root| span.starts_with(root))
        && !span.contains(['<', '…', '*', '{', ' ', '(', '$'])
        && !span.contains("/private");
    is_path.then_some(span)
}

/// The targets of `[text](target)` links to other pages (not URLs).
fn page_links(text: &str) -> Vec<&str> {
    text.match_indices("](")
        .filter_map(|(at, _)| {
            let rest = &text[at + 2..];
            let target = &rest[..rest.find(')')?];
            let target = target.split('#').next().unwrap_or(target);
            (!target.is_empty() && !target.contains("://")).then_some(target)
        })
        .collect()
}

#[test]
fn every_path_the_docs_name_exists() {
    let root = repository();
    let mut missing = Vec::new();
    for document in documents(&root) {
        let text = fs::read_to_string(&document).unwrap();
        let name = document.strip_prefix(&root).unwrap().display().to_string();
        for path in code_spans(&text).into_iter().filter_map(as_path) {
            if !root.join(path).exists() {
                missing.push(format!("{name}: `{path}`"));
            }
        }
        let folder = document.parent().unwrap();
        for link in page_links(&text) {
            if !folder.join(link).exists() {
                missing.push(format!("{name}: link to {link}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these paths moved or went; update the docs:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_check_reads_paths_and_leaves_patterns() {
    assert_eq!(
        as_path("crates/wish-core/src/store/"),
        Some("crates/wish-core/src/store")
    );
    assert_eq!(
        as_path("tools/sample-save.mjs"),
        Some("tools/sample-save.mjs")
    );
    assert_eq!(as_path("crates/<crate>/locales"), None);
    assert_eq!(
        as_path("crates/wish-core/tests/fixtures/private/good/"),
        None
    );
    assert_eq!(as_path("cargo test --workspace"), None);
    assert_eq!(as_path("model.rs"), None);
    assert_eq!(
        code_spans("a `b` c `d`\n```\n`not this`\n```\n`e`"),
        ["b", "d", "e"]
    );
    assert_eq!(
        page_links("[a](a.md) [b](https://x.org) [c](c.md#part)"),
        ["a.md", "c.md"]
    );
}
