//! The chronicle: what a HoYoLAB sync asks for and keeps (the Spiral Abyss,
//! the Imaginarium Theater, the Traveler's Diary), how a period fetched
//! again is kept or replaced, and how it travels in a snapshot. HoYoLAB is a
//! fake: tests never touch the network.

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use chrono::{DateTime, Utc};
use hoyolab::{Request, Transport};
use serde_json::{Value, json};
use wish_core::archive::ArchiveData;
use wish_core::chronicle::{Chronicle, RecordCounts, RecordKind};
use wish_core::error::Error;
use wish_core::planner::PlannerData;
use wish_core::service::hoyolab::{Fetched, Wanted, fetch, preview};
use wish_core::snapshot::Snapshot;
use wish_core::{AppPaths, Store};

/// HoYoLAB as the fake shows it: how many battles the Abyss under way has
/// had, and the addresses asked.
struct FakeHoyolab {
    battles: u32,
    asked: Mutex<Vec<String>>,
}

impl FakeHoyolab {
    fn new(battles: u32) -> Self {
        Self {
            battles,
            asked: Mutex::new(Vec::new()),
        }
    }
}

fn ok(data: Value) -> String {
    json!({ "retcode": 0, "message": "OK", "data": data }).to_string()
}

/// The fake's answers, by the address asked: one account, no character, an
/// Abyss period under way (102, ending 2025-12-16) and the one before (101,
/// not fought), a Theater period under way and one over, and the Diary of
/// October to December 2025, whose November log takes two pages.
impl Transport for FakeHoyolab {
    fn send(&self, request: &Request) -> hoyolab::Result<String> {
        let url = request.url.as_str();
        self.asked.lock().unwrap().push(url.to_owned());
        let param = |name: &str| {
            url::Url::parse(url)
                .unwrap()
                .query_pairs()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.into_owned())
        };
        let answer = if url.contains("getUserGameRolesByCookie") {
            ok(
                json!({ "list": [{ "game_biz": "hk4e_global", "region": "os_euro",
                "game_uid": "712345678", "nickname": "Lumine", "level": 60 }] }),
            )
        } else if url.contains("character/") {
            ok(json!({ "list": [] }))
        } else if url.contains("spiralAbyss") {
            match param("schedule_type").as_deref() {
                Some("1") => ok(json!({ "schedule_id": 102, "start_time": "1763258400",
                    "end_time": "1765850399", "total_battle_times": self.battles,
                    "total_star": 30, "max_floor": "12-1", "is_unlock": true })),
                _ => ok(json!({ "schedule_id": 101, "start_time": "1760580000",
                    "end_time": "1763171999", "total_battle_times": 0, "is_unlock": true })),
            }
        } else if url.contains("role_combat") {
            ok(json!({ "is_unlock": true, "data": [
                { "schedule": { "schedule_id": 62, "end_time": "1767196799" }, "has_data": true,
                  "stat": { "max_round_id": 8 } },
                { "schedule": { "schedule_id": 61, "end_time": "1764547199" }, "has_data": true,
                  "stat": { "max_round_id": 10 } },
            ]}))
        } else if url.contains("month_info") {
            let month: u32 = param("month").map_or(12, |m| m.parse().unwrap());
            ok(json!({ "data_month": month, "optional_month": [10, 11, 12],
                "month_data": { "current_primogems": month * 100, "group_by": [] } }))
        } else if url.contains("month_detail") {
            let (month, page) = (param("month").unwrap(), param("current_page").unwrap());
            let entries = match (month.as_str(), page.as_str()) {
                ("11", "1") => 100,
                ("11", _) => 3,
                _ => 1,
            };
            let entry = json!({ "action_id": 1, "action": "Daily Commission Rewards",
                "time": format!("2025-{month}-02 05:00:00"), "num": 60 });
            ok(json!({ "data_month": month.parse::<u32>().unwrap(),
                "list": vec![entry; entries] }))
        } else {
            return Err(hoyolab::Error::Network(format!("no answer for {url}")));
        };
        Ok(answer)
    }
}

