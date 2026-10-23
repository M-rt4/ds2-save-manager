import { useEffect, useState } from 'react';
import { useT, type Key } from '../i18n';

// What the game's item screen shows, from src/public/item_stats.json (scripts/build-item-stats.py).
// Upgrade levels are interpolated between +0 and the highest level and rounded down, as the
// game does. Stat names are the game's own and stay in English in both languages.

interface WeaponStats {
  type: string | null;
  hit: ('strike' | 'slash' | 'thrust')[];
  req: [number, number, number, number];
  lv: number;
  dmg: [number[], number[]];
  aux: [number[], number[]];
  /** Per level: the STR, DEX, Magic, Fire, Lightning and Dark bonus letters, '-' for none. */
  sc: string[];
  abs: number[];
  res: number[];
  stab: number | number[];
  counter: number;
  poise: number;
  cast: number;
  wt: number;
  dur: number;
}

interface ArmorStats {
  slot: number;
  req: [number, number, number, number];
  lv: number;
  /** Physical, Strike, Slash, Thrust, Magic, Fire, Lightning, Dark, Poison, Bleed, Petrify, Curse. */
  def: [number[], number[]];
  poise: number;
  wt: number;
  dur: number;
}

export interface ItemStats {
  desc?: string;
  effect?: string;
  weapon?: WeaponStats;
  armor?: ArmorStats;
  ring?: { wt: number; dur: number };
  spell?: { req: [number, number]; slots: number; casts: number[] };
  ammo?: { dmg: number[]; aux: [number, number] };
}

let table: Promise<Record<string, ItemStats>> | null = null;

function loadStats() {
  table ??= fetch('/item_stats.json')
    .then((r) => (r.ok ? (r.json() as Promise<Record<string, ItemStats>>) : {}))
    .catch(() => ({}));
  return table;
}

/** The item's stats; undefined while the table loads, null when the game has none for it. */
export function useItemStats(id: number): ItemStats | null | undefined {
  const [stats, setStats] = useState<ItemStats | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    setStats(undefined);
    void loadStats().then((t) => live && setStats(t[String(id)] ?? null));
    return () => {
      live = false;
    };
  }, [id]);
  return stats;
}

const DAMAGE = ['Physical', 'Magic', 'Fire', 'Lightning', 'Dark'];
const AUX = ['Poison', 'Bleed'];
const RESIST = ['Poison', 'Bleed', 'Petrify', 'Curse'];
const DEFENSE = ['Physical', 'Strike', 'Slash', 'Thrust', 'Magic', 'Fire', 'Lightning', 'Dark'];
const BONUS = ['Str', 'Dex', 'Mag', 'Fir', 'Lgt', 'Drk'];
const ATTUNEMENT = [10, 15, 26, 32, 38, 43, 49, 58, 79, 94];
const HIT = { strike: 'Strike', slash: 'Slash', thrust: 'Thrust' };

/** A value at upgrade level n: linear from +0 to the top level, rounded down. */
function at(pair: [number[], number[]], i: number, n: number, top: number): number {
  const [a, b] = [pair[0][i], pair[1][i]];
  return Math.floor(a + (top > 0 ? ((b - a) * Math.min(n, top)) / top : 0) + 1e-6);
}

const fmt = (v: number) => (Number.isInteger(v) ? String(v) : v.toFixed(1));
/** Weights as the game prints them: one decimal, two when the value needs it. */
const weight = (v: number) => v.toFixed(Number.isInteger(Math.round(v * 100) / 10) ? 1 : 2);

function Row({ label, value, dim }: { label: string; value: string | number; dim?: boolean }) {
  return (
    <div className={dim ? 'stats-row zero' : 'stats-row'}>
      <span>{label}</span>
      <b>{value}</b>
    </div>
  );
}

function Section({ title, full, children }: { title: Key; full?: boolean; children: React.ReactNode }) {
  const t = useT();
  return (
    <div className={full ? 'stats-sec full' : 'stats-sec'}>
      <h4>{t(title)}</h4>
      {children}
    </div>
  );
}

