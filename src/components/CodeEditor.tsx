import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';

const LINE_H = 19;
const PAD_Y = 6;
const PAD_X = 10;
const OVERSCAN = 12;

/** Lines of every `{…}` / `[…]` that spans more than one, keyed by the line it opens on. */
function foldRanges(text: string): Map<number, number> {
  const ranges = new Map<number, number>();
  const stack: number[] = [];
  let line = 0;
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (c === 10) {
      line++;
    } else if (inString) {
      if (c === 92) i++;
      else if (c === 34) inString = false;
    } else if (c === 34) {
      inString = true;
    } else if (c === 123 || c === 91) {
      stack.push(line);
    } else if (c === 125 || c === 93) {
      const start = stack.pop();
      if (start !== undefined && start < line) ranges.set(start, line);
    }
  }
  return ranges;
}

/** A stretch of the shown text that is the same stretch of the full text. */
interface Run {
  v: number;
  m: number;
  len: number;
}

/** Where a folded range stands in the shown text, as `⋯ n ⋯`. */
interface Hole {
  v0: number;
  v1: number;
  line: number;
}

interface Shown {
  text: string;
  runs: Run[];
  holes: Hole[];
  /** The full text's line of each shown line. */
  lineOf: number[];
}

function show(lines: string[], starts: number[], folds: Set<number>, ranges: Map<number, number>): Shown {
  const text = lines.join('\n');
  if (!folds.size) return { text, runs: [{ v: 0, m: 0, len: text.length }], holes: [], lineOf: lines.map((_, i) => i) };
  let out = '';
  const runs: Run[] = [];
  const holes: Hole[] = [];
  const lineOf: number[] = [];
  const nl = (k: number) => (k < lines.length - 1 ? '\n' : '');
  for (let i = 0; i < lines.length; ) {
    const end = folds.has(i) ? ranges.get(i) : undefined;
    lineOf.push(i);
    if (end === undefined) {
      runs.push({ v: out.length, m: starts[i], len: lines[i].length + nl(i).length });
      out += lines[i] + nl(i);
      i++;
      continue;
    }
    runs.push({ v: out.length, m: starts[i], len: lines[i].length });
    out += lines[i];
    const mark = `⋯ ${end - i - 1} ⋯`;
    holes.push({ v0: out.length, v1: out.length + mark.length, line: i });
    out += mark;
    const tail = lines[end].trimStart();
    runs.push({ v: out.length, m: starts[end] + lines[end].length - tail.length, len: tail.length + nl(end).length });
    out += tail + nl(end);
    i = end + 1;
  }
  return { text: out, runs, holes, lineOf };
}

function toFull(s: Shown, at: number): number {
  for (const r of s.runs) if (at >= r.v && at <= r.v + r.len) return r.m + (at - r.v);
  return s.runs.at(-1)!.m + s.runs.at(-1)!.len;
}

/** Where a place in the full text shows; a hidden place shows at its fold mark. */
function toShown(s: Shown, at: number): number {
  let best = 0;
  for (const r of s.runs) {
    if (at >= r.m && at <= r.m + r.len) return r.v + (at - r.m);
    if (r.m <= at) best = r.v + r.len;
  }
  return best;
}

