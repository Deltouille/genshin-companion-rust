#!/usr/bin/env node
// Extracts the game data the planner needs from a genshin-db checkout into
// crates/wish-core/data/planner.json, in the app's own format: characters
// (ascension and talent costs), weapons (ascension costs), the materials they
// use, and the EXP tables, which genshin-db does not have and come from the
// game's own data (AnimeGameData).
//
// Usage: node tools/extract-genshin-db.mjs [path/to/genshin-db]
// (defaults to ./genshin-db, a sparse clone of
// https://github.com/theBowja/genshin-db with src/data/English/{characters,
// talents,weapons,materials} and src/data/image). Re-run after
// `git -C genshin-db pull` when a new version comes out, then rebuild the image
// pack: `cargo run --release -p image-pack`.
//
// The app reads only the output, so a change of source only changes this
// script.

import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dbDir = resolve(process.argv[2] ?? join(repoRoot, 'genshin-db'));
const dataDir = join(dbDir, 'src', 'data');
const outFile = join(repoRoot, 'crates', 'wish-core', 'data', 'planner.json');

// EXP per level, from the game's level tables.
const GAME_DATA = 'https://gitlab.com/Dimbreath/AnimeGameData/-/raw/master/ExcelBinOutput';
// The highest level characters and weapons reach (and the levels the EXP
// tables are read up to).
const MAX_LEVEL = 90;
// Materials the costs do not list but the planner converts EXP into.
const EXP_MATERIALS = [104001, 104002, 104003, 104011, 104012, 104013];

const readJson = (path) => JSON.parse(readFileSync(path, 'utf8'));
const readCategory = (category) =>
  Object.fromEntries(
    readdirSync(join(dataDir, 'English', category))
      .filter((file) => file.endsWith('.json'))
      .map((file) => [file.slice(0, -'.json'.length), readJson(join(dataDir, 'English', category, file))]),
  );
const fail = (message) => {
  console.error(`error: ${message}`);
  process.exit(1);
};

const characters = readCategory('characters');
const talents = readCategory('talents');
const weapons = readCategory('weapons');
const materials = readCategory('materials');
const images = {
  characters: readJson(join(dataDir, 'image', 'characters.json')),
  weapons: readJson(join(dataDir, 'image', 'weapons.json')),
  materials: readJson(join(dataDir, 'image', 'materials.json')),
};

const materialsById = new Map();
for (const [key, material] of Object.entries(materials)) {
  materialsById.set(material.id, { key, ...material });
}
const used = new Set(EXP_MATERIALS);

// `[{id, name, count}]` → `[[id, count]]`, every id a known material. A count
// of 0 (a 1★ weapon's first ascension costs no Mora) is left out.
const costList = (owner, items) =>
  items
    .filter(({ count }) => count !== 0)
    .map(({ id, name, count }) => {
      if (!materialsById.has(id)) fail(`${owner} costs unknown material ${id} (${name})`);
      if (!Number.isInteger(count) || count < 0) fail(`${owner} costs ${count} × ${name}`);
      used.add(id);
      return [id, count];
    });
// The cost lists named `ascend1`…`ascendN` or `lvl2`…`lvl10`, in order.
const steps = (owner, costs, names) =>
  names.map((name) => {
    if (!costs?.[name]) fail(`${owner} has no ${name} costs`);
    return costList(owner, costs[name]);
  });
const ascensions = (count) => Array.from({ length: count }, (_, i) => `ascend${i + 1}`);
const TALENT_LEVELS = Array.from({ length: 9 }, (_, i) => `lvl${i + 2}`);

// Characters: those with talent data. The Traveler has one set of talents per
// element and the same ascension as either twin, so each element is a
// character of its own here, keyed by its talents (`travelergeo`).
const plannedCharacters = [];
const skipped = [];
for (const [key, character] of Object.entries(characters)) {
  if (key === 'aether' || key === 'lumine') continue;
  const talent = talents[key];
  if (!talent) {
    skipped.push(character.name);
    continue;
  }
  plannedCharacters.push(characterEntry(key, character, talent, images.characters[key]));
}
for (const [key, talent] of Object.entries(talents)) {
  if (!key.startsWith('traveler')) continue;
  const element = key.slice('traveler'.length);
  const elementText = element[0].toUpperCase() + element.slice(1);
  const traveler = { ...characters.aether, name: `Traveler (${elementText})`, elementText };
  plannedCharacters.push(characterEntry(key, traveler, talent, images.characters.aether));
}

