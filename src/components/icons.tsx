import { useId } from 'react';
import type { Category } from '../api';

type P = { size?: number; className?: string };

function Svg({ size = 18, className, children }: P & { children: React.ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.35}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

const CATEGORY_PATHS: Record<Category, React.ReactNode> = {
  weapon: (
    <>
      <path d="M19.5 3.5 9 14l1 1L20.5 4.5V3.5z" />
      <path d="M6.5 13.5l4 4M5 19l3.2-3.2M4.2 17.8l2 2" />
    </>
  ),
  shield: <path d="M12 3.5 5 6v5.5c0 4.2 3 7.4 7 9 4-1.6 7-4.8 7-9V6l-7-2.5zM12 3.5v17" />,
  armor: (
    <>
      <path d="M5.5 13V9.5a6.5 6.5 0 0 1 13 0V13l-1.5 6h-10z" />
      <path d="M8 11.5h8M12 11.5V19" />
    </>
  ),
  ring: (
    <>
      <circle cx="12" cy="14.5" r="5.5" />
      <path d="M9.5 6.5 12 9l2.5-2.5L12 4z" />
    </>
  ),
  spell: (
    <>
      <path d="M12 3v4M12 17v4M3 12h4M17 12h4" />
      <path d="M12 8.5 13.2 10.8 15.5 12l-2.3 1.2L12 15.5l-1.2-2.3L8.5 12l2.3-1.2z" />
    </>
  ),
  consumable: (
    <>
      <path d="M10 3.5h4M10.5 3.5v4.2L7 13.5a5.2 5.2 0 1 0 10 0l-3.5-5.8V3.5" />
      <path d="M8.2 15.5h7.6" />
    </>
  ),
  multiplayer: (
    <>
      <path d="M3.5 12s3.2-5.5 8.5-5.5 8.5 5.5 8.5 5.5-3.2 5.5-8.5 5.5S3.5 12 3.5 12z" />
      <circle cx="12" cy="12" r="2.4" />
    </>
  ),
  upgrade: (
    <>
      <path d="M12 3 18 10.5 12 21 6 10.5z" />
      <path d="M6 10.5h12M12 3v18" />
    </>
  ),
  ammo: (
    <>
      <path d="M4.5 19.5 18 6" />
      <path d="M14.5 5.5H18.5V9.5M4.5 15.5v4h4M3.5 17.5l3 3" />
    </>
  ),
  bossSoul: (
    <>
      <path d="M12 3.5c3 3.6 5 6.4 5 9.5a5 5 0 0 1-10 0c0-1.8.8-3.3 2-4.6.3 1.5 1 2.4 2 2.8-.5-2.6.2-5 1-7.7z" />
    </>
  ),
  key: (
    <>
      <circle cx="8" cy="8" r="3.5" />
      <path d="M10.5 10.5 19.5 19.5M16 16l2-2M18 18l1.6-1.6" />
    </>
  ),
  gesture: (
    <>
      <circle cx="12" cy="5.5" r="2" />
      <path d="M12 8v6l-3 6.5M12 14l3 6.5M7 10.5l5-1.5 5 1.5" />
    </>
  ),
  unknown: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M9.6 9.5a2.5 2.5 0 1 1 3.4 2.3c-.6.3-1 .8-1 1.5v.7M12 16.8v.1" />
    </>
  )
};

export function CategoryIcon({ category, ...p }: P & { category: Category }) {
  return <Svg {...p}>{CATEGORY_PATHS[category]}</Svg>;
}

/** The game's own icon for an item, on its 2:3 card; the category glyph when there is none. */
export function ItemIcon({ icon, category, className = 'item-card' }: { icon?: number | null; category: Category; className?: string }) {
  return (
    <span className={className}>
      {icon ? <img src={`/icons/${icon}.webp`} alt="" loading="lazy" draggable={false} /> : <CategoryIcon category={category} size={18} />}
    </span>
  );
}

export const Chevron = ({ dir = 'right', ...p }: P & { dir?: 'left' | 'right' | 'down' }) => (
  <Svg {...p}>
    <path d={dir === 'left' ? 'M14.5 6 8.5 12l6 6' : dir === 'down' ? 'M6 9.5l6 6 6-6' : 'M9.5 6l6 6-6 6'} />
  </Svg>
);

export const Plus = (p: P) => (
  <Svg {...p}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);

export const Cross = (p: P) => (
  <Svg {...p}>
    <path d="M6 6l12 12M18 6 6 18" />
  </Svg>
);

export const FolderOpen = (p: P) => (
  <Svg {...p}>
    <path d="M3.5 7V5.5h6l2 2h8.5V10M3.5 7v12h15l2.5-9H6.5L3.5 19" />
  </Svg>
);

export const Refresh = (p: P) => (
  <Svg {...p}>
    <path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4" />
  </Svg>
);

/** The coiled sword of the bonfire, the app's mark. */
export function Bonfire({ size = 28, lit = true, className }: { size?: number; lit?: boolean; className?: string }) {
  const glow = `bf-glow${useId()}`;
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" className={className} aria-hidden="true">
      <defs>
        <radialGradient id={glow} cx="50%" cy="85%" r="60%">
          <stop offset="0" stopColor="#ffb054" stopOpacity={lit ? 0.9 : 0} />
          <stop offset="0.6" stopColor="#c2501a" stopOpacity={lit ? 0.25 : 0} />
          <stop offset="1" stopColor="#000" stopOpacity="0" />
        </radialGradient>
      </defs>
      <circle cx="16" cy="22" r="12" fill={`url(#${glow})`} />
      <path d="M16 3l1.3 3v15h-2.6V6z" fill="#e9e3d3" />
      <path d="M14.7 9c3 .7 3 2-0 2.8-3 .8-3 2 0 2.8 3 .8 3 2 0 2.8" fill="none" stroke="#3a2a1c" strokeWidth="0.9" />
      <rect x="12.2" y="21" width="7.6" height="1.3" rx="0.5" fill="#cfc6b1" />
      {lit && (
        <path
          d="M11 29c.6-2.6 2.4-3.3 2.6-5.3 1.2 1.6.8 3.1 1.9 4.2.3-1.9 1.5-2.7 1.8-4.1 1.4 2.1 2.4 3.8 1.5 5.2z"
          fill="#ffa245"
        />
      )}
    </svg>
  );
}
