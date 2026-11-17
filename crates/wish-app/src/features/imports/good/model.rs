//! What the GOOD import's preview says, in words: where it goes, the summary
//! of its counts, and the changes by section (identical lines once, with
//! their count: a bag's fodder comes by the hundred). Pure functions over the
//! [`GoodPlan`], tested without GPUI; the characters' and weapons' lines are
//! the HoYoLAB sync's.

use wish_core::collection::{GoodPlan, MaterialChange};
use wish_core::model::Account;
use wish_core::planner::PlannerData;
use wish_ui::format;

use super::super::hoyolab::model as sync;

/// "From Irminsul, into “Main”".
pub(super) fn heading(source: &str, account: &Option<Account>) -> String {
    let into = match account {
        Some(account) => t!("good-into", account = format::quoted(&account.label)),
        None => t!("good-into-new"),
    };
    match source.trim() {
        "" => t!("good-heading-unnamed", into = into),
        source => t!("good-heading", source = source, into = into),
    }
}

/// "Characters: 112 to add · Weapons: 1,940 to add · Materials: 470 to
/// change".
pub(super) fn summary(plan: &GoodPlan) -> String {
    let mut parts = Vec::new();
    if plan.collection.changes() > 0 {
        parts.push(sync::summary(&plan.collection));
    }
    if !plan.materials.is_empty() {
        parts.push(t!("good-materials", count = plan.materials.len()));
    }
    if parts.is_empty() {
        t!("hoyolab-nothing")
    } else {
        parts.join(" · ")
    }
}

/// A part of the preview: its title, and one line per change (its sign and
/// its text).
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Section {
    pub title: String,
    pub lines: Vec<(&'static str, String)>,
}

/// The characters, the weapons and the materials that change, each section
/// left out when nothing in it does.
pub(super) fn sections(plan: &GoodPlan, planner: &PlannerData) -> Vec<Section> {
    let mut lines = sync::lines(&plan.collection, planner);
    // `lines` gives the characters first.
    let characters = (plan.collection.characters.iter())
        .filter(|c| c.changes())
        .count();
    let weapons = lines.split_off(characters);
    let materials = (plan.materials.iter())
        .map(|change| material_line(change, planner))
        .collect();
    [
        (t!("good-section-characters"), lines),
        (t!("good-section-weapons"), grouped(weapons)),
        (t!("good-section-materials"), materials),
    ]
    .into_iter()
    .filter(|(_, lines)| !lines.is_empty())
    .map(|(title, lines)| Section { title, lines })
    .collect()
}

/// Each line once, in the order first seen, with how many times it came.
fn grouped(lines: Vec<(&'static str, String)>) -> Vec<(&'static str, String)> {
    let mut grouped: Vec<((&'static str, String), usize)> = Vec::new();
    for line in lines {
        match grouped.iter_mut().find(|(seen, _)| *seen == line) {
            Some((_, count)) => *count += 1,
            None => grouped.push((line, 1)),
        }
    }
    (grouped.into_iter())
        .map(|((sign, text), count)| match count {
            1 => (sign, text),
            count => (sign, t!("good-grouped", line = text, count = count)),
        })
        .collect()
}

/// "↑ Hero's Wit: 0 → 121", "↓" when the bag has fewer.
fn material_line(change: &MaterialChange, planner: &PlannerData) -> (&'static str, String) {
    let name =
        (planner.material(change.id)).map_or_else(|| change.id.to_string(), |m| m.name.to_string());
    let sign = if change.after > change.before {
        "↑"
    } else {
        "↓"
    };
    (
        sign,
        t!(
            "good-material-line",
            name = name,
            before = change.before,
            after = change.after
        ),
    )
}

#[cfg(test)]
mod tests {
    use wish_core::collection::{Change, OwnedCharacter, OwnedWeapon, SyncPlan};
    use wish_core::model::AccountId;
    use wish_core::planner::{Level, MORA};

    use super::*;

    fn plan() -> GoodPlan {
        let cool_steel = OwnedWeapon::new("coolsteel");
        let amos = OwnedWeapon {
            level: Level::MAX,
            holder: Some("ganyu".to_owned()),
            ..OwnedWeapon::new("amosbow")
        };
        GoodPlan {
            collection: SyncPlan {
                characters: vec![Change::Added(OwnedCharacter::new("ganyu"))],
                weapons: vec![
                    Change::Added(cool_steel.clone()),
                    Change::Added(amos),
                    Change::Added(cool_steel.clone()),
                    Change::Added(cool_steel),
                ],
                notes: Vec::new(),
            },
            materials: vec![
                MaterialChange {
                    id: 104003,
                    before: 0,
                    after: 1210,
                },
                MaterialChange {
                    id: MORA,
                    before: 9,
                    after: 2,
                },
            ],
        }
    }

    #[test]
    fn fodder_comes_once_with_its_count() {
        let sections = sections(&plan(), PlannerData::embedded());
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Characters", "Weapons", "Materials"]);
        assert_eq!(
            sections[0].lines,
            [("+", "Ganyu · 1/20 · C0 · talents 1/1/1".to_owned())]
        );
        assert_eq!(
            sections[1].lines,
            [
                ("+", "Cool Steel · 1/20 · R1 ×3".to_owned()),
                ("+", "Amos' Bow · 90/90 · R1 · held by Ganyu".to_owned()),
            ]
        );
        assert_eq!(
            sections[2].lines,
            [
                ("↑", "Hero's Wit: 0 → 1,210".to_owned()),
                ("↓", "Mora: 9 → 2".to_owned()),
            ]
        );
    }

    #[test]
    fn the_summary_counts_the_materials_too() {
        assert_eq!(
            summary(&plan()),
            "Characters: 1 to add · Weapons: 4 to add · Materials: 2 to change"
        );
        assert_eq!(summary(&GoodPlan::default()), "Nothing to change.");
        assert!(sections(&GoodPlan::default(), PlannerData::embedded()).is_empty());
    }

    #[test]
    fn where_it_goes() {
        let main = Some(Account {
            id: AccountId(1),
            label: "Main".to_owned(),
            uid: None,
            server: None,
        });
        assert_eq!(heading("Irminsul", &main), "From Irminsul, into “Main”");
        assert_eq!(
            heading(" ", &None),
            "From a GOOD file, into a new account, “Main”"
        );
    }
}
