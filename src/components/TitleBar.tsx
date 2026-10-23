import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { aboutApi } from '../api';
import { useStore } from '../store';
import { useT } from '../i18n';
import { Bonfire } from './icons';

const win = () => getCurrentWindow();

/** Shown in the title bar at all times, and in the About view. */
export const AUTHOR = 'Mert';

export function TitleBar() {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const setLang = useStore((s) => s.setLang);
  const openSettings = useStore((s) => s.setSettingsOpen);
  const [devLabel, setDevLabel] = useState<string | null>(null);
  const [testRoot, setTestRoot] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void getVersion().then(setVersion).catch(() => setVersion(null));
    void aboutApi.devLabel().then(setDevLabel).catch(() => setDevLabel(null));
    void aboutApi.testSavesRoot().then(setTestRoot).catch(() => setTestRoot(null));
  }, []);

  return (
    <header className="titlebar" data-tauri-drag-region>
      <Bonfire size={24} />
      <span className="wordmark" data-tauri-drag-region>
        DARK SOULS II
      </span>
      <span className="subtitle" data-tauri-drag-region>
        {t('app.subtitle')}
      </span>
      <span className="byline" data-tauri-drag-region>
        {version && <>v{version} · </>}
        {t('app.byline', { author: AUTHOR })}
      </span>
      {devLabel && (
        <span className="dev-badge" data-tauri-drag-region title="Development copy">
          DEV · {devLabel}
        </span>
      )}
      {(testRoot || devLabel) && (
        <span
          className="test-badge"
          data-tauri-drag-region
          title={[testRoot && t('app.testFolderHint', { path: testRoot }), devLabel && t('app.devDataHint')].filter(Boolean).join('\n\n')}
        >
          {testRoot ? t('app.testFolder') : t('app.devData')}
        </span>
      )}
      <div className="spacer" data-tauri-drag-region />
      <button className="lang" onClick={() => openSettings(true)}>
        {t('settings.open')}
      </button>
      <button className="lang" onClick={() => setLang(lang === 'tr' ? 'en' : 'tr')}>
        {t('lang.switch')}
      </button>
      <button className="winbtn" title={t('window.minimize')} onClick={() => void win().minimize()}>
        <svg width="11" height="11" viewBox="0 0 11 11">
          <path d="M1 5.5h9" stroke="currentColor" />
        </svg>
      </button>
      <button className="winbtn" title={t('window.maximize')} onClick={() => void win().toggleMaximize()}>
        <svg width="11" height="11" viewBox="0 0 11 11">
          <rect x="1.5" y="1.5" width="8" height="8" fill="none" stroke="currentColor" />
        </svg>
      </button>
      <button className="winbtn close" title={t('window.close')} onClick={() => void win().close()}>
        <svg width="11" height="11" viewBox="0 0 11 11">
          <path d="M1.5 1.5l8 8M9.5 1.5l-8 8" stroke="currentColor" />
        </svg>
      </button>
    </header>
  );
}
