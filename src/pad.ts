import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useStore } from './store';
import * as osk from './osk';

// Gamepad control. The Rust side (src-tauri/src/gamepad.rs) reports buttons and sticks while
// the window has focus; here they move a focus cursor between the controls on screen by where
// they are (spatial navigation) and trigger actions.
//
// The app follows whichever input was used last. A button, or a stick or trigger pushed past
// its dead zone, switches to pad mode: pad glyphs in the hints, the cursor, the on-screen
// keyboard, the in-app file browser, no mouse pointer. Moving the mouse more than a few
// pixels, clicking, scrolling the wheel or pressing a key switches back. A pad that is plugged
// in but not used changes nothing, and unplugging it returns to keyboard and mouse.
//
//   D-pad / left stick  move          A / ✕   activate      B / ○   back (Escape)
//   LB / RB             detail tabs   LT / RT (or L3 / R3)  panels
//   Start               save          View / Select         settings
//   Right stick         scroll        Left / right on a number or a list changes its value

export type PadFamily = 'xbox' | 'playstation' | 'deck';
export type PadButton =
  | 'south'
  | 'east'
  | 'north'
  | 'west'
  | 'lb'
  | 'rb'
  | 'lt'
  | 'rt'
  | 'select'
  | 'start'
  | 'l3'
  | 'r3'
  | 'up'
  | 'down'
  | 'left'
  | 'right';
type Dir = 'up' | 'down' | 'left' | 'right';

interface PadInfo {
  name: string;
  family: PadFamily;
}

type PadEvent =
  | { kind: 'button'; button: PadButton; pressed: boolean }
  | { kind: 'axis'; axis: 'lx' | 'ly' | 'rx' | 'ry'; value: number }
  | { kind: 'pads'; pads: PadInfo[] }
  | { kind: 'blur' };

/**
 * The connected pad (its glyph family and name; null when none is connected), and whether
 * the app is in pad mode, i.e. the pad was the last input used.
 */
export const usePad = create<{ family: PadFamily | null; name: string | null; active: boolean }>(() => ({
  family: null,
  name: null,
  active: false
}));

/** The glyph family to draw while in pad mode, else null (keyboard and mouse hints). */
export const usePadHints = () => usePad((s) => (s.active ? (s.family ?? 'xbox') : null));

const FOCUSABLE = 'button:not(:disabled), a[href], input:not(:disabled):not([type=hidden]), select:not(:disabled), textarea:not(:disabled), [tabindex]';
const REPEAT_DELAY = 380;
const REPEAT_EVERY = 90;
const STICK_ON = 0.55;
const STICK_OFF = 0.35;
/** The right stick scrolls past this while in pad mode, and switches to it only past STICK_ON. */
const SCROLL_DEAD = 0.25;
/** Mouse travel, in pixels, that counts as using the mouse rather than a nudge of the desk. */
const MOUSE_SLACK = 6;

let repeatTimer: number | undefined;
let stickDir: Dir | null = null;
const stick = { x: 0, y: 0 };
const scroll = { x: 0, y: 0 };
let scrolling = false;
/** The last element focused in each panel, to come back to it. */
const lastInPanel = new WeakMap<Element, HTMLElement>();
let lastFocused: HTMLElement | null = null;
/** The panel the cursor was last in, to stay there when its control is replaced. */
let lastPanel: Element | null = null;
/** Where the cursor was when a dialog opened, to return there when it closes. */
let beforeDialog: HTMLElement | null = null;
const DIRS: PadButton[] = ['up', 'down', 'left', 'right'];

function activate() {
  if (!usePad.getState().active) {
    usePad.setState({ active: true });
    document.body.dataset.pad = '';
  }
}

function deactivate() {
  if (usePad.getState().active) {
    usePad.setState({ active: false });
    delete document.body.dataset.pad;
  }
}

