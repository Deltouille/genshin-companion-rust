#!/usr/bin/env node
// Extracts the game data the wish counter needs from a paimon.moe checkout
// into crates/wish-core/data/{items,banners}.json, adding HoYoverse's item
// ids from UIGF's public dictionary (needed for UIGF export).
//
// Usage: node tools/extract-paimon-data.mjs [path/to/paimon-moe]
// (defaults to ./reference). Re-run after `git -C reference pull` when a
// new banner or character is released.
//
// Then rebuild the image pack (item icons and banner art) from the same
// checkout: `cargo run --release -p image-pack`.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// paimon.moe imports its own modules without the `.js` extension, which its
// bundler resolves and plain Node does not. Like the bundler, prefer the `.js`
// file: `./weapons` is both `weapons.js` and a `weapons/` folder.
registerHooks({
  resolve(specifier, context, nextResolve) {
    const isExtensionless = specifier.startsWith('.') && !/\.[cm]?js$/.test(specifier);
    if (isExtensionless) {
      try {
        return nextResolve(`${specifier}.js`, context);
      } catch (err) {
        if (err?.code !== 'ERR_MODULE_NOT_FOUND') throw err;
      }
    }
    return nextResolve(specifier, context);
  },
});

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const paimonDir = resolve(args[0] ?? join(repoRoot, 'reference'));
const outDir = join(repoRoot, 'crates', 'wish-core', 'data');
const UIGF_DICTIONARY = 'https://api.uigf.org/dict/genshin/en.json';

const load = async (file) => import(pathToFileURL(join(paimonDir, 'src', 'data', file)).href);

const { characters } = await load('characters.js');
const { weaponList } = await load('weaponList.js');
const { banners } = await load('banners.js');
const { bannersDual } = await load('bannersDual.js');

const commit = execFileSync('git', ['-C', paimonDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
const source = { repo: 'https://github.com/MadeBaruna/paimon-moe', commit };

const errors = [];

// Only 3★ to 5★ items can come out of a wish (1★/2★ weapons cannot).
function slimItems(table, kind) {
  return Object.entries(table)
    .map(([key, item]) => {
      if (typeof item.name !== 'string' || !Number.isInteger(item.rarity)) {
        errors.push(`${kind} '${key}': missing name or rarity`);
      }
      return { key, name: item.name, rarity: item.rarity };
    })
    .filter((item) => item.rarity >= 3)
    .sort((a, b) => a.key.localeCompare(b.key));
}

function slimBanner(banner, where) {
  for (const field of ['name', 'start', 'end']) {
    if (typeof banner[field] !== 'string') errors.push(`${where}: missing '${field}'`);
  }
  const out = {
    name: banner.name,
    image: banner.image,
    short_name: banner.shortName,
    start: banner.start,
    end: banner.end,
    color: banner.color,
    timezone_dependent: banner.timezoneDependent === true,
    timezone_dependent_end: banner.timezoneDependentEnd === true,
    featured: banner.featured ?? [],
    featured_rare: banner.featuredRare ?? [],
  };
  if (banner.version !== undefined) out.version = banner.version;
  return out;
}

const characterItems = slimItems(characters, 'character');
const weaponItems = slimItems(weaponList, 'weapon');

// HoYoverse item ids, by English name. Without network, keep the ids of the
// previous extraction.
async function itemIds() {
  try {
    const response = await fetch(UIGF_DICTIONARY);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch (err) {
    console.warn(`warning: couldn't fetch ${UIGF_DICTIONARY} (${err.message}); keeping the previous item ids`);
    const previous = join(outDir, 'items.json');
    if (!existsSync(previous)) return {};
    const old = JSON.parse(readFileSync(previous, 'utf8'));
    return Object.fromEntries(
      [...old.characters, ...old.weapons].filter((i) => i.item_id).map((i) => [i.name, i.item_id]),
    );
  }
}
const ids = await itemIds();
const withoutId = [];
for (const item of [...characterItems, ...weaponItems]) {
  if (Number.isInteger(ids[item.name])) {
    item.item_id = ids[item.name];
  } else if (!item.key.startsWith('traveler')) {
    withoutId.push(item.key);
  }
}
if (withoutId.length > 0) {
  console.warn(`warning: no HoYoverse item id for ${withoutId.join(', ')} (UIGF exports leave it empty)`);
}

// Imports match API names to items, so a name must identify a single item.
const seenNames = new Map();
for (const item of [...characterItems, ...weaponItems]) {
  if (seenNames.has(item.name)) {
    console.warn(`warning: duplicate item name '${item.name}' (${seenNames.get(item.name)}, ${item.key})`);
  }
  seenNames.set(item.name, item.key);
}

const bannerData = {
  source,
  beginners: banners.beginners.map((b, i) => slimBanner(b, `beginners[${i}]`)),
  standard: banners.standard.map((b, i) => slimBanner(b, `standard[${i}]`)),
  character: banners.characters.map((b, i) => slimBanner(b, `characters[${i}]`)),
  weapon: banners.weapons.map((b, i) => slimBanner(b, `weapons[${i}]`)),
  chronicled: banners.chronicled.map((b, i) => slimBanner(b, `chronicled[${i}]`)),
  dual: Object.fromEntries(
    Object.entries(bannersDual).map(([id, pair]) => [id, pair.map((b, i) => slimBanner(b, `dual '${id}'[${i}]`))]),
  ),
};

// Typos in paimon's banner data, mapped to the real item key.
const KEY_FIXES = {
  primordial_jade_winged_spear: 'primordial_jade_winged-spear',
};

// Every featured key must exist, otherwise the 50/50 engine silently miscounts.
const knownKeys = new Set([...characterItems, ...weaponItems].map((item) => item.key));
const allBanners = [
  ...['beginners', 'standard', 'character', 'weapon', 'chronicled'].map((pool) => [pool, bannerData[pool]]),
  ...Object.entries(bannerData.dual).map(([id, pair]) => [`dual '${id}'`, pair]),
];
for (const [pool, list] of allBanners) {
  for (const banner of list) {
    const where = `${pool} banner '${banner.name} ${banner.image}'`;
    const fix = (key) => {
      if (knownKeys.has(key)) return key;
      if (KEY_FIXES[key] !== undefined) {
        console.warn(`fixed: ${where}: '${key}' → '${KEY_FIXES[key]}'`);
        return KEY_FIXES[key];
      }
      errors.push(`${where}: unknown item '${key}'`);
      return key;
    };
    banner.featured = banner.featured.map(fix);
    banner.featured_rare = banner.featured_rare.map(fix);
  }
}

if (errors.length > 0) {
  console.error(`paimon data looks wrong:\n  ${errors.join('\n  ')}`);
  process.exit(1);
}

mkdirSync(outDir, { recursive: true });
const write = (file, data) => writeFileSync(join(outDir, file), `${JSON.stringify(data, null, 1)}\n`);
write('items.json', { source, characters: characterItems, weapons: weaponItems });
write('banners.json', bannerData);

const bannerCount = ['beginners', 'standard', 'character', 'weapon', 'chronicled']
  .map((pool) => bannerData[pool].length)
  .reduce((a, b) => a + b, 0);
console.log(
  `paimon-moe ${commit.slice(0, 8)}: ${characterItems.length} characters, ${weaponItems.length} weapons, ` +
    `${bannerCount} banners, ${Object.keys(bannerData.dual).length} dual banners → ${outDir}`,
);
