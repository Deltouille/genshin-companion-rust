//! What a HoYoLAB sync changes in an account's collection: HoYoLAB's
//! characters and the weapons they hold, turned into the collection's, then
//! set against what the player entered. It only adds or raises, never
//! lowers; what the game data doesn't have is listed, never guessed.
//! Nothing is written here: [`Store::apply_sync`](crate::Store::apply_sync)
//! writes a plan once the player confirmed it.
//!
//! What HoYoLAB sends, and how it is read (checked on a real account):
//!
//! - characters and weapons by the game's ids, as `planner.json` has them;
//!   the Traveler as 10000005 (Aether) or 10000007 (Lumine), its element
//!   picking `traveler<element>`;
//! - a character's level without its ascension phase: at a level cap (20, 40
//!   … 80) it is counted as not ascended yet, and said so; above 90 (the
//!   game's newer levels) as 90/90, and said so. A weapon's phase is sent;
//! - talent levels with the +3 of constellations 3 and 5: the skill an
//!   active C3 or C5 names after "Level of" gets 3 back; the three talents
//!   are told apart by their names in `archive.json`.

use std::collections::HashSet;

use hoyolab::{CharacterDetail, EquippedWeapon, Skill};

use super::{
    Collection, MAX_CONSTELLATION, MAX_REFINEMENT, MAX_TALENT, OwnedCharacter, OwnedWeapon,
};
use crate::archive::{ArchiveData, CharacterPage};
use crate::planner::{Level, PlannerData};

/// The Traveler's ids: Aether and Lumine.
const TRAVELER_IDS: [u32; 2] = [10000005, 10000007];

/// What happens to one character or weapon copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change<T> {
    /// New to the collection.
    Added(T),
    /// Already there, some of it raised (a weapon copy may also change
    /// hands: one the player entered without a holder is given to its
    /// character).
    Raised { before: T, after: T },
    /// Already there as HoYoLAB has it, or higher.
    Unchanged(T),
}

impl<T> Change<T> {
    /// What the collection holds after the sync.
    pub fn after(&self) -> &T {
        match self {
            Change::Added(after) | Change::Raised { after, .. } | Change::Unchanged(after) => after,
        }
    }

    pub fn changes(&self) -> bool {
        !matches!(self, Change::Unchanged(_))
    }
}

/// Something the player should know about a sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncNote {
    /// At a level cap: HoYoLAB doesn't say whether it ascended. Counted as
    /// not yet.
    PhaseUnknown { name: String, level: u8 },
    /// Above the levels the app knows: counted as 90/90.
    AboveMaxLevel { name: String, level: u8 },
    /// A character the game data doesn't have: left out, with its weapon.
    UnknownCharacter { name: String, id: u32 },
    /// A weapon the game data doesn't have: left out.
    UnknownWeapon { name: String, id: u32 },
    /// Talents the archive doesn't name: left as they were (1 for a new
    /// character), to set on its page.
    TalentsUnread { name: String },
}

/// What a sync would change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncPlan {
    pub characters: Vec<Change<OwnedCharacter>>,
    pub weapons: Vec<Change<OwnedWeapon>>,
    pub notes: Vec<SyncNote>,
}

impl SyncPlan {
    /// How many characters and weapon copies it adds or raises.
    pub fn changes(&self) -> usize {
        self.characters.iter().filter(|c| c.changes()).count()
            + self.weapons.iter().filter(|w| w.changes()).count()
    }
}

