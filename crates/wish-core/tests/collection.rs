//! What each account owns and plans: accounts added by hand, the plan and the
//! inventory per account (and migration 007, which gave them one), owned
//! characters and weapons.

use rusqlite::{Connection, params};
use wish_core::collection::{OwnedCharacter, OwnedWeapon};
use wish_core::error::Error;
use wish_core::model::{AccountId, Server};
use wish_core::planner::{CharacterGoal, CustomItem, Level, PlanTarget};
use wish_core::{AppPaths, Store};

const MORA: u32 = 202;

fn ganyu() -> PlanTarget {
    PlanTarget::Character {
        key: "ganyu".to_owned(),
        goal: CharacterGoal::FULL,
    }
}

/// A database as version 6 left it (the plan and inventory were the whole
/// app's), with `accounts` and, when `planned`, a plan entry and some Mora.
fn database_at_version_6(dir: &std::path::Path, accounts: &[&str], planned: bool) {
    const BEFORE_007: [&str; 6] = [
        include_str!("../src/store/schema/001_initial.sql"),
        include_str!("../src/store/schema/002_links_and_undo.sql"),
        include_str!("../src/store/schema/003_planner.sql"),
        include_str!("../src/store/schema/004_inventory_and_custom_characters.sql"),
        include_str!("../src/store/schema/005_planner_weapons.sql"),
        include_str!("../src/store/schema/006_planner_custom_weapons.sql"),
    ];
    let paths = AppPaths::at(dir);
    let conn = Connection::open(paths.database()).unwrap();
    for sql in BEFORE_007 {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 6).unwrap();
    for label in accounts {
        conn.execute(
            "INSERT INTO accounts (label, created_at) VALUES (?1, '2026-01-01T00:00:00Z')",
            [label],
        )
        .unwrap();
    }
    if planned {
        conn.execute(
            "INSERT INTO plan_entries (position, character_key, level_to, phase_to, created_at) \
             VALUES (0, 'ganyu', 90, 6, '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO inventory (material_id, count, updated_at) VALUES (?1, 420000, 'x')",
            params![MORA],
        )
        .unwrap();
    }
}

mod migration {
    use super::*;

    #[test]
    fn a_plan_without_any_account_goes_to_a_new_main_account() {
        let dir = tempfile::tempdir().unwrap();
        database_at_version_6(dir.path(), &[], true);
        let store = Store::open(AppPaths::at(dir.path())).unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].label, "Main");
        assert_eq!(accounts[0].uid, None);
        let main = accounts[0].id;
        assert_eq!(store.plan(main).unwrap().len(), 1);
        assert_eq!(
            store
                .inventory(main)
                .unwrap()
                .owned(&CustomItem::Material(MORA)),
            420_000
        );
        // The migration was backed up first.
        assert!(!store.backups().unwrap().is_empty());
    }

    #[test]
    fn the_plan_goes_to_the_oldest_account() {
        let dir = tempfile::tempdir().unwrap();
        database_at_version_6(dir.path(), &["First", "Second"], true);
        let store = Store::open(AppPaths::at(dir.path())).unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 2, "no Main account when there are accounts");
        let (first, second) = (accounts[0].id, accounts[1].id);
        assert_eq!(store.plan(first).unwrap().len(), 1);
        assert!(store.plan(second).unwrap().is_empty());
        assert_eq!(
            store
                .inventory(first)
                .unwrap()
                .owned(&CustomItem::Material(MORA)),
            420_000
        );
        assert_eq!(
            store
                .inventory(second)
                .unwrap()
                .owned(&CustomItem::Material(MORA)),
            0
        );
    }

    #[test]
    fn nothing_planned_makes_no_account() {
        let dir = tempfile::tempdir().unwrap();
        database_at_version_6(dir.path(), &[], false);
        let store = Store::open(AppPaths::at(dir.path())).unwrap();
        assert!(store.accounts().unwrap().is_empty());
    }
}

mod accounts {
    use super::*;

    #[test]
    fn an_account_is_added_and_renamed_by_hand() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let main = store
            .add_account("  Main  ", Some("700000001"), Some(Server::Europe))
            .unwrap();
        assert_eq!(main.label, "Main");
        assert_eq!(main.uid.as_deref(), Some("700000001"));
        assert_eq!(store.accounts().unwrap(), std::slice::from_ref(&main));
        // Made by hand: no import ever went to it.
        assert!(!store.has_imports(main.id).unwrap());

