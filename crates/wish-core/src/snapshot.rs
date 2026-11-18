//! Lossless JSON snapshot of every account, for backups outside the app's
//! folder (the mirror folder) and for moving data between machines: its
//! wishes, and since version 2 its characters, weapons, plan and inventory.
//! Version 1 files still read (without those).
//!
//! UIGF is not used here: it requires numeric API ids and HoYoverse item ids,
//! which migrated paimon.moe wishes do not have. A snapshot keeps every field
//! of every record, so importing one gives back exactly what was exported.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{Local, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::collection::{MAX_TALENT, OwnedCharacter, OwnedWeapon};
use crate::error::{Error, Result};
use crate::import::{AccountBatch, AccountExtras, ImportBatch, ImportSource, WishLink};
use crate::model::{AccountId, ManualCounters, Server, Wish};
use crate::paths::write_atomic;
use crate::planner::{
    CharacterGoal, CustomCost, CustomItem, CustomMaterials, Inventory, Level, MaterialId,
    PlanTarget, Role, WeaponGoal,
};
use crate::store::Store;

/// The `format` field every snapshot carries. Named after the app's former
/// name (Genshin Wishes), kept so that every snapshot written so far still
/// reads, and the mirror file stays the same.
pub const FORMAT: &str = "genshin-wishes-snapshot";
/// The version this app writes. Version 1 (wishes only) still reads; a newer
/// one is refused ([`Error::SnapshotTooNew`]).
pub const VERSION: u32 = 2;

/// Name of the snapshot kept in the mirror folder. One fixed name, so a
/// syncing tool replaces the file instead of piling up copies.
pub const MIRROR_FILE: &str = "genshin-wishes-snapshot.json";

/// Writes a snapshot of the whole store to `folder`, atomically.
pub fn write_mirror(store: &Store, folder: &Path) -> Result<PathBuf> {
    let path = folder.join(MIRROR_FILE);
    Snapshot::from_store(store)?.write_to(&path)?;
    Ok(path)
}

/// The app's own lossless export, and the mirror's file: every account with its
/// wishes, manual counters, game ids, collection, plan and inventory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub version: u32,
    /// Local time of the export, RFC 3339.
    pub exported_at: String,
    pub app_version: String,
    pub accounts: Vec<SnapshotAccount>,
}

/// One account of a snapshot. The collection, plan and inventory are records of
/// the format's own, so a change of the app's types never changes the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotAccount {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<Server>,
    pub wishes: Vec<Wish>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manual_counters: Vec<ManualCounters>,
    /// Game ids learned for migrated wishes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<WishLink>,
    /// Since version 2: what the account owns, its plan and its inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<SnapshotCharacter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapons: Vec<SnapshotWeapon>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plan: Vec<SnapshotPlanEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inventory: Vec<SnapshotStock>,
}

impl Snapshot {
    /// Everything in the store, accounts in creation order and wishes in
    /// chronological order.
    pub fn from_store(store: &Store) -> Result<Self> {
        let accounts = store
            .accounts()?
            .into_iter()
            .map(|account| {
                let extras = extras_of(store, account.id)?;
                Ok(SnapshotAccount {
                    wishes: store.wishes(account.id)?,
                    manual_counters: store.manual_counters(account.id)?,
                    links: store.links(account.id)?,
                    label: account.label,
                    uid: account.uid,
                    server: account.server,
                    characters: extras.characters,
                    weapons: extras.weapons,
                    plan: extras.plan,
                    inventory: extras.inventory,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            exported_at: Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            accounts,
        })
    }

    /// Reads a snapshot: its header first (another format, or a newer version,
    /// is refused with a clear message), then the whole file.
    pub fn from_json(json: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct Header {
            #[serde(default)]
            format: String,
            #[serde(default)]
            version: u32,
        }

        // Check the header first so a newer file gets a clear message
        // rather than a field-level parse error.
        let header: Header = serde_json::from_slice(json)?;
        if header.format != FORMAT {
            return Err(Error::NotSnapshot {
                expected: FORMAT,
                found: header.format,
            });
        }
        if header.version > VERSION {
            return Err(Error::SnapshotTooNew {
                found: header.version,
                known: VERSION,
            });
        }
        Ok(serde_json::from_slice(json)?)
    }

    /// The snapshot as pretty-printed JSON.
    pub fn to_json(&self) -> Vec<u8> {
        let mut json = serde_json::to_vec_pretty(self).expect("snapshot types always serialize");
        json.push(b'\n');
        json
    }

    /// Writes the snapshot atomically: readers never see a partial file.
    pub fn write_to(&self, path: &Path) -> Result<()> {
        write_atomic(path, &self.to_json())
    }

    /// A batch that merges this snapshot into a store.
    pub fn into_batch(self) -> ImportBatch {
        let mut batch = ImportBatch::new(ImportSource::Snapshot);
        batch.accounts = self
            .accounts
            .into_iter()
            .map(|mut account| AccountBatch {
                extras: account.extras(),
                label: account.label,
                uid: account.uid,
                server: account.server,
                wishes: account.wishes,
                manual_counters: account.manual_counters,
                links: account.links,
                merge_into: None,
            })
            .collect();
        batch
    }
}

// What a snapshot keeps of an account besides its wishes (version 2). These
// are the format's own records, not the app's types, so the file stays the
// same when the app's types change.

/// A level and its ascension phase: `[80, 6]` is 80/90.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotLevel(pub u8, pub u8);

impl From<Level> for SnapshotLevel {
    fn from(level: Level) -> Self {
        SnapshotLevel(level.level, level.phase)
    }
}

impl From<SnapshotLevel> for Level {
    /// A level outside its phase (a file edited by hand) is brought back
    /// into it.
    fn from(SnapshotLevel(level, phase): SnapshotLevel) -> Self {
        Level::new(level, phase).unwrap_or_else(|| Level::clamped(level, phase))
    }
}

/// A character the account owns, as the snapshot writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCharacter {
    pub key: String,
    pub level: SnapshotLevel,
    pub constellation: u8,
    /// Normal attack, skill, burst, without the constellations' bonus.
    pub talents: [u8; 3],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub favourite: bool,
}

/// One weapon copy the account owns, as the snapshot writes it; `holder` is the
/// key of the character holding it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotWeapon {
    pub key: String,
    pub level: SnapshotLevel,
    pub refinement: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub favourite: bool,
}