/// Sets HoYoLAB's `details` against `current`, the account's collection.
pub fn plan_sync(
    details: &[CharacterDetail],
    current: &Collection,
    planner: &PlannerData,
    archive: &ArchiveData,
) -> SyncPlan {
    let mut plan = SyncPlan::default();
    // Copies without a holder that a character of the sync took.
    let mut claimed: HashSet<i64> = HashSet::new();
    for detail in details {
        let base = &detail.base;
        let Some(key) = character_key(detail, planner) else {
            plan.notes.push(SyncNote::UnknownCharacter {
                name: base.name.clone(),
                id: base.id,
            });
            continue;
        };
        let (level, note) = character_level(base.level);
        let talents = read_talents(detail, archive.character(&key));
        if talents.is_none() {
            plan.notes.push(SyncNote::TalentsUnread {
                name: base.name.clone(),
            });
        }
        let synced = OwnedCharacter {
            key: key.clone(),
            level,
            constellation: base.constellation.min(MAX_CONSTELLATION),
            talents: talents.unwrap_or([1; 3]),
            favourite: false,
        };
        let change = match current.character(&key) {
            None => Change::Added(synced),
            Some(before) => {
                let mut after = before.clone();
                after.level = after.level.max(synced.level);
                after.constellation = after.constellation.max(synced.constellation);
                if let Some(talents) = talents {
                    for (have, read) in after.talents.iter_mut().zip(talents) {
                        *have = (*have).max(read);
                    }
                }
                if after == *before {
                    Change::Unchanged(after)
                } else {
                    Change::Raised {
                        before: before.clone(),
                        after,
                    }
                }
            }
        };
        // The level note matters only when HoYoLAB's level is the one kept.
        if let Some(note) = note
            && change.after().level == level
        {
            plan.notes.push(match note {
                LevelNote::PhaseUnknown => SyncNote::PhaseUnknown {
                    name: base.name.clone(),
                    level: base.level,
                },
                LevelNote::AboveMax => SyncNote::AboveMaxLevel {
                    name: base.name.clone(),
                    level: base.level,
                },
            });
        }
        plan.characters.push(change);

        match planner.weapons().iter().find(|w| w.id == detail.weapon.id) {
            Some(weapon) => {
                let synced = held_weapon(&weapon.key, &key, &detail.weapon);
                plan.weapons
                    .push(weapon_change(synced, current, &mut claimed));
            }
            None => plan.notes.push(SyncNote::UnknownWeapon {
                name: detail.weapon.name.clone(),
                id: detail.weapon.id,
            }),
        }
    }
    plan
}

/// The planner's key of the character in `detail`.
fn character_key(detail: &CharacterDetail, planner: &PlannerData) -> Option<String> {
    let base = &detail.base;
    if TRAVELER_IDS.contains(&base.id) {
        let key = format!("traveler{}", base.element.to_lowercase());
        return planner.character(&key).map(|c| c.key.clone());
    }
    (planner.characters().iter())
        .find(|c| c.id == base.id)
        .map(|c| c.key.clone())
}

enum LevelNote {
    PhaseUnknown,
    AboveMax,
}

/// A character's level as the collection keeps it: the lowest phase that
/// holds it (not yet ascended at a cap, which is said), 90/90 above 90.
fn character_level(level: u8) -> (Level, Option<LevelNote>) {
    if level > Level::MAX.level {
        return (Level::MAX, Some(LevelNote::AboveMax));
    }
    lowest_phase(level)
}

/// `level` at the lowest phase that holds it, and whether the next phase
/// does too (a level cap).
fn lowest_phase(level: u8) -> (Level, Option<LevelNote>) {
    let level = level.max(1);
    let Some(phase) = (0..=Level::MAX.phase).find(|&p| Level::new(level, p).is_some()) else {
        return (Level::MAX, None);
    };
    let ambiguous = Level::new(level, phase + 1).is_some();
    (
        Level { level, phase },
        ambiguous.then_some(LevelNote::PhaseUnknown),
    )
}

/// The three talents (Normal Attack, Elemental Skill, Elemental Burst)
/// without the constellations' +3, or `None` when one isn't named in the
/// archive.
fn read_talents(detail: &CharacterDetail, page: Option<&CharacterPage>) -> Option<[u8; 3]> {
    let page = page?;
    let skills: Vec<&Skill> = detail.skills.iter().filter(|s| s.skill_type == 1).collect();
    // The skills the active C3 and C5 raise.
    let boosted: Vec<u32> = (detail.constellations.iter())
        .filter(|c| c.active && (c.pos == 3 || c.pos == 5))
        .filter_map(|c| {
            let after = c
                .effect
                .find("Level of")
                .map_or(&c.effect[..], |at| &c.effect[at..]);
            (skills.iter())
                .filter_map(|s| after.find(&s.name).map(|at| (at, s.id)))
                .min_by_key(|(at, _)| *at)
                .map(|(_, id)| id)
        })
        .collect();
    let mut talents = [None; 3];
    for skill in &skills {
        let kind = page
            .talents
            .iter()
            .find(|t| t.name == skill.name)?
            .kind
            .as_str();
        let slot = match kind {
            "Normal Attack" => 0,
            "Elemental Skill" => 1,
            "Elemental Burst" => 2,
            // The alternate sprint of Mona and Ayaka.
            _ => continue,
        };
        let boost = if boosted.contains(&skill.id) { 3 } else { 0 };
        talents[slot] = Some(skill.level.saturating_sub(boost).clamp(1, MAX_TALENT));
    }
    Some([talents[0]?, talents[1]?, talents[2]?])
}

