//! Frozen witnesses (`tests/fixtures/witness/`) of the files the app writes
//! outside its database: a snapshot (and mirror file) of each version, and
//! `settings.json`. The app wrote them once, the version 1 snapshot with
//! 0.5.0's own code, and they never change: a file on a player's disk must
//! keep reading, whatever the types behind it become. A new version of a
//! format adds a witness; it never edits one.

use serde_json::Value;
use wish_core::collection::{OwnedCharacter, OwnedWeapon};
use wish_core::model::{ManualCounters, Pool, Rarity, Server, Source};
use wish_core::planner::{CustomItem, Level, MORA, PlanTarget};
use wish_core::settings::Settings;
use wish_core::snapshot::Snapshot;
use wish_core::{AppPaths, Store};

const SNAPSHOT_V1: &str = include_str!("fixtures/witness/snapshot-v1.json");
const SNAPSHOT_V2: &str = include_str!("fixtures/witness/snapshot-v2.json");
const SNAPSHOT_V3: &str = include_str!("fixtures/witness/snapshot-v3.json");
const SETTINGS_V0_6: &str = include_str!("fixtures/witness/settings-v0.6.json");

fn open_store(dir: &std::path::Path) -> Store {
    Store::open(AppPaths::at(dir)).unwrap()
}

/// A file's JSON without what changes at each export.
fn content(json: &[u8]) -> Value {
    let mut value: Value = serde_json::from_slice(json).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("exported_at");
    object.remove("app_version");
    value
}

#[test]
fn the_version_1_witness_still_reads() {
    let snapshot = Snapshot::from_json(SNAPSHOT_V1.as_bytes()).unwrap();
    assert_eq!(
        (snapshot.version, snapshot.app_version.as_str()),
        (1, "0.5.0")
    );
    let [main, second] = snapshot.accounts.as_slice() else {
        panic!("two accounts");
    };

    assert_eq!(main.label, "Main");
    assert_eq!(main.uid.as_deref(), Some("700000001"));
    assert_eq!(main.server, Some(Server::Europe));
    assert_eq!(main.wishes.len(), 18);
    let from = |source| main.wishes.iter().filter(|w| w.source == source).count();
    assert_eq!(
        (
            from(Source::Paimon),
            from(Source::Manual),
            from(Source::Api)
        ),
        (14, 3, 1)
    );
    let first = &main.wishes[0];
    assert_eq!(first.id.as_str(), "paimon:character-event:20240101100000:0");
    assert_eq!(first.time.to_string(), "2024-01-01 10:00:00");
    assert_eq!(
        (first.name.as_str(), first.rarity),
        ("Slingshot", Rarity::Three)
    );
    let venti = (main.wishes.iter())
        .find(|w| w.name == "Venti" && w.source == Source::Manual)
        .unwrap();
    assert_eq!(venti.manual_pity, Some(77));
    assert_eq!(venti.pool(), Pool::CharacterEvent);
    let fetched = main
        .wishes
        .iter()
        .find(|w| w.source == Source::Api)
        .unwrap();
    assert!(fetched.id.is_api());
    assert_eq!(fetched.item_key.as_deref(), Some("debate_club"));
    assert_eq!(main.links.len(), 1);
    assert_eq!(main.links[0].wish_id, first.id);
    assert_eq!(main.links[0].api_id.as_str(), "1712345678000000001");
    assert!(main.characters.is_empty() && main.plan.is_empty());

    assert_eq!(
        (second.label.as_str(), second.uid.as_ref()),
        ("Account 2", None)
    );
    assert_eq!(second.server, Some(Server::Asia));
    assert_eq!(second.wishes.len(), 3);
    assert_eq!(
        second.manual_counters,
        [ManualCounters {
            pool: Pool::Standard,
            total: 80,
            pity5: 3,
            pity4: 5,
        }]
    );

    // Imported, every record is kept as it was.
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    let report = store.apply_import(snapshot.clone().into_batch()).unwrap();
    assert_eq!(report.added(), 21);
    let back = Snapshot::from_store(&store).unwrap();
    for (read, kept) in snapshot.accounts.iter().zip(&back.accounts) {
        assert_eq!(read.wishes, kept.wishes);
        assert_eq!(read.links, kept.links);
        assert_eq!(read.manual_counters, kept.manual_counters);
        assert_eq!(
            (&read.label, &read.uid, read.server),
            (&kept.label, &kept.uid, kept.server)
        );
    }
}

