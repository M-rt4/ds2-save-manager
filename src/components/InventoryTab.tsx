import { useMemo, useState } from 'react';
import type { Category, CharacterView, Item, RecordChange } from '../api';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';
import { CategoryIcon, Cross, ItemIcon, Plus } from './icons';
import { AddItemView } from './AddItemDialog';
import { ItemDetail } from './ItemDetail';
import { ChangeFields, ChangeSummary, ChangeTag, GhostRow, UndoButton, changed, useInventoryChanges } from './InventoryChanges';
import { CATEGORY_ORDER } from '../game';

export function InventoryTab({ character }: { character: CharacterView }) {
  const t = useT();
  const [cat, setCat] = useState<Category | 'all'>('all');
  const [query, setQuery] = useState('');
  const [adding, setAdding] = useState(false);
  // The record whose stats are open under its row, by its offset in the save.
  const [open, setOpen] = useState<number | null>(null);
  const inv = character.inventory;
  const changes = useInventoryChanges(useStore((s) => s.doc), character);

  const counts = useMemo(() => {
    const m = new Map<Category, number>();
    for (const i of inv.items) m.set(i.category, (m.get(i.category) ?? 0) + 1);
    return m;
  }, [inv.items]);

  const q = query.trim().toLowerCase();
  const matches = (i: Item) => (cat === 'all' || i.category === cat) && (!q || i.name.toLowerCase().includes(q));
  const shown = inv.items.filter((i) => matches(i) && (!changes.only || changes.byOffset.has(i.offset)));
  // Removed items stay in the list as ghost rows, where they were, until the save is written.
  const ghosts = changes.removed.filter((r) => r.before && matches(r.before));
  const categoryOf = (e: Item | RecordChange) => ('kind' in e ? e.before!.category : e.category);
  const sorted = [...shown, ...ghosts].sort(
    (a, b) => CATEGORY_ORDER.indexOf(categoryOf(a)) - CATEGORY_ORDER.indexOf(categoryOf(b)) || a.offset - b.offset
  );

  if (adding) return <AddItemView onClose={() => setAdding(false)} />;

  return (
    <div className="inv">
      <div className="inv-cats">
        <button className="row" aria-selected={cat === 'all'} onClick={() => setCat('all')}>
          {t('inv.all')}
          {changes.categories.size > 0 && <span className="cat-chg" title={t('invchg.inCategory')} />}
          <span className="n">{inv.items.length}</span>
        </button>
        {CATEGORY_ORDER.filter((c) => counts.has(c) || changes.categories.has(c)).map((c) => (
          <button key={c} className="row" aria-selected={cat === c} onClick={() => setCat(c)}>
            <CategoryIcon category={c} size={16} />
            {t(`cat.${c}` as Key)}
            {changes.categories.has(c) && <span className="cat-chg" title={t('invchg.inCategory')} />}
            <span className="n">{counts.get(c) ?? 0}</span>
          </button>
        ))}
      </div>
      <div className="inv-main">
        <div className="inv-tools">
          <input className="field" placeholder={t('inv.search')} value={query} onChange={(e) => setQuery(e.target.value)} spellCheck={false} />
          <button className="btn" disabled={!character.layoutOk} onClick={() => setAdding(true)}>
            <Plus size={16} />
            {t('inv.add')}
          </button>
          <span className="counts">
            {t('inv.count', { n: inv.items.length })} · {t('inv.free', { n: inv.freeMain })}
            {inv.unknown > 0 && <> · {t('inv.unknown', { n: inv.unknown })}</>}
          </span>
        </div>
        <ChangeSummary changes={changes} />
        <div className="inv-list">
          {sorted.length === 0 && <p className="empty-note">{t('inv.empty')}</p>}
          {sorted.map((i) =>
            'kind' in i ? (
              <GhostRow key={`gone-${i.offset}`} change={i} onUndo={() => changes.undo(i.offset)} />
            ) : (
              <ItemRow
                key={i.offset}
                item={i}
                editable={character.layoutOk}
                open={open === i.offset}
                onToggle={() => setOpen(open === i.offset ? null : i.offset)}
                change={changes.byOffset.get(i.offset)}
                onUndo={() => changes.undo(i.offset)}
              />
            )
          )}
          {(cat === 'key' || cat === 'gesture') && <p className="note" style={{ padding: '0 18px' }}>{t('inv.readOnly')}</p>}
        </div>
      </div>
    </div>
  );
}

function ItemRow({
  item,
  editable,
  open,
  onToggle,
  change,
  onUndo
}: {
  item: Item;
  editable: boolean;
  open: boolean;
  onToggle: () => void;
  /** How the record differs from the file on disk, if it does. */
  change?: RecordChange;
  onUndo: () => void;
}) {
  const t = useT();
  const edit = useStore((s) => s.editInventory);
  const [qty, setQty] = useState<string | null>(null);
  // Key items and gestures live in their own region, which the editor leaves alone.
  const readOnly = !editable || item.region === 'keys';
  const max = item.maxQuantity;

  const commit = async () => {
    if (qty === null) return;
    const n = Math.round(Number(qty));
    setQty(null);
    if (Number.isFinite(n) && n > 0 && n !== item.quantity) {
      await edit([{ op: 'setQuantity', offset: item.offset, quantity: Math.min(n, max ?? 1) }]);
    }
  };

  const detail = [
    item.armorSlot,
    item.charges ? t('inv.charges', { cur: item.charges[0], max: item.charges[1] }) : null,
    item.durability !== null && item.durability > 0 ? t('inv.durability', { n: Math.round(item.durability) }) : null,
    item.category === 'unknown' ? `id ${item.id}` : null
  ].filter(Boolean);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.target === e.currentTarget && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      onToggle();
    }
  };

  return (
    <>
      <div
        className={change ? `row item chg-${change.kind}` : 'row item'}
        role="listitem"
        tabIndex={0}
        aria-selected={open}
        aria-expanded={open}
        title={t('stats.show')}
        onClick={onToggle}
        onKeyDown={onKey}
      >
        <ItemIcon icon={item.icon} category={item.category} />
        <span className="nm">
          {item.infusion && <span className="inf">{item.infusion} </span>}
          {item.name}
          {item.reinforcement > 0 && <span className={changed(change, 'reinforcement') ? 'plus changed' : 'plus'}>+{item.reinforcement}</span>}
          {change && <ChangeTag kind={change.kind} />}
          {detail.length > 0 && <span className="detail">{detail.join(', ')}</span>}
        </span>
        <span className="actions" onClick={(e) => e.stopPropagation()}>
          {max !== null && !readOnly ? (
            <input
              className={changed(change, 'quantity') ? 'field changed' : 'field'}
              type="number"
              min={1}
              max={max}
              value={qty ?? item.quantity ?? 0}
              onChange={(e) => setQty(e.target.value)}
              onBlur={() => void commit()}
              onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
              aria-label={t('inv.quantity')}
            />
          ) : (
            item.quantity !== null &&
            item.category !== 'gesture' &&
            item.category !== 'spell' && <span className={changed(change, 'quantity') ? 'qty changed' : 'qty'}>{item.quantity}</span>
          )}
          {change && <UndoButton onUndo={onUndo} />}
          {!readOnly && (
            <button className="icon-btn" title={t('inv.remove')} onClick={() => void edit([{ op: 'remove', offset: item.offset }])}>
              <Cross size={15} />
            </button>
          )}
        </span>
      </div>
      {open && change && <ChangeFields change={change} />}
      {open && (
        <ItemDetail id={item.id} plus={item.reinforcement} infused={!!item.infusion} durability={item.durability} wide />
      )}
    </>
  );
}
