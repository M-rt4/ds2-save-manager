import { create } from 'zustand';
import {
  api,
  type Attributes,
  type Change,
  type Edition,
  type CharacterEdit,
  type CharacterView,
  type FoundSave,
  gamesApi,
  type GameInfo,
  type InventoryOp,
  type ItemDef,
  type SaveView
} from './api';
import { errorText, initialLang, translate, type Key, type Lang } from './i18n';

export type Tab = 'status' | 'inventory' | 'transfer' | 'file' | 'backups' | 'raw';

/** Per-user preferences, kept in this machine's WebView storage. */
export interface Settings {
  /** Show the "what changed" summary after a transfer. */
  showTransferReport: boolean;
}

const DEFAULT_SETTINGS: Settings = { showTransferReport: true };

/**
 * The install folder conversions read a release's format from: one the user added by hand
 * wins over the detected ones (all kept in settings.json by the games commands).
 */
export function gameDirFor(games: GameInfo[], edition: Edition): string | null {
  const installs = games.find((g) => g.edition === edition)?.installs ?? [];
  return (installs.find((i) => i.source === 'manual') ?? installs[0])?.path ?? null;
}

function loadSettings(): Settings {
  try {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(localStorage.getItem('settings') ?? '{}') };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export interface TransferReportView {
  path: string;
  changes: Change[];
}

/** Status values being edited; shown in blue until saved, like the Level Up screen. */
export interface Draft {
  name: string;
  souls: number;
  attributes: Attributes;
  journey: number;
}

export interface Toast {
  id: number;
  text: string;
  tone: 'info' | 'error';
}

/** Text being edited in the JSON view, kept while the user is on other tabs. */
export interface JsonDraft {
  docId: string;
  /** The document as exported when editing began. */
  base: string;
  text: string;
}

interface Confirm {
  text: string;
  onYes: () => void;
}

interface State {
  lang: Lang;
  saves: FoundSave[];
  savesLoaded: boolean;
  doc: SaveView | null;
  slot: number | null;
  character: CharacterView | null;
  tab: Tab;
  draft: Draft | null;
  gameRunning: boolean;
  catalog: ItemDef[] | null;
  toasts: Toast[];
  confirm: Confirm | null;
  busy: boolean;
  settings: Settings;
  settingsOpen: boolean;
  report: TransferReportView | null;
  games: GameInfo[];
  gamesLoaded: boolean;
  jsonDraft: JsonDraft | null;

  setLang: (lang: Lang) => void;
  t: (key: Key, vars?: Record<string, string | number>) => string;
  init: () => Promise<void>;
  refreshSaves: () => Promise<void>;
  openSave: (path: string) => Promise<void>;
  setDoc: (doc: SaveView) => void;
  selectSlot: (slot: number) => Promise<void>;
  setTab: (tab: Tab) => void;
  setDraft: (patch: Partial<Draft>) => void;
  editInventory: (ops: InventoryOp[]) => Promise<boolean>;
  loadCatalog: () => Promise<ItemDef[]>;
  saveChanges: () => Promise<void>;
  /** Puts the Status draft into the in-memory save, for steps that read the save (save, transfer). */
  commitDraft: () => Promise<void>;
  revert: () => void;
  toast: (text: string, tone?: Toast['tone']) => void;
  fail: (e: unknown) => void;
  ask: (text: string, onYes: () => void) => void;
  closeConfirm: () => void;
  setSettings: (patch: Partial<Settings>) => void;
  setSettingsOpen: (open: boolean) => void;
  showReport: (report: TransferReportView | null) => void;
  refreshGames: () => Promise<void>;
  setGames: (games: GameInfo[]) => void;
  addGameFolder: (dir: string) => Promise<void>;
  removeGameFolder: (dir: string) => Promise<void>;
  setJsonDraft: (draft: JsonDraft | null) => void;
}

export function draftOf(c: CharacterView): Draft {
  return { name: c.name, souls: c.souls, attributes: { ...c.attributes }, journey: c.journey };
}

/** Only the fields that differ from the character, so untouched values are never written. */
export function draftEdit(c: CharacterView, d: Draft): CharacterEdit | null {
  const edit: CharacterEdit = {};
  if (d.name.trim() !== c.name) edit.name = d.name.trim();
  if (d.souls !== c.souls) edit.souls = d.souls;
  if (d.journey !== c.journey) edit.journey = d.journey;
  if ((Object.keys(d.attributes) as (keyof Attributes)[]).some((k) => d.attributes[k] !== c.attributes[k])) {
    edit.attributes = d.attributes;
  }
  return Object.keys(edit).length ? edit : null;
}

let toastId = 0;

export const useStore = create<State>((set, get) => ({
  lang: initialLang(),
  saves: [],
  savesLoaded: false,
  doc: null,
  slot: null,
  character: null,
  tab: 'status',
  draft: null,
  gameRunning: false,
  catalog: null,
  toasts: [],
  confirm: null,
  busy: false,
  settings: loadSettings(),
  settingsOpen: false,
  report: null,
  games: [],
  gamesLoaded: false,
  jsonDraft: null,

  setLang: (lang) => {
    try {
      localStorage.setItem('lang', lang);
    } catch {
      /* storage unavailable */
    }
    set({ lang });
  },
  t: (key, vars) => translate(get().lang, key, vars),

  init: async () => {
    await get().refreshSaves();
    const first = get().saves[0];
    if (first) await get().openSave(first.path);
    const poll = async () => set({ gameRunning: await api.gameRunning().catch(() => false) });
    void poll();
    setInterval(poll, 4000);
  },

  refreshSaves: async () => {
    const saves = await api.listSaves().catch(() => [] as FoundSave[]);
    set({ saves, savesLoaded: true });
    void get().refreshGames();
  },

  openSave: async (path) => {
    const current = get().doc;
    const go = async () => {
      set({ busy: true });
      try {
        const doc = await api.openSave(path);
        if (current && doc.id === current.id) {
          // The save already open, picked again by any spelling of its path. With unsaved
          // edits the backend kept them, and the character on screen is still the one being
          // edited. Otherwise the file was read afresh (the game may have written it since),
          // so the character is read again too: a draft of the old one would carry its old
          // values back over the game's.
          set({ doc });
          const slot = get().slot;
          if (!doc.dirty && slot !== null) await get().selectSlot(slot);
          return;
        }
        if (current) await api.closeSave(current.id).catch(() => undefined);
        // Text being edited in the JSON view belonged to the save that just closed.
        set({ doc, slot: null, character: null, draft: null, jsonDraft: null });
        const pick = doc.slots.find((s) => s.slot === doc.lastSlot && s.occupied) ?? doc.slots.find((s) => s.occupied);
        if (pick) await get().selectSlot(pick.slot);
      } catch (e) {
        get().fail(e);
      } finally {
        set({ busy: false });
      }
    };
    // Asked first whenever unsaved work would go: another save, or this one read afresh while
    // only a Status draft (which the backend does not hold) is pending; and text edited in the
    // JSON view when another save opens.
    const kept = current?.path === path && current.dirty;
    const changes = !!current && hasChanges(get()) && !kept;
    const json = get().jsonDraft;
    const jsonEdited = !!current && current.path !== path && json?.docId === current.id && json.text !== json.base;
    if (current && (changes || jsonEdited)) {
      get().ask(get().t(changes ? 'confirm.discard' : 'confirm.discardJson', { file: current.fileName }), () => void go());
    } else {
      await go();
    }
  },

  setDoc: (doc) => set({ doc }),

  selectSlot: async (slot) => {
    const { doc } = get();
    if (!doc) return;
    try {
      const character = await api.getCharacter(doc.id, slot);
      const occupied = doc.slots[slot]?.occupied;
      set({ slot, character: occupied ? character : null, draft: occupied ? draftOf(character) : null });
    } catch (e) {
      get().fail(e);
    }
  },

  setTab: (tab) => set({ tab }),

  setDraft: (patch) => {
    const d = get().draft;
    if (d) set({ draft: { ...d, ...patch } });
  },

  editInventory: async (ops) => {
    const { doc, slot } = get();
    if (!doc || slot === null) return false;
    try {
      const character = await api.editCharacter(doc.id, slot, { inventory: ops });
      const fresh = await api.getSave(doc.id);
      // Another save or character may have been picked while this was on its way.
      const now = get();
      if (now.doc?.id === doc.id) set({ doc: fresh, ...(now.slot === slot ? { character } : {}) });
      return true;
    } catch (e) {
      get().fail(e);
      return false;
    }
  },

  loadCatalog: async () => {
    const have = get().catalog;
    if (have) return have;
    const catalog = await api.itemCatalog();
    set({ catalog });
    return catalog;
  },

  saveChanges: async () => {
    const { doc } = get();
    if (!doc) return;
    // Ctrl S and the pad's Start end up here too: say why instead of failing at the write.
    if (doc.readOnly) {
      get().toast(get().t('guard.readOnly'), 'error');
      return;
    }
    set({ busy: true });
    try {
      await get().commitDraft();
      const report = await api.writeSave(doc.id);
      const file = doc.path;
      get().toast(report.backup ? get().t('toast.saved', { file, path: report.backup }) : get().t('toast.savedNoBackup', { file }));
      void get().refreshSaves();
    } catch (e) {
      get().fail(e);
    } finally {
      // Show the save as it now stands either way: when writing failed (the game running, the
      // file changed on disk) the draft folded in above is in memory only and must read as
      // unsaved, or Save, the switch guard and the close guard would all think there is nothing.
      const fresh = await api.getSave(doc.id).catch(() => null);
      if (fresh && get().doc?.id === doc.id) set({ doc: fresh });
      set({ busy: false });
    }
  },

  commitDraft: async () => {
    const { doc, slot, character, draft } = get();
    if (!doc || slot === null || !character || !draft) return;
    const edit = draftEdit(character, draft);
    if (!edit) return;
    const updated = await api.editCharacter(doc.id, slot, edit);
    const now = get();
    if (now.doc?.id === doc.id && now.slot === slot) set({ character: updated, draft: draftOf(updated) });
  },

  revert: () => {
    const { doc } = get();
    if (!doc) return;
    get().ask(get().t('confirm.discard', { file: doc.fileName }), async () => {
      try {
        const fresh = await api.reloadSave(doc.id);
        set({ doc: fresh });
        const slot = get().slot;
        if (slot !== null) await get().selectSlot(slot);
        get().toast(get().t('toast.reverted'));
      } catch (e) {
        get().fail(e);
      }
    });
  },

  toast: (text, tone = 'info') => {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { id, text, tone }] });
    setTimeout(() => set({ toasts: get().toasts.filter((t) => t.id !== id) }), tone === 'error' ? 7000 : 4200);
  },
  fail: (e) => get().toast(errorText(get().lang, e), 'error'),
  ask: (text, onYes) => set({ confirm: { text, onYes } }),
  closeConfirm: () => set({ confirm: null }),
  setSettings: (patch) => {
    const settings = { ...get().settings, ...patch };
    try {
      localStorage.setItem('settings', JSON.stringify(settings));
    } catch {
      /* storage unavailable: the setting lasts for this session */
    }
    set({ settings });
  },
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  showReport: (report) => set({ report }),

  /** Installs and save locations; listed again whenever the saves are. */
  refreshGames: async () => {
    const games = await gamesApi.listGames().catch(() => [] as GameInfo[]);
    set({ games, gamesLoaded: true });
  },
  setGames: (games) => set({ games, gamesLoaded: true }),
  addGameFolder: async (dir) => {
    try {
      const games = await gamesApi.addGameFolder(dir);
      set({ games, gamesLoaded: true });
      const game = games.find((g) => g.installs.some((i) => i.path.toLowerCase() === dir.toLowerCase()));
      if (game) get().toast(get().t('games.added', { path: dir, edition: game.edition === 'scholar' ? 'Scholar of the First Sin' : 'Dark Souls II' }));
    } catch (e) {
      if (typeof e === 'object' && e && (e as { code?: string }).code === 'notGameFolder') get().toast(get().t('games.notGameFolder'), 'error');
      else get().fail(e);
    }
  },
  setJsonDraft: (jsonDraft) => set({ jsonDraft }),
  removeGameFolder: async (dir) => {
    try {
      set({ games: await gamesApi.removeGameFolder(dir), gamesLoaded: true });
    } catch (e) {
      get().fail(e);
    }
  }
}));

/** Whether anything is waiting to be written: in-memory edits or a status draft. */
export const hasChanges = (s: State): boolean => !!s.doc?.dirty || !!(s.character && s.draft && draftEdit(s.character, s.draft));

export const useHasChanges = () => useStore(hasChanges);
