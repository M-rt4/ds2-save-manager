import type { CharacterView } from '../api';
import { useStore } from '../store';
import { useT } from '../i18n';
import { ATTRIBUTE_ORDER, COVENANTS, derived, journeyLabel, levelOf, num, playTime } from '../game';
import { Chevron } from './icons';

const MAX_SOULS = 999_999_999;

function tone(was: number, now: number) {
  return now > was ? 'up' : now < was ? 'down' : '';
}

function fmt(n: number) {
  return Number.isInteger(n) ? num(n) : n.toFixed(1);
}

/** One Level Up row: label, the value now, an arrow, and the value after the change. */
function Compare({ label, was, now, children }: { label: React.ReactNode; was: number; now: number; children?: React.ReactNode }) {
  return (
    <div className={`stat${children ? ' editable' : ''}`}>
      <span className="label">{label}</span>
      <span className="was">{fmt(was)}</span>
      <span className="arrow">›</span>
      <span className={`now ${tone(was, now)}`}>{children ?? fmt(now)}</span>
    </div>
  );
}

function Stepper({ value, min, max, onChange, changed }: { value: number; min: number; max: number; onChange: (v: number) => void; changed: boolean }) {
  return (
    <span className="stepper">
      <button tabIndex={-1} disabled={value <= min} onClick={() => onChange(value - 1)} aria-label="-1">
        <Chevron dir="left" size={14} />
      </button>
      <input
        className={`field${changed ? ' changed' : ''}`}
        type="number"
        value={value}
        min={min}
        max={max}
        onChange={(e) => {
          const v = Number(e.target.value);
          if (Number.isFinite(v)) onChange(Math.max(min, Math.min(max, Math.round(v))));
        }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowUp' || e.key === 'ArrowRight') {
            e.preventDefault();
            onChange(Math.min(max, value + 1));
          } else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') {
            e.preventDefault();
            onChange(Math.max(min, value - 1));
          }
        }}
      />
      <button tabIndex={-1} disabled={value >= max} onClick={() => onChange(value + 1)} aria-label="+1">
        <Chevron size={14} />
      </button>
    </span>
  );
}

export function StatusTab({ character: c }: { character: CharacterView }) {
  const t = useT();
  const draft = useStore((s) => s.draft);
  const setDraft = useStore((s) => s.setDraft);
  if (!draft) return null;
  const editable = c.layoutOk;
  const a = draft.attributes;
  const before = derived(c.attributes);
  const after = derived(a);
  const level = levelOf(a);

  return (
    <div className="panel-body">
      <div className="status">
        <div>
          <h3>{t('status.name')}</h3>
          <div className="stat simple editable">
            <input
              className={`field text${draft.name.trim() !== c.name ? ' changed' : ''}`}
              value={draft.name}
              maxLength={16}
              disabled={!editable}
              onChange={(e) => setDraft({ name: e.target.value })}
              spellCheck={false}
            />
          </div>

          <div style={{ height: 14 }} />
          <Compare label={t('status.level')} was={c.level} now={level} />
          <Compare label={t('status.souls')} was={c.souls} now={draft.souls}>
            <input
              className={`field${draft.souls !== c.souls ? ' changed' : ''}`}
              type="number"
              min={0}
              max={MAX_SOULS}
              value={draft.souls}
              disabled={!editable}
              onChange={(e) => setDraft({ souls: Math.max(0, Math.min(MAX_SOULS, Math.round(Number(e.target.value) || 0))) })}
            />
          </Compare>
          <Compare label={<>{t('status.journey')}<small>{journeyLabel(draft.journey)}</small></>} was={c.journey} now={draft.journey}>
            <Stepper value={draft.journey} min={1} max={99} changed={draft.journey !== c.journey} onChange={(journey) => setDraft({ journey })} />
          </Compare>

          <h3>{t('status.attributes')}</h3>
          {ATTRIBUTE_ORDER.map(({ key, label }) => (
            <Compare key={key} label={label} was={c.attributes[key]} now={a[key]}>
              {editable ? (
                <Stepper
                  value={a[key]}
                  min={1}
                  max={99}
                  changed={a[key] !== c.attributes[key]}
                  onChange={(v) => setDraft({ attributes: { ...a, [key]: v } })}
                />
              ) : (
                a[key]
              )}
            </Compare>
          ))}
          <p className="note">{t('status.levelRule')}</p>
        </div>

        <div>
          <h3>{t('status.derived')}</h3>
          <div className="stat simple">
            <span className="label">HP</span>
            <span>{num(c.hp)}</span>
          </div>
          <Compare label="Stamina" was={before.stamina} now={after.stamina} />
          <Compare label="Equip Load" was={before.equipLoad} now={after.equipLoad} />
          <Compare label="Poise" was={before.poise} now={after.poise} />
          <Compare label="Agility" was={before.agility} now={after.agility} />
          <Compare label="Attunement Slots" was={before.slots} now={after.slots} />
          <Compare label="ATK: Str" was={before.atkStr} now={after.atkStr} />
          <Compare label="ATK: Dex" was={before.atkDex} now={after.atkDex} />
          <Compare label="Magic Def" was={before.magicDef} now={after.magicDef} />
          <Compare label="Fire Def" was={before.fireDef} now={after.fireDef} />
          <Compare label="Lightning Def" was={before.lightningDef} now={after.lightningDef} />
          <Compare label="Dark Def" was={before.darkDef} now={after.darkDef} />
          <p className="note">{t('status.derivedNote')}</p>

          <div style={{ height: 18 }} />
          <div className="stat simple">
            <span className="label">{t('status.playTime')}</span>
            <span>{playTime(c.playTime)}</span>
          </div>
          <div className="stat simple">
            <span className="label">{t('status.soulMemory')}</span>
            <span>{num(c.soulMemory)}</span>
          </div>
          <div className="stat simple">
            <span className="label">{t('status.location')}</span>
            <span title={c.mapLabel}>{c.area ?? c.mapLabel}</span>
          </div>
          <div className="stat simple">
            <span className="label">{t('status.deaths')}</span>
            <span>{num(c.deaths)}</span>
          </div>
          <div className="stat simple">
            <span className="label">{t('status.hollowing')}</span>
            <span>{c.hollowing}</span>
          </div>

          <h3>{t('status.covenants')}</h3>
          <div className="cov-list">
            {c.covenants
              .filter((v) => v.discovered)
              .map((v) => (
                <div key={v.id} style={{ display: 'contents' }}>
                  <div className={v.id === c.covenant ? 'active' : ''}>{COVENANTS[v.id]}</div>
                  <div className="r">{v.rank ? t('status.rank', { n: v.rank }) : t('status.discovered')}</div>
                </div>
              ))}
            {!c.covenants.some((v) => v.discovered) && <div style={{ color: 'var(--dim)' }}>{t('status.none')}</div>}
          </div>
        </div>
      </div>
    </div>
  );
}
