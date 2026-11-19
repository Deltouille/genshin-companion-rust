//! The interface's texts in each language.
//!
//! Each crate keeps its texts in [Fluent](https://projectfluent.org) files,
//! `locales/<code>/<part>.ftl`, read into one [`Texts`] of its own with
//! [`texts!`](crate::texts), and looks them up with a `t!` macro over
//! [`tr!`](crate::tr): `t!("history-wish-count", count = n)`. Plurals and
//! variants are the translation's business, so the code passes numbers as
//! numbers and never picks a form itself.
//!
//! The language is one for the whole process ([`set_language`]); a text
//! missing in it comes in English. Each crate's tests run [`check`]: every
//! key in every language, with the same variables, and every key the code
//! names written in English with the variables the code passes.

pub mod check;
mod game;
mod number;
mod text;

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
pub use fluent_bundle::{FluentArgs, FluentValue};
use serde::{Deserialize, Deserializer, Serialize};

pub use game::item_name;
pub(crate) use game::translations;
pub use number::{decimal, integer};
pub use text::Text;
pub(crate) use text::translate;

/// A language the app is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "fr")]
    French,
}

impl Language {
    /// Every language, English first: the one a missing text falls back to.
    pub const ALL: [Language; 2] = [Language::English, Language::French];

    /// Its code, as in `locales/` and `settings.json`: "en", "fr".
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
        }
    }

    /// Its name in itself, as a language picker shows it.
    pub fn name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
        }
    }

    /// The language a locale names, whatever its region or encoding: "fr",
    /// "fr-CA", "fr_FR.UTF-8". `None` for one the app isn't written in.
    pub fn from_locale(locale: &str) -> Option<Self> {
        let language = locale
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default();
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.code().eq_ignore_ascii_case(language))
    }

    /// The first of the system's languages the app is written in, else
    /// English.
    pub fn system() -> Self {
        sys_locale::get_locales()
            .find_map(|locale| Self::from_locale(&locale))
            .unwrap_or(Language::English)
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A language as `settings.json` stores it, read leniently: a language this
/// version doesn't know (written by a newer one) reads as none, so the
/// system's is used, rather than failing the whole file.
pub fn deserialize_language<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Language>, D::Error> {
    let code = Option::<String>::deserialize(deserializer)?;
    Ok(code.and_then(|code| Language::from_locale(&code)))
}

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// The language of [`with_language`], on its thread only.
    static SCOPED: Cell<Option<Language>> = const { Cell::new(None) };
}

/// The language texts come in.
pub fn language() -> Language {
    SCOPED
        .get()
        .unwrap_or_else(|| Language::ALL[usize::from(LANGUAGE.load(Ordering::Relaxed))])
}

/// Changes the language of the whole process.
pub fn set_language(language: Language) {
    LANGUAGE.store(language as u8, Ordering::Relaxed);
}

/// Runs `f` with texts in `language`, on this thread only: tests, which run
/// side by side, use it rather than [`set_language`].
pub fn with_language<R>(language: Language, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Language>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED.set(self.0);
        }
    }
    let _restore = Restore(SCOPED.replace(Some(language)));
    f()
}

/// One part of a crate's texts: its file name, for messages, and its text.
pub type Source = (&'static str, &'static str);

/// A crate's texts in every language.
pub struct Texts {
    /// By [`Language::index`].
    bundles: Vec<FluentBundle<FluentResource>>,
    /// The files read, by language, for [`check`].
    sources: Vec<(Language, Vec<Source>)>,
}

impl Texts {
    /// Reads every language's files. Panics on a file that doesn't parse or
    /// a text written twice: each crate's tests build its texts, so neither
    /// reaches a player.
    pub fn new(sources: &[(Language, &[Source])]) -> Self {
        let bundles = Language::ALL
            .into_iter()
            .map(|language| {
                let id = language.code().parse().expect("language codes parse");
                let mut bundle = FluentBundle::new_concurrent(vec![id]);
                // Fluent wraps each placeable in Unicode isolation marks,
                // which the window would draw as boxes.
                bundle.set_use_isolating(false);
                // NUMBER(), for the decimals a text wants.
                bundle
                    .add_builtins()
                    .expect("Fluent's built-in functions register once");
                bundle.set_formatter(Some(match language {
                    Language::English => number::format_english,
                    Language::French => number::format_french,
                }));
                let files = sources
                    .iter()
                    .filter(|(candidate, _)| *candidate == language)
                    .flat_map(|(_, files)| files.iter());
                for (name, text) in files {
                    let resource = FluentResource::try_new((*text).to_owned()).unwrap_or_else(
                        |(_, errors)| {
                            panic!("{}/{name}.ftl doesn't parse: {errors:?}", language.code())
                        },
                    );
                    bundle.add_resource(resource).unwrap_or_else(|errors| {
                        panic!("{}/{name}.ftl: {errors:?}", language.code())
                    });
                }
                bundle
            })
            .collect();
        Self {
            bundles,
            sources: sources
                .iter()
                .map(|(language, files)| (*language, files.to_vec()))
                .collect(),
        }
    }