        store.rename_account(main.id, "Alt").unwrap();
        assert_eq!(store.accounts().unwrap()[0].label, "Alt");
        assert!(matches!(
            store.rename_account(main.id, " "),
            Err(Error::EmptyAccountName)
        ));
        assert!(matches!(
            store.rename_account(AccountId(99), "Ghost"),
            Err(Error::UnknownAccount(_))
        ));
    }

    #[test]
    fn a_bad_or_taken_uid_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        assert!(matches!(
            store.add_account("Main", Some("12ab"), None),
            Err(Error::InvalidUid(_))
        ));
        assert!(matches!(
            store.add_account("", None, None),
            Err(Error::EmptyAccountName)
        ));
        store.add_account("Main", Some("800000001"), None).unwrap();
        assert!(matches!(
            store.add_account("Alt", Some("800000001"), None),
            Err(Error::UidTaken { .. })
        ));
        // A blank UID is no UID.
        let alt = store.add_account("Alt", Some("  "), None).unwrap();
        assert_eq!(alt.uid, None);
    }
}

mod per_account {
    use super::*;

    #[test]
    fn each_account_has_its_own_plan_and_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let main = store.add_account("Main", None, None).unwrap().id;
        let alt = store.add_account("Alt", None, None).unwrap().id;

        let first = store.add_plan_entry(main, &ganyu()).unwrap();
        let second = store.add_plan_entry(main, &ganyu()).unwrap();
        let other = store.add_plan_entry(alt, &ganyu()).unwrap();
        let ids = |store: &Store, account| -> Vec<i64> {
            store.plan(account).unwrap().iter().map(|e| e.id).collect()
        };
        assert_eq!(ids(&store, main), [first, second]);
        assert_eq!(ids(&store, alt), [other]);

        // Reordering one account's plan leaves the other's alone, even when
        // handed an id of the other.
        store.reorder_plan(main, &[second, first, other]).unwrap();
        assert_eq!(ids(&store, main), [second, first]);
        assert_eq!(ids(&store, alt), [other]);

        let mora = CustomItem::Material(MORA);
        let named = CustomItem::Named("Moon Lily".to_owned());
        store.set_owned(main, &mora, 1_000).unwrap();
        store.set_owned(alt, &mora, 2_000).unwrap();
        store.set_owned(main, &named, 3).unwrap();
        store.set_owned(alt, &named, 4).unwrap();
        assert_eq!(store.inventory(main).unwrap().owned(&mora), 1_000);
        assert_eq!(store.inventory(alt).unwrap().owned(&mora), 2_000);
        store.set_owned(main, &named, 0).unwrap();
        assert_eq!(store.inventory(main).unwrap().owned(&named), 0);
        assert_eq!(store.inventory(alt).unwrap().owned(&named), 4);
    }
}

mod owned {
    use super::*;

    fn at(level: u8, phase: u8) -> Level {
        Level::new(level, phase).unwrap()
    }

    #[test]
    fn characters_are_added_edited_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let main = store.add_account("Main", None, None).unwrap().id;
        let alt = store.add_account("Alt", None, None).unwrap().id;
        assert!(store.collection(main).unwrap().is_empty());

        let ganyu = OwnedCharacter {
            level: at(80, 6),
            constellation: 2,
            talents: [6, 9, 9],
            favourite: true,
            ..OwnedCharacter::new("ganyu")
        };
        store.save_owned_character(main, &ganyu).unwrap();
        store
            .save_owned_character(main, &OwnedCharacter::new("amber"))
            .unwrap();
        let collection = store.collection(main).unwrap();
        assert_eq!(
            collection.characters,
            [OwnedCharacter::new("amber"), ganyu.clone()],
            "by key"
        );
        assert!(store.collection(alt).unwrap().is_empty());

        // Saving again replaces what was entered.
        let raised = OwnedCharacter {
            level: at(90, 6),
            constellation: 3,
            ..ganyu
        };
        store.save_owned_character(main, &raised).unwrap();
        assert_eq!(
            store.collection(main).unwrap().character("ganyu"),
            Some(&raised)
        );

        store.remove_owned_character(main, "amber").unwrap();
        assert_eq!(store.collection(main).unwrap().characters, [raised]);
    }

    #[test]
    fn values_out_of_the_games_ranges_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let main = store.add_account("Main", None, None).unwrap().id;
        let c7 = OwnedCharacter {
            constellation: 7,
            ..OwnedCharacter::new("ganyu")
        };
        assert!(store.save_owned_character(main, &c7).is_err());
        let r6 = OwnedWeapon {
            refinement: 6,
            ..OwnedWeapon::new("amosbow")
        };
        assert!(store.add_owned_weapon(main, &r6).is_err());
        assert!(store.collection(main).unwrap().is_empty());
    }

    #[test]
    fn weapons_are_copies_each_held_by_one_character() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let main = store.add_account("Main", None, None).unwrap().id;

        let amos = OwnedWeapon {
            level: at(90, 6),
            refinement: 1,
            holder: Some("ganyu".to_owned()),
            ..OwnedWeapon::new("amosbow")
        };
        let first = store.add_owned_weapon(main, &amos).unwrap();
        // A second copy of the same weapon.
        let second = store
            .add_owned_weapon(main, &OwnedWeapon::new("amosbow"))
            .unwrap();
        let collection = store.collection(main).unwrap();
        assert_eq!(collection.copies("amosbow").count(), 2);
        assert_eq!(collection.weapon_of("ganyu").map(|w| w.id), Some(first));

        // Giving Ganyu the other copy takes the first from her.
        let mut other = collection.weapons[1].clone();
        assert_eq!(other.id, second);
        other.holder = Some("ganyu".to_owned());
        other.refinement = 5;
        store.update_owned_weapon(main, &other).unwrap();
        let collection = store.collection(main).unwrap();
        assert_eq!(collection.weapon_of("ganyu"), Some(&other));
        assert_eq!(collection.weapons[0].holder, None);

        // A character removed lets go of her weapon, which stays.
        store
            .save_owned_character(main, &OwnedCharacter::new("ganyu"))
            .unwrap();
        store.remove_owned_character(main, "ganyu").unwrap();
        let collection = store.collection(main).unwrap();
        assert_eq!(collection.weapons.len(), 2);
        assert!(collection.weapons.iter().all(|w| w.holder.is_none()));

        store.remove_owned_weapon(main, first).unwrap();
        let ids: Vec<i64> = store
            .collection(main)
            .unwrap()
            .weapons
            .iter()
            .map(|w| w.id)
            .collect();
        assert_eq!(ids, [second]);
    }
}