const TOKEN = /("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|\b(true|false|null)\b|(⋯ \d+ ⋯)|([{}[\],])/g;

function highlight(line: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let k = 0;
  TOKEN.lastIndex = 0;
  for (let m = TOKEN.exec(line); m; m = TOKEN.exec(line)) {
    if (m.index > last) out.push(line.slice(last, m.index));
    const cls = m[1] ? (m[2] ? 'k' : 's') : m[3] ? 'n' : m[4] ? 'lit' : m[5] ? 'ph' : 'p';
    out.push(
      <span key={k++} className={cls}>
        {m[1] ?? m[0]}
      </span>
    );
    if (m[2]) out.push(m[2]);
    last = m.index + m[0].length;
  }
  if (last < line.length) out.push(line.slice(last));
  return out;
}

export interface CodeMark {
  /** 0-based line of the full text. */
  line: number;
  message: string;
}

/**
 * A plain-text editor for JSON: the text in a textarea, its colours on a layer under it,
 * line numbers, and folding. A folded range shows as `⋯ n ⋯`; typing into that mark opens
 * it again rather than editing what it hides.
 */
export function CodeEditor({
  value,
  onChange,
  marks,
  jump,
  label
}: {
  value: string;
  onChange: (text: string) => void;
  marks: CodeMark[];
  /** A line to bring into view; `seq` makes asking twice for one line count. */
  jump: { line: number; seq: number } | null;
  label: string;
}) {
  const area = useRef<HTMLTextAreaElement>(null);
  const [folds, setFolds] = useState<Set<number>>(new Set());
  const [scroll, setScroll] = useState({ top: 0, left: 0 });
  const [height, setHeight] = useState(400);
  const caret = useRef<number | null>(null);

  const lines = useMemo(() => value.split('\n'), [value]);
  const starts = useMemo(() => {
    const s: number[] = [];
    let at = 0;
    for (const l of lines) {
      s.push(at);
      at += l.length + 1;
    }
    return s;
  }, [lines]);
  const ranges = useMemo(() => foldRanges(value), [value]);
  const shown = useMemo(() => show(lines, starts, folds, ranges), [lines, starts, folds, ranges]);
  const marked = useMemo(() => new Map(marks.map((m) => [m.line, m.message])), [marks]);

  // Folds that no longer open a range (their bracket was edited away) go.
  useEffect(() => {
    if ([...folds].some((f) => !ranges.has(f))) setFolds(new Set([...folds].filter((f) => ranges.has(f))));
  }, [folds, ranges]);

  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setHeight(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useLayoutEffect(() => {
    if (caret.current === null || !area.current) return;
    const at = toShown(shown, caret.current);
    caret.current = null;
    area.current.setSelectionRange(at, at);
  });

  const lineOfFull = (at: number) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= at) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  };

  const keepCaret = () => {
    if (area.current) caret.current = toFull(shown, area.current.selectionStart);
  };

  const toggle = (line: number) => {
    keepCaret();
    const next = new Set(folds);
    if (next.has(line)) next.delete(line);
    else next.add(line);
    setFolds(next);
  };

  useEffect(() => {
    if (!jump || !area.current) return;
    const open = [...folds].filter((f) => f < jump.line && (ranges.get(f) ?? -1) >= jump.line);
    const next = new Set(folds);
    open.forEach((f) => next.delete(f));
    const after = show(lines, starts, next, ranges);
    setFolds(next);
    caret.current = starts[Math.min(jump.line, starts.length - 1)] ?? 0;
    const row = after.lineOf.findIndex((l) => l >= jump.line);
    const el = area.current;
    requestAnimationFrame(() => {
      el.scrollTop = Math.max(0, (row < 0 ? 0 : row) * LINE_H - el.clientHeight / 3);
      el.focus();
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jump]);

  const edit = (next: string) => {
    if (!shown.holes.length) return onChange(next);
    const prev = shown.text;
    const max = Math.min(prev.length, next.length);
    let p = 0;
    while (p < max && prev.charCodeAt(p) === next.charCodeAt(p)) p++;
    let s = 0;
    while (s < max - p && prev.charCodeAt(prev.length - 1 - s) === next.charCodeAt(next.length - 1 - s)) s++;
    const [a, b] = [p, prev.length - s];
    const hit = shown.holes.find((h) => (a < h.v1 && b > h.v0) || (a === b && a > h.v0 && a < h.v1));
    if (hit) {
      caret.current = starts[hit.line] + lines[hit.line].length;
      const rest = new Set(folds);
      rest.delete(hit.line);
      setFolds(rest);
      return;
    }
    const [m0, m1] = [toFull(shown, a), toFull(shown, b)];
    const inserted = next.slice(p, next.length - s);
    const count = (t: string) => t.split('\n').length - 1;
    const delta = count(inserted) - count(value.slice(m0, m1));
    if (delta) {
      const at = lineOfFull(m0);
      setFolds(new Set([...folds].map((f) => (f > at ? f + delta : f))));
    }
    onChange(value.slice(0, m0) + inserted + value.slice(m1));
  };

  const rows = shown.lineOf.length;
  const first = Math.max(0, Math.floor((scroll.top - PAD_Y) / LINE_H) - OVERSCAN);
  const last = Math.min(rows - 1, Math.ceil((scroll.top + height) / LINE_H) + OVERSCAN);
  const shownLines = useMemo(() => shown.text.split('\n'), [shown]);
  const visible = [];
  for (let r = first; r <= last; r++) visible.push(r);

  return (
    <div className="code">
      <div className="code-gutter" aria-hidden="true">
        <div style={{ transform: `translateY(${-scroll.top}px)` }}>
          {visible.map((r) => {
            const line = shown.lineOf[r];
            const foldable = ranges.has(line);
            const message = marked.get(line);
            return (
              <div key={r} className={`g${message ? ' err' : ''}`} style={{ top: PAD_Y + r * LINE_H }} title={message}>
                <span className="no">{line + 1}</span>
                {foldable ? (
                  <button tabIndex={-1} className="fold" onMouseDown={(e) => (e.preventDefault(), toggle(line))}>
                    {folds.has(line) ? '▸' : '▾'}
                  </button>
                ) : (
                  <span className="fold" />
                )}
              </div>
            );
          })}
        </div>
      </div>
      <div className="code-main">
        <div className="code-hl" aria-hidden="true" style={{ transform: `translate(${-scroll.left}px, ${-scroll.top}px)` }}>
          {visible.map((r) => (
            <div key={r} className={`ln${marked.has(shown.lineOf[r]) ? ' err' : ''}`} style={{ top: PAD_Y + r * LINE_H, left: PAD_X }}>
              {highlight(shownLines[r] ?? '')}
            </div>
          ))}
        </div>
        <textarea
          ref={area}
          aria-label={label}
          value={shown.text}
          wrap="off"
          spellCheck={false}
          autoCapitalize="off"
          autoComplete="off"
          onChange={(e) => edit(e.target.value)}
          onScroll={(e) => setScroll({ top: e.currentTarget.scrollTop, left: e.currentTarget.scrollLeft })}
          onKeyDown={(e) => {
            // Tab indents instead of leaving the editor.
            if (e.key === 'Tab' && !e.shiftKey && !e.ctrlKey) {
              e.preventDefault();
              document.execCommand('insertText', false, '  ');
            }
          }}
        />
      </div>
    </div>
  );
}
