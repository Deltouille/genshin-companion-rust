//! Spreadsheets in paimon.moe's export layout (version 3): one sheet per
//! banner type with the columns Type, Name, Time, ⭐, Pity, #Roll, Group,
//! Banner, Part, plus "Banner List" and "Information" sheets.
//!
//! Exports follow the layout closely enough for paimon.moe to import them.
//! Imports read paimon.moe's exports and this app's. Their wishes get the
//! same ids as those of a paimon.moe JSON save, so importing both adds
//! nothing twice.

use std::collections::{BTreeSet, HashMap};
use std::io::Cursor;

use calamine::{Data, DataType as _, Reader as _, Xlsx, open_workbook_from_rs};
use chrono::Local;
use rust_xlsxwriter::{Color, Format, Workbook, Worksheet, XlsxError};

use crate::data::GameData;
use crate::engine::Analysis;
use crate::error::{Error, Result};
use crate::import::{AccountBatch, ImportBatch, ImportSource};
use crate::model::{Account, GachaType, ItemType, Pool, Rarity, Source, Wish, WishId, WishTime};

/// Cell A1 of the "Information" sheet, which identifies the layout.
pub const MARKER: &str = "Paimon.moe Wish History Export";
const LAYOUT_VERSION: f64 = 3.0;
const HEADERS: [&str; 9] = [
    "Type", "Name", "Time", "⭐", "Pity", "#Roll", "Group", "Banner", "Part",
];
/// Hand-counted 3★, as paimon.moe writes them.
const UNKNOWN_TYPE: &str = "Unknown_3_star";

/// Sheet names, in paimon.moe's order; Chronicled Wish is an addition.
fn sheet_names(pool: Pool) -> &'static [&'static str] {
    match pool {
        Pool::CharacterEvent => &["Character Event"],
        Pool::WeaponEvent => &["Weapon Event"],
        Pool::Standard => &["Standard"],
        Pool::Beginners => &["Beginners' Wish", "Beginners Wish"],
        Pool::Chronicled => &["Chronicled Wish"],
    }
}

/// One account's history as an .xlsx file.
pub fn export(account: &Account, analysis: &Analysis, data: &GameData) -> Result<Vec<u8>> {
    build(account, analysis, data).map_err(|e| Error::Excel(e.to_string()))
}

