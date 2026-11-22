#!/usr/bin/env node
// Extracts the Battle Pass's rewards into crates/wish-core/data/battle_pass.json:
// for each reward plan (the three reward types a player picks in the game out
// of Mora, Character EXP, Weapon EXP and Artifact EXP), what each of the 50
// levels gives on the free track (Sojourner's Battle Pass) and in addition
// with the Gnostic Hymn, kept to what the planner counts: Mora, EXP books,
// enhancement ores, and the boxes whose talent books or local specialties the
// player picks.
//
// Usage: node tools/extract-battle-pass.mjs
// It reads the Genshin Impact Wiki through its API (the current period's page,
// the boxes' pages): the rewards have been the same in every period, so the
// file changes only when the game changes them. tools/update-game-data.sh
// runs it. It fails on a reward it doesn't know, so a new one is looked at.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dataDir = join(repoRoot, 'crates', 'wish-core', 'data');
const API = 'https://genshin-impact.fandom.com/api.php';
const HEADERS = { 'User-Agent': 'genshin-companion (tools/extract-battle-pass.mjs)' };
const LEVELS = 50;

const fail = (message) => {
  console.error(`error: ${message}`);
  process.exit(1);
};
const query = async (params) => {
  const url = `${API}?${new URLSearchParams({ format: 'json', ...params })}`;
  const response = await fetch(url, { headers: HEADERS });
  if (!response.ok) fail(`${url}: HTTP ${response.status}`);
  return response.json();
};
const wikitext = async (page) => {
  const json = await query({ action: 'parse', page, prop: 'wikitext|revid' });
  if (json.error) fail(`${page}: ${json.error.info}`);
  return { text: json.parse.wikitext['*'], revision: json.parse.revid };
};

// The planner's materials by English name.
const planner = JSON.parse(readFileSync(join(dataDir, 'planner.json'), 'utf8'));
const materialId = (name) => {
  const found = planner.materials.find((m) => (m.name?.en ?? m.name) === name);
  if (!found) fail(`planner.json has no material named "${name}"`);
  return found.id;
};
// What the planner counts, and how; and what it leaves out.
const COUNTED = {
  Mora: { material: materialId('Mora') },
  "Hero's Wit": { material: materialId("Hero's Wit") },
  "Adventurer's Experience": { material: materialId("Adventurer's Experience") },
  'Mystic Enhancement Ore': { material: materialId('Mystic Enhancement Ore') },
  'Guides of a Journey': { box: 'guides' },
  'Philosophies of a Journey': { box: 'philosophies' },
  'Traveling Bounties': { box: 'specialties' },
};
const LEFT_OUT = new Set([
  'Sanctifying Unction', 'Sanctifying Essence', 'Sanctifying Elixir', 'Dust of Enlightenment',
  'Fragile Resin', 'Acquaint Fate', 'Intertwined Fate', 'Primogem', 'BP Bounty',
]);
// A reward plan by the type it leaves out, as the app stores it.
const TYPES = { 'Mora': 'mora', 'Character EXP': 'character-exp', 'Weapon EXP': 'weapon-exp', 'Artifact EXP': 'artifact-exp' };

// The current period: the latest "Battle Pass/…" page already started.
const pages = [];
let from = '';
do {
  const json = await query({ action: 'query', list: 'allpages', apprefix: 'Battle Pass/', aplimit: '100', ...(from && { apcontinue: from }) });
  pages.push(...json.query.allpages.map((p) => p.title));
  from = json.continue?.apcontinue ?? '';
} while (from);
const periods = [];
for (let i = 0; i < pages.length; i += 40) {
  const json = await query({ action: 'query', prop: 'revisions', rvprop: 'content', rvslots: 'main', titles: pages.slice(i, i + 40).join('|') });
  for (const page of Object.values(json.query.pages)) {
    const text = page.revisions?.[0]?.slots?.main?.['*'] ?? '';
    const start = text.match(/\|\s*time_start\s*=\s*(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d)/)?.[1];
    const offset = text.match(/\|\s*time_start_offset\s*=\s*GMT([+-]\d+)/)?.[1];
    if (start && offset) {
      const sign = offset.startsWith('-') ? '-' : '+';
      const hours = String(Math.abs(Number(offset))).padStart(2, '0');
      const iso = `${start.replace(' ', 'T')}${sign}${hours}:00`;
      periods.push({ title: page.title, start: iso, at: Date.parse(iso) });
    }
  }
}
const now = Date.now();
const current = periods.filter((p) => p.at <= now).sort((a, b) => b.at - a.at)[0];
if (!current) fail('no Battle Pass period has started');

