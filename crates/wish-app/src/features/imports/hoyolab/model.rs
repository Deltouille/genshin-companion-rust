//! What the preview says of a sync, in words: the summary of its counts, one
//! line per change, and where it goes. Pure functions over the
//! [`SyncPlan`], tested without GPUI.

use wish_core::collection::{Change, OwnedCharacter, OwnedWeapon, SyncPlan};
use wish_core::hoyolab::GameAccount;
use wish_core::model::Account;
use wish_core::planner::{Level, PlannerData};
use wish_ui::format;

/// An app account a sync can go to, as the choice names it: “Main”, or a new
/// account named after the game's nickname.
pub(super) fn target_label(target: &Option<Account>, game: &GameAccount) -> String {
    match target {
        Some(account) => format!("“{}”", account.label),
        None => format!("A new account, “{}”", game.nickname),
    }
}

/// Where the sync goes, after the UID: "into “Main”".
pub(super) fn into(target: &Option<Account>, game: &GameAccount) -> String {
    match target {
        Some(account) => format!("into “{}”", account.label),
        None => format!("into a new account, “{}”", game.nickname),
    }
}

/// "Characters: 112 to add, 3 to raise · Weapons: 110 to add, 2 to move".
pub(super) fn summary(plan: &SyncPlan) -> String {
    let count = |counts: [(usize, &str); 3], what: &str| -> Option<String> {
        let parts: Vec<String> = (counts.iter())
            .filter(|(n, _)| *n > 0)
            .map(|(n, verb)| format!("{} to {verb}", format::count(*n as u64)))
            .collect();
        (!parts.is_empty()).then(|| format!("{what}: {}", parts.join(", ")))
    };
    let (characters_added, characters_raised) = tally(&plan.characters);
    let (weapons_added, weapons_raised) = tally(&plan.weapons);
    let weapons_moved = (plan.weapons.iter())
        .filter(|change| moved(change).is_some())
        .count();
    let parts: Vec<String> = [
        count(
            [
                (characters_added, "add"),
                (characters_raised, "raise"),
                (0, ""),
            ],
            "Characters",
        ),
        count(
            [
                (weapons_added, "add"),
                (weapons_raised - weapons_moved, "raise"),
                (weapons_moved, "move"),
            ],
            "Weapons",
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        "Nothing to change.".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// One line per character or weapon copy the sync adds (+), raises (↑) or
/// moves to another holder (⇄): its sign and its text.
pub(super) fn lines(plan: &SyncPlan, planner: &PlannerData) -> Vec<(&'static str, String)> {
    let level = |level: Level| format!("{}/{}", level.level, level.cap());
    let name = |key: &str| {
        planner
            .character(key)
            .map_or(key, |c| c.name.as_str())
            .to_owned()
    };
    let character = |c: &OwnedCharacter| {
        let [a, e, q] = c.talents;
        format!(
            "{} · {} · C{} · talents {a}/{e}/{q}",
            name(&c.key),
            level(c.level),
            c.constellation
        )
    };
    let weapon = |w: &OwnedWeapon| {
        let weapon = planner
            .weapon(&w.key)
            .map_or(w.key.as_str(), |w| w.name.as_str());
        let holder = w
            .holder
            .as_deref()
            .map_or(String::new(), |h| format!(" · held by {}", name(h)));
        format!("{weapon} · {} · R{}{holder}", level(w.level), w.refinement)
    };
    let characters = plan.characters.iter().filter_map(|change| match change {
        Change::Added(c) => Some(("+", character(c))),
        Change::Raised { before, after } => Some((
            "↑",
            format!("{}  (was {})", character(after), character(before)),
        )),
        Change::Unchanged(_) => None,
    });
    let holder = |holder: &Option<String>| holder.as_deref().map_or("nobody".to_owned(), name);
    let weapons = plan.weapons.iter().filter_map(|change| match change {
        Change::Added(w) => Some(("+", weapon(w))),
        Change::Raised { .. } if let Some((before, after)) = moved(change) => {
            let name = planner
                .weapon(&after.key)
                .map_or(after.key.as_str(), |w| w.name.as_str());
            Some((
                "⇄",
                format!(
                    "{name} · {} · R{}: from {} to {}",
                    level(after.level),
                    after.refinement,
                    holder(&before.holder),
                    holder(&after.holder)
                ),
            ))
        }
        Change::Raised { before, after } => {
            Some(("↑", format!("{}  (was {})", weapon(after), weapon(before))))
        }
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
