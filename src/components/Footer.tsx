import { useStore, useHasChanges } from '../store';
import { useT } from '../i18n';
import { usePadHints } from '../pad';
import { Glyph, PadHints } from './PadHints';

export function Footer() {
  const t = useT();
  const doc = useStore((s) => s.doc);
  const busy = useStore((s) => s.busy);
  const gameRunning = useStore((s) => s.gameRunning);
  const save = useStore((s) => s.saveChanges);
  const revert = useStore((s) => s.revert);
  const changes = useHasChanges();
  // Pad glyphs while the pad was the last input, keyboard keys otherwise.
  const pad = usePadHints();

  const state = gameRunning ? 'blocked' : changes ? 'dirty' : '';
  const text = gameRunning ? t('footer.gameRunning') : changes ? t('footer.unsaved') : doc ? t('footer.saved') : '';

  return (
    <footer className="footer">
      <div className="footer-status">
        <span className={`state ${state}`}>
          {doc && <span className="dot" />}
          {text}
        </span>
        {/* Why Save stays off, under the state like the running game's reason. */}
        {doc?.readOnly && (
          <span className="guard-note" title={doc.readOnlyReason?.message}>
            {t('guard.readOnly')}
          </span>
        )}
      </div>
      <span className="spacer" />
      {pad ? <PadHints /> : doc && !doc.readOnly && <span className="note" style={{ margin: 0 }}>{t('footer.backupNote')}</span>}
      <button className="btn quiet" disabled={!doc || !changes || busy} onClick={revert}>
        {t('footer.revert')}
      </button>
      <button className="btn ember" disabled={!doc || !changes || busy || gameRunning || doc.readOnly} onClick={() => void save()}>
        {t('footer.save')} {pad ? <Glyph button="start" family={pad} /> : <kbd>Ctrl S</kbd>}
      </button>
    </footer>
  );
}
