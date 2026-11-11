//! Number and label formatting for English UI text.

use wish_core::engine::RateUpOutcome;
use wish_core::model::{PRIMOGEMS_PER_WISH, Pool};

/// The odds of a rate-up item when not guaranteed: 50/50 on character
/// banners, 75/25 on the weapon banner.
pub fn rate_up_odds(pool: Pool) -> &'static str {
    match pool {
        Pool::WeaponEvent => "75/25",
        _ => "50/50",
    }
}

/// What the next item of a rarity will be, for a pool with rate-up items.
pub fn next_rate_up(pool: Pool, guaranteed: bool) -> &'static str {
    if guaranteed {
        "guaranteed rate-up"
    } else {
        rate_up_odds(pool)
    }
}

/// `0.6` → `60%`.
pub fn whole_percent(fraction: f64) -> String {
    format!("{:.0}%", fraction * 100.0)
}

/// `1234567` → `1,234,567`.
pub fn count(value: impl Into<u64>) -> String {
    let digits = value.into().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub fn primogems(wishes: u32) -> String {
    count(u64::from(wishes) * u64::from(PRIMOGEMS_PER_WISH))
}

/// `0.016` → `1.6%`.
pub fn percent(fraction: f64) -> String {
    format!("{:.1}%", fraction * 100.0)
}

/// One decimal, or a dash when there is nothing to average.
pub fn average(value: Option<f64>) -> String {
    value.map_or_else(|| "–".to_owned(), |v| format!("{v:.1}"))
}

/// Up to two decimals, trailing zeros dropped, as paimon.moe's summary
/// prints them: `54.049` → `54.05`, `7.5` → `7.5`, `100.0` → `100`.
pub fn number(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// A share in [`number`]'s precision: `0.0185` → `1.85%`, `1.0` → `100%`.
pub fn share(fraction: f64) -> String {
    format!("{}%", number(fraction * 100.0))
}

/// A count short enough for a material tile: `9,999`, `420K`, `12.5K`,
/// `1.67M`.
pub fn compact(value: u64) -> String {
    match value {
        0..10_000 => count(value),
        10_000..1_000_000 => format!("{}K", number(value as f64 / 1_000.0).trim_end_matches('.')),
        _ => format!("{}M", number(value as f64 / 1_000_000.0)),
    }
}

pub fn plural(count: u32, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

pub fn outcome(outcome: RateUpOutcome) -> &'static str {
    match outcome {
        RateUpOutcome::Won => "Won",
        RateUpOutcome::Lost => "Lost",
        RateUpOutcome::Guaranteed => "Guaranteed",
        // Never happens in the game; the banner data does not match the wish.
        RateUpOutcome::LostWhileGuaranteed => "Unexpected",
    }
}

/// A size on disk, in decimal units: "850 KB", "11 MB", "1.2 GB".
pub fn bytes(value: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let value = value as f64;
    if value < MB {
        format!(
            "{:.0} KB",
            (value / KB).max(if value > 0.0 { 1.0 } else { 0.0 })
        )
    } else if value < 10.0 * MB {
        format!("{:.1} MB", value / MB)
    } else if value < GB {
        format!("{:.0} MB", value / MB)
    } else {
        format!("{:.1} GB", value / GB)
    }
}

/// A whole number as a person types it: thousands separators (commas,
/// spaces, underscores) are ignored, and a sign may lead (`+60,000`,
/// `-20`, `−20`). `None` when it is not a number.
pub fn parse_number(text: &str) -> Option<i64> {
    let text: String = text
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '_' | ' ' | '\u{a0}' | '\u{202f}'))
        .map(|c| if c == '\u{2212}' { '-' } else { c })
        .collect();
    let digits = text.strip_prefix('+').unwrap_or(&text);
    if digits.trim_start_matches('-').is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(bytes(0), "0 KB");
        assert_eq!(bytes(300), "1 KB");
        assert_eq!(bytes(850_000), "850 KB");
        assert_eq!(bytes(2_650_000), "2.6 MB");
        assert_eq!(bytes(11_200_000), "11 MB");
        assert_eq!(bytes(1_234_000_000), "1.2 GB");
    }

    #[test]
    fn numbers_are_read_as_typed() {
        assert_eq!(parse_number("360,000"), Some(360_000));
        assert_eq!(parse_number(" 360 000 "), Some(360_000));
        assert_eq!(parse_number("+60,000"), Some(60_000));
        assert_eq!(parse_number("-20"), Some(-20));
        assert_eq!(parse_number("\u{2212}20"), Some(-20));
        assert_eq!(parse_number("0"), Some(0));
        for bad in ["", "+", "-", "12a", "1.5", "--3"] {
            assert_eq!(parse_number(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(count(0u32), "0");
        assert_eq!(count(999u32), "999");
        assert_eq!(count(1000u32), "1,000");
        assert_eq!(count(1234567u32), "1,234,567");
        assert_eq!(primogems(80), "12,800");
    }

    #[test]
    fn percentages_and_averages() {
        assert_eq!(percent(0.016), "1.6%");
        assert_eq!(average(Some(62.04)), "62.0");
        assert_eq!(average(None), "–");
        assert_eq!(number(54.049), "54.05");
        assert_eq!(number(7.5), "7.5");
        assert_eq!(number(100.0), "100");
        assert_eq!(number(0.0), "0");
        assert_eq!(share(0.0185), "1.85%");
        assert_eq!(share(0.6875), "68.75%");
        assert_eq!(share(1.0), "100%");
        assert_eq!(compact(9_999), "9,999");
        assert_eq!(compact(420_000), "420K");
        assert_eq!(compact(12_500), "12.5K");
        assert_eq!(compact(1_672_530), "1.67M");
    }
}
