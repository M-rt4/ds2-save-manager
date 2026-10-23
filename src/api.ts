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
  /** A development build never writes a save in the game's real folders; edits in memory still work. */
  readOnly: boolean;
  readOnlyReason: SaveError | null;
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
  icon: number | null;
  /** Set when the count can be edited: the most the player can hold. */
  maxQuantity: number | null;
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
  icon?: number;
  maxHeld: number;
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

export interface RegulationInfo {
  /** Regulation version text, e.g. 02000100. */
  version: string;
  /** Game version stamp (exe file version a*100+b*10+c). */
  stamp: number;
  gameVersion: string;
  params: number;
}

export interface ConversionRequest {
  sourceId: string;
  /** null moves the whole save; [from, to] moves one character into slot `to`. */
  slot: [number, number] | null;
  targetEdition: Edition;
  targetPath: string;
  /** A SteamID64 typed by the user; otherwise the target folder's account. */
  account: string | null;
  /** The target game's install folder. */
  gameDir: string | null;
}

export interface FormatSource {
  kind: 'install' | 'targetFile' | 'otherSave';
  path: string;
  regulation: RegulationInfo;
}

export interface LocalAccount {
  id: string;
  folder: string;
  files: string[];
}

export interface AccountPlan {
  source: string | null;
  targetFile: string | null;
  targetFolder: string | null;
  result: string | null;
  origin: 'manual' | 'folder' | 'targetFile' | 'source';
  local: LocalAccount[];
}

export type Change =
  | { kind: 'key'; from: Edition; to: Edition }
  | { kind: 'regulation'; from: RegulationInfo | null; to: RegulationInfo; source: FormatSource }
  | { kind: 'titleVersion'; from: number; to: number }
  | { kind: 'entrySize'; entry: number; from: number; to: number }
  | { kind: 'account'; from: string | null; to: string; origin: AccountPlan['origin'] }
  | { kind: 'slot'; slot: number; before: string | null; after: string | null }
  | { kind: 'keptTargetSystem'; path: string }
  | { kind: 'newFile'; path: string }
  | { kind: 'backup'; path: string };

export interface ConversionPlan {
  targetPath: string;
  targetExists: boolean;
  targetEdition: Edition | null;
  targetSlots: SlotView[] | null;
  targetError: SaveError | null;
  compatibility: Compatibility | null;
  sourceRegulation: RegulationInfo | null;
  format: FormatSource | null;
  accounts: AccountPlan;
  changes: Change[];
  blocked: SaveError | null;
  targetOpenAndDirty: boolean;
}

export interface ConversionReport {
  path: string;
  backup: string | null;
  edition: Edition;
  changes: Change[];
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
  runConversion: (req: ConversionRequest) => invoke<ConversionReport>('run_conversion', { req }),
  defaultTarget: (path: string, edition: Edition) => invoke<string>('default_target', { path, edition })
};

export function isSaveError(e: unknown): e is SaveError {
  return typeof e === 'object' && e !== null && 'code' in e && 'message' in e;
}

export type BackupReason = 'manual' | 'save' | 'transfer' | 'restore';

export interface BackupCharacter {
  slot: number;
  name: string;
  level: number;
}

export interface Backup {
  path: string;
  fileName: string;
  /** When it was taken, in ms since the epoch. */
  created: number;
  /** null on backups from before reasons were recorded. */
  reason: BackupReason | null;
  size: number;
  readable: boolean;
  edition: Edition | null;
  characters: BackupCharacter[];
}

export interface RestoreReport {
  path: string;
  /** Where the file that was there before went. */
  backup: string | null;
  edition: Edition;
}

export const backupApi = {
  list: (savePath: string) => invoke<Backup[]>('list_backups', { savePath }),
  create: (savePath: string) => invoke<Backup>('create_backup', { savePath }),
  restore: (backupPath: string, targetPath: string) =>
    invoke<RestoreReport>('restore_backup', { backupPath, targetPath }),
  remove: (path: string) => invoke<void>('delete_backup', { path })
};

// ---------- Games: installs and save locations ----------

export type InstallSource = 'steam' | 'registry' | 'manual';

