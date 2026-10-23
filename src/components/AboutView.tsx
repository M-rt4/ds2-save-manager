import { Fragment, useEffect, useState, version as reactVersion } from 'react';
import { getTauriVersion, getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { aboutApi, type GameInfo, type InstallSource } from '../api';
import { useStore } from '../store';
import { useT, type Key } from '../i18n';
import { editionName } from '../game';
import { Bonfire } from './icons';

const AUTHOR = { label: 'github.com/M-rt4', url: 'https://github.com/M-rt4' };
const REPO = { label: 'github.com/M-rt4/ds2-save-manager', url: 'https://github.com/M-rt4/ds2-save-manager' };

/** The projects NOTICE credits, with what each one gave. */
const CREDITS: { name: string; url: string; license?: string; what: Key }[] = [
  {
    name: 'darthdemono/sl2-analyzer',
    url: 'https://github.com/darthdemono/sl2-analyzer',
    license: 'Apache-2.0',
    what: 'about.credit.sl2'
  },
  {
    name: 'alfizari/Dark-Souls-2-Save-Editor-PS4-PC',
    url: 'https://github.com/alfizari/Dark-Souls-2-Save-Editor-PS4-PC',
    license: 'MIT',
    what: 'about.credit.alfizari'
  },
  { name: 'TKGP/SoulsFormats', url: 'https://github.com/JKAnderson/SoulsFormats', what: 'about.credit.soulsformats' },
  { name: 'Atvaark/BinderTool', url: 'https://github.com/Atvaark/BinderTool', what: 'about.credit.bindertool' },
  { name: 'soulsmods/Paramdex', url: 'https://github.com/soulsmods/Paramdex', what: 'about.credit.paramdex' }
];

const SOURCE: Record<InstallSource, Key> = {
  steam: 'games.source.steam',
  registry: 'games.source.registry',
  manual: 'games.source.manual'
};

interface UaData {
  getHighEntropyValues(hints: string[]): Promise<{ fullVersionList?: { brand: string; version: string }[] }>;
}

/** The WebView2 runtime's full version; the user agent string only carries its major. */
async function webviewVersion(): Promise<string | null> {
  const data = (navigator as Navigator & { userAgentData?: UaData }).userAgentData;
  const brands = await data?.getHighEntropyValues(['fullVersionList']).catch(() => undefined);
  const full = brands?.fullVersionList?.find((b) => b.brand.includes('WebView2'))?.version;
  return full ?? /Edg\/([\d.]+)/.exec(navigator.userAgent)?.[1] ?? null;
}

function Link({ url, children }: { url: string; children: string }) {
  const fail = useStore((s) => s.fail);
  return (
    <button className="link" title={url} onClick={() => void openUrl(url).catch(fail)}>
      {children}
    </button>
  );
}

/** The About tab of the settings: the app, its author and sources, and the games on this PC. */
export function AboutView() {
  const t = useT();
  const games = useStore((s) => s.games);
  const gamesLoaded = useStore((s) => s.gamesLoaded);
  const [app, setApp] = useState<{ version: string; tauri: string } | null>(null);
  const [os, setOs] = useState<string | null>(null);
  const [webview, setWebview] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getVersion(), getTauriVersion()])
      .then(([version, tauri]) => setApp({ version, tauri }))
      .catch(() => undefined);
    aboutApi
      .systemInfo()
      .then((s) => setOs(s.os ? `${s.os}, ${s.arch}` : s.arch))
      .catch(() => undefined);
    void webviewVersion().then(setWebview);
  }, []);

  useEffect(() => {
    if (!gamesLoaded) void useStore.getState().refreshGames();
  }, [gamesLoaded]);

  const system = [os, webview && `WebView2 ${webview}`].filter(Boolean).join(' · ');

  return (
    <div className="about">
      <div className="about-head">
        <Bonfire size={44} />
        <div>
          <div className="about-mark">
            DARK SOULS II <span>{t('app.subtitle')}</span>
          </div>
          <div className="about-version">{app ? t('about.version', { v: app.version }) : ' '}</div>
        </div>
      </div>

      <p className="about-lead">{t('about.lead')}</p>
      <div className="kv">
        <div className="k">{t('about.author')}</div>
        <div className="v">
          Mert · <Link url={AUTHOR.url}>{AUTHOR.label}</Link>
        </div>
        <div className="k">{t('about.repo')}</div>
        <div className="v">
          <Link url={REPO.url}>{REPO.label}</Link> <span className="dim">({t('about.private')})</span>
        </div>
        <div className="k">{t('about.stack')}</div>
        <div className="v">
          Tauri {app?.tauri ?? '2'} · React {reactVersion} · Rust · TypeScript
        </div>
        <div className="k">{t('about.system')}</div>
        <div className="v">{system || '—'}</div>
      </div>

      <h3 className="about-h">{t('about.games')}</h3>
      {games.map((g) => (
        <GameFacts key={g.edition} game={g} />
      ))}

      <h3 className="about-h">{t('about.credits')}</h3>
      <ul className="credits">
        {CREDITS.map((c) => (
          <li key={c.name}>
            <Link url={c.url}>{c.name}</Link>
            {c.license && <span className="dim"> · {c.license}</span>}
            <span className="what">{t(c.what)}</span>
          </li>
        ))}
      </ul>
      <p className="note">{t('about.assets')}</p>
      <p className="note">{t('about.trademark')}</p>
      <p className="note">© 2026 Mert</p>
    </div>
  );
}

function GameFacts({ game }: { game: GameInfo }) {
  const t = useT();
  const unread = <span className="dim">{t('about.notRead')}</span>;
  return (
    <div className="about-game">
      <div className={`about-game-name ${game.edition}`}>
        {editionName(game.edition)} <span className="dim">{t('about.steamApp', { id: game.steamAppId })}</span>
      </div>
      <div className="kv">
        {game.installs.length === 0 ? (
          <>
            <div className="k">{t('about.install')}</div>
            <div className="v dim">{t('about.notInstalled')}</div>
          </>
        ) : (
          game.installs.map((i) => (
            <Fragment key={i.path}>
              <div className="k">{t('about.install')}</div>
              <div className="v">
                {i.path} <span className="dim">({t(SOURCE[i.source])})</span>
              </div>
              <div className="k">{t('about.exeVersion')}</div>
              <div className="v">{i.exeVersion ?? unread}</div>
              <div className="k">{t('about.regulation')}</div>
              <div className="v">{i.regulationVersion ?? unread}</div>
            </Fragment>
          ))
        )}
        <div className="k">{t('about.saves')}</div>
        <div className="v">{game.savesRoot ?? '—'}</div>
        <div className="k">{t('about.accounts')}</div>
        <div className="v">
          {game.saves.length === 0
            ? '—'
            : game.saves.map((s) => (
                <div key={s.path}>
                  {s.account ?? s.folder}
                  {s.account && <span className="dim"> ({s.folder})</span>}
                  <span className="dim">
                    {' — '}
                    {s.exists
                      ? t('about.hasSave', { file: s.fileName, n: s.slots.length })
                      : t('about.noSave', { file: s.fileName })}
                  </span>
                </div>
              ))}
        </div>
      </div>
    </div>
  );
}