    /// The text `key` names, in the current [`language`], else in English,
    /// else the key itself.
    pub fn text(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let current = language();
        [current, Language::English]
            .into_iter()
            .find_map(|language| self.format(language, key, args))
            .unwrap_or_else(|| key.to_owned())
    }

    fn format(&self, language: Language, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let bundle = &self.bundles[language.index()];
        let pattern = bundle.get_message(key)?.value()?;
        let mut errors = Vec::new();
        let text = bundle.format_pattern(pattern, args, &mut errors);
        debug_assert!(errors.is_empty(), "{key} ({}): {errors:?}", language.code());
        Some(text.into_owned())
    }
}

/// Declares a crate's texts: the parts named, each read from
/// `locales/<code>/<part>.ftl` in the calling crate, in every language. A
/// part missing in a language fails the build.
#[macro_export]
macro_rules! texts {
    ($($part:literal),+ $(,)?) => {
        $crate::i18n::Texts::new(&[
            ($crate::i18n::Language::English, &[$((
                $part,
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/locales/en/", $part, ".ftl")),
            )),+]),
            ($crate::i18n::Language::French, &[$((
                $part,
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/locales/fr/", $part, ".ftl")),
            )),+]),
        ])
    };
}

/// A text of `$texts` in the current language, with its variables:
/// `tr!(TEXTS, "key")`, `tr!(TEXTS, "key", count = n, name = &name)`. Each
/// crate wraps it in a `t!` of its own; the key is a literal, so the tests
/// can find every key the code uses.
#[macro_export]
macro_rules! tr {
    ($texts:expr, $key:literal $(,)?) => {
        $texts.text($key, None)
    };
    ($texts:expr, $key:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = $crate::i18n::FluentArgs::new();
        $(args.set(stringify!($name), $crate::i18n::FluentValue::from($value));)+
        $texts.text($key, Some(&args))
    }};
}

/// The core's own texts: errors and the notes it writes for the player.
pub(crate) static TEXTS: std::sync::LazyLock<Texts> =
    std::sync::LazyLock::new(|| crate::texts!("core"));

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Texts {
        Texts::new(&[
            (
                Language::English,
                &[(
                    "sample",
                    "wishes = { $count ->\n    [one] { $count } wish\n   *[other] { $count } wishes\n}\nonly-english = Only in English\n",
                )],
            ),
            (
                Language::French,
                &[(
                    "sample",
                    "wishes = { $count ->\n    [one] { $count } vœu\n   *[other] { $count } vœux\n}\n",
                )],
            ),
        ])
    }

    #[test]
    fn plurals_follow_each_language() {
        let texts = sample();
        let wishes = |count: u64| crate::tr!(texts, "wishes", count = count);
        assert_eq!(with_language(Language::English, || wishes(0)), "0 wishes");
        assert_eq!(with_language(Language::English, || wishes(1)), "1 wish");
        assert_eq!(with_language(Language::French, || wishes(0)), "0 vœu");
        assert_eq!(with_language(Language::French, || wishes(1)), "1 vœu");
        assert_eq!(with_language(Language::French, || wishes(2)), "2 vœux");
        assert_eq!(
            with_language(Language::French, || wishes(12_800)),
            "12\u{202f}800 vœux"
        );
        assert_eq!(
            with_language(Language::English, || wishes(12_800)),
            "12,800 wishes"
        );
    }

    #[test]
    fn a_missing_text_comes_in_english_then_as_its_key() {
        let texts = sample();
        with_language(Language::French, || {
            assert_eq!(crate::tr!(texts, "only-english"), "Only in English");
            assert_eq!(crate::tr!(texts, "nowhere"), "nowhere");
        });
    }

    #[test]
    fn locales_name_their_language() {
        assert_eq!(Language::from_locale("fr"), Some(Language::French));
        assert_eq!(Language::from_locale("fr-CA"), Some(Language::French));
        assert_eq!(Language::from_locale("fr_FR.UTF-8"), Some(Language::French));
        assert_eq!(Language::from_locale("EN-us"), Some(Language::English));
        assert_eq!(Language::from_locale("de-DE"), None);
        assert_eq!(Language::from_locale(""), None);
    }

    #[test]
    fn the_scoped_language_ends_with_its_closure() {
        let outside = language();
        with_language(Language::French, || {
            assert_eq!(language(), Language::French);
            with_language(Language::English, || {
                assert_eq!(language(), Language::English)
            });
            assert_eq!(language(), Language::French);
        });
        assert_eq!(language(), outside);
    }

    #[test]
    fn the_core_s_texts_are_complete() {
        check::check(&TEXTS, std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    }
}
