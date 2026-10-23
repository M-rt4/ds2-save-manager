import { create } from 'zustand';

// The on-screen keyboard, for typing with a gamepad alone. It opens when A is pressed on a
// text or number field in pad mode (never for mouse or keyboard focus), edits a copy of the
// value, and on OK hands it to the field as typing would: the field's own change, Enter and
// blur handling run, so its limits and commit behaviour stay as they are. Cancel leaves the
// field untouched.

export type OskKind = 'text' | 'number' | 'digits' | 'hex';
type Field = HTMLInputElement | HTMLTextAreaElement;

export interface Osk {
  target: Field;
  kind: OskKind;
  value: string;
  caret: number;
  max: number | null;
  /** Upper case for the next letter ('once') or until turned off ('caps'). */
  shift: false | 'once' | 'caps';
  symbols: boolean;
  /** Whether a minus sign may be typed (number fields that go below zero). */
  negative: boolean;
  /** A text area (the JSON editor): it starts at its caret, shows lines around it, and LT/RT move by line. */
  multiline: boolean;
}

export const useOsk = create<{ osk: Osk | null }>(() => ({ osk: null }));

/** Which keys a field needs: data-osk when given, else from its type and look. */
function kindOf(el: Field): OskKind {
  const given = el.dataset.osk as OskKind | undefined;
  if (given) return given;
  if (el instanceof HTMLInputElement && el.type === 'number') return 'number';
  if (el.inputMode === 'numeric') return 'digits';
  if (el.classList.contains('mono')) return 'hex';
  return 'text';
}

export function isTextField(el: Element | null): el is Field {
  if (el instanceof HTMLTextAreaElement) return !el.readOnly && !el.disabled;
  return el instanceof HTMLInputElement && ['text', 'search', 'number', ''].includes(el.type) && !el.readOnly && !el.disabled;
}

export function openKeyboard(target: Field) {
  const kind = kindOf(target);
  let max: number | null = target.maxLength > 0 ? target.maxLength : null;
  let negative = false;
  if (target instanceof HTMLInputElement && target.type === 'number') {
    if (target.max !== '') max = String(Math.trunc(Math.abs(Number(target.max)))).length;
    negative = target.min === '' || Number(target.min) < 0;
  }
  const value = target.value;
  const multiline = target instanceof HTMLTextAreaElement;
  const caret = multiline ? target.selectionStart : value.length;
  useOsk.setState({ osk: { target, kind, value, caret, max, shift: false, symbols: false, negative, multiline } });
}

function update(patch: (o: Osk) => Partial<Osk>) {
  const o = useOsk.getState().osk;
  if (o) useOsk.setState({ osk: { ...o, ...patch(o) } });
}

export function type(text: string) {
  update((o) => {
    if (o.max !== null && o.value.length + text.length > o.max) return {};
    const value = o.value.slice(0, o.caret) + text + o.value.slice(o.caret);
    return { value, caret: o.caret + text.length, shift: o.shift === 'once' ? false : o.shift };
  });
}

export function backspace() {
  update((o) => (o.caret === 0 ? {} : { value: o.value.slice(0, o.caret - 1) + o.value.slice(o.caret), caret: o.caret - 1 }));
}

export function moveCaret(step: number) {
  update((o) => ({ caret: Math.max(0, Math.min(o.value.length, o.caret + step)) }));
}

/** Up or down a line in a text area, keeping the column where the line is long enough. */
export function moveLine(step: 1 | -1) {
  update((o) => {
    const start = o.value.lastIndexOf('\n', o.caret - 1) + 1;
    const column = o.caret - start;
    if (step < 0) {
      if (start === 0) return { caret: 0 };
      const prev = o.value.lastIndexOf('\n', start - 2) + 1;
      return { caret: Math.min(prev + column, start - 1) };
    }
    const next = o.value.indexOf('\n', o.caret);
    if (next < 0) return { caret: o.value.length };
    const end = o.value.indexOf('\n', next + 1);
    return { caret: Math.min(next + 1 + column, end < 0 ? o.value.length : end) };
  });
}

/** Y: off, then one capital, then Caps Lock, then off again. */
export function toggleShift() {
  update((o) => ({ shift: o.shift === false ? 'once' : o.shift === 'once' ? 'caps' : false, symbols: false }));
}

export function toggleSymbols() {
  update((o) => ({ symbols: !o.symbols }));
}

export function cancel() {
  const o = useOsk.getState().osk;
  useOsk.setState({ osk: null });
  o?.target.focus({ preventScroll: true });
}

/** Hands the new value to the field the way typing it and pressing Enter would. */
export async function ok() {
  const o = useOsk.getState().osk;
  if (!o) return;
  useOsk.setState({ osk: null });
  const { target } = o;
  target.focus({ preventScroll: true });
  if (o.value !== target.value) {
    const proto = target instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value')!.set!.call(target, o.value);
    target.dispatchEvent(new Event('input', { bubbles: true }));
    // Let the field render its new value before its Enter handler reads it.
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));
  }
  // Enter commits a single-line field; in a text area it would start a new line instead.
  if (!o.multiline) target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  target.blur();
  if (target.isConnected) {
    target.focus({ preventScroll: true });
    if (o.multiline) (target as HTMLTextAreaElement).setSelectionRange(o.caret, o.caret);
  }
}
