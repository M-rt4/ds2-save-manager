import type { ReactNode } from 'react';
import { usePadHints, type PadButton, type PadFamily } from '../pad';
import { useT, type Key } from '../i18n';

// Button glyphs drawn for the connected pad: Xbox letters, PlayStation shapes, or the Steam
// Deck's letters with its L1/R1 shoulders. Drawn in the current colour, like the menu text.

const FACE: Record<'south' | 'east' | 'west' | 'north', string> = { south: 'A', east: 'B', west: 'X', north: 'Y' };

function PlayStationShape({ button }: { button: 'south' | 'east' | 'west' | 'north' }) {
  switch (button) {
    case 'south':
      return <path d="M5.6 5.6l6.8 6.8M12.4 5.6l-6.8 6.8" />;
    case 'east':
      return <circle cx="9" cy="9" r="3.6" />;
    case 'west':
      return <rect x="5.6" y="5.6" width="6.8" height="6.8" />;
    default:
      return <path d="M9 5.2l4 7H5z" />;
  }
}

function Shoulder({ label }: { label: string }) {
  return (
    <svg className="glyph wide" width="26" height="18" viewBox="0 0 26 18" aria-hidden>
      <rect x="0.75" y="2.75" width="24.5" height="12.5" rx="6.25" fill="none" stroke="currentColor" />
      <text x="13" y="12.6" textAnchor="middle" fontSize="9" fill="currentColor">
        {label}
      </text>
    </svg>
  );
}

export function Glyph({ button, family }: { button: PadButton; family: PadFamily }) {
  if (button === 'south' || button === 'east' || button === 'west' || button === 'north') {
    return (
      <svg className="glyph" width="18" height="18" viewBox="0 0 18 18" aria-hidden>
        <circle cx="9" cy="9" r="8.25" fill="none" stroke="currentColor" />
        {family === 'playstation' ? (
          <g fill="none" stroke="currentColor" strokeWidth="1.2">
            <PlayStationShape button={button} />
          </g>
        ) : (
          <text x="9" y="12.6" textAnchor="middle" fontSize="10" fill="currentColor">
            {FACE[button]}
          </text>
        )}
      </svg>
    );
  }
  const xbox = family === 'xbox';
  switch (button) {
    case 'lb':
      return <Shoulder label={xbox ? 'LB' : 'L1'} />;
    case 'rb':
      return <Shoulder label={xbox ? 'RB' : 'R1'} />;
    case 'lt':
      return <Shoulder label={xbox ? 'LT' : 'L2'} />;
    case 'rt':
      return <Shoulder label={xbox ? 'RT' : 'R2'} />;
    case 'start':
      // Menu / Options: three lines.
      return (
        <svg className="glyph" width="18" height="18" viewBox="0 0 18 18" aria-hidden>
          <circle cx="9" cy="9" r="8.25" fill="none" stroke="currentColor" />
          <path d="M5.5 6.5h7M5.5 9h7M5.5 11.5h7" stroke="currentColor" />
        </svg>
      );
    case 'select':
      // View (two windows) on Xbox and the Deck; Create / Share on PlayStation.
      return (
        <svg className="glyph" width="18" height="18" viewBox="0 0 18 18" aria-hidden>
          <circle cx="9" cy="9" r="8.25" fill="none" stroke="currentColor" />
          {family === 'playstation' ? (
            <path d="M9 5v6M6.6 7.2 9 4.8l2.4 2.4M5.5 9.5v3h7v-3" fill="none" stroke="currentColor" />
          ) : (
            <path d="M5.2 6.2h5v4h-5zM7.8 8.4h5v4h-5" fill="none" stroke="currentColor" />
          )}
        </svg>
      );
    default:
      return null;
  }
}

const HINTS: { buttons: PadButton[]; key: Key }[] = [
  { buttons: ['south'], key: 'pad.select' },
  { buttons: ['east'], key: 'pad.back' },
  { buttons: ['lb', 'rb'], key: 'pad.tabs' },
  { buttons: ['lt', 'rt'], key: 'pad.panels' },
  { buttons: ['start'], key: 'pad.save' },
  { buttons: ['select'], key: 'pad.settings' }
];

/** The key guide along the bottom, as in the game, while the app is in pad mode. */
export function PadHints() {
  const t = useT();
  const family = usePadHints();
  if (!family) return null;
  return (
    <span className="pad-hints">
      {HINTS.map((h) => (
        <Hint key={h.key} label={t(h.key)}>
          {h.buttons.map((b) => (
            <Glyph key={b} button={b} family={family} />
          ))}
        </Hint>
      ))}
    </span>
  );
}

function Hint({ label, children }: { label: string; children: ReactNode }) {
  return (
    <span className="pad-hint">
      {children}
      {label}
    </span>
  );
}