/** Where the cursor may go: the on-screen keyboard, else the topmost dialog, else the window. */
function scope(): Element {
  const keyboard = document.querySelector('.osk');
  if (keyboard) return keyboard;
  const scrims = document.querySelectorAll('.scrim');
  return scrims[scrims.length - 1] ?? document.querySelector('.app') ?? document.body;
}

function visible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return false;
  const s = getComputedStyle(el);
  return s.visibility !== 'hidden' && s.display !== 'none';
}

// The title bar stays out of reach: its window buttons would close or shrink the app on a
// stray press, and Settings (with the language) has its own button. So do controls that open
// another program ([data-external], e.g. Show in Explorer): in the Steam Deck's Game Mode they
// would take the user out of the app.
function targets(root: Element = scope()): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => (el.tabIndex >= 0 || el.hasAttribute('data-pad')) && !el.closest('[inert], .titlebar, [data-external]') && visible(el)
  );
}

function current(): HTMLElement | null {
  const el = document.activeElement as HTMLElement | null;
  return el && el !== document.body && scope().contains(el) ? el : null;
}

const panelOf = (el: Element) => el.closest('.panel, .footer, .titlebar, .dialog');

function focus(el: HTMLElement) {
  document.querySelector('[data-pad-focus]')?.removeAttribute('data-pad-focus');
  el.setAttribute('data-pad-focus', '');
  el.focus({ preventScroll: true });
  el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  const panel = panelOf(el);
  if (panel) lastInPanel.set(panel, el);
  lastFocused = el;
  lastPanel = panel;
}

const usable = (el: HTMLElement | null): el is HTMLElement =>
  !!el && el.isConnected && !(el as HTMLButtonElement).disabled && scope().contains(el) && visible(el);

/** Shows the cursor where it was, or on the first sensible control, without moving it. */
function reveal() {
  const at = current();
  const to = at && !at.closest('.titlebar') ? at : usable(lastFocused) ? lastFocused : first();
  if (to) focus(to);
}

/** After an action: into a dialog that opened, back to where the cursor was when it closes. */
function settle() {
  const check = () => {
    if (!usePad.getState().active) return;
    const at = current();
    if (at) {
      // Focus came back by itself (the keyboard returning to its field): show the cursor there.
      if (!at.hasAttribute('data-pad-focus')) focus(at);
      return;
    }
    const root = scope();
    if (root.classList.contains('scrim')) {
      if (lastFocused?.isConnected && !root.contains(lastFocused)) beforeDialog = lastFocused;
      const to = first(root);
      if (to) focus(to);
    } else {
      const panel = lastPanel?.isConnected && root.contains(lastPanel) ? lastPanel : root;
      const back = usable(beforeDialog) ? beforeDialog : usable(lastFocused) ? lastFocused : first(panel) ?? first(root);
      beforeDialog = null;
      if (back) focus(back);
    }
  };
  window.setTimeout(check, 60);
  window.setTimeout(check, 300);
}

/** The best target in a direction: nearest along it, little sideways drift, same panel first. */
/** The nearest ancestor that scrolls vertically: a list, or a view's body. */
function scrollBox(el: Element): Element | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    if (/auto|scroll/.test(getComputedStyle(p).overflowY)) return p;
  }
  return null;
}

/** Whether an element lies scrolled out of sight in its scrolling box. */
function scrolledAway(el: Element, box: Element | null): boolean {
  if (!box) return false;
  const r = el.getBoundingClientRect();
  const b = box.getBoundingClientRect();
  return r.bottom <= b.top + 1 || r.top >= b.bottom - 1;
}

