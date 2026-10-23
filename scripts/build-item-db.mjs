// Builds src-tauri/data/items.json from the db_ds2 tables of darthdemono/sl2-analyzer
// (Apache-2.0, see NOTICE). Usage: node scripts/build-item-db.mjs <path-to-sl2-analyzer>
//
// The source tables are keyed by the little-endian hex of the item id. Shields sit in
// armors.json there; here they get their own category, told apart by the id range
// (shields are 11,000,000-11,999,999; armour pieces start at 20,000,000).
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = process.argv[2];
if (!root) {
  console.error('usage: node scripts/build-item-db.mjs <path-to-sl2-analyzer>');
  process.exit(1);
}

const TABLES = {
  weapons: 'weapon',
  armors: 'armor',
  rings: 'ring',
  spells: 'spell',
  consumables: 'consumable',
  online: 'multiplayer',
  upgrade: 'upgrade',
  bolts: 'ammo',
  bosssouls: 'bossSoul',
  key: 'key',
  emotes: 'gesture'
};

const ARMOR_SLOTS = ['head', 'chest', 'hands', 'legs'];
// Names the game's own menu texts spell differently (scripts/build-item-stats.py reports them).
const NAME_FIXES = {
  'Blue Flame Sword': 'Blue Flame',
  'Drakekeeper Ultra Greatsword': 'Drakewing Ultra Greatsword',
  'Seed Of A Tree Of Giants': 'Seed of a Tree of Giants'
};
const seen = new Map();

for (const [stem, category] of Object.entries(TABLES)) {
  const table = JSON.parse(readFileSync(join(root, 'db_ds2', `${stem}.json`), 'utf8'));
  for (const [hex, name] of Object.entries(table)) {
    const id = Buffer.from(hex.replace(/\s+/g, ''), 'hex').readUInt32LE(0);
    if (seen.has(id)) continue;
    let cat = category;
    const item = { id, name: NAME_FIXES[name] ?? name };
    if ((stem === 'armors' || stem === 'weapons') && id >= 11_000_000 && id < 12_000_000) cat = 'shield';
    else if (stem === 'armors' && id < 20_000_000) cat = 'weapon';
    if (cat === 'armor') item.slot = ARMOR_SLOTS[id % 10] ?? 'chest';
    item.cat = cat;
    seen.set(id, item);
  }
}

const items = [...seen.values()].sort((a, b) => a.id - b.id);
const out = resolve('src-tauri/data/items.json');
mkdirSync(resolve('src-tauri/data'), { recursive: true });
writeFileSync(out, JSON.stringify(items, null, 0).replace(/},{/g, '},\n{') + '\n');
const counts = items.reduce((m, i) => ((m[i.cat] = (m[i.cat] ?? 0) + 1), m), {});
console.log(`wrote ${items.length} items to ${out}`, counts);
