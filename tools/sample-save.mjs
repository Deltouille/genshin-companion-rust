#!/usr/bin/env node
// Writes a synthetic paimon.moe save with a plausible history (about 600
// wishes from June 2023 to July 2024: Character Event, Weapon Event and
// Standard), to review the UI with realistic volume:
//
//   node tools/sample-save.mjs <out.json>
//   cargo run -p wish-core --example wishctl -- --data-dir <dir> import-paimon <out.json>
//   SCREENSHOT_HEIGHT=1500 cargo run -p wish-app --example screenshot -- <dir> <shots>
//
// Banners and items come from crates/wish-core/data. Pity is simulated (a 5★
// between 60 and 85 pulls, a 4★ at least every 10) with 50/50s; the same
// seed gives the same save. Not real data: never use it for engine tests.
import { readFileSync, writeFileSync } from 'node:fs';
const root = new URL('..', import.meta.url).pathname;
const itemData = JSON.parse(readFileSync(`${root}/crates/wish-core/data/items.json`, 'utf8'));
const items = [...itemData.characters.map((i) => ({ ...i, item_type: 'Character' })), ...itemData.weapons.map((i) => ({ ...i, item_type: 'Weapon' }))];
const banners = JSON.parse(readFileSync(`${root}/crates/wish-core/data/banners.json`, 'utf8'));
let seed = 7;
const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const pick = (list) => list[Math.floor(rand() * list.length)];
const of = (rarity, type) => items.filter((i) => i.rarity === rarity && (!type || i.item_type === type)).map((i) => i.key);
const threeStars = of(3, 'Weapon');
const fourChars = of(4, 'Character');
const fourWeapons = of(4, 'Weapon');
const standardFives = ['diluc', 'jean', 'keqing', 'mona', 'qiqi', 'tighnari', 'dehya'];
const typeOf = (key) => (items.find((i) => i.key === key)?.item_type === 'Character' ? 'character' : 'weapon');

function simulate(list, code, perBanner, fiveCap, featuredOf, standardOf) {
  const pulls = [];
  let pity5 = 0, pity4 = 0, guaranteed = false, softAt = 60 + Math.floor(rand() * 25);
  for (const banner of list) {
    const count = perBanner();
    const clampDate = (s, lo, hi) => (s < lo ? lo : s > hi ? hi : s);
    const start = Date.parse(clampDate(banner.start, '2023-06-01', '2024-06-30').replace(' ', 'T') + 'Z') + 86400000;
    const end = Date.parse(clampDate(banner.end, '2023-06-20', '2024-07-20').replace(' ', 'T') + 'Z') - 86400000;
    const times = Array.from({ length: Math.ceil(count / 10) }, () => new Date(start + rand() * (end - start)).toISOString().slice(0, 19).replace('T', ' ')).sort();

    for (let n = 0; n < count; n += 10) {
      const time = times[n / 10];
      for (let k = 0; k < 10 && n + k < count; k++) {
        pity5++; pity4++;
        let pull;
        if (pity5 >= softAt || pity5 >= fiveCap) {
          const win = guaranteed || rand() < 0.5;
          const id = win ? featuredOf(banner) : pick(standardOf());
          pull = { type: typeOf(id), id, time, pity: pity5, code };
          guaranteed = !win; pity5 = 0; softAt = 60 + Math.floor(rand() * 25);
        } else if (pity4 >= 10 || rand() < 0.06) {
          const id = rand() < 0.5 && banner.featured_rare.length ? pick(banner.featured_rare) : pick(rand() < 0.5 ? fourChars : fourWeapons);
          pull = { type: typeOf(id), id, time, pity: pity4, code };
          pity4 = 0;
        } else {
          pull = { type: 'weapon', id: pick(threeStars), time, pity: 1, code };
        }
        pulls.push(pull);
      }
    }
  }
  pulls.sort((a, b) => (a.time < b.time ? -1 : a.time > b.time ? 1 : 0));
  const since = (rarity) => { let n = 0; for (let i = pulls.length - 1; i >= 0; i--) { const r = items.find((x) => x.key === pulls[i].id)?.rarity; if (r === rarity) break; n++; } return n; };
  return { total: pulls.length, legendary: since(5), rare: since(4), pulls };
}
const inRange = (list) => list.filter((b) => b.start >= '2023-06-01' && b.start < '2024-07-01');
const save = {
  locale: 'en',
  server: 'Europe',
  'wish-uid': '700000123',
  'wish-counter-setting': { firstTime: false, manualInput: false },
  'wish-counter-character-event': simulate(inRange(banners.character).filter((_, i) => i % 2 === 0), '301', () => 20 + Math.floor(rand() * 60), 90, (b) => b.featured[0], () => standardFives),
  'wish-counter-weapon-event': simulate(inRange(banners.weapon).filter((_, i) => i % 4 === 0), '302', () => 10 + Math.floor(rand() * 30), 80, (b) => b.featured[0], () => ['skyward_harp', 'wolfs_gravestone', 'aquila_favonia']),
  'wish-counter-standard': simulate(banners.standard, '200', () => 60, 90, () => pick(standardFives), () => standardFives),
};
if (!process.argv[2]) throw new Error('usage: node tools/sample-save.mjs <out.json>');
writeFileSync(process.argv[2], JSON.stringify(save, null, 1));
for (const k of Object.keys(save).filter((k) => k.startsWith('wish-counter-') && save[k].pulls)) console.log(k, save[k].pulls.length);