function nearest(from: HTMLElement, dir: Dir): HTMLElement | null {
  const a = from.getBoundingClientRect();
  const home = panelOf(from);
  const fromBox = scrollBox(from);
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of targets()) {
    // A row's own buttons (a backup's Restore) are reachable from it; its ancestors are not.
    if (el === from || el.contains(from)) continue;
    // Within its own box the cursor scrolls on to hidden controls; into another box it only
    // goes to what shows, or content scrolled up under a view's tabs would sit "above" them.
    const box = scrollBox(el);
    if (box !== fromBox && scrolledAway(el, box)) continue;
    const b = el.getBoundingClientRect();
    let along: number;
    let across: number;
    if (dir === 'down' || dir === 'up') {
      along = dir === 'down' ? b.top - a.bottom : a.top - b.bottom;
      if ((dir === 'down' ? b.top + b.height / 2 <= a.top + a.height / 2 : b.top + b.height / 2 >= a.top + a.height / 2)) continue;
      across = Math.max(0, Math.max(a.left, b.left) - Math.min(a.right, b.right));
      // Up and down keep to their own scrolling box: at the end of a list the cursor stops
      // rather than jumping into the list beside it, which left, right or LT / RT reach.
      if (across > 0 && (panelOf(el) !== home || (box && fromBox && box !== fromBox))) continue;
      if (across === 0) across = Math.abs(a.left - b.left) / 8;
    } else {
      along = dir === 'right' ? b.left - a.right : a.left - b.right;
      if ((dir === 'right' ? b.left + b.width / 2 <= a.left + a.width / 2 : b.left + b.width / 2 >= a.left + a.width / 2)) continue;
      across = Math.max(0, Math.max(a.top, b.top) - Math.min(a.bottom, b.bottom));
      // Sideways the cursor may cross into the panel beside, never down into the footer.
      if (panelOf(el) !== home && !el.closest('.main > .panel')) continue;
      if (across === 0) across = Math.abs(a.top + a.height / 2 - (b.top + b.height / 2)) / 8;
    }
    const score = Math.max(0, along) + across * 3 + (panelOf(el) === home ? 0 : 4000);
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  return best;
}

/** Where the cursor lands in a view: a control marked data-pad-default, else the selected one. */
function first(root: Element = scope()): HTMLElement | null {
  const all = targets(root);
  return (
    all.find((el) => el.hasAttribute('data-pad-default')) ??
    all.find((el) => el.getAttribute('aria-selected') === 'true') ??
    all[0] ??
    null
  );
}

function move(dir: Dir) {
  const from = current();
  if (!from) {
    const start = first();
    if (start) focus(start);
    return;
  }
  if ((dir === 'left' || dir === 'right') && adjust(from, dir === 'right' ? 1 : -1)) return;
  const to = nearest(from, dir);
  if (to) focus(to);
}

/** Left and right change a number field or a list instead of leaving it. */
function adjust(el: HTMLElement, step: 1 | -1): boolean {
  if (el instanceof HTMLSelectElement) {
    const i = Math.max(0, Math.min(el.options.length - 1, el.selectedIndex + step));
    if (i !== el.selectedIndex) {
      el.selectedIndex = i;
      el.dispatchEvent(new Event('change', { bubbles: true }));
    }
    return true;
  }
  if (el instanceof HTMLInputElement && el.type === 'number' && !el.readOnly) {
    // Fields with their own arrow-key handling (the Status steppers) get the key itself.
    const key = new KeyboardEvent('keydown', { key: step > 0 ? 'ArrowRight' : 'ArrowLeft', bubbles: true, cancelable: true });
    if (!el.dispatchEvent(key)) return true;
    const min = el.min === '' ? -Infinity : Number(el.min);
    const max = el.max === '' ? Infinity : Number(el.max);
    const next = Math.max(min, Math.min(max, (Number(el.value) || 0) + step));
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(el, String(next));
    el.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  }
  return false;
}

function key(key: string, init: KeyboardEventInit = {}) {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
}

/** LB / RB: the tabs of the open dialog, else of the detail panel. */
function cycleTabs(step: 1 | -1) {
  const dialog = scope();
  const root = dialog.classList.contains('scrim') ? dialog : document.querySelector('.main > :last-child');
  const tabs = [...(root?.querySelectorAll<HTMLElement>('.tabs .tab') ?? [])].filter(visible);
  if (tabs.length === 0) return;
  const at = tabs.findIndex((t) => t.getAttribute('aria-selected') === 'true');
  const next = tabs[(at + step + tabs.length) % tabs.length];
  next.click();
  requestAnimationFrame(() => focus(next));
}

