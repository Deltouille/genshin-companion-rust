//! Numbers, sizes and dates as the current language writes them
//! (`wish_core::i18n::language`), and the labels every page shares.

use chrono::{Datelike, Timelike, Weekday};
use wish_core::engine::RateUpOutcome;
use wish_core::i18n::{decimal, integer};
use wish_core::model::{PRIMOGEMS_PER_WISH, Pool, Server};

/// The odds of a rate-up item when not guaranteed: 50/50 on character
/// banners, 75/25 on the weapon banner.
pub fn rate_up_odds(pool: Pool) -> &'static str {
    match pool {
        Pool::WeaponEvent => "75/25",
        _ => "50/50",
    }
}

/// What the next item of a rarity will be, for a pool with rate-up items.
pub fn next_rate_up(pool: Pool, guaranteed: bool) -> String {
    if guaranteed {
        t!("ui-guaranteed-rate-up")
    } else {
        rate_up_odds(pool).to_owned()
    }
}

/// `0.6` → `60%`, or `60 %`.
pub fn whole_percent(fraction: f64) -> String {
    t!("ui-percent", value = decimal(fraction * 100.0, 0, 0))
}

/// `1234567` → `1,234,567`, or `1 234 567`.
pub fn count(value: impl Into<u64>) -> String {
    integer(value.into())
}

/// What `wishes` cost in primogems, written as the current language writes
/// numbers.
pub fn primogems(wishes: u32) -> String {
    count(u64::from(wishes) * u64::from(PRIMOGEMS_PER_WISH))
}

/// `0.016` → `1.6%`, or `1,6 %`.
pub fn percent(fraction: f64) -> String {
    t!("ui-percent", value = decimal(fraction * 100.0, 1, 1))
}

/// One decimal, or a dash when there is nothing to average.
pub fn average(value: Option<f64>) -> String {
    value.map_or_else(|| "–".to_owned(), |v| decimal(v, 1, 1))
}

/// Up to two decimals, trailing zeros dropped, as paimon.moe's summary
/// prints them: `54.049` → `54.05`, `7.5` → `7.5`, `100.0` → `100`.
pub fn number(value: f64) -> String {
    decimal(value, 0, 2)
}

/// A share in [`number`]'s precision: `0.0185` → `1.85%`, `1.0` → `100%`.
pub fn share(fraction: f64) -> String {
    t!("ui-percent", value = number(fraction * 100.0))
}

/// A count short enough for a material tile: `9,999`, `420K`, `12.5K`,
/// `1.67M` (in French `9 999`, `420 k`, `12,5 k`, `1,67 M`).
pub fn compact(value: u64) -> String {
    match value {
        0..10_000 => count(value),
        10_000..1_000_000 => t!("ui-thousands", value = number(value as f64 / 1_000.0)),
        _ => t!("ui-millions", value = number(value as f64 / 1_000_000.0)),
    }
}

