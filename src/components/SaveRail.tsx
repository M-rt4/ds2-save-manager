import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { choosePath } from '../browse';
import { gamesApi, isSaveError, type FoundSave, type GameInfo, type GameSave, type Install, type InstallSource } from '../api';
import { useStore } from '../store';
import { errorText, useT, type Key, type Lang } from '../i18n';
import { editionName } from '../game';
import { FolderOpen, Plus, Refresh } from './icons';
import { PanelChip } from './PadHints';

function relative(ms: number, lang: string): string {
  const diff = (ms - Date.now()) / 1000;
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' });
  const steps: [number, Intl.RelativeTimeFormatUnit][] = [
    [60, 'second'],
    [3600, 'minute'],
    [86400, 'hour'],
    [86400 * 30, 'day'],
    [86400 * 365, 'month']
  ];
  for (const [limit, unit] of steps) {
    if (Math.abs(diff) < limit) {
      const div = unit === 'second' ? 1 : unit === 'minute' ? 60 : unit === 'hour' ? 3600 : unit === 'day' ? 86400 : 86400 * 30;
      return rtf.format(Math.round(diff / div), unit);
    }
  }
  return new Date(ms).toLocaleDateString(lang);
}

const samePath = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

const SOURCE: Record<InstallSource, Key> = {
  steam: 'games.source.steam',
  registry: 'games.source.registry',
  manual: 'games.source.manual'
};

/** Explorer with the file (or folder) selected; a path that is gone shows as an error toast. */
function reveal(path: string) {
  revealItemInDir(path).catch((e: unknown) => useStore.getState().fail(e));
}

export function SaveRail() {
  const t = useT();
  const games = useStore((s) => s.games);
  const doc = useStore((s) => s.doc);
  const openSave = useStore((s) => s.openSave);
  const refresh = useStore((s) => s.refreshSaves);
  const setGames = useStore((s) => s.setGames);
  const toast = useStore((s) => s.toast);
  const fail = useStore((s) => s.fail);

  const root = games.find((g) => g.savesRoot)?.savesRoot ?? null;

  const pick = async () => {
    const path = await choosePath({ mode: 'file', extensions: ['sl2'], filterName: 'Dark Souls II save', defaultPath: root ?? undefined });
    if (path) await openSave(path);
  };

  const addFolder = async () => {
    const dir = await choosePath({ mode: 'folder' });
    if (!dir) return;
    try {
      const list = await gamesApi.addGameFolder(dir);
      setGames(list);
      const game = list.find((g) => g.installs.some((i) => samePath(i.path, dir)));
      if (game) toast(t('games.added', { path: dir, edition: editionName(game.edition) }));
    } catch (e) {
      if (isSaveError(e) && e.code === 'notGameFolder') toast(t('games.notGameFolder'), 'error');
      else fail(e);
    }
  };

  const forget = async (install: Install) => {
    try {
      setGames(await gamesApi.removeGameFolder(install.path));
    } catch (e) {
      fail(e);
    }
  };

  const outside = doc && !games.some((g) => g.saves.some((s) => s.exists && samePath(s.path, doc.path)));

  return (
    <section className="panel">
      <PanelChip index={0} label={t('games.title')} />
      <div className="panel-head">
        <h2 className="panel-title">{t('games.title')}</h2>
        <hr className="rule" />
      </div>
      <div className="panel-body">
        {root && (
          <div className="games-root">
            <div className="group-label">{t('games.savesRoot')}</div>
            <button className="place" data-external title={t('file.reveal')} onClick={() => reveal(root)}>
              {root}
            </button>
          </div>
        )}
        {games.map((g) => (
          <GameSection key={g.edition} game={g} onAdd={() => void addFolder()} onForget={(i) => void forget(i)} />
        ))}
        {root && (
          <p className="note games-note">{t('games.shared', { original: 'DARKSII0000.sl2', scholar: 'DS2SOFS0000.sl2' })}</p>
        )}
        {outside && doc && (
          <div className="game">
            <div className="group-label">{t('rail.otherFolder')}</div>
            <button className="row save-row" aria-selected title={doc.path}>
              <span className="file">
                <b>{doc.fileName}</b>
                <EditionBadge edition={doc.sourceEdition} />
              </span>
              <span className="meta place">{doc.path}</span>
            </button>
          </div>
        )}
      </div>
      <div className="rail-actions">
        <button className="row" onClick={() => void pick()}>
          <FolderOpen size={17} />
          {t('rail.open')}
        </button>
        <button className="row" onClick={() => void addFolder()}>
          <Plus size={17} />
          {t('games.addFolder')}
        </button>
        <button className="row" onClick={() => void refresh()}>
          <Refresh size={17} />
          {t('rail.rescan')}
        </button>
      </div>
    </section>
  );
}