function characterEntry(key, character, talent, image) {
  if (character.rarity !== 4 && character.rarity !== 5) fail(`${key} has rarity ${character.rarity}`);
  return {
    key,
    id: character.id,
    name: character.name,
    rarity: character.rarity,
    element: character.elementText,
    weapon: character.weaponText,
    icon: image?.filename_icon ?? null,
    // Phases 1 to 6, then talent levels 2 to 10.
    ascension: steps(key, character.costs, ascensions(6)),
    talents: steps(key, talent.costs, TALENT_LEVELS),
  };
}

// Weapons: 1★ and 2★ ascend 4 times (to level 70), the others 6 times.
const plannedWeapons = Object.entries(weapons).map(([key, weapon]) => ({
  key,
  id: weapon.id,
  name: weapon.name,
  rarity: weapon.rarity,
  weapon: weapon.weaponText,
  icon: images.weapons[key]?.filename_icon ?? null,
  ascension: steps(key, weapon.costs, ascensions(weapon.rarity <= 2 ? 4 : 6)),
}));

const plannedMaterials = [...used]
  .map((id) => materialsById.get(id))
  .map((material) => ({
    id: material.id,
    key: material.key,
    name: material.name,
    rarity: material.rarity ?? 1,
    kind: material.typeText,
    sort: material.sortRank ?? 0,
    days: material.daysOfWeek ?? [],
    domain: material.dropDomainName ?? null,
    icon: images.materials[material.key]?.filename_icon ?? null,
  }));

// Icons genshin-db does not have yet (the newest characters): Project Amber,
// which the image pack downloads them from, names them by the game's id.
const AMBER_API = 'https://gi.yatta.moe/api/v2/en';
const amberIcon = async (kind, id) => {
  const response = await fetch(`${AMBER_API}/${kind}/${id}`);
  if (!response.ok) return null;
  return (await response.json())?.data?.icon ?? null;
};
for (const [kind, entries] of [
  ['avatar', plannedCharacters],
  ['weapon', plannedWeapons],
  ['material', plannedMaterials],
]) {
  for (const entry of entries.filter((entry) => !entry.icon)) {
    entry.icon = await amberIcon(kind, entry.id);
    console.log(`icon of ${entry.name} from Project Amber: ${entry.icon ?? 'none'}`);
  }
}

// EXP to go from each level to the next, levels 1 to 89.
const fetchJson = async (file) => {
  const response = await fetch(`${GAME_DATA}/${file}`);
  if (!response.ok) fail(`fetching ${file}: HTTP ${response.status}`);
  return response.json();
};
const avatarLevels = await fetchJson('AvatarLevelExcelConfigData.json');
const weaponLevels = await fetchJson('WeaponLevelExcelConfigData.json');
const upToMax = (rows) =>
  rows.filter((row) => row.level < MAX_LEVEL).sort((a, b) => a.level - b.level);
const characterExp = upToMax(avatarLevels).map((row) => row.exp);
const weaponExp = [0, 1, 2, 3, 4].map((rarity) => upToMax(weaponLevels).map((row) => row.requiredExps[rarity]));
if (characterExp.length !== MAX_LEVEL - 1) fail(`character EXP table has ${characterExp.length} levels`);
for (const table of weaponExp) {
  if (table.length !== MAX_LEVEL - 1 || table.some((exp) => !(exp > 0))) fail('weapon EXP table is incomplete');
}