/// A material: one the game data knows, by id, or one it doesn't, by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SnapshotItem {
    Material { id: MaterialId },
    Named { name: String },
}

impl From<&CustomItem> for SnapshotItem {
    fn from(item: &CustomItem) -> Self {
        match item {
            CustomItem::Material(id) => SnapshotItem::Material { id: *id },
            CustomItem::Named(name) => SnapshotItem::Named { name: name.clone() },
        }
    }
}

impl From<SnapshotItem> for CustomItem {
    fn from(item: SnapshotItem) -> Self {
        match item {
            SnapshotItem::Material { id } => CustomItem::Material(id),
            SnapshotItem::Named { name } => CustomItem::Named(name),
        }
    }
}

/// So many of a material: in the inventory, or a custom entry's cost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotStock {
    #[serde(flatten)]
    pub item: SnapshotItem,
    pub count: u64,
}

/// One entry of the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SnapshotPlanEntry {
    Character {
        key: String,
        from: SnapshotLevel,
        to: SnapshotLevel,
        /// Each talent's from and to.
        talents: [(u8, u8); 3],
    },
    Weapon {
        key: String,
        from: SnapshotLevel,
        to: SnapshotLevel,
    },
    CustomCharacter {
        name: String,
        stars: u8,
        /// By role (`talent-book`, `boss-material`…).
        materials: BTreeMap<Role, SnapshotItem>,
        from: SnapshotLevel,
        to: SnapshotLevel,
        talents: [(u8, u8); 3],
    },
    CustomWeapon {
        name: String,
        stars: u8,
        materials: BTreeMap<Role, SnapshotItem>,
        from: SnapshotLevel,
        to: SnapshotLevel,
    },
    Custom {
        name: String,
        costs: Vec<SnapshotStock>,
    },
}

impl From<&PlanTarget> for SnapshotPlanEntry {
    fn from(target: &PlanTarget) -> Self {
        let materials = |materials: &CustomMaterials| {
            (materials.iter())
                .map(|(role, item)| (*role, SnapshotItem::from(item)))
                .collect()
        };
        match target {
            PlanTarget::Character { key, goal } => SnapshotPlanEntry::Character {
                key: key.clone(),
                from: goal.from.into(),
                to: goal.to.into(),
                talents: goal.talents,
            },
            PlanTarget::Weapon { key, goal } => SnapshotPlanEntry::Weapon {
                key: key.clone(),
                from: goal.from.into(),
                to: goal.to.into(),
            },
            PlanTarget::CustomCharacter {
                name,
                stars,
                materials: chosen,
                goal,
            } => SnapshotPlanEntry::CustomCharacter {
                name: name.clone(),
                stars: *stars,
                materials: materials(chosen),
                from: goal.from.into(),
                to: goal.to.into(),
                talents: goal.talents,
            },
            PlanTarget::CustomWeapon {
                name,
                stars,
                materials: chosen,
                goal,
            } => SnapshotPlanEntry::CustomWeapon {
                name: name.clone(),
                stars: *stars,
                materials: materials(chosen),
                from: goal.from.into(),
                to: goal.to.into(),
            },
            PlanTarget::Custom { name, costs } => SnapshotPlanEntry::Custom {
                name: name.clone(),
                costs: (costs.iter())
                    .map(|cost| SnapshotStock {
                        item: (&cost.item).into(),
                        count: u64::from(cost.count),
                    })
                    .collect(),
            },
        }
    }
}

