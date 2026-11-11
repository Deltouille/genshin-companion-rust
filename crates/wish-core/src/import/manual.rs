//! Wishes counted by hand, like paimon.moe's manual mode: `+1` and `+10`
//! add 3★ wishes without an item, and a 4★ or 5★ is added with its item.
//!
//! Each entry is an import of its own (`ImportSource::Manual`), so it shows in
//! the import log and can be undone like any import.
//!
//! Without an account (counting from scratch), the wishes go to the account
//! labelled [`NEW_ACCOUNT_LABEL`], which the first entry creates.

use chrono::Utc;

use super::{AccountBatch, ImportBatch, ImportSource};
use crate::data::Item;
use crate::model::{Account, GachaType, ItemType, Pool, Rarity, Source, Wish, WishId, WishTime};

/// The label of the account that counting by hand creates when there is none.
pub const NEW_ACCOUNT_LABEL: &str = "Main";

/// `count` 3★ wishes without an item, made now.
pub fn three_stars(
    account: Option<&Account>,
    pool: Pool,
    count: u32,
    time: WishTime,
) -> ImportBatch {
    let stamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let wishes = (0..count)
        .map(|ix| Wish {
            id: WishId::manual(time, stamp, ix),
            gacha_type: pool.default_gacha_type(),
            time,
            seq: ix,
            name: "Unknown".to_owned(),
            item_type: ItemType::Unknown,
            rarity: Rarity::Three,
            item_key: None,
            manual_pity: None,
            source: Source::Manual,
        })
        .collect();
    batch(account, wishes, format!("+{count}"))
}

/// One wish of `item`, made now. `pity` is kept as typed when given (for a
/// history entered after the fact); without it, pity is counted from the
/// wishes before it.
pub fn item(
    account: Option<&Account>,
    gacha_type: GachaType,
    item: &Item,
    pity: Option<u32>,
    time: WishTime,
) -> ImportBatch {
    let stamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let wish = Wish {
        id: WishId::manual(time, stamp, 0),
        gacha_type,
        time,
        seq: 0,
        name: item.name.clone(),
        item_type: item.item_type,
        rarity: item.rarity,
        item_key: Some(item.key.clone()),
        manual_pity: pity,
        source: Source::Manual,
    };
    batch(account, vec![wish], item.name.clone())
}

fn batch(account: Option<&Account>, wishes: Vec<Wish>, detail: String) -> ImportBatch {
    let mut batch = ImportBatch::new(ImportSource::Manual);
    batch.detail = Some(detail);
    batch.accounts.push(match account {
        Some(account) => AccountBatch {
            label: account.label.clone(),
            uid: account.uid.clone(),
            server: account.server,
            wishes,
            merge_into: Some(account.id),
            ..AccountBatch::default()
        },
        None => AccountBatch {
            label: NEW_ACCOUNT_LABEL.to_owned(),
            wishes,
            ..AccountBatch::default()
        },
    });
    batch
}