/// The copy of weapon `key` that `holder` holds, as HoYoLAB has it.
fn held_weapon(key: &str, holder: &str, weapon: &EquippedWeapon) -> OwnedWeapon {
    let level = match weapon.promote_level {
        Some(phase) => {
            Level::new(weapon.level, phase).unwrap_or_else(|| Level::clamped(weapon.level, phase))
        }
        None => lowest_phase(weapon.level).0,
    };
    OwnedWeapon {
        id: 0,
        key: key.to_owned(),
        level,
        refinement: weapon.refinement.clamp(1, MAX_REFINEMENT),
        holder: Some(holder.to_owned()),
        favourite: false,
    }
}

/// Where `synced` goes: the copy its holder already holds, else one of that
/// weapon without a holder (each taken once), else a new copy.
fn weapon_change(
    synced: OwnedWeapon,
    current: &Collection,
    claimed: &mut HashSet<i64>,
) -> Change<OwnedWeapon> {
    let held = current
        .weapons
        .iter()
        .find(|w| w.key == synced.key && w.holder == synced.holder);
    let free = || {
        current
            .weapons
            .iter()
            .find(|w| w.key == synced.key && w.holder.is_none() && !claimed.contains(&w.id))
    };
    let Some(before) = held.or_else(free) else {
        return Change::Added(synced);
    };
    claimed.insert(before.id);
    let mut after = before.clone();
    after.level = after.level.max(synced.level);
    after.refinement = after.refinement.max(synced.refinement);
    after.holder = synced.holder;
    if after == *before {
        Change::Unchanged(after)
    } else {
        Change::Raised {
            before: before.clone(),
            after,
        }
    }
}

#[cfg(test)]
mod tests {
    use hoyolab::{Constellation, OwnedCharacter as HoyolabCharacter};

    use super::*;

    fn ganyu(level: u8, constellation: u8, skills: [u8; 3], weapon_level: u8) -> CharacterDetail {
        let constellations = [
            (
                3,
                "Increases the Level of <color=#FFD780FF>Celestial Shower</color> by 3.",
            ),
            (5, "Increases the Level of Trail of the Qilin by 3."),
        ]
        .into_iter()
        .map(|(pos, effect)| Constellation {
            id: 370 + u32::from(pos),
            pos,
            name: format!("C{pos}"),
            effect: effect.to_owned(),
            active: constellation >= pos,
        })
        .collect();
        let names = ["Liutian Archery", "Trail of the Qilin", "Celestial Shower"];
        CharacterDetail {
            base: HoyolabCharacter {
                id: 10000037,
                name: "Ganyu".to_owned(),
                element: "Cryo".to_owned(),
                rarity: 5,
                level,
                constellation,
                weapon: None,
            },
            weapon: EquippedWeapon {
                id: 15502,
                name: "Amos' Bow".to_owned(),
                rarity: 5,
                level: weapon_level,
                refinement: 1,
                promote_level: Some(if weapon_level >= 80 { 6 } else { 0 }),
            },
            constellations,
            skills: names
                .iter()
                .zip(skills)
                .enumerate()
                .map(|(i, (name, level))| Skill {
                    id: 10371 + i as u32,
                    skill_type: 1,
                    name: (*name).to_owned(),
                    level,
                })
                .collect(),
        }
    }

    fn plan(details: &[CharacterDetail], current: &Collection) -> SyncPlan {
        plan_sync(
            details,
            current,
            PlannerData::embedded(),
            ArchiveData::embedded(),
        )
    }

    #[test]
    fn a_new_character_comes_with_its_talents_without_the_constellations() {
        // C5: the burst (C3) and the skill (C5) show 3 more.
        let plan = plan(&[ganyu(90, 5, [6, 12, 13], 90)], &Collection::default());
        let Change::Added(ganyu) = &plan.characters[0] else {
            panic!("{:?}", plan.characters[0]);
        };
        assert_eq!(ganyu.key, "ganyu");
        assert_eq!(ganyu.level, Level::MAX);
        assert_eq!(ganyu.constellation, 5);
        assert_eq!(ganyu.talents, [6, 9, 10]);
        let Change::Added(amos) = &plan.weapons[0] else {
            panic!("{:?}", plan.weapons[0]);
        };
        assert_eq!(amos.key, "amosbow");
        assert_eq!(amos.holder.as_deref(), Some("ganyu"));
        assert!(plan.notes.is_empty());
        assert_eq!(plan.changes(), 2);
    }

