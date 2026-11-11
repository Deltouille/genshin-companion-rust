//! The engine checked against paimon.moe's own numbers, through
//! `import::paimon_check` (see there for what a save stores).

use std::fs;
use std::path::Path;

use wish_core::data::GameData;
use wish_core::engine::{self, Analysis, Counters, Guarantee};
use wish_core::import::{paimon, paimon_check};
use wish_core::model::Pool;

const SAMPLE: &str = include_str!("fixtures/paimon-sample.json");
const ENGINE: &str = include_str!("fixtures/paimon-engine.json");

fn assert_matches_paimon(name: &str, json: &[u8]) {
    let comparison =
        paimon_check::compare(json, GameData::embedded()).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(comparison.checked > 0, "{name}: nothing was compared");
    if !comparison.matches() {
        let shown: Vec<_> = comparison.differences.iter().take(30).cloned().collect();
        panic!(
            "{name}: {} of {} values differ from paimon.moe's:\n  {}",
            comparison.differences.len(),
            comparison.checked,
            shown.join("\n  ")
        );
    }
    eprintln!("{name}: {} values match paimon.moe", comparison.checked);
}

#[test]
fn sample_save_matches_paimon() {
    assert_matches_paimon("paimon-sample.json", SAMPLE.as_bytes());
}

#[test]
fn engine_save_matches_paimon() {
    assert_matches_paimon("paimon-engine.json", ENGINE.as_bytes());
}

/// Real exports dropped into `tests/fixtures/private/` (git-ignored).
#[test]
fn private_exports_match_paimon() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private");
    let Ok(entries) = fs::read_dir(&dir) else {
        eprintln!("no private fixtures in {}", dir.display());
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") {
            assert_matches_paimon(&path.display().to_string(), &fs::read(&path).unwrap());
        }
    }
}

/// Guards against a checker that compares nothing.
#[test]
fn differences_are_reported() {
    let altered = ENGINE
        .replacen(
            "\"pity\": 1, \"code\": \"301\", \"rate\": 1",
            "\"pity\": 1, \"code\": \"301\", \"rate\": 0",
            1,
        )
        .replacen(
            "\"pity\": 2, \"code\": \"301\", \"rate\": 2",
            "\"pity\": 9, \"code\": \"301\", \"rate\": 2",
            1,
        )
        .replacen("\"total\": 15", "\"total\": 16", 1)
        .replacen(
            "\"guaranteed\": { \"legendary\": false",
            "\"guaranteed\": { \"legendary\": true",
            1,
        )
        .replacen("\"win\": 1, \"lose\": 1", "\"win\": 2, \"lose\": 1", 1);
    let comparison = paimon_check::compare(altered.as_bytes(), GameData::embedded()).unwrap();
    let expected = [
        "Navia: rate Some(1), paimon.moe 0",
        "Raiden Shogun: pity Some(2), paimon.moe 9",
        "counters (total, 5★, 4★) (Some(15)",
        "legendary guaranteed false, paimon.moe true",
        "legendary rate-up record",
    ];
    let problems = &comparison.differences;
    assert_eq!(problems.len(), expected.len(), "{problems:#?}");
    for (problem, expected) in problems.iter().zip(expected) {
        assert!(
            problem.contains(expected),
            "{problem} should mention {expected}"
        );
    }
}

/// The America account of `paimon-engine.json`, checked number by number.
#[test]
fn engine_save_summary() {
    let analysis = analyze_first_account(ENGINE.as_bytes());
    assert_eq!(analysis.total_wishes(), 20);

    let character = analysis.pool(Pool::CharacterEvent).unwrap();
    assert_eq!(
        character.counters,
        Counters {
            total: 15,
            pity5: 1,
            pity4: 2
        }
    );
    assert_eq!(
        character.guarantee,
        Some(Guarantee {
            five: false,
            four: false
        })
    );
    // The Raiden Shogun wish falls between "Reign of Serenity 4" and the next banner.
    assert_eq!(character.outside_banners, 1);

    let banners = &GameData::embedded().banners.character;
    let per_banner: Vec<_> = character
        .banners
        .iter()
        .map(|b| (banners[b.banner].id(), b.pulls.len()))
        .collect();
    assert_eq!(
        per_banner,
        [
            ("In the Name of the Rosula 1".to_owned(), 11),
            ("Reign of Serenity 4".to_owned(), 3),
            ("The Crane Soars Skyward 1".to_owned(), 1),
        ]
    );

    let stats = &character.stats;
    assert_eq!((stats.five.count, stats.five.pity_sum), (3, 14));
    assert!((stats.five.share(stats.total) - 0.2).abs() < 1e-9);
    assert_eq!((stats.four.count, stats.four.pity_sum), (3, 13));
    assert_eq!(
        (stats.four_character.count, stats.four_weapon.count),
        (3, 0)
    );
    let rate = stats.rate_five.unwrap();
    assert_eq!((rate.won, rate.lost, rate.win_rate()), (1, 1, Some(0.5)));

    let standard = analysis.pool(Pool::Standard).unwrap();
    assert_eq!(standard.guarantee, None);
    assert_eq!(standard.stats.five.average_pity(), Some(3.0));

    let months: Vec<_> = analysis
        .monthly
        .iter()
        .map(|m| (m.label(), m.total, m.five, m.four))
        .collect();
    assert_eq!(
        months,
        [
            ("2023-12".to_owned(), 11, 1, 2),
            ("2024-01".to_owned(), 9, 4, 2)
        ]
    );
}

fn analyze_first_account(json: &[u8]) -> Analysis {
    let data = GameData::embedded();
    let account = paimon::parse(json, data).unwrap().accounts.remove(0);
    engine::analyze(
        account.wishes,
        account.server,
        &account.manual_counters,
        data,
    )
}

#[test]
fn overview_totals_and_five_stars_in_order() {
    use wish_core::model::Rarity;

    let analysis = analyze_first_account(include_bytes!("fixtures/paimon-engine.json"));
    let totals = analysis.totals();
    assert_eq!(totals.wishes, analysis.total_wishes());
    let fives: u32 = analysis.pools.iter().map(|p| p.stats.five.count).sum();
    assert_eq!(totals.five.count, fives);
    let won: u32 = analysis
        .pools
        .iter()
        .filter_map(|p| p.stats.rate_five)
        .map(|r| r.won)
        .sum();
    assert_eq!(totals.five_won, won);

    // Every 5★ over all banner types, oldest first.
    let pulls = analysis.pulls_of(Rarity::Five);
    assert_eq!(pulls.len() as u32, fives);
    assert!(
        pulls
            .windows(2)
            .all(|w| (w[0].1.wish.time, w[0].1.wish.seq) <= (w[1].1.wish.time, w[1].1.wish.seq))
    );
    assert!(
        pulls
            .iter()
            .all(|(_, pull)| pull.wish.rarity == Rarity::Five)
    );
}

#[test]
fn banner_rolls_count_from_one_on_each_banner() {
    let analysis = analyze_first_account(include_bytes!("fixtures/paimon-engine.json"));
    for pool in &analysis.pools {
        let rolls = pool.banner_rolls();
        assert_eq!(rolls.len(), pool.pulls.len());
        for range in &pool.banners {
            let numbers: Vec<Option<u32>> = rolls[range.pulls.clone()].to_vec();
            let expected: Vec<Option<u32>> = (1..=range.pulls.len() as u32).map(Some).collect();
            assert_eq!(numbers, expected, "{:?}", pool.pool);
        }
    }
}
