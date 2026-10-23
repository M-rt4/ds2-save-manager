import type { Attributes, Category, Edition } from './api';

/** Inventory categories in the order the game's menu tabs run. */
export const CATEGORY_ORDER: Category[] = [
  'weapon',
  'shield',
  'armor',
  'ring',
  'spell',
  'consumable',
  'upgrade',
  'ammo',
  'bossSoul',
  'multiplayer',
  'key',
  'gesture',
  'unknown'
];

export const CLASSES: Record<number, string> = {
  1: 'Warrior',
  2: 'Knight',
  4: 'Bandit',
  6: 'Cleric',
  7: 'Sorcerer',
  8: 'Explorer',
  9: 'Swordsman',
  10: 'Deprived'
};

export const COVENANTS: Record<number, string> = {
  1: 'Heirs of the Sun',
  2: 'Blue Sentinels',
  3: 'Brotherhood of Blood',
  4: 'Way of Blue',
  5: 'Rat King',
  6: 'Bell Keepers',
  7: 'Dragon Remnants',
  8: 'Company of Champions',
  9: 'Pilgrims of Dark'
};

/** Attributes in the order the Level Up screen lists them. */
export const ATTRIBUTE_ORDER: { key: keyof Attributes; label: string; short: string }[] = [
  { key: 'vigor', label: 'Vigor', short: 'VGR' },
  { key: 'endurance', label: 'Endurance', short: 'END' },
  { key: 'vitality', label: 'Vitality', short: 'VIT' },
  { key: 'attunement', label: 'Attunement', short: 'ATN' },
  { key: 'strength', label: 'Strength', short: 'STR' },
  { key: 'dexterity', label: 'Dexterity', short: 'DEX' },
  { key: 'adaptability', label: 'Adaptability', short: 'ADP' },
  { key: 'intelligence', label: 'Intelligence', short: 'INT' },
  { key: 'faith', label: 'Faith', short: 'FTH' }
];

export const levelOf = (a: Attributes) => Object.values(a).reduce((s, v) => s + v, 0) - 53;

export function editionName(e: Edition | null | undefined): string {
  return e === 'original' ? 'Dark Souls II' : e === 'scholar' ? 'Scholar of the First Sin' : '—';
}

export function playTime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export const num = (n: number) => n.toLocaleString('en-US');

export function journeyLabel(j: number): string {
  return j <= 1 ? 'NG' : `NG+${j - 1}`;
}

/**
 * Base derived stats from the attributes, before rings and equipment. Breakpoints and
 * curves follow darthdemono/sl2-analyzer's `ds2_derived_stats` (Apache-2.0, see NOTICE),
 * which were checked there against the in-game Level Up screen.
 */
export function derived(a: Attributes) {
  const end = a.endurance;
  const vit = a.vitality;
  let stamina = 80 + 2 * Math.min(end, 20) + Math.max(0, Math.min(end, 99) - 20);
  if (end >= 99) stamina += 1;
  let load = 38.5 + 1.5 * Math.min(vit, 29);
  if (vit > 29) load += Math.min(vit, 49) - 29;
  if (vit > 49) load += 0.5 * (Math.min(vit, 70) - 49);
  if (vit > 70) load += 0.5 * Math.floor((Math.min(vit, 99) - 70) / 2);
  const agility = 80 + Math.floor(0.75 * a.adaptability + 0.25 * a.attunement + 1e-9);
  const slots = [10, 13, 16, 20, 25, 30, 40, 50, 60, 75, 94].filter((b) => a.attunement >= b).length;
  const n = Math.min(end, a.adaptability);
  let poise = 0.3 * Math.min(n, 30);
  if (n > 30) poise += 0.2 * (Math.min(n, 50) - 30);
  if (n > 50) poise += 0.1 * (Math.min(n, 98) - 50);
  if (n >= 99) poise += 0.2;
  return {
    stamina,
    equipLoad: load,
    agility,
    slots,
    poise,
    atkStr: physAtk(a.strength),
    atkDex: physAtk(a.dexterity),
    magicDef: elemDef(a.intelligence),
    fireDef: elemDef(Math.floor((a.intelligence + a.faith) / 2)),
    lightningDef: elemDef(a.faith),
    darkDef: elemDef(Math.min(a.intelligence, a.faith))
  };
}

const PHYS_BP: [number, number][] = [
  [0, 50],
  [10, 57],
  [20, 80],
  [30, 102],
  [40, 140],
  [50, 155],
  [60, 162],
  [70, 170],
  [80, 185],
  [90, 192],
  [99, 200]
];

function physAtk(stat: number): number {
  const s = Math.max(0, Math.min(stat, 99));
  const lo = Math.min(Math.floor(s / 10) * 10, 90);
  const hi = lo === 90 ? 99 : lo + 10;
  const vlo = PHYS_BP.find((b) => b[0] === lo)![1];
  const vhi = PHYS_BP.find((b) => b[0] === hi)![1];
  return Math.floor(vlo + ((vhi - vlo) * (s - lo)) / (hi - lo));
}

function elemDef(stat: number): number {
  const s = Math.max(0, Math.min(stat, 99));
  let d = 6 * Math.min(s, 10);
  if (s > 10) d += 8 * (Math.min(s, 20) - 10);
  if (s > 20) d += Math.min(s, 60) - 20;
  if (s > 60) d += Math.floor((s - 60) / 2);
  return d;
}

export const INFUSIONS = ['None', 'Fire', 'Magic', 'Lightning', 'Dark', 'Poison', 'Bleed', 'Raw', 'Enchanted', 'Mundane'];