/** Required attributes, and for weapons the six bonus letters, as two small tables. */
function Requirements({ req, labels, bonus }: { req: number[]; labels: string[]; bonus?: string }) {
  const t = useT();
  return (
    <div className="stats-reqs">
      <div className="stats-req">
        {labels.map((l, i) => (
          <div key={l}>
            <small>{l}</small>
            <b>{req[i] > 0 ? req[i] : '–'}</b>
          </div>
        ))}
      </div>
      {bonus && (
        <div className="stats-req bonus" title={t('stats.bonus')}>
          {BONUS.map((l, i) => (
            <div key={l}>
              <small>{l}</small>
              <b>{bonus[i] === '-' ? '–' : bonus[i]}</b>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Weapon({ w, plus, durability }: { w: WeaponStats; plus: number; durability?: number | null }) {
  const n = Math.min(plus, w.lv);
  const stability = Array.isArray(w.stab) ? w.stab[Math.min(n, w.stab.length - 1)] : w.stab;
  const aux = AUX.map((_, i) => Math.floor(at(w.aux, i, n, w.lv) / 5));
  return (
    <>
      <Section title="stats.attack">
        {DAMAGE.map((d, i) => {
          const v = at(w.dmg, i, n, w.lv);
          return <Row key={d} label={d} value={v} dim={v === 0} />;
        })}
        {AUX.map((d, i) => aux[i] > 0 && <Row key={d} label={d} value={aux[i]} />)}
        <Row label="Counter Strength" value={w.counter} />
        <Row label="Poise Damage" value={w.poise} />
        {w.cast > 0 && <Row label="Casting Speed" value={w.cast} />}
      </Section>
      <Section title="stats.reduction">
        {DAMAGE.map((d, i) => (
          <Row key={d} label={d} value={fmt(w.abs[i])} />
        ))}
        {RESIST.map((d, i) => (
          <Row key={d} label={d} value={fmt(w.res[i])} />
        ))}
        <Row label="Stability" value={stability} />
      </Section>
      <Section title="stats.requirements" full>
        <Requirements req={w.req} labels={['Str', 'Dex', 'Int', 'Fth']} bonus={w.sc[Math.min(n, w.sc.length - 1)]} />
      </Section>
      <Section title="stats.equipment">
        <Row label="Durability" value={durability != null && durability > 0 ? `${Math.round(durability)}/${w.dur}` : w.dur} />
        <Row label="Weight" value={weight(w.wt)} />
      </Section>
    </>
  );
}

function Armor({ a, plus, durability }: { a: ArmorStats; plus: number; durability?: number | null }) {
  const n = Math.min(plus, a.lv);
  const needs = a.req.some((r) => r > 0);
  return (
    <>
      <Section title="stats.defense">
        {DEFENSE.map((d, i) => (
          <Row key={d} label={d} value={at(a.def, i, n, a.lv)} />
        ))}
      </Section>
      <Section title="stats.resistance">
        {RESIST.map((d, i) => (
          <Row key={d} label={d} value={at(a.def, 8 + i, n, a.lv)} />
        ))}
        <Row label="Poise" value={fmt(a.poise)} />
        <Row label="Durability" value={durability != null && durability > 0 ? `${Math.round(durability)}/${a.dur}` : a.dur} />
        <Row label="Weight" value={weight(a.wt)} />
      </Section>
      {needs && (
        <Section title="stats.requirements">
          <Requirements req={a.req} labels={['Str', 'Dex', 'Int', 'Fth']} />
        </Section>
      )}
    </>
  );
}

export function ItemDetail({
  id,
  plus = 0,
  infused = false,
  durability,
  wide = false
}: {
  id: number;
  plus?: number;
  infused?: boolean;
  durability?: number | null;
  /** Sections side by side, for the inventory list; otherwise one column. */
  wide?: boolean;
}) {
  const t = useT();
  const s = useItemStats(id);
  if (s === undefined) return <div className={wide ? 'stats wide' : 'stats'} aria-busy />;
  if (s === null) return <p className="note stats-none">{t('stats.none')}</p>;
  const w = s.weapon;
  const kind = w && [w.type, w.hit.map((h) => HIT[h]).join(' / ')].filter(Boolean).join(' · ');

  return (
    <div className={wide ? 'stats wide' : 'stats'}>
      {kind && <div className="stats-type">{kind}</div>}
      {w && <Weapon w={w} plus={plus} durability={durability} />}
      {s.armor && <Armor a={s.armor} plus={plus} durability={durability} />}
      {s.ring && (
        <Section title="stats.equipment">
          <Row label="Durability" value={durability != null && durability > 0 ? `${Math.round(durability)}/${s.ring.dur}` : s.ring.dur} />
          <Row label="Weight" value={weight(s.ring.wt)} />
        </Section>
      )}
      {s.spell && (
        <>
          <Section title="stats.spell">
            <Row label="Uses" value={s.spell.casts[0]} />
            <Row label="Attunement Slots" value={s.spell.slots} />
            <Requirements req={s.spell.req} labels={['Int', 'Fth']} />
          </Section>
          <Section title="stats.uses">
            <div className="stats-uses">
              {ATTUNEMENT.map((a, i) => (
                <div key={a}>
                  <small>{a}</small>
                  <b>{s.spell!.casts[i]}</b>
                </div>
              ))}
            </div>
          </Section>
        </>
      )}
      {s.ammo && (
        <Section title="stats.attack">
          {DAMAGE.map((d, i) => (
            <Row key={d} label={d} value={s.ammo!.dmg[i]} dim={s.ammo!.dmg[i] === 0} />
          ))}
        </Section>
      )}
      {s.effect && <p className="stats-effect">{s.effect}</p>}
      {infused && (w || s.armor) && <p className="note stats-note">{t('stats.infusion')}</p>}
      {s.desc && <p className="stats-desc">{s.desc}</p>}
    </div>
  );
}