fn build(
    account: &Account,
    analysis: &Analysis,
    data: &GameData,
) -> std::result::Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold();
    let five = Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0xCC9832));
    let four = Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0x8A6995));
    let plain = Format::new();

    for pool in [
        Pool::CharacterEvent,
        Pool::WeaponEvent,
        Pool::Standard,
        Pool::Beginners,
        Pool::Chronicled,
    ] {
        let sheet = workbook.add_worksheet();
        sheet.set_name(sheet_names(pool)[0])?;
        write_header(
            sheet,
            &header,
            &[9.0, 32.0, 22.0, 4.0, 6.0, 7.0, 7.0, 30.0, 9.0],
        )?;
        sheet.set_freeze_panes(1, 0)?;
        let Some(pool_analysis) = analysis.pool(pool) else {
            continue;
        };
        let banners = data.banners.for_pool(pool);

        let mut row = 1;
        for range in &pool_analysis.banners {
            let banner = &banners[range.banner];
            let mut group = 0;
            let mut last_time = None;
            for (at, pull) in pool_analysis.pulls[range.pulls.clone()].iter().enumerate() {
                let wish = &pull.wish;
                if last_time != Some(wish.time) {
                    group += 1;
                    last_time = Some(wish.time);
                }
                let format = match wish.rarity {
                    Rarity::Five => &five,
                    Rarity::Four => &four,
                    Rarity::Three => &plain,
                };
                let (banner_name, part) = if wish.gacha_type == GachaType::CharacterEvent2 {
                    let second = data
                        .banners
                        .dual
                        .get(&banner.id())
                        .map(|pair| pair[1].name.as_str());
                    (second.unwrap_or(&banner.name), "Wish 2")
                } else {
                    (banner.name.as_str(), "")
                };
                let kind = match wish.item_type {
                    ItemType::Unknown => UNKNOWN_TYPE,
                    known => known.as_str(),
                };
                sheet.write_string_with_format(row, 0, kind, format)?;
                sheet.write_string_with_format(row, 1, &wish.name, format)?;
                sheet.write_string_with_format(row, 2, wish.time.to_string(), format)?;
                sheet.write_number_with_format(row, 3, wish.rarity.stars(), format)?;
                // paimon.moe writes 1 as the pity of a 3★.
                sheet.write_number_with_format(row, 4, pull.pity.unwrap_or(1), format)?;
                sheet.write_number_with_format(row, 5, (at + 1) as f64, format)?;
                sheet.write_number_with_format(row, 6, group, format)?;
                sheet.write_string_with_format(row, 7, banner_name, format)?;
                sheet.write_string_with_format(row, 8, part, format)?;
                row += 1;
            }
        }
    }

    let banners = workbook.add_worksheet();
    banners.set_name("Banner List")?;
    write_header(banners, &header, &[32.0, 22.0, 22.0])?;
    banners.write_string(0, 1, "Start (UTC+8)")?;
    banners.write_string(0, 2, "End (UTC+8)")?;
    let mut row = 1;
    for pool in Pool::ALL {
        for banner in data.banners.for_pool(*pool) {
            banners.write_string(row, 0, &banner.name)?;
            banners.write_string(row, 1, banner.start.to_string())?;
            banners.write_string(row, 2, banner.end.to_string())?;
            row += 1;
        }
    }

    let info = workbook.add_worksheet();
    info.set_name("Information")?;
    info.set_column_width(0, 28)?;
    info.set_column_width(1, 28)?;
    info.write_string_with_format(0, 0, MARKER, &header)?;
    info.write_string(1, 0, "Version")?;
    info.write_number(1, 1, LAYOUT_VERSION)?;
    info.write_string(2, 0, "Export Date")?;
    info.write_string(2, 1, Local::now().format("%Y-%m-%d %H:%M:%S").to_string())?;
    info.write_string(3, 0, "Exported by")?;
    info.write_string(
        3,
        1,
        format!("Genshin Companion {}", env!("CARGO_PKG_VERSION")),
    )?;
    info.write_string(4, 0, "Account")?;
    info.write_string(4, 1, &account.label)?;
    if let Some(uid) = &account.uid {
        info.write_string(5, 0, "UID")?;
        info.write_string(5, 1, uid)?;
    }

    workbook.save_to_buffer()
}

fn write_header(
    sheet: &mut Worksheet,
    header: &Format,
    widths: &[f64],
) -> std::result::Result<(), XlsxError> {
    for (col, (title, width)) in HEADERS.iter().zip(widths).enumerate() {
        let col = col as u16;
        sheet.write_string_with_format(0, col, *title, header)?;
        sheet.set_column_width(col, *width)?;
    }
    Ok(())
}

