//! The game's texts in the other languages, `data/<code>.json`, written by
//! `tools/extract-genshin-db.mjs` from genshin-db: by key (materials by id),
//! what `planner.json` and `archive.json` hold in English. The planner and
//! archive data merge them in as they load ([`translations`]); a text a
//! language lacks stays in English.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::{Language, language};

/// One language's `data/<code>.json`.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct GameTexts {
    #[serde(default)]
    pub characters: HashMap<String, CharacterTexts>,
    #[serde(default)]
    pub weapons: HashMap<String, WeaponTexts>,
    #[serde(default)]
    pub materials: HashMap<u32, MaterialTexts>,
    /// Every character's and weapon's name, by its English one: the wish
    /// history names its items as the game did, in English.
    #[serde(default)]
    pub names: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct CharacterTexts {
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub region: Option<String>,
    pub affiliation: Option<String>,
    pub birthday: Option<String>,
    pub constellation: Option<String>,
    /// In the order of the archive's talents.
    #[serde(default)]
    pub talents: Vec<TalentTexts>,
    #[serde(default)]
    pub constellations: Vec<NamedTexts>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct TalentTexts {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Each row's label and template, the same parameters as the English
    /// rows; none when they don't match.
    pub rows: Option<Vec<(String, String)>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct NamedTexts {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct WeaponTexts {
    pub name: Option<String>,
    pub description: Option<String>,
    pub passive: Option<PassiveTexts>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct PassiveTexts {
    pub name: String,
    pub refinements: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct MaterialTexts {
    pub name: Option<String>,
    pub domain: Option<String>,
}

/// Each language's texts but English's, compiled into this build.
static TRANSLATIONS: LazyLock<Vec<(Language, GameTexts)>> = LazyLock::new(|| {
    [(Language::French, include_str!("../../data/fr.json"))]
        .into_iter()
        .map(|(language, json)| {
            let texts = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("data/{}.json doesn't read: {e}", language.code()));
            (language, texts)
        })
        .collect()
});

/// Every language's texts but English's, read from the embedded files once.
pub(crate) fn translations() -> &'static [(Language, GameTexts)] {
    &TRANSLATIONS
}

/// A character's or weapon's name, from the English the wish history
/// stores, in the current language; as it is when the language lacks it.
pub fn item_name(english: &str) -> &str {
    let current = language();
    (translations().iter())
        .find(|(candidate, _)| *candidate == current)
        .and_then(|(_, texts)| texts.names.get(english))
        .map_or(english, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::super::with_language;
    use super::*;

    #[test]
    fn every_language_reads_and_names_items() {
        assert!(!translations().is_empty());
        with_language(Language::French, || {
            assert_eq!(item_name("Amos' Bow"), "Arc d'Amos");
            assert_eq!(item_name("Not an item"), "Not an item");
        });
        with_language(Language::English, || {
            assert_eq!(item_name("Amos' Bow"), "Amos' Bow")
        });
    }
}