fn at(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

fn cookie() -> hoyolab::Cookie {
    hoyolab::Cookie::parse("ltuid_v2=123; ltoken_v2=v2_token; ltmid_v2=mid").unwrap()
}

fn fetched(fake: &FakeHoyolab, fetched_at: DateTime<Utc>) -> Fetched {
    let cancelled = AtomicBool::new(false);
    let mut all = fetch(
        cookie(),
        fake,
        Duration::ZERO,
        fetched_at,
        Wanted::Everything,
        &cancelled,
    )
    .unwrap();
    assert_eq!(all.len(), 1);
    all.remove(0)
}

fn open(dir: &std::path::Path) -> Store {
    Store::open(AppPaths::at(dir)).unwrap()
}

/// Syncs `fetched` into the account with its UID, or a new one.
fn sync(store: &mut Store, fetched: Fetched) -> wish_core::store::HoyolabSynced {
    let (planner, archive) = (PlannerData::embedded(), ArchiveData::embedded());
    let preview = preview(fetched, store, None, planner, archive).unwrap();
    let target = preview.choices[preview.suggested]
        .target
        .as_ref()
        .map(|a| a.id);
    let synced = store
        .apply_hoyolab(
            target,
            &preview.account,
            &preview.details,
            &preview.records,
            planner,
            archive,
        )
        .unwrap();
    assert_eq!(synced.records, preview.choices[preview.suggested].records);
    synced
}

#[test]
fn a_sync_asks_for_what_hoyolab_still_shows_and_keeps_it_whole() {
    let fake = FakeHoyolab::new(10);
    let fetched = fetched(&fake, at("2025-12-10T12:00:00Z"));
    let kinds: Vec<(RecordKind, u32, bool)> = (fetched.records.iter())
        .map(|r| (r.kind, r.period, r.ended))
        .collect();
    assert_eq!(
        kinds,
        [
            // The Abyss under way; the one before wasn't fought.
            (RecordKind::Abyss, 102, false),
            (RecordKind::Theater, 62, false),
            (RecordKind::Theater, 61, true),
            (RecordKind::Diary, 202510, true),
            (RecordKind::Diary, 202511, true),
            (RecordKind::Diary, 202512, false),
        ]
    );
    // November's log came in two pages, and is kept whole.
    let november = &fetched.records[4];
    assert_eq!(november.data["primogems"].as_array().unwrap().len(), 2);
    let asked = fake.asked.lock().unwrap();
    let pages = asked
        .iter()
        .filter(|url| url.contains("month_detail"))
        .count();
    assert_eq!(pages, 4);
    // The month under way is asked for once.
    let months = asked
        .iter()
        .filter(|url| url.contains("month_info"))
        .count();
    assert_eq!(months, 3);

    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let synced = sync(&mut store, fetched.clone());
    assert_eq!(
        synced.records.abyss,
        RecordCounts {
            added: 1,
            ..RecordCounts::default()
        }
    );
    assert_eq!(synced.records.diary.added, 3);
    let stored = store.chronicle(synced.account).unwrap();
    assert_eq!(stored.len(), 6);
    let abyss = stored.iter().find(|r| r.kind == RecordKind::Abyss).unwrap();
    assert_eq!(abyss.data, fetched.records[0].data, "as HoYoLAB sent it");

    let chronicle = Chronicle::read(&stored);
    assert_eq!(chronicle.unreadable, 0);
    assert_eq!(chronicle.diary[0].month, 12);
    assert_eq!(chronicle.diary[1].primogems.len(), 103);
    assert_eq!(chronicle.theater[0].theater.stat.max_round_id, 8);
}

#[test]
fn a_period_under_way_is_brought_up_to_date_until_it_is_over() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let first = sync(
        &mut store,
        fetched(&FakeHoyolab::new(10), at("2025-12-10T12:00:00Z")),
    );
    let account = first.account;
    let battles = |store: &Store| {
        let records = store.chronicle(account).unwrap();
        Chronicle::read(&records).abyss[0].abyss.total_battle_times
    };

    // More battles since: the period under way is replaced; what was over
    // stays as it was.
    let again = sync(
        &mut store,
        fetched(&FakeHoyolab::new(14), at("2025-12-12T12:00:00Z")),
    );
    assert_eq!(again.records.abyss.updated, 1);
    assert_eq!(
        again.records.theater,
        RecordCounts {
            added: 0,
            updated: 0,
            kept: 2
        }
    );
    assert_eq!(battles(&store), 14);

    // Fetched once over, the period is final: a later answer changes nothing.
    let over = sync(
        &mut store,
        fetched(&FakeHoyolab::new(15), at("2025-12-17T12:00:00Z")),
    );
    assert_eq!(over.records.abyss.updated, 1);
    let late = sync(
        &mut store,
        fetched(&FakeHoyolab::new(3), at("2025-12-18T12:00:00Z")),
    );
    assert_eq!(
        late.records.abyss,
        RecordCounts {
            added: 0,
            updated: 0,
            kept: 1
        }
    );
    assert_eq!(battles(&store), 15);

    // What HoYoLAB no longer shows is what the app keeps: the account stays.
    assert!(matches!(
        store.remove_account(account),
        Err(Error::AccountHasWishes(_))
    ));
}