#[test]
fn the_version_2_witness_still_reads() {
    let snapshot = Snapshot::from_json(SNAPSHOT_V2.as_bytes()).unwrap();
    assert_eq!(snapshot.version, 2);
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    let report = store.apply_import(snapshot.into_batch()).unwrap();
    assert_eq!(report.added(), 21);
    let accounts = store.accounts().unwrap();
    let (main, second) = (accounts[0].id, accounts[1].id);
    let level = |level, phase| Level::new(level, phase).unwrap();

    let owned = store.collection(main).unwrap();
    assert_eq!(
        owned.character("ganyu"),
        Some(&OwnedCharacter {
            key: "ganyu".to_owned(),
            level: level(70, 4),
            constellation: 1,
            talents: [6, 8, 8],
            favourite: true,
        })
    );
    assert_eq!(owned.character("bennett").unwrap().constellation, 6);
    assert_eq!(owned.character("travelergeo").unwrap().level, level(1, 0));
    let copies: Vec<OwnedWeapon> = (owned.weapons.iter())
        .map(|weapon| OwnedWeapon {
            id: 0,
            ..weapon.clone()
        })
        .collect();
    assert_eq!(
        copies,
        [
            OwnedWeapon {
                id: 0,
                key: "amosbow".to_owned(),
                level: level(70, 4),
                refinement: 1,
                holder: Some("ganyu".to_owned()),
                favourite: true,
            },
            OwnedWeapon {
                id: 0,
                key: "favoniussword".to_owned(),
                level: level(90, 6),
                refinement: 5,
                holder: Some("bennett".to_owned()),
                favourite: false,
            },
            OwnedWeapon::new("debateclub"),
        ]
    );

    let plan = store.plan(main).unwrap();
    let kinds: Vec<&str> = (plan.iter())
        .map(|entry| match &entry.target {
            PlanTarget::Character { .. } => "character",
            PlanTarget::Weapon { .. } => "weapon",
            PlanTarget::CustomCharacter { .. } => "custom character",
            PlanTarget::CustomWeapon { .. } => "custom weapon",
            PlanTarget::Custom { .. } => "custom",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "character",
            "weapon",
            "custom character",
            "custom weapon",
            "custom"
        ]
    );
    let goal = plan[0].target.goal().unwrap();
    assert_eq!((goal.from, goal.to), (level(70, 4), Level::MAX));
    assert_eq!(goal.talents, [(6, 9), (8, 10), (8, 10)]);
    let PlanTarget::CustomCharacter {
        name,
        stars,
        materials,
        goal,
    } = &plan[2].target
    else {
        unreachable!()
    };
    assert_eq!((name.as_str(), *stars), ("New character", 5));
    assert_eq!((goal.from, goal.to), (level(20, 1), level(80, 6)));
    let materials: Vec<&CustomItem> = materials.values().collect();
    assert!(materials.contains(&&CustomItem::Material(9_000_300)));
    assert!(materials.contains(&&CustomItem::Named("Moonfall Lily".to_owned())));
    let PlanTarget::Custom { costs, .. } = &plan[4].target else {
        unreachable!()
    };
    assert_eq!(costs[0].item, CustomItem::Material(MORA));
    assert_eq!(costs[1].count, 20);

    let inventory = store.inventory(main).unwrap();
    assert_eq!(inventory.owned(&CustomItem::Material(MORA)), 1_234_567);
    assert_eq!(inventory.owned(&CustomItem::Material(9_000_500)), 5);
    let lily = CustomItem::Named("Moonfall Lily".to_owned());
    assert_eq!(inventory.owned(&lily), 12);

    assert!(
        store
            .collection(second)
            .unwrap()
            .character("xiangling")
            .is_some()
    );
    assert_eq!(store.plan(second).unwrap().len(), 1);
    let starshroom = CustomItem::Named("Starshroom".to_owned());
    assert_eq!(store.inventory(second).unwrap().owned(&starshroom), 40);
}

#[test]
fn the_version_3_witness_still_reads() {
    use wish_core::chronicle::{Chronicle, RecordKind};

    let snapshot = Snapshot::from_json(SNAPSHOT_V3.as_bytes()).unwrap();
    assert_eq!(snapshot.version, 3);
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    let report = store.apply_import(snapshot.into_batch()).unwrap();
    assert_eq!(report.accounts[0].extras.records, 3);
    let main = store.accounts().unwrap()[0].id;
    let records = store.chronicle(main).unwrap();
    let kinds: Vec<(RecordKind, u32, bool)> = records
        .iter()
        .map(|r| (r.kind, r.period, r.ended))
        .collect();
    assert_eq!(
        kinds,
        [
            (RecordKind::Abyss, 102, true),
            (RecordKind::Diary, 202511, true),
            (RecordKind::Theater, 61, true)
        ]
    );
    // Every field the app reads still reads.
    let chronicle = Chronicle::read(&records);
    assert_eq!(chronicle.unreadable, 0);
    let abyss = &chronicle.abyss[0].abyss;
    assert_eq!(
        (abyss.total_star, abyss.floors[0].levels[0].battles.len()),
        (35, 2)
    );
    let theater = &chronicle.theater[0].theater;
    assert_eq!((theater.stat.max_round_id, theater.stat.medal_num), (8, 6));
    let diary = &chronicle.diary[0];
    assert_eq!(
        (diary.year, diary.month, diary.primogems.len()),
        (2025, 11, 2)
    );
}

/// The writer checked against the witness of the current version. When the
/// format moves to a new version, this test moves to its witness; the older
/// witness keeps its reading test.
#[test]
fn the_snapshot_writer_still_writes_the_version_3_witness() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_store(dir.path());
    let snapshot = Snapshot::from_json(SNAPSHOT_V3.as_bytes()).unwrap();
    store.apply_import(snapshot.into_batch()).unwrap();
    let written = Snapshot::from_store(&store).unwrap().to_json();
    assert_eq!(content(&written), content(SNAPSHOT_V3.as_bytes()));
}

#[test]
fn the_settings_of_0_6_still_read_and_write_alike() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::at(dir.path());
    std::fs::write(paths.settings(), SETTINGS_V0_6).unwrap();
    let settings = Settings::load(&paths).unwrap();
    assert_eq!(
        settings.mirror_folder.as_deref(),
        Some(std::path::Path::new("/home/player/Sync/genshin"))
    );
    assert!(settings.game_folder.is_some() && settings.manual_counting);
    assert_eq!(
        (settings.resin.world_level, settings.resin.resin_per_day),
        (6, 200)
    );
    assert_eq!(settings.resin.ignored.len(), 2);

    let again = tempfile::tempdir().unwrap();
    let paths = AppPaths::at(again.path());
    settings.save(&paths).unwrap();
    let written: Value = serde_json::from_slice(&std::fs::read(paths.settings()).unwrap()).unwrap();
    let witness: Value = serde_json::from_str(SETTINGS_V0_6).unwrap();
    assert_eq!(written, witness);
}
