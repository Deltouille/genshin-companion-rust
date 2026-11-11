//! Item and banner data extracted from paimon.moe by `tools/extract-paimon-data.mjs`.
//!
//! The JSON is embedded in the binary. A copy in the app's game-data folder
//! takes precedence, so a new banner does not require a rebuild.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{ItemType, Pool, Rarity, WishTime};

pub const ITEMS_FILE: &str = "items.json";
pub const BANNERS_FILE: &str = "banners.json";

const EMBEDDED_ITEMS: &str = include_str!("../data/items.json");
const EMBEDDED_BANNERS: &str = include_str!("../data/banners.json");

/// The paimon.moe commit a data file was extracted from.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DataSource {
    pub repo: String,
    pub commit: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// paimon.moe's key, such as `kaedehara_kazuha`.
    pub key: String,
    /// English name, as the API returns it with `lang=en-us`.
    pub name: String,
    pub rarity: Rarity,
    pub item_type: ItemType,
    /// HoYoverse's id for the item (from UIGF's dictionary), used by UIGF.
    pub item_id: Option<u32>,
}

/// Characters and weapons that can come out of a wish.
#[derive(Clone, Debug)]
pub struct Items {
    pub source: DataSource,
    list: Vec<Item>,
    by_key: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
    by_item_id: HashMap<u32, usize>,
}

impl Items {
    pub fn by_key(&self, key: &str) -> Option<&Item> {
        self.by_key.get(key).map(|&i| &self.list[i])
    }

    pub fn by_name(&self, name: &str) -> Option<&Item> {
        self.by_name.get(name).map(|&i| &self.list[i])
    }

    pub fn by_item_id(&self, item_id: u32) -> Option<&Item> {
        self.by_item_id.get(&item_id).map(|&i| &self.list[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            source: DataSource,
            characters: Vec<Entry>,
            weapons: Vec<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            key: String,
            name: String,
            rarity: Rarity,
            #[serde(default)]
            item_id: Option<u32>,
        }

        let file: File = serde_json::from_str(json)?;
        let characters = file
            .characters
            .into_iter()
            .map(|e| (e, ItemType::Character));
        let weapons = file.weapons.into_iter().map(|e| (e, ItemType::Weapon));

        let mut items = Items {
            source: file.source,
            list: Vec::new(),
            by_key: HashMap::new(),
            by_name: HashMap::new(),
            by_item_id: HashMap::new(),
        };
        for (entry, item_type) in characters.chain(weapons) {
            let index = items.list.len();
            if items.by_key.insert(entry.key.clone(), index).is_some() {
                return Err(Error::GameData(format!(
                    "duplicate item key `{}`",
                    entry.key
                )));
            }
            if items.by_name.insert(entry.name.clone(), index).is_some() {
                return Err(Error::GameData(format!(
                    "duplicate item name `{}`",
                    entry.name
                )));
            }
            if let Some(item_id) = entry.item_id {
                items.by_item_id.insert(item_id, index);
            }
            items.list.push(Item {
                key: entry.key,
                name: entry.name,
                rarity: entry.rarity,
                item_type,
                item_id: entry.item_id,
            });
        }
        Ok(items)
    }
}

/// A banner, with times as written in paimon.moe's data (UTC+8 wall clock).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Banner {
    pub name: String,
    /// Which rerun this is; part of the banner id and image file name.
    pub image: u32,
    #[serde(default)]
    pub short_name: String,
    pub start: WishTime,
    pub end: WishTime,
    #[serde(default)]
    pub color: String,
    /// The start is one global instant (version update) rather than the same
    /// wall-clock time on every server.
    pub timezone_dependent: bool,
    /// Same as `timezone_dependent`, for the end.
    pub timezone_dependent_end: bool,
    /// Item keys of the rate-up 5★ items.
    pub featured: Vec<String>,
    /// Item keys of the rate-up 4★ items.
    pub featured_rare: Vec<String>,
    #[serde(default)]
    pub version: Option<String>,
}

impl Banner {
    /// paimon.moe's banner id, also the name of its image file.
    pub fn id(&self) -> String {
        format!("{} {}", self.name, self.image)
    }

