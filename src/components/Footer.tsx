import { useStore, useHasChanges } from '../store';
import { useT } from '../i18n';

export function Footer() {
  const t = useT();
  const doc = useStore((s) => s.doc);
  const busy = useStore((s) => s.busy);
  const gameRunning = useStore((s) => s.gameRunning);
  const save = useStore((s) => s.saveChanges);
  const revert = useStore((s) => s.revert);
  const changes = useHasChanges();

  const state = gameRunning ? 'blocked' : changes ? 'dirty' : '';
  const text = gameRunning ? t('footer.gameRunning') : changes ? t('footer.unsaved') : doc ? t('footer.saved') : '';

  return (
    <footer className="footer">
      <span className={`state ${state}`}>
        {doc && <span className="dot" />}
        {text}
      </span>
      <span className="spacer" />
      {doc && <span className="note" style={{ margin: 0 }}>{t('footer.backupNote')}</span>}
      <button className="btn quiet" disabled={!doc || !changes || busy} onClick={revert}>
        {t('footer.revert')}
      </button>
      <button className="btn ember" disabled={!doc || !changes || busy || gameRunning} onClick={() => void save()}>
        {t('footer.save')} <kbd>Ctrl S</kbd>
      </button>
    </footer>
  );
}
