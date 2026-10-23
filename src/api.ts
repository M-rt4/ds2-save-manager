import { invoke } from '@tauri-apps/api/core';

export type Edition = 'original' | 'scholar';

export interface SaveError {
  code: string;
  message: string;
}

export interface FoundSave {
  path: string;
  fileName: string;
  folder: string;
  account: string | null;
  edition: Edition | null;
  size: number;
  modified: number;
}

export interface Chunk {
  kind: number;
  version: number;
  offset: number;
  size: number;
}

export interface EntryInfo {
  index: number;
  name: string;
  size: number;
  chunks: Chunk[];
}

export interface SlotView {
  slot: number;
  occupied: boolean;
  ghost: boolean;
  name: string;
  level: number;
  class: number;
  journey: number;
  soulMemory: number;
  playTime: number;
  map: number;
  layoutOk: boolean;
  area: string | null;
  mapLabel: string;
}

export interface SaveView {
  id: string;
  path: string;
  fileName: string;
  folder: string;
  edition: Edition;
  sourceEdition: Edition;
  account: string | null;
  folderAccount: string | null;
  lastSlot: number | null;
  slots: SlotView[];
  checksumFailures: number[];
  dirty: boolean;
  size: number;
  entries: EntryInfo[];
}

export interface Attributes {
  vigor: number;
  endurance: number;
  vitality: number;
  attunement: number;
  strength: number;
  dexterity: number;
  adaptability: number;
  intelligence: number;
  faith: number;
}

export type Category =
  | 'weapon'
  | 'shield'
  | 'armor'
  | 'ring'
  | 'spell'
  | 'consumable'
  | 'multiplayer'
  | 'upgrade'
  | 'ammo'
  | 'bossSoul'
  | 'key'
  | 'gesture'
  | 'unknown';

export interface Item {
  offset: number;
  region: 'main' | 'keys';
  id: number;
  name: string;
  category: Category;
  armorSlot: string | null;
  quantity: number | null;
  durability: number | null;
  reinforcement: number;
  infusion: string | null;
  charges: [number, number] | null;
}

export interface Inventory {
  items: Item[];
  freeMain: number;
  freeKeys: number;
  unknown: number;
}

export interface Covenant {
  id: number;
  discovered: boolean;
  rank: number;
}

export interface CharacterView {
  slot: number;
  name: string;
  level: number;
  attributes: Attributes;
  souls: number;
  soulMemory: number;
  hp: number;
  deaths: number;
  class: number;
  covenant: number;
  covenants: Covenant[];
  female: boolean;
  hollowing: number;
  journey: number;
  playTime: number;
  map: number;
  inventory: Inventory;
  layoutOk: boolean;
  area: string | null;
  mapLabel: string;
}

export interface ItemDef {
  id: number;
  name: string;
  cat: Category;
  slot?: string;
}

export type InventoryOp =
  | { op: 'setQuantity'; offset: number; quantity: number }
  | { op: 'setReinforcement'; offset: number; level: number }
  | { op: 'remove'; offset: number }
  | { op: 'add'; id: number; quantity: number; reinforcement: number; infusion: number };

export interface CharacterEdit {
  name?: string;
  souls?: number;
  attributes?: Attributes;
  journey?: number;
  inventory?: InventoryOp[];
}

export interface WriteReport {
  path: string;
  backup: string | null;
  edition: Edition;
}

export interface VersionNote {
  entry: number;
  chunk: number;
  source: number;
  target: number;
}

export interface Compatibility {
  compatible: boolean;
  problems: string[];
  versionNotes: VersionNote[];
  systemSizeDifferences: number[];
}

export type ConversionMode = 'replace' | 'merge';

export interface ConversionRequest {
  sourceId: string;
  targetEdition: Edition;
  targetPath: string;
  mode: ConversionMode;
}

export interface ConversionPlan {
  targetPath: string;
  targetExists: boolean;
  targetEdition: Edition | null;
  targetSlots: SlotView[] | null;
  targetError: SaveError | null;
  compatibility: Compatibility | null;
  accountChange: [string, string] | null;
  targetOpenAndDirty: boolean;
}

export const api = {
  listSaves: () => invoke<FoundSave[]>('list_saves'),
  savesRoot: () => invoke<string | null>('saves_root'),
  gameRunning: () => invoke<boolean>('game_running'),
  openSave: (path: string) => invoke<SaveView>('open_save', { path }),
  getSave: (id: string) => invoke<SaveView>('get_save', { id }),
  closeSave: (id: string) => invoke<void>('close_save', { id }),
  reloadSave: (id: string) => invoke<SaveView>('reload_save', { id }),
  getCharacter: (id: string, slot: number) => invoke<CharacterView>('get_character', { id, slot }),
  editCharacter: (id: string, slot: number, edit: CharacterEdit) =>
    invoke<CharacterView>('edit_character', { id, slot, edit }),
  itemCatalog: () => invoke<ItemDef[]>('item_catalog'),
  writeSave: (id: string) => invoke<WriteReport>('write_save', { id }),
  setAccount: (id: string, account: string) => invoke<SaveView>('set_account', { id, account }),
  planConversion: (req: ConversionRequest) => invoke<ConversionPlan>('plan_conversion', { req }),
  runConversion: (req: ConversionRequest) => invoke<WriteReport>('run_conversion', { req }),
  compareSaves: (sourceId: string, targetId: string) =>
    invoke<Compatibility>('compare_saves', { sourceId, targetId }),
  transferCharacter: (sourceId: string, fromSlot: number, targetId: string, toSlot: number) =>
    invoke<SaveView>('transfer_character', { sourceId, fromSlot, targetId, toSlot }),
  defaultTarget: (path: string, edition: Edition) => invoke<string>('default_target', { path, edition })
};

export function isSaveError(e: unknown): e is SaveError {
  return typeof e === 'object' && e !== null && 'code' in e && 'message' in e;
}
