//! A button without fill is `ButtonExt::quiet` (`wish_ui`), never the kit's
//! `.ghost()`: the kit presses a ghost button with the filled buttons' colour
//! (the theme's `button.active`, parchment), a near-white flash on the app's
//! slate that hides the button's text.
//!
//! This test reads the sources of the app and of `wish-ui` (their examples
//! too) and fails on a `.ghost()` written in the code. Comments are not read.

use std::fs;
use std::path::{Path, PathBuf};

fn sources(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            sources(&entry.unwrap().path(), files);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        files.push(path.to_owned());
    }
}

#[test]
fn no_button_presses_with_the_filled_buttons_colour() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for dir in [
        "wish-app/src",
        "wish-app/examples",
        "wish-ui/src",
        "wish-ui/examples",
    ] {
        sources(&crates.join(dir), &mut files);
    }
    assert!(!files.is_empty());
    let mut found = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains(".ghost()") {
                let file = file.strip_prefix(&crates).unwrap().display();
                found.push(format!("crates/{file}:{}", number + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "a ghost button flashes parchment when pressed; use wish_ui's ButtonExt::quiet:\n{}",
        found.join("\n")
    );
}
