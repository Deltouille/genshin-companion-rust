//! Numbers as each language writes them: `12,800.5` in English,
//! `12 800,5` in French (a narrow no-break space between thousands, as the
//! French typographic rules and Unicode's CLDR have it).

use fluent_bundle::FluentValue;

use super::{Language, language};

impl Language {
    fn thousands(self) -> &'static str {
        match self {
            Language::English => ",",
            Language::French => "\u{202f}",
        }
    }

    fn decimal_point(self) -> &'static str {
        match self {
            Language::English => ".",
            Language::French => ",",
        }
    }
}

/// A whole number in the current language: `1234567` → `1,234,567`, or
/// `1 234 567`.
pub fn integer(value: impl Into<i128>) -> String {
    let value = value.into();
    let digits = value.unsigned_abs().to_string();
    let separator = language().thousands();
    let mut out = String::with_capacity(digits.len() * 2);
    if value < 0 {
        out.push('-');
    }
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

/// A number in the current language, rounded to `max_fraction` decimals and
/// written with at least `min_fraction`: `decimal(54.049, 0, 2)` →
/// `54.05`, or `54,05`; `decimal(7.0, 1, 1)` → `7.0`, or `7,0`.
pub fn decimal(value: f64, min_fraction: usize, max_fraction: usize) -> String {
    let max_fraction = max_fraction.max(min_fraction);
    let text = format!("{:.*}", max_fraction, value.abs());
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let mut fraction = fraction.to_owned();
    while fraction.len() > min_fraction && fraction.ends_with('0') {
        fraction.pop();
    }
    let whole: i128 = whole.parse().unwrap_or_default();
    // `-0.001` rounds to 0: no sign then.
    let negative = value < 0.0 && (whole != 0 || fraction.bytes().any(|b| b != b'0'));
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&integer(whole));
    if !fraction.is_empty() {
        out.push_str(language().decimal_point());
        out.push_str(&fraction);
    }
    out
}

/// Fluent's numbers in English: grouped, up to two decimals unless the text
/// asks otherwise (`NUMBER($x, maximumFractionDigits: 1)`).
pub(super) fn format_english<M>(value: &FluentValue, _: &M) -> Option<String> {
    super::with_language(Language::English, || format(value))
}

pub(super) fn format_french<M>(value: &FluentValue, _: &M) -> Option<String> {
    super::with_language(Language::French, || format(value))
}

fn format(value: &FluentValue) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    let options = &number.options;
    let min = options.minimum_fraction_digits.unwrap_or(0);
    let max = options.maximum_fraction_digits.unwrap_or(2);
    Some(decimal(number.value, min, max))
}

#[cfg(test)]
mod tests {
    use super::super::with_language;
    use super::*;

    #[test]
    fn each_language_groups_and_points_its_way() {
        with_language(Language::English, || {
            assert_eq!(integer(0), "0");
            assert_eq!(integer(999), "999");
            assert_eq!(integer(1_000), "1,000");
            assert_eq!(integer(-1_234_567), "-1,234,567");
            assert_eq!(decimal(54.049, 0, 2), "54.05");
            assert_eq!(decimal(7.5, 0, 2), "7.5");
            assert_eq!(decimal(100.0, 0, 2), "100");
            assert_eq!(decimal(62.04, 1, 1), "62.0");
            assert_eq!(decimal(-0.001, 0, 2), "0");
            assert_eq!(decimal(12_345.6, 0, 1), "12,345.6");
        });
        with_language(Language::French, || {
            assert_eq!(integer(1_234_567), "1\u{202f}234\u{202f}567");
            assert_eq!(decimal(54.049, 0, 2), "54,05");
            assert_eq!(decimal(62.04, 1, 1), "62,0");
            assert_eq!(decimal(12_345.6, 0, 1), "12\u{202f}345,6");
        });
    }
}