impl From<SnapshotPlanEntry> for PlanTarget {
    fn from(entry: SnapshotPlanEntry) -> Self {
        let materials = |materials: BTreeMap<Role, SnapshotItem>| {
            (materials.into_iter())
                .map(|(role, item)| (role, CustomItem::from(item)))
                .collect()
        };
        let talents = |talents: [(u8, u8); 3]| {
            talents.map(|(from, to)| (from.clamp(1, MAX_TALENT), to.clamp(1, MAX_TALENT)))
        };
        match entry {
            SnapshotPlanEntry::Character {
                key,
                from,
                to,
                talents: t,
            } => PlanTarget::Character {
                key,
                goal: CharacterGoal {
                    from: from.into(),
                    to: to.into(),
                    talents: talents(t),
                },
            },
            SnapshotPlanEntry::Weapon { key, from, to } => PlanTarget::Weapon {
                key,
                goal: WeaponGoal {
                    from: from.into(),
                    to: to.into(),
                },
            },
            SnapshotPlanEntry::CustomCharacter {
                name,
                stars,
                materials: chosen,
                from,
                to,
                talents: t,
            } => PlanTarget::CustomCharacter {
                name,
                stars,
                materials: materials(chosen),
                goal: CharacterGoal {
                    from: from.into(),
                    to: to.into(),
                    talents: talents(t),
                },
            },
            SnapshotPlanEntry::CustomWeapon {
                name,
                stars,
                materials: chosen,
                from,
                to,
            } => PlanTarget::CustomWeapon {
                name,
                stars,
                materials: materials(chosen),
                goal: WeaponGoal {
                    from: from.into(),
                    to: to.into(),
                },
            },
            SnapshotPlanEntry::Custom { name, costs } => PlanTarget::Custom {
                name,
                costs: (costs.into_iter())
                    .map(|stock| CustomCost {
                        item: stock.item.into(),
                        count: u32::try_from(stock.count).unwrap_or(u32::MAX),
                    })
                    .collect(),
            },
        }
    }
}

/// An account's characters, weapons, plan and inventory as a snapshot keeps
/// them.
fn extras_of(store: &Store, account: AccountId) -> Result<SnapshotExtras> {
    let collection = store.collection(account)?;
    let inventory = store.inventory(account)?;
    Ok(SnapshotExtras {
        characters: (collection.characters.iter())
            .map(|c| SnapshotCharacter {
                key: c.key.clone(),
                level: c.level.into(),
                constellation: c.constellation,
                talents: c.talents,
                favourite: c.favourite,
            })
            .collect(),
        weapons: (collection.weapons.iter())
            .map(|w| SnapshotWeapon {
                key: w.key.clone(),
                level: w.level.into(),
                refinement: w.refinement,
                holder: w.holder.clone(),
                favourite: w.favourite,
            })
            .collect(),
        plan: (store.plan(account)?.iter())
            .map(|entry| (&entry.target).into())
            .collect(),
        inventory: (inventory.materials.iter())
            .map(|(id, count)| SnapshotStock {
                item: SnapshotItem::Material { id: *id },
                count: *count,
            })
            .chain(inventory.named.iter().map(|(name, count)| SnapshotStock {
                item: SnapshotItem::Named { name: name.clone() },
                count: *count,
            }))
            .collect(),
    })
}

/// The four lists, together, to read and write them in one go.
struct SnapshotExtras {
    characters: Vec<SnapshotCharacter>,
    weapons: Vec<SnapshotWeapon>,
    plan: Vec<SnapshotPlanEntry>,
    inventory: Vec<SnapshotStock>,
}

impl SnapshotAccount {
    /// What the account carries besides its wishes, as an import merges it.
    fn extras(&mut self) -> AccountExtras {
        let mut inventory = Inventory::default();
        for stock in std::mem::take(&mut self.inventory) {
            inventory.set(&stock.item.into(), stock.count);
        }
        AccountExtras {
            characters: (std::mem::take(&mut self.characters).into_iter())
                .map(|c| OwnedCharacter {
                    key: c.key,
                    level: c.level.into(),
                    constellation: c.constellation,
                    talents: c.talents,
                    favourite: c.favourite,
                })
                .collect(),
            weapons: (std::mem::take(&mut self.weapons).into_iter())
                .map(|w| OwnedWeapon {
                    id: 0,
                    key: w.key,
                    level: w.level.into(),
                    refinement: w.refinement,
                    holder: w.holder,
                    favourite: w.favourite,
                })
                .collect(),
            plan: (std::mem::take(&mut self.plan).into_iter())
                .map(PlanTarget::from)
                .collect(),
            inventory,
        }
    }
}
