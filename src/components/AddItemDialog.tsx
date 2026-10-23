import { useEffect, useMemo, useState } from 'react';
import type { Category, ItemDef } from '../api';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';
import { CATEGORY_ORDER, INFUSIONS } from '../game';
import { CategoryIcon, Chevron, ItemIcon } from './icons';
import { ItemDetail } from './ItemDetail';

const ADDABLE = CATEGORY_ORDER.filter((c) => !['key', 'gesture', 'unknown'].includes(c));
const REINFORCEABLE: Category[] = ['weapon', 'shield', 'armor'];

/**
 * The game's whole item catalogue, as its own view in place of the inventory list: the
 * inventory itself only ever shows what the save holds.
 */
export function AddItemView({ onClose }: { onClose: () => void }) {
  const t = useT();
  const loadCatalog = useStore((s) => s.loadCatalog);
  const edit = useStore((s) => s.editInventory);
  const [catalog, setCatalog] = useState<ItemDef[]>([]);
  const [cat, setCat] = useState<Category>('weapon');
  const [query, setQuery] = useState('');
  const [picked, setPicked] = useState<ItemDef | null>(null);
  const [quantity, setQuantity] = useState(1);
  const [plus, setPlus] = useState(0);
  const [infusion, setInfusion] = useState(0);

  useEffect(() => {
    void loadCatalog().then(setCatalog);
  }, [loadCatalog]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const q = query.trim().toLowerCase();
  const list = useMemo(() => {
    const seen = new Set<string>();
    return catalog.filter((i) => {
      if (q ? !ADDABLE.includes(i.cat) : i.cat !== cat) return false;
      if (q && !i.name.toLowerCase().includes(q)) return false;
      // The tables list some names under several ids (variants); offer the first.
      const k = `${i.cat}:${i.name}`;
      if (seen.has(k)) return false;
      seen.add(k);
      return true;
    });
  }, [catalog, cat, q]);

  const max = picked ? Math.max(1, picked.maxHeld || 1) : 1;
  const countable = !!picked && max > 1;

  const add = async () => {
    if (!picked) return;
    const ok = await edit([
      {
        op: 'add',
        id: picked.id,
        quantity: countable ? Math.min(quantity, max) : 1,
        reinforcement: REINFORCEABLE.includes(picked.cat) ? plus : 0,
        infusion: picked.cat === 'weapon' || picked.cat === 'shield' ? infusion : 0
      }
    ]);
    if (ok) useStore.getState().toast(t('inv.added', { name: picked.name }));
  };

  return (
    <div className="add-view">
      <div className="inv-tools">
        <button className="btn quiet" onClick={onClose}>
          <Chevron dir="left" size={15} />
          {t('inv.back')}
        </button>
        <input className="field" autoFocus placeholder={t('inv.searchAll')} value={query} onChange={(e) => setQuery(e.target.value)} spellCheck={false} />
        <span className="counts">{t('inv.addLead')}</span>
      </div>
      <div className="add-grid">
        <div className="list inv-cats">
          {ADDABLE.map((c) => (
            <button key={c} className="row" aria-selected={!q && cat === c} onClick={() => (setCat(c), setQuery(''))}>
              <CategoryIcon category={c} size={16} />
              {t(`cat.${c}` as Key)}
            </button>
          ))}
        </div>
        <div className="list">
          {list.length === 0 && <p className="empty-note">{t('inv.none')}</p>}
          {list.map((i) => (
            <button
              key={i.id}
              className="row catalog-row"
              aria-selected={picked?.id === i.id}
              onClick={() => (setPicked(i), setQuantity(1), setPlus(0), setInfusion(0))}
            >
              <ItemIcon icon={i.icon} category={i.cat} className="item-card small" />
              <span>{i.name}</span>
              {i.slot && <span className="dim slot-tag">{i.slot}</span>}
            </button>
          ))}
        </div>
        <div className="add-detail">
          {picked ? (
            <>
              <ItemIcon icon={picked.icon} category={picked.cat} className="item-card large" />
              <h3 className="item-title">{picked.name}</h3>
              <div className="dim">
                {t(`cat.${picked.cat}` as Key)}
                {picked.maxHeld > 1 && <>, {t('inv.maxHeld', { n: picked.maxHeld })}</>}
              </div>
              <div className="add-opts">
                {countable && (
                  <label>
                    {t('inv.quantity')}
                    <input className="field" type="number" min={1} max={max} value={quantity} onChange={(e) => setQuantity(Math.max(1, Math.min(max, Number(e.target.value) || 1)))} />
                  </label>
                )}
                {REINFORCEABLE.includes(picked.cat) && (
                  <label>
                    {t('inv.reinforcement')}
                    <input className="field" type="number" min={0} max={10} value={plus} onChange={(e) => setPlus(Math.max(0, Math.min(10, Number(e.target.value) || 0)))} />
                  </label>
                )}
                {(picked.cat === 'weapon' || picked.cat === 'shield') && (
                  <label>
                    {t('inv.infusion')}
                    <select className="field" value={infusion} onChange={(e) => setInfusion(Number(e.target.value))}>
                      {INFUSIONS.map((n, i) => (
                        <option key={n} value={i}>
                          {n}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
              <button className="btn ember" onClick={() => void add()}>
                {t('inv.addConfirm')}
              </button>
              <ItemDetail id={picked.id} plus={REINFORCEABLE.includes(picked.cat) ? plus : 0} infused={infusion > 0} />
              <p className="note">{t('inv.addWarning')}</p>
            </>
          ) : (
            <p className="note">{t('inv.pick')}</p>
          )}
        </div>
      </div>
    </div>
  );
}
