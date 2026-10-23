import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { api, rawApi, type Chunk, type EntryInfo, type RawMatches, type SaveView } from '../api';
import { draftEdit, useStore, type Tab } from '../store';
import { useT } from '../i18n';
import { JsonView } from './JsonView';

const ROW = 16; // bytes per row
const ROW_H = 21; // px
const PAGE = 0x10000; // bytes per read, the backend's limit
const OVERSCAN = 8; // rows drawn above and below the visible ones
const CHUNK_HEADER = 32;
const MAX_HEX_EDIT = 256;
/** The zlib-compressed copy of the game's regulation (save::regulation::ENTRY). */
const REGULATION_ENTRY = 21;

type T = ReturnType<typeof useT>;
type NumKind = 'u8' | 'u16' | 'u32' | 'i32' | 'f32';
const NUM_KINDS: NumKind[] = ['u8', 'u16', 'u32', 'i32', 'f32'];
const WIDTH: Record<NumKind, number> = { u8: 1, u16: 2, u32: 4, i32: 4, f32: 4 };

// The tab Raw data was entered from, so Cancel on its warning goes back there.
let previousTab: Tab = 'status';
useStore.subscribe((s, prev) => {
  if (s.tab === 'raw' && prev.tab !== 'raw') previousTab = prev.tab;
});

// Bytes changed here since the save was last written: offset -> the value it had before,
// per `${doc id}#${block}`. Kept outside the component so leaving the tab keeps them.
const changed = new Map<string, Map<number, number | undefined>>();

const hex = (n: number, width = 2) => n.toString(16).toUpperCase().padStart(width, '0');

function blockLabel(e: EntryInfo, doc: SaveView, t: T): string {
  const i = e.index;
  if (i === 0) return t('raw.block.menu');
  if (i <= 10) {
    const s = doc.slots[i - 1];
    return t('raw.block.character', { n: i }) + (s?.occupied ? ` · ${s.name}` : '');
  }
  if (i <= 20) return t('raw.block.world', { n: i - 10 });
  if (i === REGULATION_ENTRY) return t('raw.block.regulation');
  if (i === 22) return t('raw.block.titles');
  return e.name;
}

/** `4E 00 65`, `4e0065` or `0x4E 0x00`; a lone digit is one byte. */
function parseHexBytes(s: string): number[] | null {
  const out: number[] = [];
  for (let tok of s.trim().split(/[\s,]+/)) {
    tok = tok.replace(/^0x/i, '');
    if (!tok) continue;
    if (/[^0-9a-f]/i.test(tok) || (tok.length > 1 && tok.length % 2)) return null;
    for (const pair of tok.length === 1 ? [tok] : tok.match(/../g)!) out.push(parseInt(pair, 16));
  }
  return out.length ? out : null;
}

/** `0x3C0` and `3C0h` are hex, plain digits decimal, anything with a-f hex. */
function parseOffset(s: string): number | null {
  const v = s.trim();
  if (/^0x[0-9a-f]+$/i.test(v)) return parseInt(v.slice(2), 16);
  if (/^[0-9]+$/.test(v)) return parseInt(v, 10);
  if (/^[0-9a-f]+h?$/i.test(v)) return parseInt(v.replace(/h$/i, ''), 16);
  return null;
}

const utf16 = (text: string) => [...text].flatMap((ch) => {
  const out: number[] = [];
  for (let i = 0; i < ch.length; i++) out.push(ch.charCodeAt(i) & 0xff, ch.charCodeAt(i) >> 8);
  return out;
});

function encodeNumber(kind: NumKind, text: string): number[] | null {
  const v = Number(text.trim());
  if (!text.trim() || !Number.isFinite(v)) return null;
  const dv = new DataView(new ArrayBuffer(4));
  const int = Number.isInteger(v);
  if (kind === 'u8' && int && v >= 0 && v <= 0xff) dv.setUint8(0, v);
  else if (kind === 'u16' && int && v >= 0 && v <= 0xffff) dv.setUint16(0, v, true);
  else if (kind === 'u32' && int && v >= 0 && v <= 0xffffffff) dv.setUint32(0, v, true);
  else if (kind === 'i32' && int && v >= -0x80000000 && v <= 0x7fffffff) dv.setInt32(0, v, true);
  else if (kind === 'f32') dv.setFloat32(0, v, true);
  else return null;
  return [...new Uint8Array(dv.buffer, 0, WIDTH[kind])];
}

