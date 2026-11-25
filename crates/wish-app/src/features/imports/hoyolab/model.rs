//! What the preview says of a sync, in words: the summary of its counts, one
//! line per change, and where it goes. Pure functions over the
//! [`SyncPlan`], tested without GPUI.

use wish_core::chronicle::{RecordCounts, RecordsPlan};
use wish_core::collection::{Change, OwnedCharacter, OwnedWeapon, SyncPlan};
use wish_core::hoyolab::GameAccount;
use wish_core::model::Account;
use wish_core::planner::{Level, PlannerData};
use wish_ui::format;

/// An app account a sync can go to, as the choice names it: “Main”, or a new
/// account named after the game's nickname.
pub(super) fn target_label(target: &Option<Account>, game: &GameAccount) -> String {
    match target {
        Some(account) => format::quoted(&account.label),
        None => t!("hoyolab-new-account", name = format::quoted(&game.nickname)),
    }
}

/// Where the sync goes, after the UID: "into “Main”".
pub(super) fn into(target: &Option<Account>, game: &GameAccount) -> String {
    match target {
        Some(account) => t!("hoyolab-into", account = format::quoted(&account.label)),
        None => t!("hoyolab-into-new", name = format::quoted(&game.nickname)),
    }
}

/// "Characters: 112 to add, 3 to raise · Weapons: 110 to add, 2 to move".
pub(in crate::features::imports) fn summary(plan: &SyncPlan) -> String {
    let parts = |[added, raised, moved]: [usize; 3]| -> Option<String> {
        let parts: Vec<String> = [
            (added > 0).then(|| t!("hoyolab-to-add", count = added)),
            (raised > 0).then(|| t!("hoyolab-to-raise", count = raised)),
            (moved > 0).then(|| t!("hoyolab-to-move", count = moved)),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    };
    let (characters_added, characters_raised) = tally(&plan.characters);
    let (weapons_added, weapons_raised) = tally(&plan.weapons);
    let weapons_moved = (plan.weapons.iter())
        .filter(|change| moved(change).is_some())
        .count();
    let parts: Vec<String> = [
        parts([characters_added, characters_raised, 0])
            .map(|parts| t!("hoyolab-characters", parts = parts)),
        parts([weapons_added, weapons_raised - weapons_moved, weapons_moved])
            .map(|parts| t!("hoyolab-weapons", parts = parts)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        t!("hoyolab-nothing")
    } else {
        parts.join(" · ")
    }
}

/// One line per character or weapon copy the sync adds (+), raises (↑) or
/// moves to another holder (⇄): its sign and its text.
pub(in crate::features::imports) fn lines(
    plan: &SyncPlan,
    planner: &PlannerData,
) -> Vec<(&'static str, String)> {
    let level = |level: Level| format!("{}/{}", level.level, level.cap());
    let name = |key: &str| {
        planner
            .character(key)
            .map_or(key, |c| c.name.as_str())
            .to_owned()
    };
    let character = |c: &OwnedCharacter| {
        let [a, e, q] = c.talents;
        t!(
            "hoyolab-character-line",
            name = name(&c.key),
            level = level(c.level),
            constellation = c.constellation,
            talents = format!("{a}/{e}/{q}")
        )
    };
    let weapon = |w: &OwnedWeapon| {
        let weapon = planner
            .weapon(&w.key)
            .map_or(w.key.as_str(), |w| w.name.as_str());
        match w.holder.as_deref() {
            Some(holder) => t!(
                "hoyolab-weapon-held",
                weapon = weapon,
                level = level(w.level),
                refinement = w.refinement,
                holder = name(holder)
            ),
            None => t!(
                "hoyolab-weapon-line",
                weapon = weapon,
                level = level(w.level),
                refinement = w.refinement
            ),
        }
    };
    let characters = plan.characters.iter().filter_map(|change| match change {
        Change::Added(c) => Some(("+", character(c))),
        Change::Raised { before, after } => Some((
            "↑",
            t!(
                "hoyolab-was",
                after = character(after),
                before = character(before)
            ),
        )),
        Change::Unchanged(_) => None,
    });
    let holder =
        |holder: &Option<String>| holder.as_deref().map_or_else(|| t!("hoyolab-nobody"), name);
    let weapons = plan.weapons.iter().filter_map(|change| match change {
        Change::Added(w) => Some(("+", weapon(w))),
        Change::Raised { .. } if let Some((before, after)) = moved(change) => {
            let name = planner
                .weapon(&after.key)
                .map_or(after.key.as_str(), |w| w.name.as_str());
            Some((
                "⇄",
                t!(
                    "hoyolab-moved",
                    weapon = name,
                    level = level(after.level),
                    refinement = after.refinement,
                    from = holder(&before.holder),
                    to = holder(&after.holder)
                ),
            ))
        }
        Change::Raised { before, after } => Some((
            "↑",
            t!(
                "hoyolab-was",
                after = weapon(after),
                before = weapon(before)
            ),
        )),
        Change::Unchanged(_) => None,
    });
    characters.chain(weapons).collect()
}

/// A weapon copy that only changes hands: another character of the sync
/// holds it in the game now (two characters swapped weapons).
fn moved(change: &Change<OwnedWeapon>) -> Option<(&OwnedWeapon, &OwnedWeapon)> {
    match change {
        Change::Raised { before, after }
            if before.level == after.level
                && before.refinement == after.refinement
                && before.holder != after.holder =>
        {
            Some((before, after))
        }
        _ => None,
    }
}

/// A line of the chronicle's preview, from its counts in words.
type Line = fn(String) -> String;

/// What the sync keeps of the chronicle, one line per kind that changes:
/// "Spiral Abyss: 1 to keep, 1 to update".
pub(in crate::features::imports) fn chronicle(records: &RecordsPlan) -> Vec<String> {
    let kinds: [(RecordCounts, Line); 3] = [
        (records.abyss, |parts| {
            t!("hoyolab-chronicle-abyss", parts = parts)
        }),
        (records.theater, |parts| {
            t!("hoyolab-chronicle-theater", parts = parts)
        }),
        (records.diary, |parts| {
            t!("hoyolab-chronicle-diary", parts = parts)
        }),
    ];
    kinds
        .into_iter()
        .filter(|(counts, _)| counts.changes() > 0)
        .map(|(counts, line)| {
            let parts: Vec<String> = [
                (counts.added > 0).then(|| t!("hoyolab-to-keep", count = counts.added)),
                (counts.updated > 0).then(|| t!("hoyolab-to-update", count = counts.updated)),
            ]
            .into_iter()
            .flatten()
            .collect();
            line(parts.join(", "))
        })
        .collect()
}

/// How many of `changes` add, and how many raise.
fn tally<T>(changes: &[Change<T>]) -> (usize, usize) {
    changes
        .iter()
        .fold((0, 0), |(added, raised), change| match change {
            Change::Added(_) => (added + 1, raised),
            Change::Raised { .. } => (added, raised + 1),
            Change::Unchanged(_) => (added, raised),
        })
}

#[cfg(test)]
mod tests {
    use wish_core::model::AccountId;

    use super::*;

    fn level(level: u8, phase: u8) -> Level {
        Level::new(level, phase).unwrap()
    }

    fn held(key: &str, holder: &str) -> OwnedWeapon {
        OwnedWeapon {
            level: level(90, 6),
            holder: Some(holder.to_owned()),
            ..OwnedWeapon::new(key)
        }
    }

    /// Ganyu new, Bennett raised; Amos' Bow new, a Favonius Sword raised, and
    /// a Sacrificial Sword moved from Bennett to Xingqiu.
    fn plan() -> SyncPlan {
        let mut bennett = OwnedCharacter::new("bennett");
        let mut raised = bennett.clone();
        raised.level = level(80, 6);
        raised.constellation = 6;
        bennett.level = level(70, 4);
        let favonius = held("favoniussword", "xingqiu");
        let mut refined = favonius.clone();
        refined.refinement = 3;
        SyncPlan {
            characters: vec![
                Change::Added(OwnedCharacter::new("ganyu")),
                Change::Raised {
                    before: bennett,
                    after: raised,
                },
                Change::Unchanged(OwnedCharacter::new("xiangling")),
            ],
            weapons: vec![
                Change::Added(held("amosbow", "ganyu")),
                Change::Raised {
                    before: favonius,
                    after: refined,
                },
                Change::Raised {
                    before: held("sacrificialsword", "bennett"),
                    after: held("sacrificialsword", "xingqiu"),
                },
            ],
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_summary_counts_what_moves_apart() {
        assert_eq!(
            summary(&plan()),
            "Characters: 1 to add, 1 to raise · Weapons: 1 to add, 1 to raise, 1 to move"
        );
        let nothing = SyncPlan {
            characters: Vec::new(),
            weapons: Vec::new(),
            notes: Vec::new(),
        };
        assert_eq!(summary(&nothing), "Nothing to change.");
    }

    #[test]
    fn the_chronicle_says_what_it_keeps_and_updates() {
        let records = RecordsPlan {
            abyss: RecordCounts {
                added: 1,
                updated: 1,
                kept: 0,
            },
            theater: RecordCounts {
                added: 0,
                updated: 0,
                kept: 2,
            },
            diary: RecordCounts {
                added: 3,
                updated: 0,
                kept: 0,
            },
        };
        assert_eq!(
            chronicle(&records),
            [
                "Spiral Abyss: 1 to keep, 1 to update",
                "Traveler’s Diary: 3 to keep"
            ]
        );
        assert!(chronicle(&RecordsPlan::default()).is_empty());
    }

    #[test]
    fn one_line_per_change() {
        let lines = lines(&plan(), PlannerData::embedded());
        let signs: Vec<&str> = lines.iter().map(|(sign, _)| *sign).collect();
        assert_eq!(signs, ["+", "↑", "+", "↑", "⇄"]);
        assert_eq!(lines[0].1, "Ganyu · 1/20 · C0 · talents 1/1/1");
        assert_eq!(
            lines[1].1,
            "Bennett · 80/90 · C6 · talents 1/1/1  (was Bennett · 70/70 · C0 · talents 1/1/1)"
        );
        assert_eq!(lines[2].1, "Amos' Bow · 90/90 · R1 · held by Ganyu");
        assert_eq!(
            lines[4].1,
            "Sacrificial Sword · 90/90 · R1: from Bennett to Xingqiu"
        );
    }

    #[test]
    fn where_it_goes() {
        let game = GameAccount {
            uid: "712345678".to_owned(),
            region: "os_euro".to_owned(),
            nickname: "Traveler".to_owned(),
            level: 60,
            game_biz: "hk4e_global".to_owned(),
        };
        let main = Some(Account {
            id: AccountId(1),
            label: "Main".to_owned(),
            uid: None,
            server: None,
        });
        assert_eq!(target_label(&main, &game), "“Main”");
        assert_eq!(target_label(&None, &game), "A new account, “Traveler”");
        assert_eq!(into(&main, &game), "into “Main”");
        assert_eq!(into(&None, &game), "into a new account, “Traveler”");
    }
}
