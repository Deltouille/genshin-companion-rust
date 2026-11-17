//! The game's own texts (names, descriptions, talents) in each language the
//! data has them: English from `planner.json` and `archive.json`, the others
//! from `data/<code>.json`, merged in when the data loads.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer};

use super::{Language, language};

/// A text of the game's data in every language it comes in. It reads (as a
/// `str`, or shown) in the current language, else in English.
///
/// What the game, HoYoLAB or a file says is matched against [`english`]
/// (`Text::english`), never against the text shown: a comparison has to say
/// which, so `Text` has no `==` with a string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    english: String,
    /// The other languages', where the data has them.
    others: Vec<(Language, String)>,
}

impl Text {
    pub fn new(english: impl Into<String>) -> Self {
        Self {
            english: english.into(),
            others: Vec::new(),
        }
    }

    /// One text per language, `text(language)` each: the app's own words
    /// (a placeholder material's name), from its translations.
    pub fn from_each(mut text: impl FnMut(Language) -> String) -> Self {
        let mut this = Self::new(text(Language::English));
        for language in Language::ALL.into_iter().skip(1) {
            this.set(language, text(language));
        }
        this
    }

    /// The text in the current language (as `*text` reads it).
    pub fn as_str(&self) -> &str {
        self
    }

    /// The text as the data's English has it, which imports and HoYoLAB use.
    pub fn english(&self) -> &str {
        &self.english
    }

    /// The text in `language`, else in English.
    pub fn get(&self, language: Language) -> &str {
        (self.others.iter())
            .find(|(candidate, _)| *candidate == language)
            .map_or(&self.english, |(_, text)| text)
    }

    /// Sets the text in `language` (English is the data's own); an empty one
    /// is left out.
    pub fn set(&mut self, language: Language, text: impl Into<String>) {
        let text = text.into();
        if language == Language::English || text.is_empty() {
            return;
        }
        self.others.retain(|(candidate, _)| *candidate != language);
        self.others.push((language, text));
    }

    /// Whether `name` is this text in any language, whatever the case: a
    /// name the player typed.
    pub fn matches(&self, name: &str) -> bool {
        let name = name.trim().to_lowercase();
        std::iter::once(&self.english)
            .chain(self.others.iter().map(|(_, text)| text))
            .any(|text| text.to_lowercase() == name)
    }
}

impl Deref for Text {
    type Target = str;

    /// The text in the current language.
    fn deref(&self) -> &str {
        self.get(language())
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        self
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl From<&str> for Text {
    fn from(english: &str) -> Self {
        Self::new(english)
    }
}

impl From<String> for Text {
    fn from(english: String) -> Self {
        Self::new(english)
    }
}

/// The data files hold the English text.
impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::new)
    }
}

/// Sets `text` in `language` from a translation, when the translation has
/// it. For the merges of `data/<code>.json`.
pub(crate) fn translate(text: &mut Text, language: Language, translation: Option<&String>) {
    if let Some(translation) = translation {
        text.set(language, translation.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::super::with_language;
    use super::*;

    #[test]
    fn a_text_reads_in_the_current_language_else_in_english() {
        let mut bow = Text::new("Amos' Bow");
        let plain = Text::new("Slingshot");
        bow.set(Language::French, "Arc d'Amos");
        with_language(Language::French, || {
            assert_eq!(&*bow, "Arc d'Amos");
            assert_eq!(bow.to_string(), "Arc d'Amos");
            assert_eq!(&*plain, "Slingshot");
        });
        with_language(Language::English, || assert_eq!(&*bow, "Amos' Bow"));
        assert_eq!(bow.english(), "Amos' Bow");
        assert!(bow.matches("arc d'amos") && bow.matches(" AMOS' BOW "));
        assert!(!bow.matches("Amos"));
    }
}