/** The chunk an offset falls in; chunks tile the block in order. */
function chunkAt(chunks: Chunk[], o: number): Chunk | undefined {
  let lo = 0;
  let hi = chunks.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const c = chunks[mid];
    if (o < c.offset) hi = mid - 1;
    else if (o >= c.offset + CHUNK_HEADER + c.size) lo = mid + 1;
    else return c;
  }
  return undefined;
}

export function RawTab({ doc }: { doc: SaveView }) {
  const t = useT();
  const [accepted, setAccepted] = useState(false);
  const [view, setView] = useState(lastView);
  const setTab = useStore((s) => s.setTab);
  if (!accepted) {
    return (
      <>
        <div className="panel-body" />
        <RawWarning onAccept={() => setAccepted(true)} onCancel={() => setTab(previousTab)} />
      </>
    );
  }
  const pick = (v: typeof view) => setView((lastView = v));
  return (
    <div className="raw-shell">
      <div className="raw-views seg" role="group">
        <button aria-pressed={view === 'json'} onClick={() => pick('json')}>
          {t('json.viewReadable')}
        </button>
        <button aria-pressed={view === 'bytes'} onClick={() => pick('bytes')}>
          {t('json.viewBytes')}
        </button>
      </div>
      {view === 'json' ? <JsonView doc={doc} /> : <RawEditor doc={doc} />}
    </div>
  );
}

/** The view last picked, kept while the app runs; the readable one comes first. */
let lastView: 'json' | 'bytes' = 'json';

/** Shown every time the tab opens; there is no "don't show again". */
function RawWarning({ onAccept, onCancel }: { onAccept: () => void; onCancel: () => void }) {
  const t = useT();
  const [yes, setYes] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
      else if (e.key === 'Enter') (yes ? onAccept : onCancel)();
      else if (['ArrowLeft', 'ArrowRight', 'Tab'].includes(e.key)) {
        e.preventDefault();
        setYes((v) => !v);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="panel dialog wide raw-warning" role="alertdialog" aria-labelledby="raw-warning-title">
        <div className="panel-head">
          <h2 id="raw-warning-title" className="panel-title">
            {t('raw.warnTitle')}
          </h2>
          <hr className="rule" />
        </div>
        <ul>
          <li>{t('raw.warnDirect')}</li>
          <li>{t('raw.warnBreak')}</li>
          <li>{t('raw.warnOnline')}</li>
          <li>{t('raw.warnChecksum')}</li>
          <li>{t('raw.warnRegulation')}</li>
          <li>{t('raw.warnBackup')}</li>
        </ul>
        <div className="choices">
          <button className="row" aria-selected={yes} onMouseEnter={() => setYes(true)} onClick={onAccept}>
            {t('raw.warnAccept')}
          </button>
          <button className="row" aria-selected={!yes} onMouseEnter={() => setYes(false)} onClick={onCancel}>
            {t('raw.warnCancel')}
          </button>
        </div>
      </div>
    </div>
  );
}