const commit = execFileSync('git', ['-C', dbDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
const byKey = (a, b) => a.key.localeCompare(b.key);
const planner = {
  source: { repo: 'https://github.com/theBowja/genshin-db', commit },
  exp_source: 'https://gitlab.com/Dimbreath/AnimeGameData (AvatarLevelExcelConfigData, WeaponLevelExcelConfigData)',
  character_exp: characterExp,
  weapon_exp: weaponExp,
  characters: plannedCharacters.sort(byKey),
  weapons: plannedWeapons.sort(byKey),
  materials: plannedMaterials.sort((a, b) => a.id - b.id),
};

writeData(outFile, planner);
console.log(
  `${plannedCharacters.length} characters, ${plannedWeapons.length} weapons, ${plannedMaterials.length} materials → ${outFile}`,
);
if (skipped.length) console.log(`skipped (no talent data): ${skipped.join(', ')}`);

// The archive: what the character pages show (profile, stats, talents and
// constellations), for the same characters as the planner.
const archiveFile = join(repoRoot, 'crates', 'wish-core', 'data', 'archive.json');
const stats = readJson(join(dataDir, 'stats', 'characters.json'));
const talentStats = readJson(join(dataDir, 'stats', 'talents.json'));
const curves = readJson(join(dataDir, 'curve', 'characters.json'));
const constellations = readCategory('constellations');
const versions = readJson(join(dataDir, 'version', 'characters.json'));
const talentImages = readJson(join(dataDir, 'image', 'talents.json'));
const constellationImages = readJson(join(dataDir, 'image', 'constellations.json'));

const TALENT_KINDS = {
  combat1: 'Normal Attack',
  combat2: 'Elemental Skill',
  combatsp: 'Alternate Sprint',
  combatju: 'Alternate Jump',
  combat3: 'Elemental Burst',
};
const archiveCharacters = plannedCharacters.map((planned) => {
  const { key } = planned;
  // The Traveler's profile and stats are either twin's; talents and
  // constellations are the element's.
  const profileKey = key.startsWith('traveler') ? 'aether' : key;
  const character = characters[profileKey];
  const stat = stats[profileKey];
  const talent = talents[key];
  const constellation = constellations[key];
  if (!stat) fail(`${key} has no stats`);
  if (!constellation) fail(`${key} has no constellations`);
  const talentImage = talentImages[key] ?? {};
  const params = talentStats[key] ?? {};
  const slots = [
    // Some slots are empty placeholders (`{}`).
    ...Object.keys(TALENT_KINDS).filter((slot) => talent[slot]?.name),
    ...['passive1', 'passive2', 'passive3', 'passive4'].filter((slot) => talent[slot]?.name),
  ];
  return {
    key,
    title: character.title ?? null,
    description: character.description ?? null,
    region: character.region || null,
    affiliation: character.affiliation || null,
    birthday: character.birthday || null,
    constellation: character.constellation || null,
    version: versions[profileKey] ?? null,
    // The wish art; for the newest characters and the Traveler genshin-db has
    // none yet, and the game names it after the icon.
    splash:
      images.characters[profileKey]?.filename_gachaSplash ??
      planned.icon?.replace('UI_AvatarIcon_', 'UI_Gacha_AvatarImg_') ??
      null,
    stats: {
      base: { hp: stat.base.hp, attack: stat.base.attack, defense: stat.base.defense },
      curve: stat.curve,
      special: character.substatText,
      // Every character starts at 5% CRIT Rate and 50% CRIT DMG.
      crit_rate: stat.base.critrate,
      crit_dmg: stat.base.critdmg,
      // Per ascension phase, 0 to 6: what it adds to HP, ATK, DEF and the
      // special stat.
      promotion: stat.promotion.map((p) => [p.hp, p.attack, p.defense, p.specialized]),
    },
    talents: slots.map((slot) => {
      // "1-Hit DMG|{param1:F1P}" → ["1-Hit DMG", "{param1:F1P}"].
      let rows = (talent[slot].attributes?.labels ?? []).map((label) => {
        const at = label.indexOf('|');
        return [label.slice(0, at), label.slice(at + 1)];
      });
      // param1… → its value at talent levels 1 to 15. genshin-db has values for
      // the three main talents only: an alternate sprint keeps its text.
      const values = params[slot] ?? {};
      const known = (template) =>
        [...template.matchAll(/\{(param\d+):[^}]*\}/g)].every(([, p]) => values[p]?.length === 15);
      if (!rows.every(([, template]) => known(template))) {
        console.log(`no values for ${key}'s ${talent[slot].name}: its rows are left out`);
        rows = [];
      }
      return {
        kind: TALENT_KINDS[slot] ?? 'Passive',
        name: talent[slot].name,
        description: talent[slot].description,
        icon: talentImage[`filename_${slot}`] ?? null,
        rows,
        params: rows.length ? values : {},
      };
    }),
    constellations: [1, 2, 3, 4, 5, 6].map((n) => ({
      name: constellation[`c${n}`].name,
      description: constellation[`c${n}`].description,
      icon: constellationImages[key]?.[`filename_c${n}`] ?? null,
    })),
  };
});

// The weapon pages: description, art, ATK and the secondary stat at any
// level, the passive at each refinement.
const weaponStats = readJson(join(dataDir, 'stats', 'weapons.json'));
const weaponCurves = readJson(join(dataDir, 'curve', 'weapons.json'));
const weaponVersions = readJson(join(dataDir, 'version', 'weapons.json'));
const archiveWeapons = plannedWeapons.map((planned) => {
  const { key } = planned;
  const weapon = weapons[key];
  const stat = weaponStats[key];
  if (!stat) fail(`weapon ${key} has no stats`);
  const image = images.weapons[key] ?? {};
  const special = stat.specialized && stat.curve.specialized ? stat.specialized : null;
  const refinements = [1, 2, 3, 4, 5].map((r) => weapon[`r${r}`]?.description).filter(Boolean);
  return {
    key,
    description: weapon.description || null,
    version: weaponVersions[key] ?? null,
    // The wish art, as for the characters: genshin-db's name, else the
    // game's, after the icon.
    art: image.filename_gacha ?? planned.icon?.replace('UI_EquipIcon_', 'UI_Gacha_EquipIcon_') ?? null,
    stats: {
      attack: stat.base.attack,
      attack_curve: stat.curve.attack,
      // The secondary stat: its name as the game shows it, its value at
      // level 1 and its curve; 1★ and 2★ weapons have none.
      special: special ? weapon.mainStatText : null,
      special_base: special ? stat.base.specialized : 0,
      special_curve: special ? stat.curve.specialized : null,
      // Elemental Mastery is a number; every other secondary stat a share.
      special_percent: special !== 'FIGHT_PROP_ELEMENT_MASTERY',
      // Per ascension phase: what it adds to ATK.
      promotion: stat.promotion.map((phase) => phase.attack ?? 0),
    },
    passive: weapon.effectName && refinements.length ? { name: weapon.effectName, refinements } : null,
  };
});

// Growth curves: each one's multiplier at levels 1 to 90.
const usedCurves = new Set(archiveCharacters.flatMap((c) => Object.values(c.stats.curve)));
const usedWeaponCurves = new Set(
  archiveWeapons.flatMap((w) => [w.stats.attack_curve, w.stats.special_curve]).filter(Boolean),
);
for (const curve of usedWeaponCurves) {
  if (usedCurves.has(curve)) fail(`curve ${curve} is both a character's and a weapon's`);
}
const curveValues = (table, curve) =>
  Array.from({ length: MAX_LEVEL }, (_, i) => {
    const value = table[String(i + 1)]?.[curve];
    if (typeof value !== 'number') fail(`curve ${curve} has no level ${i + 1}`);
    return value;
  });
const archive = {
  source: { repo: 'https://github.com/theBowja/genshin-db', commit },
  curves: Object.fromEntries([
    ...[...usedCurves].sort().map((curve) => [curve, curveValues(curves, curve)]),
    ...[...usedWeaponCurves].sort().map((curve) => [curve, curveValues(weaponCurves, curve)]),
  ]),
  characters: archiveCharacters,
  weapons: archiveWeapons,
};
writeData(archiveFile, archive);
console.log(`${archiveCharacters.length} character and ${archiveWeapons.length} weapon pages → ${archiveFile}`);

// One line per list of numbers or names (a cost, an EXP table, the days, a
// talent's values) and per cost list keeps the files readable and their
// diffs small.
function writeData(file, data) {
  const number = String.raw`-?\d+(?:\.\d+)?(?:e-?\d+)?`;
  const flat = new RegExp(String.raw`\[\s*((?:"[^"]*"|${number})(?:,\s*(?:"[^"]*"|${number}))*)\s*\]`, 'g');
  const json = JSON.stringify(data, null, 1)
    .replace(flat, (_, items) => `[${items.replace(/,\s+/g, ',')}]`)
    .replace(/\[\s*((?:\[\d+,\d+\],?\s*)+)\]/g, (_, pairs) => `[${pairs.replace(/\s+/g, '')}]`);
  writeFileSync(file, `${json}\n`);
}