export interface Install {
  path: string;
  exe: string | null;
  source: InstallSource;
  exeVersion: string | null;
  regulationVersion: string | null;
}

export interface SlotBrief {
  slot: number;
  name: string;
  level: number;
}

/** A release's save in one folder under the saves root, or where the game would write it. */
export interface GameSave {
  path: string;
  fileName: string;
  folder: string;
  account: string | null;
  exists: boolean;
  size: number;
  modified: number;
  slots: SlotBrief[];
  error: SaveError | null;
}

export interface GameInfo {
  edition: Edition;
  steamAppId: number;
  installs: Install[];
  savesRoot: string | null;
  saves: GameSave[];
}

export const gamesApi = {
  listGames: () => invoke<GameInfo[]>('list_games'),
  addGameFolder: (path: string) => invoke<GameInfo[]>('add_game_folder', { path }),
  removeGameFolder: (path: string) => invoke<GameInfo[]>('remove_game_folder', { path })
};

// ---------- About ----------

export interface SystemInfo {
  os: string | null;
  arch: string;
}

export const aboutApi = {
  systemInfo: () => invoke<SystemInfo>('system_info'),
  /** Set only in development builds. */
  devLabel: () => invoke<string | null>('dev_label'),
  /** Set when this copy lists and writes a scratch folder instead of the real saves. */
  testSavesRoot: () => invoke<string | null>('test_saves_root')
};

export interface RawMatch {
  index: number;
  offset: number;
}

export interface RawMatches {
  matches: RawMatch[];
  /** Whether the search stopped at its limit. */
  more: boolean;
}

export const rawApi = {
  read: (id: string, index: number, offset: number, length: number) =>
    invoke<ArrayBuffer>('raw_read', { id, index, offset, length }).then((b) => new Uint8Array(b)),
  /** Resolves to whether any byte changed. */
  write: (id: string, index: number, offset: number, bytes: number[]) =>
    invoke<boolean>('raw_write', { id, index, offset, bytes }),
  /** `index` null searches every block. */
  find: (id: string, index: number | null, pattern: number[]) => invoke<RawMatches>('raw_find', { id, index, pattern })
};

/** Where in a save's JSON document something is. */
export interface DocPlace {
  /** e.g. `characters[slot 2].inventory[record 7724].quantity` */
  path: string;
  slot: number | null;
  record: number | null;
  field: string;
}

export interface DocChange extends DocPlace {
  from: string;
  to: string;
  /** False for fields the document only shows; those are not written. */
  applies: boolean;
  /** A new entry that joins a stack the character already holds. */
  stack?: { name: string; added: number; total: number };
}

export interface DocProblem extends DocPlace {
  message: string;
}

export interface DocPreview {
  syntax: { line: number; column: number; message: string } | null;
  problems: DocProblem[];
  changes: DocChange[];
}

export const documentApi = {
  exportText: (id: string) => invoke<string>('document_export', { id }),
  preview: (id: string, text: string) => invoke<DocPreview>('document_preview', { id, text }),
  apply: (id: string, text: string) => invoke<DocChange[]>('document_apply', { id, text }),
  writeFile: (path: string, text: string) => invoke<void>('document_write_file', { path, text }),
  readFile: (path: string) => invoke<string>('document_read_file', { path })
};

export type RecordChangeKind = 'added' | 'modified' | 'removed';

export interface RecordChange {
  /** The record's place in the character block, as `Item.offset`. */
  offset: number;
  kind: RecordChangeKind;
  /** What the record holds on disk: the removed item, the one an added item replaced, the modified item before. */
  before: Item | null;
  /** For modified records: `quantity`, `durability`, `reinforcement`, `infusion`, `charges` or `bytes`. */
  fields: { field: string; from: unknown; to: unknown }[];
}

/** A character's inventory against the file on disk. */
export interface InventoryChanges {
  records: RecordChange[];
  added: number;
  modified: number;
  removed: number;
}

export const inventoryApi = {
  changes: (id: string, slot: number) => invoke<InventoryChanges>('inventory_changes', { id, slot }),
  /** Puts one record back as the file on disk holds it; resolves to whether anything changed. */
  revert: (id: string, slot: number, offset: number) => invoke<boolean>('inventory_revert', { id, slot, offset })
};