    /// The id as a file name stem: lowercase ASCII letters and digits joined
    /// by `-` ("Reign of Serenity 1" → `reign-of-serenity-1`). It can never be
    /// mistaken for a URL, unlike a name with a `:` in it.
    pub fn slug(&self) -> String {
        let mut slug = String::new();
        for word in self
            .id()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
        {
            if !slug.is_empty() {
                slug.push('-');
            }
            slug.push_str(&word.to_ascii_lowercase());
        }
        slug
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Banners {
    pub source: DataSource,
    pub beginners: Vec<Banner>,
    pub standard: Vec<Banner>,
    pub character: Vec<Banner>,
    pub weapon: Vec<Banner>,
    pub chronicled: Vec<Banner>,
    /// Character banners that ran side by side, keyed by the id of the one
    /// listed in `character`.
    pub dual: BTreeMap<String, [Banner; 2]>,
}

impl Banners {
    pub fn for_pool(&self, pool: Pool) -> &[Banner] {
        match pool {
            Pool::Beginners => &self.beginners,
            Pool::Standard => &self.standard,
            Pool::CharacterEvent => &self.character,
            Pool::WeaponEvent => &self.weapon,
            Pool::Chronicled => &self.chronicled,
        }
    }

    /// When the last banner ends: how recent the data is.
    fn last_end(&self) -> Option<WishTime> {
        let dual = self.dual.values().flatten();
        let all = Pool::ALL.iter().flat_map(|&pool| self.for_pool(pool));
        all.chain(dual).map(|banner| banner.end).max()
    }

    fn from_json(json: &str) -> Result<Self> {
        let banners: Banners = serde_json::from_str(json)?;
        for pool in Pool::ALL {
            for banner in banners.for_pool(*pool) {
                if banner.end <= banner.start {
                    return Err(Error::GameData(format!(
                        "banner `{}` ends before it starts",
                        banner.id()
                    )));
                }
            }
        }
        Ok(banners)
    }
}

#[derive(Clone, Debug)]
pub struct GameData {
    pub items: Items,
    pub banners: Banners,
}

static EMBEDDED: LazyLock<GameData> = LazyLock::new(|| {
    GameData::from_json(EMBEDDED_ITEMS, EMBEDDED_BANNERS)
        .expect("embedded game data is validated by the extraction script and tests")
});

impl GameData {
    /// The data compiled into this build.
    pub fn embedded() -> &'static GameData {
        &EMBEDDED
    }

    pub fn from_json(items: &str, banners: &str) -> Result<Self> {
        let data = Self {
            items: Items::from_json(items)?,
            banners: Banners::from_json(banners)?,
        };
        data.check_featured_items()?;
        Ok(data)
    }

    /// Loads `items.json` and `banners.json` from `dir`, each one present in
    /// place of the embedded copy, and says what was wrong with them.
    ///
    /// The wishes must always open, and wrong data would be worse than old
    /// data (unknown items, wishes outside any banner, wrong 50/50 results):
    /// a copy that can't be read, or older than the embedded one (an item
    /// missing, banners ending earlier), is left out, and a pair that doesn't
    /// fit together gives way to the embedded data. Each is named in the
    /// returned warnings.
    pub fn load(dir: &Path) -> (Self, Vec<String>) {
        let embedded = Self::embedded();
        let mut warnings = Vec::new();
        let mut read = |file: &str| -> Option<String> {
            let path = dir.join(file);
            match fs::read_to_string(&path) {
                Ok(json) => Some(json),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => {
                    warnings.push(format!("{file}: {}", Error::io("reading", &path)(e)));
                    None
                }
            }
        };
        let (items, banners) = (read(ITEMS_FILE), read(BANNERS_FILE));

        let items = match items.map(|json| Items::from_json(&json)) {
            None => embedded.items.clone(),
            Some(Err(e)) => {
                warnings.push(format!("{ITEMS_FILE}: {e}; the built-in one is used"));
                embedded.items.clone()
            }
            Some(Ok(items)) => match embedded
                .items
                .iter()
                .find(|i| items.by_key(&i.key).is_none())
            {
                Some(missing) => {
                    warnings.push(format!(
                        "{ITEMS_FILE} is older than the built-in one (it lacks {}); it is ignored",
                        missing.name
                    ));
                    embedded.items.clone()
                }
                None => items,
            },
        };
        let banners = match banners.map(|json| Banners::from_json(&json)) {
            None => embedded.banners.clone(),
            Some(Err(e)) => {
                warnings.push(format!("{BANNERS_FILE}: {e}; the built-in one is used"));
                embedded.banners.clone()
            }
            Some(Ok(banners)) if banners.last_end() < embedded.banners.last_end() => {
                warnings.push(format!(
                    "{BANNERS_FILE} is older than the built-in one (its banners end earlier); \
                     it is ignored"
                ));
                embedded.banners.clone()
            }
            Some(Ok(banners)) => banners,
        };

        let data = Self { items, banners };
        match data.check_featured_items() {
            Ok(()) => (data, warnings),
            Err(e) => {
                warnings.push(format!(
                    "the game-data files don't fit together ({e}); the built-in data is used"
                ));
                (embedded.clone(), warnings)
            }
        }
    }

    /// Every rate-up key must name a known item, or 50/50 results would be wrong.
    fn check_featured_items(&self) -> Result<()> {
        let dual = self.banners.dual.values().flatten();
        let all = Pool::ALL
            .iter()
            .flat_map(|&pool| self.banners.for_pool(pool))
            .chain(dual);
        for banner in all {
            for key in banner.featured.iter().chain(&banner.featured_rare) {
                if self.items.by_key(key).is_none() {
                    return Err(Error::GameData(format!(
                        "banner `{}` features unknown item `{key}`",
                        banner.id()
                    )));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_data_loads() {
        let data = GameData::embedded();
        assert!(data.items.len() > 300);
        assert!(data.banners.character.len() > 100);
        assert_eq!(data.items.source.commit, data.banners.source.commit);
    }

    #[test]
    fn items_resolve_by_key_and_by_api_name() {
        let items = &GameData::embedded().items;
        let kazuha = items.by_key("kaedehara_kazuha").unwrap();
        assert_eq!(kazuha.name, "Kaedehara Kazuha");
        assert_eq!(kazuha.rarity, Rarity::Five);
        assert_eq!(kazuha.item_type, ItemType::Character);
        assert_eq!(
            items.by_name("Wolf's Gravestone").unwrap().key,
            "wolfs_gravestone"
        );
        assert_eq!(items.by_name("Slingshot").unwrap().rarity, Rarity::Three);
    }

    #[test]
    fn banners_keep_paimon_fields() {
        let banners = &GameData::embedded().banners;
        let kazuha = banners
            .character
            .iter()
            .find(|b| b.id() == "Leaves in the Wind 1")
            .unwrap();
        assert_eq!(kazuha.start.to_string(), "2021-06-29 18:00:00");
        assert_eq!(kazuha.featured, ["kaedehara_kazuha"]);
        assert!(!kazuha.timezone_dependent);
    }

    #[test]
    fn override_dir_replaces_embedded_files() {
        let dir = tempfile::tempdir().unwrap();
        let items = EMBEDDED_ITEMS.replace("\"Kaedehara Kazuha\"", "\"Kazuha (override)\"");
        fs::write(dir.path().join(ITEMS_FILE), items).unwrap();

        let (data, warnings) = GameData::load(dir.path());
        assert!(data.items.by_name("Kazuha (override)").is_some());
        assert!(warnings.is_empty(), "{warnings:?}");
        // banners.json was not overridden, so the embedded one is used.
        assert_eq!(
            data.banners.character.len(),
            GameData::embedded().banners.character.len()
        );
    }

    #[test]
    fn a_damaged_copy_gives_way_to_the_embedded_data() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(BANNERS_FILE), "{ not json").unwrap();
        let (data, warnings) = GameData::load(dir.path());
        assert_eq!(
            data.banners.character.len(),
            GameData::embedded().banners.character.len()
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with(BANNERS_FILE), "{warnings:?}");
    }

    #[test]
    fn an_older_copy_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        // The embedded banners, less the last character banner.
        let mut banners: serde_json::Value = serde_json::from_str(EMBEDDED_BANNERS).unwrap();
        let latest = (banners["character"].as_array().unwrap().iter())
            .map(|b| b["end"].as_str().unwrap().to_owned())
            .max()
            .unwrap();
        for pool in ["beginners", "standard", "character", "weapon", "chronicled"] {
            (banners[pool].as_array_mut().unwrap()).retain(|b| b["end"].as_str() < Some(&latest));
        }
        banners["dual"] = serde_json::json!({});
        fs::write(dir.path().join(BANNERS_FILE), banners.to_string()).unwrap();
        // The embedded items, less one.
        let items = EMBEDDED_ITEMS.replace("\"kaedehara_kazuha\"", "\"someone_else\"");
        fs::write(dir.path().join(ITEMS_FILE), items).unwrap();

        let (data, warnings) = GameData::load(dir.path());
        assert!(data.items.by_key("kaedehara_kazuha").is_some());
        assert_eq!(
            data.banners.character.len(),
            GameData::embedded().banners.character.len()
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().all(|w| w.contains("older")), "{warnings:?}");
    }

    #[test]
    fn unknown_featured_item_is_rejected() {
        let banners = EMBEDDED_BANNERS.replacen("\"kaedehara_kazuha\"", "\"nobody\"", 1);
        let err = GameData::from_json(EMBEDDED_ITEMS, &banners).unwrap_err();
        assert!(err.to_string().contains("unknown item `nobody`"), "{err}");
    }
}