function GameSection({ game, onAdd, onForget }: { game: GameInfo; onAdd: () => void; onForget: (i: Install) => void }) {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const doc = useStore((s) => s.doc);
  return (
    <div className="game">
      <h3 className={`game-name ${game.edition}`}>{editionName(game.edition)}</h3>

      <div className="group-label">{t('games.install')}</div>
      {game.installs.length === 0 ? (
        <>
          <p className="game-none">{t('games.notFound')}</p>
          <button className="row game-add" onClick={onAdd}>
            <Plus size={15} />
            {t('games.addFolder')}
          </button>
        </>
      ) : (
        game.installs.map((i) => (
          <div key={i.path} className="install">
            <div className="place">{i.path}</div>
            <div className="install-meta">
              <span>{t(SOURCE[i.source])}</span>
              {i.exeVersion && <span>{t('games.exeVersion', { v: i.exeVersion })}</span>}
              {i.regulationVersion && <span>{t('games.regulation', { v: i.regulationVersion })}</span>}
            </div>
            {!i.exe && <div className="game-warn">{t('games.exeMissing')}</div>}
            <div className="install-actions">
              <button className="btn quiet" data-external onClick={() => reveal(i.exe ?? i.path)}>
                {t('file.reveal')}
              </button>
              {i.source === 'manual' && (
                <button className="btn quiet" onClick={() => onForget(i)}>
                  {t('games.forget')}
                </button>
              )}
            </div>
          </div>
        ))
      )}

      <div className="group-label">{t('games.saves')}</div>
      {game.saves.length === 0 && <p className="game-none">{t('games.noAccounts')}</p>}
      {game.saves.map((s) =>
        s.exists ? (
          <SaveRow key={s.path} save={s} lang={lang} selected={!!doc && samePath(doc.path, s.path)} />
        ) : (
          <div key={s.path} className="row save-row free" title={s.path}>
            <span className="file">
              <b>{s.fileName}</b>
            </span>
            <span className="meta">{t('games.folder', { name: s.folder })}</span>
            {s.account && <span className="meta">{t('rail.account', { id: s.account })}</span>}
            <span className="meta">{t('games.willWrite')}</span>
          </div>
        )
      )}
    </div>
  );
}

function SaveRow({ save, selected, lang }: { save: GameSave; selected: boolean; lang: Lang }) {
  const t = useT();
  const openSave = useStore((s) => s.openSave);
  const chars = save.slots.map((c) => `${c.name} ${t('slots.level', { n: c.level })}`).join(' · ');
  return (
    <button className="row save-row" aria-selected={selected} title={save.path} onClick={() => void openSave(save.path)}>
      <span className="file">
        <b>{save.fileName}</b>
        <span className="when">{relative(save.modified, lang)}</span>
      </span>
      <span className="meta">{t('games.folder', { name: save.folder })}</span>
      {save.account && <span className="meta">{t('rail.account', { id: save.account })}</span>}
      <span className={save.error ? 'chars bad' : 'chars'}>
        {save.error
          ? t('games.unreadable', { msg: errorText(lang, save.error) })
          : chars || t('games.noCharacters')}
      </span>
    </button>
  );
}

export function EditionBadge({ edition }: { edition: FoundSave['edition'] }) {
  const t = useT();
  if (!edition) return null;
  return (
    <span className={`badge ${edition}`}>
      {edition === 'scholar' ? t('edition.scholarShort') : t('edition.originalShort')}
    </span>
  );
}