function RawEditor({ doc }: { doc: SaveView }) {
  const t = useT();
  const toast = useStore((s) => s.toast);
  const fail = useStore((s) => s.fail);
  const setDoc = useStore((s) => s.setDoc);
  const selectSlot = useStore((s) => s.selectSlot);
  const [index, setIndex] = useState(() => {
    const slot = useStore.getState().slot;
    return slot !== null ? 1 + slot : 0;
  });
  const entry = doc.entries.find((e) => e.index === index) ?? doc.entries[0];
  const size = entry?.size ?? 0;
  const chunks = entry?.chunks ?? [];
  const key = `${doc.id}#${index}`;

  // Once the save is written or reloaded, nothing here differs from the file any more.
  if (!doc.dirty) for (const k of [...changed.keys()]) if (k.startsWith(`${doc.id}#`)) changed.delete(k);
  const changedHere = changed.get(key);

  // Pages of the block as read for this version of the doc; any change to it reads again.
  const cache = useMemo(() => ({ pages: new Map<number, Uint8Array>(), loading: new Set<number>() }), [doc, index]);
  const [, setLoaded] = useState(0);
  const byteAt = (o: number): number | undefined => (o < size ? cache.pages.get(Math.floor(o / PAGE))?.[o % PAGE] : undefined);

  const scroller = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewH, setViewH] = useState(400);
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setViewH(el.clientHeight));
    ro.observe(el);
    setViewH(el.clientHeight);
    return () => ro.disconnect();
  }, []);

  const rows = Math.ceil(size / ROW);
  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN);
  const last = Math.min(rows - 1, Math.ceil((scrollTop + viewH) / ROW_H) + OVERSCAN);

  useEffect(() => {
    if (rows === 0) return;
    const p1 = Math.floor((Math.min(size, (last + 1) * ROW) - 1) / PAGE);
    for (let p = Math.floor((first * ROW) / PAGE); p <= p1; p++) {
      if (cache.pages.has(p) || cache.loading.has(p)) continue;
      cache.loading.add(p);
      rawApi
        .read(doc.id, index, p * PAGE, PAGE)
        .then((bytes) => {
          cache.pages.set(p, bytes);
          setLoaded((n) => n + 1);
        })
        .catch(fail)
        .finally(() => cache.loading.delete(p));
    }
  }, [cache, first, last, rows, size, doc.id, index, fail]);

  const [sel, setSel] = useState<{ anchor: number; focus: number } | null>(null);
  const selStart = sel ? Math.min(sel.anchor, sel.focus) : -1;
  const selEnd = sel ? Math.max(sel.anchor, sel.focus) : -2;
  const selLen = selEnd - selStart + 1;

  const ensureVisible = (o: number, center = false) => {
    const el = scroller.current;
    if (!el) return;
    const y = Math.floor(o / ROW) * ROW_H;
    if (center) el.scrollTop = y - el.clientHeight / 2 + ROW_H;
    else if (y < el.scrollTop) el.scrollTop = y;
    else if (y + ROW_H > el.scrollTop + el.clientHeight) el.scrollTop = y + ROW_H - el.clientHeight;
  };

  // A jump into another block lands once that block is laid out.
  const pending = useRef<{ offset: number; length: number } | null>(null);
  useLayoutEffect(() => {
    const p = pending.current;
    pending.current = null;
    if (p) {
      setSel({ anchor: p.offset, focus: p.offset + p.length - 1 });
      ensureVisible(p.offset, true);
    } else {
      setSel(null);
      if (scroller.current) scroller.current.scrollTop = 0;
    }
  }, [index]);

  const jump = (i: number, offset: number, length = 1) => {
    if (i === index) {
      setSel({ anchor: offset, focus: offset + Math.max(1, length) - 1 });
      ensureVisible(offset, true);
    } else {
      pending.current = { offset, length: Math.max(1, length) };
      setIndex(i);
    }
  };

  const dragging = useRef(false);
  useEffect(() => {
    const up = () => (dragging.current = false);
    window.addEventListener('mouseup', up);
    return () => window.removeEventListener('mouseup', up);
  }, []);
  const offsetOf = (e: React.MouseEvent) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('[data-o]');
    return el ? Number(el.dataset.o) : null;
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (!sel) return;
    const page = Math.max(1, Math.floor(viewH / ROW_H) - 1) * ROW;
    const step: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -ROW, ArrowDown: ROW, PageUp: -page, PageDown: page };
    if (!(e.key in step)) return;
    e.preventDefault();
    const focus = Math.min(size - 1, Math.max(0, sel.focus + step[e.key]));
    setSel(e.shiftKey ? { anchor: sel.anchor, focus } : { anchor: focus, focus });
    ensureVisible(focus);
  };

  const [busy, setBusy] = useState(false);
  const write = async (offset: number, bytes: number[]) => {
    if (offset < 0 || offset + bytes.length > size) {
      fail({ code: 'rawOutOfRange', message: '' });
      return;
    }
    setBusy(true);
    try {
      const before = bytes.map((_, i) => byteAt(offset + i));
      const st = useStore.getState();
      // Re-reading the character below resets the status draft, so fold it in first.
      if (st.character && st.draft && st.slot !== null) {
        const edit = draftEdit(st.character, st.draft);
        if (edit) await api.editCharacter(doc.id, st.slot, edit);
      }
      if (await rawApi.write(doc.id, index, offset, bytes)) {
        const map = changed.get(key) ?? new Map<number, number | undefined>();
        bytes.forEach((b, i) => {
          const o = offset + i;
          if (!map.has(o)) map.set(o, before[i]);
          if (map.get(o) === b) map.delete(o);
        });
        changed.set(key, map);
      }
      setDoc(await api.getSave(doc.id));
      if (st.slot !== null) await selectSlot(st.slot);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  // ---- Search and go to ----
  const [gotoText, setGotoText] = useState('');
  const [query, setQuery] = useState('');
  const [mode, setMode] = useState<'text' | 'hex'>('text');
  const [scopeAll, setScopeAll] = useState(false);
  const [found, setFound] = useState<{ res: RawMatches; len: number; i: number } | null>(null);

  const goto = () => {
    const o = parseOffset(gotoText);
    if (o === null || o >= size) toast(t('raw.badOffset', { at: gotoText.trim() }), 'error');
    else jump(index, o);
  };

  const search = async () => {
    const pattern = mode === 'hex' ? parseHexBytes(query) : query ? utf16(query) : null;
    if (!pattern) {
      if (mode === 'hex') toast(t('raw.badHex'), 'error');
      return;
    }
    try {
      const res = await rawApi.find(doc.id, scopeAll ? null : index, pattern);
      setFound({ res, len: pattern.length, i: 0 });
      const m = res.matches[0];
      if (m) jump(m.index, m.offset, pattern.length);
    } catch (e) {
      fail(e);
    }
  };

  const step = (d: number) => {
    if (!found?.res.matches.length) return;
    const n = found.res.matches.length;
    const i = (found.i + d + n) % n;
    setFound({ ...found, i });
    const m = found.res.matches[i];
    jump(m.index, m.offset, found.len);
  };

  // ---- Hex rows (only the visible ones) ----
  const digits = Math.max(4, Math.max(0, size - 1).toString(16).length);
  const lines = [];
  for (let r = first; r <= last; r++) {
    const base = r * ROW;
    const cells = [];
    const chars = [];
    for (let i = 0; i < ROW && base + i < size; i++) {
      const o = base + i;
      const v = byteAt(o);
      const c = chunkAt(chunks, o);
      let cls = 'b';
      if (v === 0) cls += ' z';
      if (c && o < c.offset + CHUNK_HEADER) cls += ' hd';
      if (c && o === c.offset) cls += ' cs';
      if (changedHere?.has(o)) cls += ' chg';
      if (o >= selStart && o <= selEnd) cls += ' sel';
      cells.push(
        <span key={i} className={cls} data-o={o}>
          {v === undefined ? '··' : hex(v)}
        </span>
      );
      chars.push(
        <span key={i} className={cls} data-o={o}>
          {v === undefined ? ' ' : v >= 0x20 && v < 0x7f ? String.fromCharCode(v) : '.'}
        </span>
      );
    }
    lines.push(
      <div key={r} className="hx-row" style={{ top: r * ROW_H }}>
        <span className="hx-off">{hex(base, digits)}</span>
        <span className="hx-bytes">{cells}</span>
        <span className="hx-ascii">{chars}</span>
      </div>
    );
  }

  // ---- Inspector ----
  const at = sel ? selStart : null;
  const look = (n: number) => (at === null ? [] : Array.from({ length: n }, (_, i) => byteAt(at + i)));
  const numbers: Record<NumKind, string | null> = { u8: null, u16: null, u32: null, i32: null, f32: null };
  if (at !== null) {
    for (const kind of NUM_KINDS) {
      const bytes = look(WIDTH[kind]);
      if (bytes.some((b) => b === undefined)) continue;
      const dv = new DataView(new Uint8Array(bytes as number[]).buffer);
      numbers[kind] =
        kind === 'u8'
          ? String(dv.getUint8(0))
          : kind === 'u16'
            ? String(dv.getUint16(0, true))
            : kind === 'u32'
              ? String(dv.getUint32(0, true))
              : kind === 'i32'
                ? String(dv.getInt32(0, true))
                : String(Number(dv.getFloat32(0, true).toPrecision(7)));
    }
  }
  let text = '';
  if (at !== null) {
    for (let i = 0; i < 64; i++) {
      const [lo, hi] = [byteAt(at + 2 * i), byteAt(at + 2 * i + 1)];
      if (lo === undefined || hi === undefined || (lo | (hi << 8)) === 0) break;
      const code = lo | (hi << 8);
      text += code < 0x20 ? '·' : String.fromCharCode(code);
    }
  }
  const selBytes = at !== null && selLen <= MAX_HEX_EDIT ? look(selLen) : null;
  const selHex = selBytes && selBytes.every((b) => b !== undefined) ? selBytes.map((b) => hex(b!)).join(' ') : null;
  const where = at !== null ? chunkAt(chunks, at) : undefined;
  const kb =(n: number) => (n < 10_240 ? `${n} B` : `${Math.round(n / 1024)} KB`);

  return (
    <div className="panel-body raw">
      <aside className="raw-side">
        <div className="raw-blocks scroll" role="listbox" aria-label={t('raw.tab')}>
          {doc.entries.map((e) => (
            <button
              key={e.index}
              className="row raw-block"
              role="option"
              aria-selected={e.index === index}
              title={`${e.name} · ${kb(e.size)}`}
              onClick={() => e.index !== index && setIndex(e.index)}
            >
              <span className="n">{e.index}</span>
              <span className="nm">{blockLabel(e, doc, t)}</span>
            </button>
          ))}
        </div>
        <div className="raw-chunks scroll">
          <div className="group-label">
            {t('raw.chunks')} · {kb(size)}
          </div>
          {chunks.length === 0 ? (
            <p className="note">{t('raw.noChunks')}</p>
          ) : (
            <table className="blocks raw-chunk-table">
              <thead>
                <tr>
                  <th>{t('raw.col.type')}</th>
                  <th>{t('raw.col.version')}</th>
                  <th>{t('raw.col.offset')}</th>
                  <th>{t('raw.col.size')}</th>
                </tr>
              </thead>
              <tbody>
                {chunks.map((c) => (
                  <tr key={c.offset} className={where === c ? 'cur' : ''} tabIndex={-1} data-pad onClick={() => jump(index, c.offset)}>
                    <td>{c.kind.toString(16).toUpperCase()}</td>
                    <td>{c.version.toString(16).toUpperCase()}</td>
                    <td>{hex(c.offset, 4)}</td>
                    <td>{c.size}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </aside>

      <section className="raw-main">
        <div className="raw-tools">
          <input
            className="field text mono raw-goto"
            placeholder={t('raw.goto')}
            aria-label={t('raw.goto')}
            value={gotoText}
            spellCheck={false}
            onChange={(e) => setGotoText(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && goto()}
          />
          <div className="seg" role="group">
            {(['text', 'hex'] as const).map((m) => (
              <button key={m} aria-pressed={mode === m} onClick={() => setMode(m)}>
                {m === 'hex' ? t('raw.modeHex') : t('raw.modeText')}
              </button>
            ))}
          </div>
          <input
            className={`field text raw-query${mode === 'hex' ? ' mono' : ''}`}
            placeholder={t('raw.search')}
            aria-label={t('raw.search')}
            value={query}
            spellCheck={false}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void search()}
          />
          <div className="seg" role="group">
            <button aria-pressed={!scopeAll} onClick={() => setScopeAll(false)}>
              {t('raw.scopeBlock')}
            </button>
            <button aria-pressed={scopeAll} onClick={() => setScopeAll(true)}>
              {t('raw.scopeAll')}
            </button>
          </div>
          {found &&
            (found.res.matches.length === 0 ? (
              <span className="raw-found">{t('raw.noMatch')}</span>
            ) : (
              <span className="raw-found">
                <button className="btn quiet" onClick={() => step(-1)} aria-label="‹">
                  ‹
                </button>
                {t(found.res.more ? 'raw.matchMore' : 'raw.matchCount', { i: found.i + 1, n: found.res.matches.length })}
                <button className="btn quiet" onClick={() => step(1)} aria-label="›">
                  ›
                </button>
              </span>
            ))}
        </div>

        {index === REGULATION_ENTRY && <div className="warn raw-note">{t('raw.compressedNote')}</div>}

        <div
          className="hex scroll"
          ref={scroller}
          tabIndex={0}
          onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
          onKeyDown={onKey}
          onMouseDown={(e) => {
            const o = offsetOf(e);
            if (o === null || e.button !== 0) return;
            e.preventDefault();
            scroller.current?.focus();
            dragging.current = true;
            setSel((s) => (e.shiftKey && s ? { anchor: s.anchor, focus: o } : { anchor: o, focus: o }));
          }}
          onMouseOver={(e) => {
            const o = dragging.current ? offsetOf(e) : null;
            if (o !== null) setSel((s) => s && { anchor: s.anchor, focus: o });
          }}
        >
          <div className="hex-space" style={{ height: rows * ROW_H }}>
            {lines}
          </div>
        </div>

        <div className="raw-inspector">
          {at === null ? (
            <p className="note">{t('raw.pick')}</p>
          ) : (
            <>
              <div className="raw-where">
                <span className="mono">
                  {t('raw.offset')} 0x{hex(at, digits)} · {at}
                </span>
                {where && (
                  <span className="mono">
                    {at < where.offset + CHUNK_HEADER
                      ? t('raw.inHeader', { kind: where.kind.toString(16).toUpperCase() })
                      : t('raw.inChunk', {
                          kind: where.kind.toString(16).toUpperCase(),
                          at: `0x${hex(at - where.offset - CHUNK_HEADER, 1)}`
                        })}
                  </span>
                )}
                {selLen > 1 && <span>{t('raw.selected', { n: selLen })}</span>}
              </div>
              <div className="raw-nums">
                {NUM_KINDS.map((kind) => (
                  <NumField
                    key={kind}
                    label={kind}
                    value={numbers[kind]}
                    disabled={busy}
                    onCommit={(v) => {
                      const bytes = encodeNumber(kind, v);
                      if (!bytes) toast(t('raw.badValue', { type: kind }), 'error');
                      else void write(at, bytes);
                    }}
                  />
                ))}
                <label className="raw-num wide">
                  <span>UTF-16</span>
                  <input className="field text" value={text} readOnly tabIndex={-1} />
                </label>
              </div>
              {selHex === null && selLen > MAX_HEX_EDIT ? (
                <p className="note">{t('raw.tooLong')}</p>
              ) : (
                <HexField
                  value={selHex}
                  disabled={busy}
                  onCommit={(v) => {
                    const bytes = parseHexBytes(v);
                    if (!bytes) toast(t('raw.badHex'), 'error');
                    else void write(at, bytes);
                  }}
                />
              )}
            </>
          )}
          <p className="note">{t('raw.memoryNote')}</p>
        </div>
      </section>
    </div>
  );
}

/** A value that is written when Enter is pressed; leaving the field throws the edit away. */
function useCommitted(value: string | null) {
  const [text, setText] = useState(value ?? '');
  useEffect(() => setText(value ?? ''), [value]);
  return [text, setText, text !== (value ?? '')] as const;
}

function NumField({
  label,
  value,
  disabled,
  onCommit
}: {
  label: string;
  value: string | null;
  disabled: boolean;
  onCommit: (text: string) => void;
}) {
  const [text, setText, edited] = useCommitted(value);
  return (
    <label className="raw-num">
      <span>{label}</span>
      <input
        className={`field mono${edited ? ' changed' : ''}`}
        value={text}
        disabled={disabled || value === null}
        spellCheck={false}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && edited) onCommit(text);
          if (e.key === 'Escape') setText(value ?? '');
        }}
        onBlur={() => setText(value ?? '')}
      />
    </label>
  );
}

function HexField({ value, disabled, onCommit }: { value: string | null; disabled: boolean; onCommit: (text: string) => void }) {
  const t = useT();
  const [text, setText, edited] = useCommitted(value);
  return (
    <div className="raw-hexedit">
      <label className="raw-num wide">
        <span>{t('raw.bytes')}</span>
        <input
          className={`field text mono${edited ? ' changed' : ''}`}
          value={text}
          disabled={disabled || value === null}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && edited) onCommit(text);
            if (e.key === 'Escape') setText(value ?? '');
          }}
        />
      </label>
      <button className="btn" disabled={disabled || !edited} onClick={() => onCommit(text)}>
        {t('raw.apply')}
      </button>
    </div>
  );
}
