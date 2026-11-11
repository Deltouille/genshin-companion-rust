//! The planner's entries in the database. Every write is one transaction,
//! after at most one backup an hour (`before-planning`).

use chrono::{Local, SecondsFormat};
use rusqlite::{Row, Transaction, params};

use super::Store;
use crate::error::Result;
use crate::planner::{
    CharacterGoal, CustomCost, CustomItem, CustomMaterials, Level, PlanEntry, PlanTarget, Role,
    WeaponGoal,
};

const ENTRY_COLUMNS: &str = "id, character_key, name, level_from, phase_from, level_to, \
     phase_to, talent1_from, talent1_to, talent2_from, talent2_to, talent3_from, talent3_to, stars, \
     weapon_key, custom_weapon";

/// A row of `plan_entries`, before its custom parts are read.
struct EntryRow {
    id: i64,
    key: Option<String>,
    name: Option<String>,
    goal: CharacterGoal,
    stars: Option<u8>,
    weapon_key: Option<String>,
    custom_weapon: bool,
}

impl Store {
    /// The plan, in the order the player added its entries.
    pub fn plan(&self) -> Result<Vec<PlanEntry>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM plan_entries ORDER BY position, id"
        ))?;
        let rows: Vec<EntryRow> = stmt
            .query_map([], read_entry)?
            .collect::<rusqlite::Result<_>>()?;
        let mut costs = self.conn.prepare(
            "SELECT material_id, name, count FROM plan_custom_costs \
             WHERE entry_id = ?1 ORDER BY position",
        )?;
        let mut materials = self.conn.prepare(
            "SELECT role, material_id, name FROM plan_custom_materials WHERE entry_id = ?1",
        )?;
        let mut plan = Vec::with_capacity(rows.len());
        for row in rows {
            let weapon_goal = WeaponGoal {
                from: row.goal.from,
                to: row.goal.to,
            };
            let custom_materials = |materials: &mut rusqlite::Statement| {
                materials
                    .query_map([row.id], |r| {
                        Ok((r.get::<_, Role>(0)?, read_item(r, 1, 2)?))
                    })?
                    .collect::<rusqlite::Result<CustomMaterials>>()
            };
            let target = match (row.key, row.stars, row.weapon_key) {
                (_, _, Some(key)) => PlanTarget::Weapon {
                    key,
                    goal: WeaponGoal {
                        from: row.goal.from,
                        to: row.goal.to,
                    },
                },
                (Some(key), _, None) => PlanTarget::Character {
                    key,
                    goal: row.goal,
                },
                (None, Some(stars), None) if row.custom_weapon => PlanTarget::CustomWeapon {
                    name: row.name.unwrap_or_default(),
                    stars,
                    materials: custom_materials(&mut materials)?,
                    goal: weapon_goal,
                },
                (None, Some(stars), None) => PlanTarget::CustomCharacter {
                    name: row.name.unwrap_or_default(),
                    stars,
                    materials: custom_materials(&mut materials)?,
                    goal: row.goal,
                },
                (None, None, None) => PlanTarget::Custom {
                    name: row.name.unwrap_or_default(),
                    costs: costs
                        .query_map([row.id], |r| {
                            Ok(CustomCost {
                                item: read_item(r, 0, 1)?,
                                count: r.get(2)?,
                            })
                        })?
                        .collect::<rusqlite::Result<_>>()?,
                },
            };
            plan.push(PlanEntry { id: row.id, target });
        }
        Ok(plan)
    }

    /// Adds an entry at the end of the plan; returns its id.
    pub fn add_plan_entry(&mut self, target: &PlanTarget) -> Result<i64> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        let position: i64 = tx.query_row(
            "SELECT coalesce(max(position), -1) + 1 FROM plan_entries",
            [],
            |row| row.get(0),
        )?;
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        let id = insert_entry(&tx, position, &now, target)?;
        tx.commit()?;
        Ok(id)
    }

    /// Replaces an entry's goal, or a custom entry's name and materials.
    pub fn update_plan_entry(&mut self, entry: &PlanEntry) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        write_target(&tx, entry.id, &entry.target)?;
        tx.commit()?;
        Ok(())
    }

    /// Puts the plan in the order of `ids`, which lists its entries: the
    /// order the inventory is shared out in.
    pub fn reorder_plan(&mut self, ids: &[i64]) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        for (position, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE plan_entries SET position = ?2 WHERE id = ?1",
                params![id, position as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Removes an entry, and what its custom parts were.
    pub fn remove_plan_entry(&mut self, id: i64) -> Result<()> {
        self.backup_for_planning()?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM plan_custom_costs WHERE entry_id = ?1", [id])?;
        tx.execute(
            "DELETE FROM plan_custom_materials WHERE entry_id = ?1",
            [id],
        )?;
        tx.execute("DELETE FROM plan_entries WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }
}

fn read_entry(row: &Row) -> rusqlite::Result<EntryRow> {
    let level = |level: usize, phase: usize| -> rusqlite::Result<Level> {
        let (level, phase): (u8, u8) = (row.get(level)?, row.get(phase)?);
        // The CHECKs keep both in range; a level outside its phase is read
        // as the phase's nearest level.
        Ok(Level::new(level, phase).unwrap_or_else(|| Level::clamped(level, phase)))
    };
    let talent = |from: usize, to: usize| -> rusqlite::Result<(u8, u8)> {
        Ok((row.get(from)?, row.get(to)?))
    };
    Ok(EntryRow {
        id: row.get(0)?,
        key: row.get(1)?,
        name: row.get(2)?,
        goal: CharacterGoal {
            from: level(3, 4)?,
            to: level(5, 6)?,
            talents: [talent(7, 8)?, talent(9, 10)?, talent(11, 12)?],
        },
        stars: row.get(13)?,
        weapon_key: row.get(14)?,
        custom_weapon: row.get(15)?,
    })
}

/// A material by its id (column `id`) or its name (column `name`).
fn read_item(row: &Row, id: usize, name: usize) -> rusqlite::Result<CustomItem> {
    Ok(match row.get::<_, Option<i64>>(id)? {
        Some(material) => CustomItem::Material(material as u32),
        None => CustomItem::Named(row.get(name)?),
    })
}

/// An item as its two columns: the material's id, or its name.
pub(super) fn item_columns(item: &CustomItem) -> (Option<i64>, Option<&str>) {
    match item {
        CustomItem::Material(id) => (Some(i64::from(*id)), None),
        CustomItem::Named(name) => (None, Some(name.trim())),
    }
}

fn insert_entry(tx: &Transaction, position: i64, now: &str, target: &PlanTarget) -> Result<i64> {
    let (key, name) = match target {
        PlanTarget::Character { key, .. } => (Some(key.as_str()), None),
        // 003's CHECK wants a name: a weapon's is its key (see 005).
        PlanTarget::Weapon { key, .. } => (None, Some(key.as_str())),
        PlanTarget::CustomCharacter { name, .. }
        | PlanTarget::CustomWeapon { name, .. }
        | PlanTarget::Custom { name, .. } => (None, Some(name.trim())),
    };
    tx.execute(
        "INSERT INTO plan_entries (position, character_key, name, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        params![position, key, name, now],
    )?;
    let id = tx.last_insert_rowid();
    write_target(tx, id, target)?;
    Ok(id)
}

fn write_target(tx: &Transaction, id: i64, target: &PlanTarget) -> Result<()> {
    let write_goal = |goal: &CharacterGoal| -> Result<()> {
        let [(t1, t1_to), (t2, t2_to), (t3, t3_to)] = goal.talents;
        tx.execute(
            "UPDATE plan_entries SET \
             level_from = ?2, phase_from = ?3, level_to = ?4, phase_to = ?5, \
             talent1_from = ?6, talent1_to = ?7, talent2_from = ?8, talent2_to = ?9, \
             talent3_from = ?10, talent3_to = ?11 WHERE id = ?1",
            params![
                id,
                goal.from.level,
                goal.from.phase,
                goal.to.level,
                goal.to.phase,
                t1,
                t1_to,
                t2,
                t2_to,
                t3,
                t3_to
            ],
        )?;
        Ok(())
    };
    match target {
        PlanTarget::Character { key, goal } => {
            tx.execute(
                "UPDATE plan_entries SET character_key = ?2, name = NULL, stars = NULL, \
                 weapon_key = NULL, custom_weapon = 0 WHERE id = ?1",
                params![id, key],
            )?;
            write_goal(goal)?;
        }
        PlanTarget::Weapon { key, goal } => {
            tx.execute(
                "UPDATE plan_entries SET character_key = NULL, name = ?2, stars = NULL, \
                 weapon_key = ?2, custom_weapon = 0 WHERE id = ?1",
                params![id, key],
            )?;
            write_goal(&CharacterGoal {
                from: goal.from,
                to: goal.to,
                talents: [(1, 1); 3],
            })?;
        }
        PlanTarget::CustomCharacter {
            name,
            stars,
            materials,
            goal,
        } => {
            tx.execute(
                "UPDATE plan_entries SET character_key = NULL, name = ?2, stars = ?3, \
                 weapon_key = NULL, custom_weapon = 0 WHERE id = ?1",
                params![id, name.trim(), stars],
            )?;
            write_goal(goal)?;
            write_custom_materials(tx, id, materials)?;
        }
        PlanTarget::CustomWeapon {
            name,
            stars,
            materials,
            goal,
        } => {
            tx.execute(
                "UPDATE plan_entries SET character_key = NULL, name = ?2, stars = ?3, \
                 weapon_key = NULL, custom_weapon = 1 WHERE id = ?1",
                params![id, name.trim(), stars],
            )?;
            write_goal(&CharacterGoal {
                from: goal.from,
                to: goal.to,
                talents: [(1, 1); 3],
            })?;
            write_custom_materials(tx, id, materials)?;
        }
        PlanTarget::Custom { name, costs } => {
            tx.execute(
                "UPDATE plan_entries SET character_key = NULL, name = ?2, stars = NULL, \
                 weapon_key = NULL, custom_weapon = 0 WHERE id = ?1",
                params![id, name.trim()],
            )?;
            tx.execute("DELETE FROM plan_custom_costs WHERE entry_id = ?1", [id])?;
            for (position, cost) in costs.iter().enumerate() {
                let (material, name) = item_columns(&cost.item);
                tx.execute(
                    "INSERT INTO plan_custom_costs (entry_id, position, material_id, name, count) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, position as i64, material, name, cost.count],
                )?;
            }
        }
    }
    Ok(())
}

/// A custom character's or weapon's materials, in place of its earlier ones.
fn write_custom_materials(tx: &Transaction, id: i64, materials: &CustomMaterials) -> Result<()> {
    tx.execute(
        "DELETE FROM plan_custom_materials WHERE entry_id = ?1",
        [id],
    )?;
    for (role, item) in materials {
        let (material, name) = item_columns(item);
        tx.execute(
            "INSERT INTO plan_custom_materials (entry_id, role, material_id, name) \
             VALUES (?1, ?2, ?3, ?4)",
            params![id, role, material, name],
        )?;
    }
    Ok(())
}
