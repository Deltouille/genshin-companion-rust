//! Importing a GOOD file into the store: one transaction after a backup,
//! into the account chosen or a new "Main", the same file twice changing
//! nothing; and the real exports in `tests/fixtures/private/good/`, whole.

use std::collections::BTreeMap;
use std::path::Path;

use wish_core::collection::{Change, GoodFile, SyncNote, plan_good};
use wish_core::planner::{CustomItem, Inventory, Level, MORA, PlannerData};
use wish_core::{AppPaths, Store};

/// Ganyu at 90 holding Amos' Bow, the Traveler holding a Favonius Sword,
/// two Cool Steels; Hero's Wit and a steak.
const SAMPLE: &str = r#"{
    "format": "GOOD", "version": 2, "source": "Test",
    "characters": [
        {"key": "Ganyu", "level": 90, "constellation": 1, "ascension": 6,
         "talent": {"auto": 8, "skill": 9, "burst": 10}},
        {"key": "TravelerAnemo", "level": 70, "constellation": 6, "ascension": 4,
         "talent": {"auto": 1, "skill": 4, "burst": 4}}
    ],
    "weapons": [
        {"key": "AmosBow", "level": 90, "ascension": 6, "refinement": 1, "location": "Ganyu"},
        {"key": "FavoniusSword", "level": 40, "ascension": 1, "refinement": 3, "location": "Traveler"},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""},
        {"key": "CoolSteel", "level": 1, "ascension": 0, "refinement": 1, "location": ""}
    ],
    "materials": {"HerosWit": 40, "DeliciousSteak": 3}
}"#;

const HEROS_WIT: u32 = 104003;

fn planner() -> &'static PlannerData {
    PlannerData::embedded()
}

#[test]
fn a_good_file_goes_into_a_new_main_account_when_there_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
    let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
    let (account, plan, backup) = store.apply_good(None, &file, planner()).unwrap();
    assert!(backup.exists(), "a backup first");
    assert_eq!(store.account(account).unwrap().label, "Main");
    assert_eq!(plan.changes(), 2 + 4 + 1);

    let owned = store.collection(account).unwrap();
    assert_eq!(owned.characters.len(), 2);
    let ganyu = owned.character("ganyu").unwrap();
    assert_eq!((ganyu.level, ganyu.constellation), (Level::MAX, 1));
    assert_eq!(ganyu.talents, [8, 9, 10]);
    assert_eq!(
        owned.weapon_of("ganyu").map(|w| w.key.as_str()),
        Some("amosbow")
    );
    assert_eq!(
        owned.weapon_of("traveleranemo").map(|w| w.refinement),
        Some(3)
    );
    assert_eq!(owned.copies("coolsteel").count(), 2);
    let inventory = store.inventory(account).unwrap();
    assert_eq!(inventory.owned(&CustomItem::Material(HEROS_WIT)), 40);

    // The same file again: nothing more.
    let (_, again, _) = store.apply_good(Some(account), &file, planner()).unwrap();
    assert_eq!(again.changes(), 0, "{again:#?}");
    assert_eq!(store.collection(account).unwrap().weapons.len(), 4);
}

#[test]
fn the_bag_replaces_the_inventory_but_the_mora() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
    let account = store.add_account("Mine", None, None).unwrap().id;
    let guide = (planner().materials().iter())
        .find(|m| m.name.english() == "Guide to Freedom")
        .unwrap()
        .id;
    store
        .set_owned_many(
            account,
            &[
                (CustomItem::Material(MORA), 420_000),
                (CustomItem::Material(guide), 12),
                (CustomItem::Material(HEROS_WIT), 300),
                (CustomItem::Named("Something typed".to_owned()), 2),
            ],
        )
        .unwrap();
    let file = GoodFile::parse(SAMPLE.as_bytes()).unwrap();
    store.apply_good(Some(account), &file, planner()).unwrap();
    let inventory = store.inventory(account).unwrap();
    // Used up since: lowered to the bag's count, or to none.
    assert_eq!(inventory.owned(&CustomItem::Material(HEROS_WIT)), 40);
    assert_eq!(inventory.owned(&CustomItem::Material(guide)), 0);
    // What the file can't say is kept.
    assert_eq!(inventory.owned(&CustomItem::Material(MORA)), 420_000);
    assert_eq!(
        inventory.owned(&CustomItem::Named("Something typed".to_owned())),
        2
    );
}

/// Real exports dropped into `tests/fixtures/private/good/` (git-ignored;
/// apart from the paimon.moe saves the other tests read): every character
/// but the Manekins and every weapon known, and the import twice changing
/// nothing. Passes trivially when there are none.
#[test]
fn real_exports_read_whole() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/good");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("no private GOOD exports in {}", dir.display());
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") {
            reads_whole(&path);
        }
    }
}

fn reads_whole(path: &Path) {
    let file = GoodFile::parse(&std::fs::read(path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let plan = plan_good(&file, &Default::default(), &Inventory::default(), planner());
    for note in &plan.collection.notes {
        match note {
            SyncNote::UnknownGoodCharacter { key } => {
                assert!(key.starts_with("Manekin"), "unknown character {key}")
            }
            SyncNote::UnknownGoodWeapon { key, .. } => panic!("unknown weapon {key}"),
            _ => {}
        }
    }
    let weapons = plan.collection.weapons.len();
    assert_eq!(weapons, file.weapons.len(), "every copy");
    assert!(
        plan.collection
            .weapons
            .iter()
            .all(|change| matches!(change, Change::Added(_)))
    );
    // Held copies keep their holder, the Traveler's too.
    let held = (file.weapons.iter())
        .filter(|w| !w.location.is_empty())
        .count();
    let holders = (plan.collection.weapons.iter())
        .filter(|w| w.after().holder.is_some())
        .count();
    assert_eq!(holders, held);

    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
    let (account, _, _) = store.apply_good(None, &file, planner()).unwrap();
    let (_, again, _) = store.apply_good(Some(account), &file, planner()).unwrap();
    assert_eq!(again.changes(), 0);
    let bag: BTreeMap<String, u64> = file.materials.unwrap();
    assert_eq!(
        store
            .inventory(account)
            .unwrap()
            .owned(&CustomItem::Material(HEROS_WIT)),
        bag.get("HerosWit").copied().unwrap_or(0)
    );
}
