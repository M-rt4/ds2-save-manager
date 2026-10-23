import { useEffect, useState } from 'react';
import { useStore } from '../store';
import { useT } from '../i18n';

/** Toasts as the game's message strip, and the game's Yes / No prompt. */
export function Overlays() {
  const toasts = useStore((s) => s.toasts);
  const confirm = useStore((s) => s.confirm);
  return (
    <>
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((x) => (
          <div key={x.id} className={`toast ${x.tone}`}>
            {x.text}
          </div>
        ))}
      </div>
      {confirm && <Confirm text={confirm.text} onYes={confirm.onYes} />}
    </>
  );
}

function Confirm({ text, onYes }: { text: string; onYes: () => void }) {
  const t = useT();
  const close = useStore((s) => s.closeConfirm);
  const [yes, setYes] = useState(true);

  const answer = (ok: boolean) => {
    close();
    if (ok) onYes();
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') answer(false);
      else if (e.key === 'Enter') answer(yes);
      else if (['ArrowLeft', 'ArrowRight', 'Tab'].includes(e.key)) {
        e.preventDefault();
        setYes((v) => !v);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && answer(false)}>
      <div className="panel dialog" role="alertdialog" aria-label={text}>
        <p className="text">{text}</p>
        <div className="choices">
          <button className="row" aria-selected={yes} onMouseEnter={() => setYes(true)} onClick={() => answer(true)}>
            {t('confirm.yes')}
          </button>
          <button className="row" aria-selected={!yes} onMouseEnter={() => setYes(false)} onClick={() => answer(false)}>
            {t('confirm.no')}
          </button>
        </div>
      </div>
    </div>
  );
}
