import { useEffect } from 'react';
import { backspace, cancel, moveCaret, ok, toggleShift, toggleSymbols, type, useOsk, type Osk } from '../osk';
import { usePad, type PadButton } from '../pad';
import { useT, type Key } from '../i18n';
import { Glyph } from './PadHints';

// The keyboard drawn in the lower half, like the game's own name entry: the value being typed
// large on top, the keys below, the pad's buttons along the bottom. Turkish letters are always
// there (character names), laid out as on a Turkish Q keyboard.

const LETTERS = ['qwertyuıopğü', 'asdfghjklşi', 'zxcvbnmöç.-'];
const SYMBOLS = ['1234567890', '!?@#&\'"()_', '/\\:;,+=*%~'];
const upper = (c: string) => (c === 'i' ? 'İ' : c === 'ı' ? 'I' : c.toUpperCase());

function rowsFor(o: Osk): string[][] {
  switch (o.kind) {
    case 'number':
      return [['7', '8', '9'], ['4', '5', '6'], ['1', '2', '3'], o.negative ? ['-', '0'] : ['0']];
    case 'digits':
      return [['7', '8', '9'], ['4', '5', '6'], ['1', '2', '3'], ['0']];
    case 'hex':
      return [['7', '8', '9', 'A', 'B'], ['4', '5', '6', 'C', 'D'], ['1', '2', '3', 'E', 'F'], ['0', 'x', '.', '-', ' ']];
    default:
      return (o.symbols ? SYMBOLS : LETTERS).map((r) => [...r].map((c) => (o.shift ? upper(c) : c)));
  }
}

const HINTS: { button: PadButton; key: Key; kinds?: Osk['kind'][] }[] = [
  { button: 'south', key: 'osk.type' },
  { button: 'east', key: 'osk.delete' },
  { button: 'west', key: 'osk.space', kinds: ['text', 'hex'] },
  { button: 'north', key: 'osk.shift', kinds: ['text'] },
  { button: 'lb', key: 'osk.cursor' },
  { button: 'start', key: 'osk.ok' },
  { button: 'select', key: 'osk.cancel' }
];

export function PadKeyboard() {
  const osk = useOsk((s) => s.osk);
  return osk ? <Keyboard osk={osk} /> : null;
}

function Keyboard({ osk }: { osk: Osk }) {
  const t = useT();
  const family = usePad((s) => s.family) ?? 'xbox';
  const text = osk.kind === 'text';

  // Escape on a real keyboard cancels the keyboard only, not a dialog beneath it.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && e.isTrusted) {
        e.stopImmediatePropagation();
        cancel();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  return (
    <div className="osk-layer">
      <div className="panel osk" role="dialog" aria-label={t('osk.title')}>
        <div className="osk-value">
          <span>{osk.value.slice(0, osk.caret)}</span>
          <span className="caret" />
          <span>{osk.value.slice(osk.caret)}</span>
          {osk.max !== null && (
            <small className="osk-count">
              {osk.value.length}/{osk.max}
            </small>
          )}
        </div>
        <div className={`osk-keys ${osk.kind}`}>
          {rowsFor(osk).map((row, i) => (
            <div className="osk-row" key={i}>
              {/* Keyed by place, so Shift and the symbol page keep the cursor where it is. */}
              {row.map((c, j) => (
                <button key={j} className="key" onClick={() => type(c)}>
                  {c === ' ' ? '␣' : c}
                </button>
              ))}
            </div>
          ))}
          <div className="osk-row actions">
            {text && (
              <>
                <button className="key" aria-pressed={osk.shift !== false} onClick={toggleShift}>
                  {osk.shift === 'caps' ? '⇪' : '⇧'}
                </button>
                <button className="key" aria-pressed={osk.symbols} onClick={toggleSymbols}>
                  {osk.symbols ? 'abc' : '123'}
                </button>
                <button className="key space" onClick={() => type(' ')}>
                  {t('osk.space')}
                </button>
              </>
            )}
            <button className="key" aria-label={t('osk.delete')} onClick={backspace}>
              ⌫
            </button>
            <button className="key" aria-label="←" onClick={() => moveCaret(-1)}>
              ‹
            </button>
            <button className="key" aria-label="→" onClick={() => moveCaret(1)}>
              ›
            </button>
            <button className="key ok" onClick={() => void ok()}>
              {t('osk.ok')}
            </button>
            <button className="key" onClick={cancel}>
              {t('osk.cancel')}
            </button>
          </div>
        </div>
        <div className="osk-hints pad-hints">
          {HINTS.filter((h) => !h.kinds || h.kinds.includes(osk.kind)).map((h) => (
            <span key={h.key} className="pad-hint">
              <Glyph button={h.button} family={family} />
              {h.button === 'lb' && <Glyph button="rb" family={family} />}
              {t(h.key)}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
