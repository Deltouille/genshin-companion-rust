# The ascension planner

After genshin-center's: the characters and weapons the player wants to raise, each from where it is to where it
should go; what that costs in materials; what the player's bag covers when it is shared out over the plan in
order; what is still lacking, the resin it takes, and which domains give it today.

The rules live in `crates/wish-core/src/planner/`; the page in `crates/wish-app/src/features/planner/`.

```mermaid
graph LR
    json["planner.json<br/><i>characters, weapons, materials, costs</i>"] --> data["PlannerData"]
    plan["The plan<br/><i>PlanEntry: a target and its goal</i>"] --> allocate
    inventory["The inventory<br/><i>what the account has</i>"] --> allocate
    data --> allocate["PlannerData::allocate<br/><i>in the plan's order,<br/>crafting from the tiers below</i>"]
    allocate --> allocation["Allocation<br/>each entry's share: needs, lacks, crafted<br/>missing: the plan's"]
    allocation --> resin["resin_estimate<br/><i>runs per source</i>"]
    allocation --> farms["farms<br/><i>domains for the series lacking</i>"]
    allocation --> model["PlannerModel<br/><i>(wish-app)</i>"]
    resin --> model
    farms --> model
    model --> page["The page"]
```

## The game data

`PlannerData` (`planner/data.rs`) reads the embedded `crates/wish-core/data/planner.json` (or a newer copy from the
game-data folder): characters with their ascension and talent costs, weapons with their ascension costs, materials
by game id (rarity, kind, the domain that gives them and its days), the EXP tables. A cost naming a material it
lacks is refused. It is written by `tools/extract-genshin-db.mjs` ([game-data.md](game-data.md)).

Game rules the data lacks are constants in `planner/mod.rs`: level caps, what books and ores give, 1 Mora per 5
character EXP or 10 weapon EXP.

## Levels and goals

- `Level` is a level and an ascension phase: 80 is phase 5 before ascending, 6 after. `Level::steps()` lists a
  character's (1, 20, 20 ascended… 90); `Level::weapon_steps(rarity)` a weapon's (1★ and 2★ stop at 70).
- `CharacterGoal` is from → to with the three talents (1–10); `CharacterGoal::FULL` is 1 → 90/90, talents 1 → 10.
  `WeaponGoal` is from → to; `WeaponGoal::full(rarity)` goes to the rarity's top.
- `character_cost` / `weapon_cost` give a `Requirement`: materials by game id (Mora included) and the EXP.
  `Requirement::materials_with_exp` turns the EXP into Hero's Wit or Mystic Enhancement Ore (rounded up) and its
  Mora.

`crates/wish-core/tests/planner.rs` checks totals the game is known for: to 90/10/10/10, 168 local specialties, 46
boss drops, 36/96/129 common drops, 9/63/114 books, 18 weekly drops, 3 crowns; 8,362,650 EXP to 90.

## The plan

`PlanEntry` (`planner/plan.rs`) is an id and a `PlanTarget`:

| Target | What it is |
| --- | --- |
| `Character { key, goal }` | a character of the game data |
| `Weapon { key, goal }` | a weapon of the game data |
| `CustomCharacter` | a character the data doesn't have yet (a leak): a name, its stars, a material per role |
| `CustomWeapon` | the same for a weapon |
| `Custom` | a custom entry of earlier versions: a name and its typed costs; only shown and removable now |

`PlannerData::plan_totals` and `entry_totals` add costs up (the EXP before it becomes books); `material_named`
makes a typed resource that names a known material count as it. The order matters: the first entries take from the
inventory first. Each account has its own plan and inventory
(`store/plan.rs`, `store/inventory.rs`); costs are never stored.

### Custom entries and placeholders