/** LT / RT: Games, Characters and Detail, back to where the cursor last was in each. */
function cyclePanels(step: 1 | -1) {
  if (document.querySelector('.scrim')) return;
  const panels = [...document.querySelectorAll<HTMLElement>('.main > .panel')];
  if (panels.length === 0) return;
  const from = current();
  const at = from ? panels.findIndex((p) => p.contains(from)) : -1;
  const panel = panels[(at + step + panels.length) % panels.length];
  const back = lastInPanel.get(panel);
  const to = back && panel.contains(back) && visible(back) ? back : first(panel);
  if (to) focus(to);
}

function startScroll() {
  if (scrolling) return;
  scrolling = true;
  const tick = () => {
    if (Math.abs(scroll.x) < SCROLL_DEAD && Math.abs(scroll.y) < SCROLL_DEAD) {
      scrolling = false;
      return;
    }
    let el: HTMLElement | null = current() ?? (scope().querySelector('.panel-body') as HTMLElement | null);
    while (el && !(el.scrollHeight > el.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    el?.scrollBy({ left: scroll.x * 22, top: -scroll.y * 22 });
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

function stopRepeat() {
  window.clearTimeout(repeatTimer);
  repeatTimer = undefined;
}

function repeatMove(dir: Dir) {
  stopRepeat();
  move(dir);
  const again = (delay: number) => {
    repeatTimer = window.setTimeout(() => {
      move(dir);
      again(REPEAT_EVERY);
    }, delay);
  };
  again(REPEAT_DELAY);
}

export function press(button: PadButton) {
  // The first touch only brings the cursor up, as in the game's menus.
  if (!usePad.getState().active) {
    activate();
    if (DIRS.includes(button) || button === 'south') {
      reveal();
      return;
    }
  }
  // The on-screen keyboard takes the face and shoulder buttons; the cursor moves over its keys.
  const typing = osk.useOsk.getState().osk;
  if (typing) {
    const words = typing.kind === 'text' || typing.kind === 'hex';
    const action: Partial<Record<PadButton, () => void>> = {
      east: osk.backspace,
      west: () => words && osk.type(' '),
      north: () => typing.kind === 'text' && osk.toggleShift(),
      lb: () => osk.moveCaret(-1),
      rb: () => osk.moveCaret(1),
      start: () => void osk.ok().then(settle),
      select: () => (osk.cancel(), settle())
    };
    const run = action[button];
    if (run) {
      run();
      return;
    }
    if (button === 'lt' || button === 'rt' || button === 'l3' || button === 'r3') return;
    if (!current()) settle();
  }
  if (!current() && button !== 'east') {
    reveal();
    if (DIRS.includes(button) || button === 'south') return;
  }
  switch (button) {
    case 'up':
    case 'down':
    case 'left':
    case 'right':
      repeatMove(button);
      break;
    case 'south': {
      const el = current();
      if (osk.isTextField(el)) osk.openKeyboard(el);
      else el?.click();
      settle();
      break;
    }
    case 'east':
      key('Escape');
      settle();
      break;
    case 'lb':
    case 'rb':
      cycleTabs(button === 'rb' ? 1 : -1);
      settle();
      break;
    case 'lt':
    case 'rt':
    case 'l3':
    case 'r3':
      cyclePanels(button === 'rt' || button === 'r3' ? 1 : -1);
      break;
    case 'start':
      key('s', { ctrlKey: true });
      settle();
      break;
    case 'select': {
      const s = useStore.getState();
      s.setSettingsOpen(!s.settingsOpen);
      settle();
      break;
    }
    default:
      break;
  }
}

export function release(button: PadButton) {
  if (button === 'up' || button === 'down' || button === 'left' || button === 'right') stopRepeat();
}

export function axis(name: 'lx' | 'ly' | 'rx' | 'ry', value: number) {
  if (name === 'rx' || name === 'ry') {
    scroll[name === 'rx' ? 'x' : 'y'] = value;
    // A drifting stick neither scrolls nor takes over from the mouse; a real push does both.
    if (Math.abs(value) >= STICK_ON) activate();
    if (Math.abs(value) >= SCROLL_DEAD && usePad.getState().active) startScroll();
    return;
  }
  stick[name === 'lx' ? 'x' : 'y'] = value;
  const { x, y } = stick;
  const mag = Math.max(Math.abs(x), Math.abs(y));
  // Y points up on the stick, down on the screen.
  const dir: Dir | null = mag < (stickDir ? STICK_OFF : STICK_ON) ? null : Math.abs(x) > Math.abs(y) ? (x > 0 ? 'right' : 'left') : y > 0 ? 'up' : 'down';
  if (dir === stickDir) return;
  stickDir = dir;
  if (dir) press(dir);
  else stopRepeat();
}

function handle(e: PadEvent) {
  if (e.kind === 'button') (e.pressed ? press : release)(e.button);
  else if (e.kind === 'axis') axis(e.axis, e.value);
  else if (e.kind === 'pads') {
    usePad.setState({ family: e.pads[0]?.family ?? null, name: e.pads[0]?.name ?? null });
    // Unplugging the last pad hands the app back to keyboard and mouse.
    if (e.pads.length === 0) deactivate();
  } else {
    stopRepeat();
    stickDir = null;
    scroll.x = scroll.y = stick.x = stick.y = 0;
  }
}

/** Starts listening for pads; called once at startup. */
export function initPad() {
  void invoke<PadInfo[]>('gamepads')
    .then((pads) => usePad.setState({ family: pads[0]?.family ?? null, name: pads[0]?.name ?? null }))
    .catch(() => undefined);
  void listen<PadEvent>('gamepad', (e) => handle(e.payload)).catch(() => undefined);
  // Mouse or keyboard take over: a real move (not a few pixels of jitter), a click, the wheel
  // or a key. Events the pad layer itself dispatches are untrusted and never count.
  let anchor: { x: number; y: number } | null = null;
  const off = (e: Event) => e.isTrusted && deactivate();
  window.addEventListener(
    'mousemove',
    (e) => {
      if (!e.isTrusted || !usePad.getState().active) return;
      anchor ??= { x: e.screenX, y: e.screenY };
      if (Math.hypot(e.screenX - anchor.x, e.screenY - anchor.y) > MOUSE_SLACK) {
        anchor = null;
        deactivate();
      }
    },
    { passive: true }
  );
  usePad.subscribe((s, before) => s.active && !before.active && (anchor = null));
  window.addEventListener('mousedown', off, { passive: true });
  window.addEventListener('wheel', off, { passive: true });
  window.addEventListener('keydown', off);
  // A dialog that opens later than an action's own settling (a report after a long write)
  // still gets the cursor.
  new MutationObserver((changes) => {
    if (usePad.getState().active && changes.some((c) => [...c.addedNodes].some((n) => n instanceof Element && n.matches('.scrim')))) settle();
  }).observe(document.body, { childList: true, subtree: true });
  if (import.meta.env.DEV) {
    // A way in for tests without a real pad: window.__ds2pad.tap('down') over CDP.
    (window as unknown as { __ds2pad: unknown }).__ds2pad = {
      press,
      release,
      axis,
      tap: (b: PadButton) => {
        press(b);
        release(b);
      },
      // Plugs in (or with null, pulls out) a pad, through the same path as the real event.
      connect: (family: PadFamily | null) => handle({ kind: 'pads', pads: family ? [{ name: `Test pad (${family})`, family }] : [] })
    };
  }
}