// Its rewards: one `{{Battle Pass Rewards}}` per reward plan, each level's
// free reward (s), Gnostic Hymn reward (g) and Gnostic Hymn extra (ga).
const { text, revision } = await wikitext(current.title);
const section = text.slice(text.indexOf("===Sojourner's Battle Pass & Gnostic Hymn==="), text.indexOf('===BP Bounty==='));
const plans = {};
for (const [, title, body] of section.matchAll(/\|([^|\n{}]+)\n\|\{\{Battle Pass Rewards\n([\s\S]*?)\n\}\}/g)) {
  const chosen = title.split(',').map((t) => t.trim());
  if (chosen.length !== 3 || chosen.some((t) => !TYPES[t])) fail(`unknown reward plan "${title}"`);
  const without = Object.keys(TYPES).find((t) => !chosen.includes(t));
  const fields = Object.fromEntries([...body.matchAll(/^\|(\w+?)\s*=\s*(.*)$/gm)].map(([, k, v]) => [k, v.trim()]));
  const reward = (prefix, level) => {
    const name = fields[`${prefix}${level}`];
    if (!name) return [];
    const count = Number((fields[`${prefix}x${level}`] ?? '1').replace(/,/g, ''));
    if (!Number.isInteger(count) || count <= 0) fail(`${title}, level ${level}: count "${fields[`${prefix}x${level}`]}"`);
    if (LEFT_OUT.has(name)) return [];
    if (!COUNTED[name]) fail(`${title}, level ${level}: unknown reward "${name}"`);
    return [{ ...COUNTED[name], count }];
  };
  const levels = [];
  for (let level = 1; level <= LEVELS; level++) {
    levels.push({ free: reward('s', level), premium: [...reward('g', level), ...reward('ga', level)] });
  }
  plans[`no-${TYPES[without]}`] = levels;
}
if (Object.keys(plans).length !== 4) fail(`${current.title}: ${Object.keys(plans).length} reward plans, not 4`);

// How many talent books or local specialties a box gives.
const boxSize = async (page, pattern) => {
  const { text } = await wikitext(page);
  const size = Number(text.match(pattern)?.[1]);
  if (!Number.isInteger(size) || size <= 0) fail(`${page}: no box size`);
  return size;
};
const boxes = {
  guides: await boxSize('Guides of a Journey', /Card List\|[^}]*amount=(\d+)/),
  philosophies: await boxSize('Philosophies of a Journey', /Card List\|[^}]*amount=(\d+)/),
  specialties: await boxSize('Traveling Bounties', /stack of (\d+) Local Specialties/),
};

const out = {
  source: { wiki: 'https://genshin-impact.fandom.com', page: current.title, revision },
  // Each period starts this long after the one before, at the version's
  // update; a level entered in an earlier period counts as 0.
  period_start: current.start,
  period_days: 42,
  boxes,
  plans,
};
writeFileSync(join(dataDir, 'battle_pass.json'), `${JSON.stringify(out, null, 1)}\n`);
console.log(`${current.title} (revision ${revision}, from ${current.start}): ${Object.keys(plans).join(', ')}; boxes ${JSON.stringify(boxes)}`);