/// Whether bytes look like an .xlsx (a zip) file.
pub fn is_xlsx(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

/// Reads a paimon.moe-layout spreadsheet. Fails on unknown items, listing
/// them, rather than guessing their rarity.
pub fn parse(bytes: &[u8], data: &GameData) -> Result<ImportBatch> {
    let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes))
        .map_err(|e: calamine::XlsxError| Error::Excel(e.to_string()))?;
    let info = workbook
        .worksheet_range("Information")
        .map_err(|_| Error::NotPaimonExcel)?;
    if info.get((0, 0)).and_then(|c| c.get_string()) != Some(MARKER) {
        return Err(Error::NotPaimonExcel);
    }
    let uid = info.rows().find_map(|row| {
        (row.first().and_then(|c| c.get_string()) == Some("UID"))
            .then(|| row.get(1).map(cell_text))
            .flatten()
            .filter(|uid| !uid.is_empty())
    });

    let mut account = AccountBatch {
        label: "Main".to_owned(),
        uid,
        ..AccountBatch::default()
    };
    let mut unknown = BTreeSet::new();
    for pool in Pool::ALL {
        let Some(range) = sheet_names(*pool)
            .iter()
            .find_map(|name| workbook.worksheet_range(name).ok())
        else {
            continue;
        };
        let mut seqs: HashMap<WishTime, u32> = HashMap::new();
        for (line, row) in range.rows().enumerate().skip(1) {
            let cell = |col: usize| row.get(col).map(cell_text).unwrap_or_default();
            let kind = cell(0);
            if kind.is_empty() {
                continue;
            }
            let invalid = |message: String| Error::InvalidPull {
                key: sheet_names(*pool)[0].to_owned(),
                index: line + 1,
                message,
            };
            let time = match row.get(2) {
                Some(date) if date.is_datetime() => date
                    .as_datetime()
                    .map(WishTime::new)
                    .ok_or_else(|| invalid("unreadable time".to_owned()))?,
                _ => cell(2)
                    .parse::<WishTime>()
                    .map_err(|e| invalid(e.to_string()))?,
            };
            let seq = {
                let next = seqs.entry(time).or_insert(0);
                *next += 1;
                *next - 1
            };
            let gacha_type = if cell(8) == "Wish 2" {
                GachaType::CharacterEvent2
            } else {
                pool.default_gacha_type()
            };

            let (name, item_type, rarity, item_key) = match kind.to_ascii_lowercase().as_str() {
                "unknown_3_star" | "unknown" => {
                    ("Unknown".to_owned(), ItemType::Unknown, Rarity::Three, None)
                }
                "character" | "weapon" => {
                    let name = cell(1);
                    match data.items.by_name(&name) {
                        Some(item) => (
                            item.name.clone(),
                            item.item_type,
                            item.rarity,
                            Some(item.key.clone()),
                        ),
                        None => {
                            unknown.insert(format!("{kind} `{name}`"));
                            continue;
                        }
                    }
                }
                other => return Err(invalid(format!("unknown type `{other}`"))),
            };
            account.wishes.push(Wish {
                id: WishId::paimon(*pool, time, seq),
                gacha_type,
                time,
                seq,
                name,
                item_type,
                rarity,
                item_key,
                manual_pity: None,
                source: Source::Paimon,
            });
        }
    }
    if !unknown.is_empty() {
        return Err(Error::UnknownItems(unknown.into_iter().collect()));
    }

    let mut batch = ImportBatch::new(ImportSource::PaimonExcel);
    batch.accounts.push(account);
    Ok(batch)
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::String(s) | Data::DateTimeIso(s) => s.trim().to_owned(),
        Data::Int(n) => n.to_string(),
        Data::Float(f) if f.fract() == 0.0 => format!("{f:.0}"),
        Data::Float(f) => f.to_string(),
        Data::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;
    use crate::import::paimon;

    const SAMPLE: &str = include_str!("../tests/fixtures/paimon-sample.json");

    #[test]
    fn an_export_reads_back_as_the_same_wishes() {
        let data = GameData::embedded();
        let batch = paimon::parse(SAMPLE.as_bytes(), data).unwrap();
        let main = &batch.accounts[0];
        let account = Account {
            id: crate::model::AccountId(1),
            label: main.label.clone(),
            uid: main.uid.clone(),
            server: main.server,
        };
        let analysis = engine::analyze(main.wishes.clone(), main.server, &[], data);

        let xlsx = export(&account, &analysis, data).unwrap();
        assert!(is_xlsx(&xlsx));
        let back = parse(&xlsx, data).unwrap();
        let read = &back.accounts[0];
        assert_eq!(read.uid, main.uid);

        // Same ids as the JSON save: importing both adds nothing twice.
        let mut expected: Vec<_> = main
            .wishes
            .iter()
            .map(|w| (&w.id, w.gacha_type, &w.name))
            .collect();
        let mut got: Vec<_> = read
            .wishes
            .iter()
            .map(|w| (&w.id, w.gacha_type, &w.name))
            .collect();
        expected.sort();
        got.sort();
        assert_eq!(got, expected);
    }

    #[test]
    fn other_files_are_not_mistaken_for_a_paimon_spreadsheet() {
        let mut workbook = Workbook::new();
        workbook
            .add_worksheet()
            .write_string(0, 0, "hello")
            .unwrap();
        let bytes = workbook.save_to_buffer().unwrap();
        assert!(matches!(
            parse(&bytes, GameData::embedded()),
            Err(Error::NotPaimonExcel)
        ));
    }
}
