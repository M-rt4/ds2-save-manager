import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';

// The introductory tour, over the live app: one area at a time stays lit inside an ember frame
// while the rest of the window goes dark, and a card in the dark beside it says what the area is
// for. The app underneath keeps its look but takes no input (inert), so nothing can be changed
// by a stray click or press. It opens by itself once, after the first start; Settings can show
// it again.

interface Step {
  /** The area to light, or none for a card in the middle of a dark window. */
  target: () => Element | null;
  title: Key;
  text: Key;
}

const panel = (i: number) => () => document.querySelectorAll('.main > .panel')[i] ?? null;

const STEPS: Step[] = [
  { target: () => null, title: 'tour.welcomeTitle', text: 'tour.welcomeText' },
  { target: panel(0), title: 'tour.gamesTitle', text: 'tour.gamesText' },
  { target: panel(1), title: 'tour.slotsTitle', text: 'tour.slotsText' },
  { target: () => document.querySelector('.main > .panel .tabs'), title: 'tour.tabsTitle', text: 'tour.tabsText' },
  { target: panel(2), title: 'tour.editTitle', text: 'tour.editText' },
  { target: () => document.querySelector('.footer .btn.ember'), title: 'tour.saveTitle', text: 'tour.saveText' },
  { target: () => document.querySelector('.titlebar .lang'), title: 'tour.settingsTitle', text: 'tour.settingsText' }
];

/** The parts of the app the tour sits over, made inert while it is open. The title bar stays
 *  live but for its Settings and language buttons: the window can still be moved, shrunk and
 *  closed, as with any dialog. */
const APP_PARTS = '.app > .main, .app > .footer, .titlebar .lang';

interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

const PAD = 6;
const GAP = 18;
const EDGE = 16;

function boxOf(el: Element | null): Box | null {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return null;
  const left = Math.max(2, r.left - PAD);
  const top = Math.max(2, r.top - PAD);
  return {
    left,
    top,
    width: Math.min(window.innerWidth - 2, r.right + PAD) - left,
    height: Math.min(window.innerHeight - 2, r.bottom + PAD) - top
  };
}

/** Where the card goes: beside the lit area where it fits (right, left, below, above). */
function place(hole: Box | null, card: { width: number; height: number }): { left: number; top: number } {
  const w = window.innerWidth;
  const h = window.innerHeight;
  const clampTop = (t: number) => Math.max(EDGE, Math.min(h - card.height - EDGE, t));
  const clampLeft = (l: number) => Math.max(EDGE, Math.min(w - card.width - EDGE, l));
  if (!hole) return { left: (w - card.width) / 2, top: (h - card.height) / 2 };
  const right = hole.left + hole.width + GAP;
  if (right + card.width <= w - EDGE) return { left: right, top: clampTop(hole.top) };
  const left = hole.left - GAP - card.width;
  if (left >= EDGE) return { left, top: clampTop(hole.top) };
  const below = hole.top + hole.height + GAP;
  if (below + card.height <= h - EDGE) return { left: clampLeft(hole.left), top: below };
  const above = hole.top - GAP - card.height;
  if (above >= EDGE) return { left: clampLeft(hole.left), top: above };
  // A target as big as the window: the card goes over it, in its lower right.
  return { left: w - card.width - EDGE * 2, top: h - card.height - EDGE * 2 };
}

export function Tour() {
  const t = useT();
  const close = useStore((s) => s.setTourOpen);
  const confirm = useStore((s) => s.confirm);
  // Only the steps whose area is on screen (with no save open, there are no tabs to show),
  // chosen once the app beneath has been drawn: the tour opens in the same render as it.
  const [steps, setSteps] = useState<Step[]>([]);
  useLayoutEffect(() => setSteps(STEPS.filter((s, i) => i === 0 || boxOf(s.target()))), []);
  const [at, setAt] = useState(0);
  const [hole, setHole] = useState<Box | null>(null);
  const [spot, setSpot] = useState<{ left: number; top: number } | null>(null);
  const card = useRef<HTMLDivElement>(null);
  const next = useRef<HTMLButtonElement>(null);
  const step: Step | undefined = steps[at];
  const last = at === steps.length - 1;

  // The app keeps its look but takes no input while the tour is over it.
  useEffect(() => {
    const parts = [...document.querySelectorAll(APP_PARTS)];
    parts.forEach((p) => p.setAttribute('inert', ''));
    return () => parts.forEach((p) => p.removeAttribute('inert'));
  }, []);

  // Light the step's area, and put the card beside it; again whenever the window changes size.
  useLayoutEffect(() => {
    if (!step) return;
    const measure = () => {
      const box = boxOf(step.target());
      setHole(box);
      const c = card.current;
      if (c) setSpot(place(box, { width: c.offsetWidth, height: c.offsetHeight }));
    };
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  }, [step]);

  // Each step starts on Next, so Enter, A or a click carries on. Only once the card has its
  // place: until then it is hidden, and a hidden button can't take the focus.
  const placed = spot !== null;
  useEffect(() => {
    if (placed) next.current?.focus({ preventScroll: true });
  }, [at, placed]);

  const go = (to: number) => (to >= steps.length ? close(false) : setAt(Math.max(0, to)));

  // A question of the app's own (closing the window with unsaved changes) must not open under
  // the tour, where it could be neither seen nor answered: the tour gives way to it.
  useEffect(() => {
    if (confirm) close(false);
  }, [confirm, close]);

  // Arrow keys step through; Escape (and the pad's B) skips the rest. Shortcuts of the app
  // beneath (Ctrl S…) don't get through: nothing can be done there while the tour is open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        // Swallowed: the app's shortcuts. Alt+F4 and the like are the window's and pass.
      } else if (e.key === 'Escape') close(false);
      else if (e.key === 'ArrowRight') setAt((a) => (a + 1 >= steps.length ? a : a + 1));
      else if (e.key === 'ArrowLeft') setAt((a) => Math.max(0, a - 1));
      else return;
      e.preventDefault();
      e.stopImmediatePropagation();
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [close, steps.length]);

  if (!step) return null;
  const center = { left: window.innerWidth / 2, top: window.innerHeight / 2, width: 0, height: 0 };

  return (
    <div className="scrim tour" role="dialog" aria-modal="true" aria-labelledby="tour-title">
      <div className="tour-hole" data-empty={hole ? undefined : ''} style={hole ?? center} />
      <div className="panel dialog tour-card" ref={card} style={spot ? { left: spot.left, top: spot.top } : { visibility: 'hidden' }}>
        <div className="tour-progress">
          <span className="tour-count">{t('tour.step', { n: at + 1, total: steps.length })}</span>
          <span className="tour-dots" aria-hidden>
            {steps.map((_, i) => (
              <span key={i} data-on={i === at ? '' : undefined} data-done={i < at ? '' : undefined} />
            ))}
          </span>
        </div>
        <h3 id="tour-title">{t(step.title)}</h3>
        <p>{t(step.text)}</p>
        {/* Next comes first in the markup, so the pad's cursor starts on it; the row is drawn
            reversed, Skip on the left. */}
        <div className="tour-actions">
          <button ref={next} className="btn ember" onClick={() => go(at + 1)}>
            {last ? t('tour.done') : t('tour.next')}
          </button>
          {at > 0 && (
            <button className="btn" onClick={() => go(at - 1)}>
              {t('tour.back')}
            </button>
          )}
          <span className="spacer" />
          {!last && (
            <button className="btn quiet" onClick={() => close(false)}>
              {t('tour.skip')}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