`planner/custom.rs`. A character's costs come in six **roles** (`Role::CHARACTER`: common drop, local specialty,
boss material, gem, talent book, weekly boss material), told apart by the material's kind, and boss from weekly by
ascension vs talents. A weapon's in three (`Role::WEAPON`: domain material, elite drop, common drop).
`PlannerData::roles` reads a character's, `weapon_roles` a weapon's, and `families(role)` lists the series the
characters use (the picker's options). `custom_character_cost` costs the first regular character's goal with the custom materials in place of its own;
`custom_weapon_cost` copies the first regular weapon of the same rarity (`weapon_templates`). A typed name for a tiered role becomes
"Name (2★)"…

Each role also has a **placeholder**, "?" (`Role::placeholder_name`), for a material not known yet:
`add_placeholders` copies a model series under ids from `PLACEHOLDER_IDS` (9,000,000 + 100 per role + the tier).
These ids are stored in plans, inventories and snapshots, so they never change. A placeholder has no icon and no
domain days, and its costs, crafting and resin count as the real series'.

## Sharing the inventory out

`PlannerData::allocate(plan, inventory)` (`planner/allocate.rs`) walks the plan in order. Each `EntryShare` says what
the entry needs and what it still lacks once the entries before it took their share; `Allocation::missing` is the
plan's total lack.

- A tier short is **crafted 3 for 1** from the tiers below, after the entry's own needs of those tiers (the
  crafting Mora isn't counted). `Allocation::crafted` and `EntryShare::crafted` say how many, for the tiles' flask
  badge.
- EXP books and ores count for their EXP (`EXP_BOOKS`, `EXP_ORES`: the smaller ones too).
- `craft_families` / `craft_family` group the crafted series by kind with consecutive rarities: gems, talent
  books, enemy drops, weapon materials.

## Resin

`planner/resin.rs`, like genshin-center's "Average Estimated Resin". `resin_estimate(missing, ResinOptions)` counts
runs per `ResinSource`:

- talent books and weapon materials from domains: a series counted in its lowest tier, against what a level IV run
  gives of the tiers still lacking;
- normal boss materials (the 4★ level-up ones), per boss;
- Character EXP and Mora from ley lines.

The averages per world level come from the Genshin Impact wiki (the Genshin Data Gathering team). Weekly bosses,
gems, ores and enemy drops are left out, as genshin-center does. `ResinOptions` (world level, resin a day, sources
ignored) are a setting (`Settings::resin`).

## Farm today

`planner/farming.rs`. The game's day starts at 04:00 in the server's time (`DAY_STARTS_AT`; `game_day`,
`next_game_day`). `PlannerData::farms(allocation)` lists each talent book or weapon material series the plan still
lacks: its tiers lacking, the entries lacking some, its domain and the days it is open (`Material::days`; Sunday
opens them all). A placeholder has no days, so it is never one.

## The page

`crates/wish-app/src/features/planner/`, the model the other big pages follow ([pages.md](pages.md)):

| File | What it does |
| --- | --- |
| `mod.rs` | `PlannerView`: the entity, its actions (add, move, remove, open a dialog), the layout (cards in up to four columns, each at least `CARD_REMS` wide) |
| `controls.rs` | each entry's sliders (a level range, and a character's three talent ranges; while one is dragged the costs follow it, released it saves) and its move menu (the grip: to the start, one place earlier or later, to the end), the keyboard's way to reorder the plan |
| `day.rs` | the game's day for the account's server (else the computer's clock); `watch_the_day` rebuilds when it changes, looking every ten minutes at most since timers don't count a computer's sleep |
| `model.rs` | `PlannerModel::build`: the plan as the sliders show it, allocated, with each card's name, picture, tiles and tooltips, the resin and "Farm today" |
| `sections/today.rs` | "Farm today": the domains open today with the tiers lacking and who needs them, or the next day one is open |
| `sections/still_needed.rs` | what the whole plan lacks, its resin by source, "Resin options…" |
| `sections/card.rs`, `materials.rs` | an entry's card (header, sliders, tiles); a grid of material tiles |
| `sections/drag.rs` | dragging a card by its picture and name onto another, whose place it takes: a copy follows the pointer, the card dims |
| `add.rs` | "Add a character…" and "Add a weapon…": a dialog of cards (the user's choice over pickers), what the account owns first with its level and constellation or refinement, a character already planned faded, a click adds and closes |
| `custom.rs` | the custom character or weapon dialog: kind, name, stars, a material per role (a searchable grid of the role's series, its "?" first, or a typed name) |
| `inventory.rs` | clicking a material: how many the player has, a crafted series side by side, or the EXP books under Hero's Wit; what the plan needs, crafts and lacks of each, worked out again as they type; saved in one write |
| `resin.rs` | the resin line on each card, and the options dialog |

The plan and the inventory are the `Library`'s (`add_plan_entry`, `update_plan_entry`, `remove_plan_entry`,
`move_plan_entry`, `set_owned_many`); the resin options are a setting (`Preferences::set_resin_options`). Moving a card is
`Store::reorder_plan`, one transaction. Removing an entry asks first. A character's archive page has "Add to the
planner", which starts from what the account owns (`Collection::character_goal`, `weapon_goal`).
