import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Edition } from '../api';
import { choosePath } from '../browse';
import { usePad } from '../pad';
import { useHasChanges, useStore } from '../store';
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

  const games = useStore((s) => s.games);
  const addGameFolder = useStore((s) => s.addGameFolder);
  const removeGameFolder = useStore((s) => s.removeGameFolder);
  const pickGameDir = async () => {
    const dir = await choosePath({ mode: 'folder' });
    if (dir) await addGameFolder(dir);
  };

  // With only a gamepad there is no other way out (the window buttons stay out of its reach).
  const padMode = usePad((s) => s.active);
  const padName = usePad((s) => s.name);
  const changes = useHasChanges();
  const quit = () => {
    close(false);
    useStore.getState().ask(t(changes ? 'pad.quitUnsaved' : 'pad.quitConfirm'), () => void getCurrentWindow().close());
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
          {tab === 'general' && (
            <div className="setting">
              <div>
                <b>{t('pad.detected')}</b>
                <span>{padName ?? t('pad.none')}</span>
              </div>
            </div>
          )}
          {tab === 'general' && padMode && (
            <div className="setting">
              <div>
                <b>{t('pad.quit')}</b>
                <span>{t('pad.quitHint')}</span>
              </div>
              <button className="btn" onClick={quit}>
                {t('pad.quit')}
              </button>
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
              {(['scholar', 'original'] as Edition[]).map((e) => {
                const installs = games.find((g) => g.edition === e)?.installs ?? [];
                return (
                  <div className="setting" key={e}>
                    <div>
                      <b>{editionName(e)}</b>
                      {installs.length === 0 && <span>{t('games.notFound')}</span>}
                      {installs.map((i) => (
                        <span key={i.path} className="path">
                          {i.path} ({t(`games.source.${i.source}` as Key)}
                          {i.exeVersion ? `, ${i.exeVersion}` : ''}){' '}
                          {i.source === 'manual' && (
                            <button className="btn quiet small" onClick={() => void removeGameFolder(i.path)}>
                              {t('games.forget')}
                            </button>
                          )}
                        </span>
                      ))}
                    </div>
                  </div>
                );
              })}
              <div style={{ paddingTop: 12 }}>
                <button className="btn" onClick={() => void pickGameDir()}>
                  {t('games.addFolder')}
                </button>
              </div>
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