    #[test]
    fn levels_hoyolab_leaves_unclear_are_said() {
        let at_cap = plan(&[ganyu(80, 0, [1, 1, 1], 1)], &Collection::default());
        assert_eq!(
            at_cap.characters[0].after().level,
            Level::new(80, 5).unwrap()
        );
        assert!(matches!(
            at_cap.notes[..],
            [SyncNote::PhaseUnknown { level: 80, .. }]
        ));
        let above = plan(&[ganyu(100, 0, [1, 1, 1], 1)], &Collection::default());
        assert_eq!(above.characters[0].after().level, Level::MAX);
        assert!(matches!(
            above.notes[..],
            [SyncNote::AboveMaxLevel { level: 100, .. }]
        ));
        // Already ascended in the collection: kept, nothing to say.
        let collection = Collection {
            characters: vec![OwnedCharacter {
                level: Level::new(80, 6).unwrap(),
                ..OwnedCharacter::new("ganyu")
            }],
            weapons: Vec::new(),
        };
        let kept = plan(&[ganyu(80, 0, [1, 1, 1], 1)], &collection);
        assert_eq!(kept.characters[0].after().level, Level::new(80, 6).unwrap());
        assert!(kept.notes.is_empty());
    }

    #[test]
    fn what_the_player_entered_is_raised_never_lowered() {
        let collection = Collection {
            characters: vec![OwnedCharacter {
                level: Level::MAX,
                constellation: 1,
                talents: [9, 2, 9],
                favourite: true,
                ..OwnedCharacter::new("ganyu")
            }],
            weapons: vec![OwnedWeapon {
                id: 7,
                refinement: 3,
                ..OwnedWeapon::new("amosbow")
            }],
        };
        let plan = plan(&[ganyu(70, 2, [6, 9, 9], 90)], &collection);
        let Change::Raised { after, .. } = &plan.characters[0] else {
            panic!("{:?}", plan.characters[0]);
        };
        assert_eq!(after.level, Level::MAX, "not lowered to 70");
        assert_eq!(after.constellation, 2);
        assert_eq!(after.talents, [9, 9, 9]);
        assert!(after.favourite, "the favourite stays");
        // The Amos' Bow entered without a holder is Ganyu's, at 90, still R3.
        let Change::Raised { after, .. } = &plan.weapons[0] else {
            panic!("{:?}", plan.weapons[0]);
        };
        assert_eq!(after.id, 7);
        assert_eq!(after.holder.as_deref(), Some("ganyu"));
        assert_eq!(after.level, Level::MAX);
        assert_eq!(after.refinement, 3);

        // Synced twice: nothing more to do.
        let again = Collection {
            characters: plan.characters.iter().map(|c| c.after().clone()).collect(),
            weapons: plan.weapons.iter().map(|w| w.after().clone()).collect(),
        };
        let second = super::plan_sync(
            &[ganyu(70, 2, [6, 9, 9], 90)],
            &again,
            PlannerData::embedded(),
            ArchiveData::embedded(),
        );
        assert_eq!(second.changes(), 0);
    }

    #[test]
    fn the_traveler_and_characters_the_data_lacks() {
        let mut lumine = ganyu(90, 0, [1, 1, 1], 1);
        lumine.base.id = 10000007;
        lumine.base.name = "Traveler".to_owned();
        lumine.skills.clear();
        let mut manekina = ganyu(90, 0, [1, 1, 1], 1);
        manekina.base.id = 10000118;
        manekina.base.name = "Manekina".to_owned();
        let plan = plan(&[lumine, manekina], &Collection::default());
        assert_eq!(plan.characters.len(), 1);
        assert_eq!(plan.characters[0].after().key, "travelercryo");
        // The Traveler's talents weren't sent here: 1, and said so.
        assert_eq!(plan.characters[0].after().talents, [1, 1, 1]);
        assert!(plan.notes.contains(&SyncNote::TalentsUnread {
            name: "Traveler".to_owned()
        }));
        assert!(plan.notes.contains(&SyncNote::UnknownCharacter {
            name: "Manekina".to_owned(),
            id: 10000118
        }));
    }
}
