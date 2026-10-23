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

interface Confirm {
  text: string;
  onYes: () => void;
}

/** Where startup stands, for the loading screen: each step's state. */
export interface Boot {
  /** Installed games, their exe and regulation versions. */
  games: 'active' | 'done';
  /** The save files in the saves folders. */
  saves: 'active' | 'done';
  /** The save the app opens on (the first one found), or none to open. */
  open: 'waiting' | 'active' | 'done' | 'none';
}

interface State {
  lang: Lang;
  /** Set while the app starts; null once it is ready and the loading screen gives way. */
  boot: Boot | null;
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
  boot: { games: 'active', saves: 'active', open: 'waiting' },
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
    const started = Date.now();
    const step = (patch: Partial<Boot>) => {
      const boot = get().boot;
      if (boot) set({ boot: { ...boot, ...patch } });
    };
    // Both scans at once. Finding the installs (and reading their versions) is the slow one,
    // and a Steam library on a dead network or USB drive can hold it up for long: the screen
    // waits for it only so long, and the Games panel fills in whenever it arrives.
    const games = gamesApi
      .listGames()
      .catch(() => [] as GameInfo[])
      .then((games) => {
        set({ games, gamesLoaded: true });
        step({ games: 'done' });
      });
    const saves = api
      .listSaves()
      .catch(() => [] as FoundSave[])
      .then((saves) => {
        set({ saves, savesLoaded: true });
        step({ saves: 'done' });
      });
    await Promise.all([saves, Promise.race([games, new Promise((r) => setTimeout(r, 4000))])]);
    const first = get().saves[0];
    step({ open: first ? 'active' : 'none' });
    if (first) {
      await get().openSave(first.path);
      step({ open: 'done' });
    }
    // Long enough to read what was found when it all went fast; never a wait of its own.
    await new Promise((r) => setTimeout(r, Math.max(0, 700 - (Date.now() - started))));
    set({ boot: null });
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
        if (current && current.path !== path) await api.closeSave(current.id).catch(() => undefined);
        const doc = await api.openSave(path);
        set({ doc, slot: null, character: null, draft: null });
        const pick = doc.slots.find((s) => s.slot === doc.lastSlot && s.occupied) ?? doc.slots.find((s) => s.occupied);
        if (pick) await get().selectSlot(pick.slot);
      } catch (e) {
        get().fail(e);
      } finally {
        set({ busy: false });
      }
    };
    if (current?.dirty && current.path !== path) {
      get().ask(get().t('confirm.discard', { file: current.fileName }), () => void go());
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
      set({ character, doc: fresh });
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
    const { doc, slot, character, draft } = get();
    if (!doc) return;
    set({ busy: true });
    try {
      if (character && draft && slot !== null) {
        const edit = draftEdit(character, draft);
        if (edit) {
          const updated = await api.editCharacter(doc.id, slot, edit);
          set({ character: updated, draft: draftOf(updated) });
        }
      }
      const report = await api.writeSave(doc.id);
      const fresh = await api.getSave(doc.id);
      set({ doc: fresh });
      const file = doc.path;
      get().toast(report.backup ? get().t('toast.saved', { file, path: report.backup }) : get().t('toast.savedNoBackup', { file }));
      void get().refreshSaves();
    } catch (e) {
      get().fail(e);
    } finally {
      set({ busy: false });
    }
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
  removeGameFolder: async (dir) => {
    try {
      set({ games: await gamesApi.removeGameFolder(dir), gamesLoaded: true });
    } catch (e) {
      get().fail(e);
    }
  }
}));

/** Whether anything is waiting to be written: in-memory edits or a status draft. */
export function useHasChanges(): boolean {
  return useStore((s) => {
    if (s.doc?.dirty) return true;
    return !!(s.character && s.draft && draftEdit(s.character, s.draft));
  });
}
