import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import type { Edition } from '../api';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';
import { editionName } from '../game';
import { AboutView } from './AboutView';

type SettingsTab = 'general' | 'transfer' | 'games' | 'about';
const TABS: { id: SettingsTab; key: Key }[] = [
  { id: 'general', key: 'settings.general' },
  { id: 'transfer', key: 'settings.transfer' },
  { id: 'games', key: 'settings.games' },
  { id: 'about', key: 'about.title' }
];

/** The app's options, laid out like the game's own option menu: tabs over a list of rows. */
export function SettingsView() {
  const t = useT();
  const [tab, setTab] = useState<SettingsTab>('general');
  const close = useStore((s) => s.setSettingsOpen);
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  const lang = useStore((s) => s.lang);
  const setLang = useStore((s) => s.setLang);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && close(false);
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [close]);

  const pickGameDir = async (edition: Edition) => {
    const dir = await open({ directory: true, multiple: false, title: editionName(edition) });
    if (typeof dir === 'string') setSettings({ gameDirs: { ...settings.gameDirs, [edition]: dir } });
  };

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && close(false)}>
      <div className="panel dialog wide settings" role="dialog" aria-label={t('settings.title')}>
        <div className="panel-head">
          <h2 className="panel-title">{t('settings.title')}</h2>
        </div>
        <nav className="tabs" role="tablist">
          {TABS.map((x) => (
            <button key={x.id} className="tab" role="tab" aria-selected={tab === x.id} onClick={() => setTab(x.id)}>
              {t(x.key)}
            </button>
          ))}
        </nav>
        <hr className="rule" style={{ margin: 0 }} />
        <div className="settings-body">
          {tab === 'general' && (
            <div className="setting">
              <div>
                <b>{t('settings.language')}</b>
              </div>
              <div className="seg" role="group">
                <button aria-pressed={lang === 'tr'} onClick={() => setLang('tr')}>
                  Türkçe
                </button>
                <button aria-pressed={lang === 'en'} onClick={() => setLang('en')}>
                  English
                </button>
              </div>
            </div>
          )}
          {tab === 'transfer' && (
            <label className="setting check">
              <input
                type="checkbox"
                checked={settings.showTransferReport}
                onChange={(e) => setSettings({ showTransferReport: e.target.checked })}
              />
              <span>
                <b>{t('settings.showReport')}</b>
                <span>{t('settings.showReportHint')}</span>
              </span>
            </label>
          )}
          {tab === 'games' && (
            <>
              <p className="note" style={{ marginTop: 0 }}>
                {t('settings.gamesHint')}
              </p>
              {(['original', 'scholar'] as Edition[]).map((e) => (
                <div className="setting" key={e}>
                  <div>
                    <b>{editionName(e)}</b>
                    <span className="path">{settings.gameDirs[e] ?? t('settings.notSet')}</span>
                  </div>
                  <div style={{ display: 'flex', gap: 8 }}>
                    <button className="btn" onClick={() => void pickGameDir(e)}>
                      {t('settings.chooseFolder')}
                    </button>
                    {settings.gameDirs[e] && (
                      <button
                        className="btn quiet"
                        onClick={() => {
                          const next = { ...settings.gameDirs };
                          delete next[e];
                          setSettings({ gameDirs: next });
                        }}
                      >
                        {t('settings.clear')}
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </>
          )}
          {tab === 'about' && <AboutView />}
        </div>
        <div className="choices" style={{ display: 'flex', justifyContent: 'flex-end', padding: '12px 22px 20px' }}>
          <button className="btn" onClick={() => close(false)}>
            {t('settings.done')}
          </button>
        </div>
      </div>
    </div>
  );
}
