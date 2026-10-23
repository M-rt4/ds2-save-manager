import { useEffect, useState } from 'react';
import { useStore } from '../store';
import { useT } from '../i18n';
import { ChangeList } from './ChangeList';
import { SettingsView } from './SettingsView';

/** Toasts as the game's message strip, and the game's Yes / No prompt. */
export function Overlays() {
  const toasts = useStore((s) => s.toasts);
  const confirm = useStore((s) => s.confirm);
  const report = useStore((s) => s.report);
  const settingsOpen = useStore((s) => s.settingsOpen);
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
      {report && <Report />}
      {settingsOpen && <SettingsView />}
    </>
  );
}

/** After a transfer: every change made to the written file, with a way to stop showing it. */
function Report() {
  const t = useT();
  const report = useStore((s) => s.report)!;
  const close = useStore((s) => s.showReport);
  const setSettings = useStore((s) => s.setSettings);
  const [dontShow, setDontShow] = useState(false);
  const done = () => {
    if (dontShow) setSettings({ showTransferReport: false });
    close(null);
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => (e.key === 'Escape' || e.key === 'Enter') && done();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
  return (
    <div className="scrim">
      <div className="panel dialog wide" role="dialog" aria-label={t('report.title')}>
        <div className="panel-head">
          <h2 className="panel-title">{t('report.title')}</h2>
          <hr className="rule" />
        </div>
        <div className="report-body">
          <p className="note" style={{ marginTop: 0, userSelect: 'text' }}>
            {t('report.written', { path: report.path })}
          </p>
          <ChangeList changes={report.changes} />
        </div>
        <div className="report-foot">
          <label className="check">
            <input type="checkbox" checked={dontShow} onChange={(e) => setDontShow(e.target.checked)} />
            {t('report.dontShow')}
          </label>
          <button className="btn" onClick={done}>
            {t('report.ok')}
          </button>
        </div>
      </div>
    </div>
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