/// Picks the English word for a count. Gone once every text is translated:
/// Fluent picks plurals in each language.
pub fn plural(count: u32, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

/// A rate-up result in words: "Won", "Lost", "Guaranteed"…
pub fn outcome(outcome: RateUpOutcome) -> String {
    match outcome {
        RateUpOutcome::Won => t!("ui-outcome-won"),
        RateUpOutcome::Lost => t!("ui-outcome-lost"),
        RateUpOutcome::Guaranteed => t!("ui-outcome-guaranteed"),
        // Never happens in the game; the banner data does not match the wish.
        RateUpOutcome::LostWhileGuaranteed => t!("ui-outcome-unexpected"),
    }
}

/// A size on disk, in decimal units: "850 KB", "11 MB", "1.2 GB" (in
/// French "850 ko", "11 Mo", "1,2 Go").
pub fn bytes(value: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let value = value as f64;
    if value < MB {
        let kilobytes = (value / KB).max(if value > 0.0 { 1.0 } else { 0.0 });
        t!("ui-kilobytes", value = decimal(kilobytes, 0, 0))
    } else if value < 10.0 * MB {
        t!("ui-megabytes", value = decimal(value / MB, 1, 1))
    } else if value < GB {
        t!("ui-megabytes", value = decimal(value / MB, 0, 0))
    } else {
        t!("ui-gigabytes", value = decimal(value / GB, 1, 1))
    }
}

/// An element as the game names it, from the data's English: "Anemo", or
/// "Anémo". One the app doesn't know comes as it is.
pub fn element(english: &str) -> String {
    match english {
        "Pyro" => t!("ui-element-pyro"),
        "Hydro" => t!("ui-element-hydro"),
        "Anemo" => t!("ui-element-anemo"),
        "Electro" => t!("ui-element-electro"),
        "Dendro" => t!("ui-element-dendro"),
        "Cryo" => t!("ui-element-cryo"),
        "Geo" => t!("ui-element-geo"),
        other => other.to_owned(),
    }
}

/// A weapon type as the game names it, from the data's English: "Polearm",
/// or "Arme d'hast". One the app doesn't know comes as it is.
pub fn weapon_type(english: &str) -> String {
    match english {
        "Sword" => t!("ui-weapon-sword"),
        "Claymore" => t!("ui-weapon-claymore"),
        "Polearm" => t!("ui-weapon-polearm"),
        "Bow" => t!("ui-weapon-bow"),
        "Catalyst" => t!("ui-weapon-catalyst"),
        other => other.to_owned(),
    }
}

/// A stat as the game names it, from the data's English: "CRIT Rate", or
/// "Taux CRIT". One the app doesn't know comes as it is.
pub fn stat(english: &str) -> String {
    match english {
        "HP" => t!("ui-stat-hp"),
        "ATK" => t!("ui-stat-atk"),
        "DEF" => t!("ui-stat-def"),
        "Base HP" => t!("ui-stat-base-hp"),
        "Base ATK" => t!("ui-stat-base-atk"),
        "Base DEF" => t!("ui-stat-base-def"),
        "CRIT Rate" => t!("ui-stat-crit-rate"),
        "CRIT DMG" => t!("ui-stat-crit-dmg"),
        "Energy Recharge" => t!("ui-stat-energy-recharge"),
        "Elemental Mastery" => t!("ui-stat-elemental-mastery"),
        "Healing Bonus" => t!("ui-stat-healing-bonus"),
        "Physical DMG Bonus" => t!("ui-stat-physical-dmg"),
        "Pyro DMG Bonus" => t!("ui-stat-pyro-dmg"),
        "Hydro DMG Bonus" => t!("ui-stat-hydro-dmg"),
        "Anemo DMG Bonus" => t!("ui-stat-anemo-dmg"),
        "Electro DMG Bonus" => t!("ui-stat-electro-dmg"),
        "Dendro DMG Bonus" => t!("ui-stat-dendro-dmg"),
        "Cryo DMG Bonus" => t!("ui-stat-cryo-dmg"),
        "Geo DMG Bonus" => t!("ui-stat-geo-dmg"),
        other => other.to_owned(),
    }
}

/// A name in the language's quotes: “Main”, or « Main ».
pub fn quoted(text: &str) -> String {
    t!("ui-quoted", text = text)
}

/// A month's short name: "Nov", or "nov.".
fn month_short(month: u32) -> String {
    match month {
        1 => t!("ui-month-short-1"),
        2 => t!("ui-month-short-2"),
        3 => t!("ui-month-short-3"),
        4 => t!("ui-month-short-4"),
        5 => t!("ui-month-short-5"),
        6 => t!("ui-month-short-6"),
        7 => t!("ui-month-short-7"),
        8 => t!("ui-month-short-8"),
        9 => t!("ui-month-short-9"),
        10 => t!("ui-month-short-10"),
        11 => t!("ui-month-short-11"),
        _ => t!("ui-month-short-12"),
    }
}

/// A month's name: "November", or "novembre".
fn month_name(month: u32) -> String {
    match month {
        1 => t!("ui-month-1"),
        2 => t!("ui-month-2"),
        3 => t!("ui-month-3"),
        4 => t!("ui-month-4"),
        5 => t!("ui-month-5"),
        6 => t!("ui-month-6"),
        7 => t!("ui-month-7"),
        8 => t!("ui-month-8"),
        9 => t!("ui-month-9"),
        10 => t!("ui-month-10"),
        11 => t!("ui-month-11"),
        _ => t!("ui-month-12"),
    }
}

/// A message that starts lowercase (the core's errors, also used
/// mid-sentence) as a sentence's start.
pub fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A game server: "Europe", or "Asie".
pub fn server(server: Server) -> String {
    match server {
        Server::Asia => t!("ui-server-asia"),
        Server::America => t!("ui-server-america"),
        Server::Europe => t!("ui-server-europe"),
        Server::TwHkMo => t!("ui-server-tw-hk-mo"),
        Server::China => t!("ui-server-china"),
    }
}

/// A day of the week: "Tuesday", or "mardi".
pub fn weekday(day: Weekday) -> String {
    match day {
        Weekday::Mon => t!("ui-weekday-mon"),
        Weekday::Tue => t!("ui-weekday-tue"),
        Weekday::Wed => t!("ui-weekday-wed"),
        Weekday::Thu => t!("ui-weekday-thu"),
        Weekday::Fri => t!("ui-weekday-fri"),
        Weekday::Sat => t!("ui-weekday-sat"),
        Weekday::Sun => t!("ui-weekday-sun"),
    }
}

/// "9 Nov 2026", or "9 nov. 2026".
pub fn date(day: &impl Datelike) -> String {
    t!(
        "ui-date",
        day = day.day(),
        month = month_short(day.month()),
        year = day.year().to_string()
    )
}

/// "12:20".
pub fn time(time: &impl Timelike) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

/// "12:20:05".
pub fn time_seconds(time: &impl Timelike) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        time.hour(),
        time.minute(),
        time.second()
    )
}

