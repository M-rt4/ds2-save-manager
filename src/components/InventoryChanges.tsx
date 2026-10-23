import { useEffect, useMemo, useState } from 'react';
import {
  api,
  inventoryApi,
  type Category,
  type CharacterView,
  type InventoryChanges,
  type RecordChange,
  type RecordChangeKind,
  type SaveView
} from '../api';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';
import { ItemIcon } from './icons';

// The inventory's change marks: every record compared with the file on disk (see
// save/inventory_changes.rs), so edits from any tab show alike until the save is written,
// reloaded or discarded.

const NONE: InventoryChanges = { records: [], added: 0, modified: 0, removed: 0 };
const TAG: Record<RecordChangeKind, Key> = { added: 'invchg.added', modified: 'invchg.modified', removed: 'invchg.removed' };

export interface ChangeIndex {
  all: InventoryChanges;
  /** Records holding an item now (added or modified), by offset. */
  byOffset: Map<number, RecordChange>;
  /** Removed records, shown as ghost rows until the save is written. */
  removed: RecordChange[];
  /** Categories with a change, removed items' included. */
  categories: Set<Category>;
  /** The "only changes" filter; it switches itself off once nothing is changed. */
  only: boolean;
  setOnly: (only: boolean) => void;
  /** Puts one record back as the file on disk holds it. */
  undo: (offset: number) => void;
}

export function useInventoryChanges(doc: SaveView | null, character: CharacterView): ChangeIndex {
  const fail = useStore((s) => s.fail);
  const [all, setAll] = useState<InventoryChanges>(NONE);
  const [only, setOnly] = useState(false);

  useEffect(() => {
    if (!doc) return setAll(NONE);
    let live = true;
    inventoryApi
      .changes(doc.id, character.slot)
      .then((c) => live && setAll(c))
      .catch(() => live && setAll(NONE));
    return () => {
      live = false;
    };
  }, [doc, character]);

  const total = all.added + all.modified + all.removed;
  useEffect(() => {
    if (total === 0) setOnly(false);
  }, [total]);

  return useMemo(() => {
    const byOffset = new Map<number, RecordChange>();
    const removed: RecordChange[] = [];
    const categories = new Set<Category>();
    const items = new Map(character.inventory.items.map((i) => [i.offset, i]));
    for (const r of all.records) {
      if (r.kind === 'removed') removed.push(r);
      else byOffset.set(r.offset, r);
      const category = r.kind === 'removed' ? r.before?.category : items.get(r.offset)?.category;
      if (category) categories.add(category);
    }
    const undo = async (offset: number) => {
      if (!doc) return;
      try {
        await inventoryApi.revert(doc.id, character.slot, offset);
        // As editInventory does: a status draft being typed stays as it is.
        const [fresh, char] = await Promise.all([api.getSave(doc.id), api.getCharacter(doc.id, character.slot)]);
        // Another save or character may have been picked while this was on its way.
        const now = useStore.getState();
        if (now.doc?.id === doc.id) useStore.setState({ doc: fresh, ...(now.slot === character.slot ? { character: char } : {}) });
      } catch (e) {
        fail(e);
      }
    };
    return { all, byOffset, removed, categories, only: only && total > 0, setOnly, undo: (o) => void undo(o) };
  }, [all, character, doc, fail, only, total]);
}

export function ChangeTag({ kind }: { kind: RecordChangeKind }) {
  const t = useT();
  return <span className={`chg-tag ${kind}`}>{t(TAG[kind])}</span>;
}

/** Whether a modified record's `field` changed, for showing its value in the Level Up blue. */
export const changed = (change: RecordChange | undefined, field: string) => !!change?.fields.some((f) => f.field === field);

/** "Unsaved changes: 2 new, 1 changed, 1 removed" and the filter for them. */
export function ChangeSummary({ changes }: { changes: ChangeIndex }) {
  const t = useT();
  const { added, modified, removed } = changes.all;
  if (added + modified + removed === 0) return null;
  const parts = [
    added && t('invchg.nAdded', { n: added }),
    modified && t('invchg.nModified', { n: modified }),
    removed && t('invchg.nRemoved', { n: removed })
  ].filter(Boolean);
  return (
    <div className="chg-summary">
      <span className="dot" />
      <span>
        {t('invchg.lead')} {parts.join(', ')}
      </span>
      <button className="btn quiet small" aria-pressed={changes.only} onClick={() => changes.setOnly(!changes.only)}>
        {changes.only ? t('invchg.showAll') : t('invchg.onlyChanged')}
      </button>
    </div>
  );
}

export function UndoButton({ onUndo }: { onUndo: () => void }) {
  const t = useT();
  return (
    <button className="icon-btn chg-undo" title={t('invchg.undo')} aria-label={t('invchg.undo')} onClick={onUndo}>
      <svg width={15} height={15} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <path d="M9 7 4.5 11.5 9 16" />
        <path d="M4.5 11.5H14a5.5 5.5 0 0 1 0 11h-2" />
      </svg>
    </button>
  );
}

/** A removed item, kept in the list (faded, struck through) until the save is written. */
export function GhostRow({ change, onUndo }: { change: RecordChange; onUndo: () => void }) {
  const t = useT();
  const item = change.before;
  if (!item) return null;
  return (
    <div className="row item chg-removed" role="listitem" aria-disabled="true" title={t('invchg.removedTitle')}>
      <ItemIcon icon={item.icon} category={item.category} />
      <span className="nm">
        <span className="gone">
          {item.infusion && `${item.infusion} `}
          {item.name}
          {item.reinforcement > 0 && ` +${item.reinforcement}`}
        </span>
        <ChangeTag kind="removed" />
      </span>
      <span className="actions">
        {item.quantity !== null && item.category !== 'gesture' && item.category !== 'spell' && <span className="qty gone">{item.quantity}</span>}
        <UndoButton onUndo={onUndo} />
      </span>
    </div>
  );
}

function value(field: string, v: unknown): string {
  if (v === null || v === undefined) return '—';
  if (field === 'reinforcement') return `+${v}`;
  if (field === 'durability' && typeof v === 'number') return String(Math.round(v));
  if (field === 'charges' && Array.isArray(v)) return `${v[0]}/${v[1]}`;
  return String(v);
}

/** What changed in a record since the save was read: "old → new" per field. */
export function ChangeFields({ change }: { change: RecordChange }) {
  const t = useT();
  if (change.kind === 'added') {
    return change.before ? <p className="chg-fields">{t('invchg.replaced', { name: change.before.name })}</p> : null;
  }
  if (change.kind !== 'modified') return null;
  return (
    <div className="chg-fields">
      <span className="lead">{t('invchg.since')}</span>
      {change.fields.map((f) => (
        <span key={f.field} className="f">
          {t(`invchg.field.${f.field}` as Key)} <span className="from">{value(f.field, f.from)}</span> →{' '}
          <span className="to">{value(f.field, f.to)}</span>
        </span>
      ))}
    </div>
  );
}
