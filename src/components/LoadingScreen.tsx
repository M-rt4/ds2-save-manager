import type { ReactNode } from 'react';
import { useStore, type Boot } from '../store';
import { useT } from '../i18n';
import { editionName } from '../game';
import { Bonfire } from './icons';

// Between the window opening and the app being ready: the same screen as the first frame drawn
// by index.html, now saying what startup is doing and, step by step, what it found.

type StepState = Boot[keyof Boot];

function Step({ state, label, children }: { state: StepState; label: string; children?: ReactNode }) {
  return (
    <li className="boot-step" data-state={state}>
      <span className="boot-mark" aria-hidden />
      <div>
        <div className="boot-label">{label}</div>
        {children && <div className="boot-found">{children}</div>}
      </div>
    </li>
  );
}

export function LoadingScreen({ boot }: { boot: Boot }) {
  const t = useT();
  const games = useStore((s) => s.games);
  const saves = useStore((s) => s.saves);
  const doc = useStore((s) => s.doc);
  const installs = games.flatMap((g) => g.installs.map((i) => ({ edition: g.edition, ...i })));

  return (
    <div className="boot" role="status" aria-live="polite">
      <Bonfire size={84} className="boot-fire" />
      <div className="boot-word">DARK SOULS II</div>
      <div className="boot-sub">{t('app.subtitle')}</div>
      <ul className="boot-steps">
        <Step state={boot.games} label={t('boot.games')}>
          {boot.games === 'done' &&
            (installs.length
              ? installs.map((i) => (
                  <div key={i.path}>
                    {editionName(i.edition)}
                    {i.exeVersion && <> · {t('games.exeVersion', { v: i.exeVersion })}</>}
                    {i.regulationVersion && <> · {t('games.regulation', { v: i.regulationVersion })}</>}
                  </div>
                ))
              : t('boot.noGames'))}
        </Step>
        <Step state={boot.saves} label={t('boot.saves')}>
          {boot.saves === 'done' && (saves.length === 0 ? t('boot.noSaves') : saves.length === 1 ? t('boot.saveFound') : t('boot.savesFound', { n: saves.length }))}
        </Step>
        <Step state={boot.open} label={t('boot.open')}>
          {boot.open === 'done' && doc && t('boot.opened', { file: doc.fileName })}
          {boot.open === 'none' && t('boot.nothingToOpen')}
        </Step>
      </ul>
    </div>
  );
}
