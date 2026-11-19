//! What a failure says reaches the player as it is: notices and dialogs show
//! an error's whole chain (`{err:#}`), so English words added as `anyhow`
//! context ("reading …") would show in every language. A failure is a
//! `wish_core::Error` (its messages are Fluent texts, `IoAction` names what was
//! done to a file) or a text of the app's (`anyhow!(t!(…))`).
//!
//! This test reads the app's sources and fails on a context written in the
//! code. Test modules (`tests.rs`, and from the `#[cfg(test)]` that ends a
//! file) and comments are not read.

use std::fs;
use std::path::{Path, PathBuf};

/// The ways to add words of the code's own to an error.
const FORBIDDEN: [&str; 4] = [
    ".context(format!(",
    ".context(\"",
    "with_context(|| format!(",
    "with_context(|| \"",
];

fn sources(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            sources(&entry.unwrap().path(), files);
        }
    } else if path.extension().is_some_and(|e| e == "rs")
        && path.file_name().is_some_and(|name| name != "tests.rs")
    {
        files.push(path.to_owned());
    }
}

#[test]
fn no_error_carries_english_words_of_the_code() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&src, &mut files);
    assert!(!files.is_empty());
    let mut found = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            if line.trim_start().starts_with("//") {
                continue;
            }
            if let Some(what) = FORBIDDEN.iter().find(|what| line.contains(*what)) {
                let file = file.strip_prefix(&src).unwrap().display();
                found.push(format!("src/{file}:{}: {what}", number + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "errors the player reads carry the code's own words; use wish_core::Error \
         (IoAction for a file) or a Fluent text:\n{}",
        found.join("\n")
    );
}