/// "9 Nov 2026, 12:20".
pub fn date_time(when: &(impl Datelike + Timelike)) -> String {
    t!("ui-date-time", date = date(when), time = time(when))
}

/// "9 Nov 2026, 12:20:05".
pub fn date_time_seconds(when: &(impl Datelike + Timelike)) -> String {
    t!("ui-date-time", date = date(when), time = time_seconds(when))
}

/// "9 Nov 2026 at 12:20", or "9 nov. 2026 à 12:20".
pub fn date_at_time(when: &(impl Datelike + Timelike)) -> String {
    t!("ui-date-at-time", date = date(when), time = time(when))
}

/// "November 2026", or "novembre 2026".
pub fn month_year(day: &impl Datelike) -> String {
    t!(
        "ui-month-year",
        month = month_name(day.month()),
        year = day.year().to_string()
    )
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

    #[test]
    fn french_writes_its_own_numbers_and_dates() {
        use wish_core::i18n::{Language, with_language};
        let when = chrono::NaiveDate::from_ymd_opt(2026, 2, 9)
            .unwrap()
            .and_hms_opt(8, 5, 30)
            .unwrap();
        with_language(Language::English, || {
            assert_eq!(date_at_time(&when), "9 Feb 2026 at 08:05");
            assert_eq!(date_time_seconds(&when), "9 Feb 2026, 08:05:30");
            assert_eq!(month_year(&when), "February 2026");
        });
        with_language(Language::French, || {
            assert_eq!(count(1_234_567u32), "1\u{202f}234\u{202f}567");
            assert_eq!(percent(0.016), "1,6\u{202f}%");
            assert_eq!(share(1.0), "100\u{202f}%");
            assert_eq!(compact(12_500), "12,5\u{202f}k");
            assert_eq!(bytes(2_650_000), "2,6\u{202f}Mo");
            assert_eq!(date_at_time(&when), "9 févr. 2026 à 08:05");
            assert_eq!(month_year(&when), "février 2026");
            assert_eq!(outcome(RateUpOutcome::Won), "Gagné");
        });
    }

    #[test]
    fn the_design_system_s_texts_are_complete() {
        wish_core::i18n::check::check(
            &crate::TEXTS,
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        );
    }
}