#[test]
fn the_chronicle_travels_in_a_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut original = open(&dir.path().join("a"));
    let synced = sync(
        &mut original,
        fetched(&FakeHoyolab::new(10), at("2025-12-10T12:00:00Z")),
    );
    let records = original.chronicle(synced.account).unwrap();

    let exported = Snapshot::from_store(&original).unwrap();
    assert_eq!(exported.version, 3);
    let read_back = Snapshot::from_json(&exported.to_json()).unwrap();
    assert_eq!(read_back, exported);

    let mut restored = open(&dir.path().join("b"));
    let report = restored
        .apply_import(read_back.clone().into_batch())
        .unwrap();
    assert_eq!(report.accounts[0].extras.records, 6);
    let account = report.accounts[0].account.id;
    assert_eq!(restored.chronicle(account).unwrap(), records);

    // The same snapshot twice changes nothing.
    let twice = restored.apply_import(read_back.into_batch()).unwrap();
    assert_eq!(twice.accounts[0].extras.records, 0);
}

#[test]
fn the_chronicle_alone_goes_into_the_account_with_its_uid_only() {
    use wish_core::service::hoyolab::keep_chronicle;

    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let fake = FakeHoyolab::new(10);
    let alone = |fake: &FakeHoyolab, at_: &str| {
        let cancelled = AtomicBool::new(false);
        fetch(
            cookie(),
            fake,
            Duration::ZERO,
            at(at_),
            Wanted::Chronicle,
            &cancelled,
        )
        .unwrap()
    };

    // No account has the UID yet: nothing is written, and no account made.
    let sync = keep_chronicle(&mut store, alone(&fake, "2025-12-10T12:00:00Z")).unwrap();
    assert_eq!(
        (sync.uids.clone(), sync.unlinked),
        (vec!["712345678".to_owned()], sync.uids)
    );
    assert!(store.accounts().unwrap().is_empty());
    // The characters aren't asked for.
    assert!(
        !fake
            .asked
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.contains("character/"))
    );

    let main = store
        .add_account("Main", Some("712345678"), None)
        .unwrap()
        .id;
    let before = store.backups().unwrap().len();
    let sync = keep_chronicle(&mut store, alone(&fake, "2025-12-10T12:00:00Z")).unwrap();
    assert_eq!(
        sync.kept.abyss.added + sync.kept.theater.added + sync.kept.diary.added,
        6
    );
    assert_eq!(store.chronicle(main).unwrap().len(), 6);
    // One backup before, kept apart from the imports' ones.
    let backups = store.backups().unwrap();
    assert_eq!(backups.len(), before + 1);
    assert_eq!(
        backups[0].reason,
        Some(wish_core::store::BackupReason::BeforeChronicle)
    );

    // Nothing new: nothing written, no backup.
    let again = keep_chronicle(&mut store, alone(&fake, "2025-12-10T12:00:00Z")).unwrap();
    assert_eq!(again.kept.changes(), 0);
    assert_eq!(store.backups().unwrap().len(), before + 1);
}
