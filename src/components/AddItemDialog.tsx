import { useEffect, useMemo, useState } from 'react';
import type { Category, ItemDef } from '../api';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';
import { CATEGORY_ORDER, INFUSIONS } from '../game';
import { CategoryIcon } from './icons';

const ADDABLE = CATEGORY_ORDER.filter((c) => !['key', 'gesture', 'unknown'].includes(c));
const STACKABLE: Category[] = ['consumable', 'multiplayer', 'upgrade', 'ammo', 'bossSoul'];
const REINFORCEABLE: Category[] = ['weapon', 'shield', 'armor'];

export function AddItemDialog({ onClose }: { onClose: () => void }) {
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

  const add = async (item: ItemDef | null = picked) => {
    if (!item) return;
    const ok = await edit([
      {
        op: 'add',
        id: item.id,
        quantity: STACKABLE.includes(item.cat) ? quantity : 1,
        reinforcement: REINFORCEABLE.includes(item.cat) ? plus : 0,
        infusion: item.cat === 'weapon' || item.cat === 'shield' ? infusion : 0
      }
    ]);
    if (ok) onClose();
  };

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="panel dialog wide" role="dialog" aria-label={t('inv.addTitle')}>
        <div className="panel-head">
          <h2 className="panel-title">{t('inv.addTitle')}</h2>
          <hr className="rule" />
        </div>
        <div style={{ padding: '12px 22px' }}>
          <input className="field" style={{ textAlign: 'left' }} autoFocus placeholder={t('inv.search')} value={query} onChange={(e) => setQuery(e.target.value)} spellCheck={false} />
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
              <button key={i.id} className="row" style={{ padding: '6px 18px' }} aria-selected={picked?.id === i.id} onClick={() => setPicked(i)} onDoubleClick={() => void add(i)}>
                <CategoryIcon category={i.cat} size={16} />
                <span>{i.name}</span>
                {i.slot && <span style={{ marginLeft: 'auto', color: 'var(--dim)', fontSize: 14 }}>{i.slot}</span>}
              </button>
            ))}
          </div>
        </div>
        <div className="add-opts">
          {picked && STACKABLE.includes(picked.cat) && (
            <label>
              {t('inv.quantity')}
              <input className="field" type="number" min={1} max={picked.cat === 'ammo' ? 999 : 99} value={quantity} onChange={(e) => setQuantity(Math.max(1, Math.min(picked.cat === 'ammo' ? 999 : 99, Number(e.target.value) || 1)))} />
            </label>
          )}
          {picked && REINFORCEABLE.includes(picked.cat) && (
            <label>
              {t('inv.reinforcement')}
              <input className="field" type="number" min={0} max={10} value={plus} onChange={(e) => setPlus(Math.max(0, Math.min(10, Number(e.target.value) || 0)))} />
            </label>
          )}
          {picked && (picked.cat === 'weapon' || picked.cat === 'shield') && (
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
        <p className="note" style={{ padding: '0 22px' }}>{t('inv.addWarning')}</p>
        <div className="choices" style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, padding: '16px 22px 20px' }}>
          <button className="btn quiet" onClick={onClose}>
            {t('confirm.no')}
          </button>
          <button className="btn ember" disabled={!picked} onClick={() => void add()}>
            {picked ? `${t('inv.addConfirm')}: ${picked.name}` : t('inv.addConfirm')}
          </button>
        </div>
      </div>
    </div>
  );
}