mod hoyolab_sync {
    use super::*;
    use hoyolab::{CharacterDetail, EquippedWeapon, Skill};
    use wish_core::archive::ArchiveData;
    use wish_core::planner::PlannerData;

    /// A character as HoYoLAB details it, with no constellation that raises
    /// a talent.
    fn detail(
        (id, name, level): (u32, &str, u8),
        talents: [(&str, u8); 3],
        (weapon, weapon_level, phase): (u32, u8, u8),
    ) -> CharacterDetail {
        CharacterDetail {
            base: hoyolab::OwnedCharacter {
                id,
                name: name.to_owned(),
                element: String::new(),
                rarity: 5,
                level,
                constellation: 1,
                weapon: None,
            },
            weapon: EquippedWeapon {
                id: weapon,
                name: String::new(),
                rarity: 5,
                level: weapon_level,
                refinement: 1,
                promote_level: Some(phase),
            },
            constellations: Vec::new(),
            skills: talents
                .iter()
                .enumerate()
                .map(|(i, (name, level))| Skill {
                    id: i as u32,
                    skill_type: 1,
                    name: (*name).to_owned(),
                    level: *level,
                })
                .collect(),
        }
    }

    #[test]
    fn a_sync_adds_raises_and_never_lowers() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let main = store
            .add_account("Main", Some("712345678"), Some(Server::Europe))
            .unwrap()
            .id;
        assert_eq!(
            store.account_with_uid("712345678").unwrap().map(|a| a.id),
            Some(main)
        );
        assert!(store.account_with_uid("812345678").unwrap().is_none());
        let ganyu = OwnedCharacter {
            level: Level::MAX,
            talents: [9, 9, 9],
            favourite: true,
            ..OwnedCharacter::new("ganyu")
        };
        store.save_owned_character(main, &ganyu).unwrap();

        let details = [
            detail(
                (10000037, "Ganyu", 80),
                [
                    ("Liutian Archery", 6),
                    ("Trail of the Qilin", 6),
                    ("Celestial Shower", 6),
                ],
                (15502, 90, 6),
            ),
            detail(
                (10000021, "Amber", 40),
                [
                    ("Sharpshooter", 1),
                    ("Explosive Puppet", 4),
                    ("Fiery Rain", 2),
                ],
                (15401, 40, 1),
            ),
        ];
        let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
        let (plan, backup) = store.apply_sync(main, &details, planner, archive).unwrap();
        assert!(backup.exists());
        assert!(
            backup.to_string_lossy().contains("before-sync"),
            "{}",
            backup.display()
        );
        assert_eq!(plan.changes(), 4, "Ganyu's C1, Amber, two weapons");

        let collection = store.collection(main).unwrap();
        let kept = collection.character("ganyu").unwrap();
        assert_eq!(kept.level, Level::MAX, "not lowered to 80");
        assert_eq!(kept.talents, [9, 9, 9]);
        assert_eq!(kept.constellation, 1);
        assert!(kept.favourite);
        let amber = collection.character("amber").unwrap();
        assert_eq!(amber.level, Level::new(40, 1).unwrap());
        assert_eq!(amber.talents, [1, 4, 2]);
        assert_eq!(
            collection.weapon_of("amber").map(|w| w.key.as_str()),
            Some("favoniuswarbow")
        );
        assert_eq!(
            collection
                .weapon_of("ganyu")
                .map(|w| (w.key.as_str(), w.level)),
            Some(("amosbow", Level::MAX))
        );

        // Synced again: nothing changes, no copy is added twice.
        let (again, _) = store.apply_sync(main, &details, planner, archive).unwrap();
        assert_eq!(again.changes(), 0);
        assert_eq!(store.collection(main).unwrap().weapons.len(), 2);
    }
}
