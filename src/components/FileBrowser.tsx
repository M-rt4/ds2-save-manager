import { useEffect, useState } from 'react';
import { filesApi, useBrowser, type BrowseOptions, type Listing, type Place } from '../browse';
import { useStore } from '../store';
import { errorText, useT, type Key } from '../i18n';
import { editionName } from '../game';
import { Chevron, FolderOpen } from './icons';

// The in-app file browser, used instead of the system's windows while a gamepad drives the
// app: places on the left (save folders, game installs, home, drives or removable media), the
// folder's contents on the right, folders first. It only lists; choosing returns a path.

const PLACE: Record<Place['kind'], Key> = {
  saves: 'browse.saves',
  home: 'browse.home',
  drive: 'browse.drive',
  media: 'browse.media'
};

const sepOf = (path: string) => (path.includes('\\') ? '\\' : '/');
const join = (dir: string, name: string) => (dir.endsWith(sepOf(dir)) ? dir + name : dir + sepOf(dir) + name);
const dirOf = (path: string) => path.slice(0, Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/')));
const nameOf = (path: string) => path.slice(Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/')) + 1);

export function FileBrowser() {
  const request = useBrowser((s) => s.request);
  return request ? <Browser request={request} /> : null;
}

function Browser({ request }: { request: BrowseOptions & { resolve: (p: string | null) => void } }) {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const games = useStore((s) => s.games);
  const [places, setPlaces] = useState<Place[]>([]);
  const [listing, setListing] = useState<Listing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState(request.mode === 'save' && request.defaultPath ? nameOf(request.defaultPath) : '');
  const exts = request.extensions?.map((e) => e.toLowerCase());

  const done = (path: string | null) => {
    useBrowser.setState({ request: null });
    request.resolve(path);
  };

  const go = async (path: string) => {
    try {
      setListing(await filesApi.list(path));
      setError(null);
      return true;
    } catch (e) {
      setError(errorText(lang, e));
      return false;
    }
  };

  useEffect(() => {
    void filesApi.places().then(async (p) => {
      setPlaces(p);
      // A suggested folder opens itself, a suggested file its folder; a bare file name, or
      // nothing, starts at the first place (the save folder).
      const start = request.defaultPath ?? '';
      if (start && request.mode !== 'save' && (await go(start))) return;
      if (dirOf(start) && (await go(dirOf(start)))) return;
      if (p[0]) void go(p[0].path);
    });
    // Escape closes the browser only, not the settings it may have been opened from.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopImmediatePropagation();
      done(null);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  const installs = games.flatMap((g) => g.installs.map((i) => ({ path: i.path, label: editionName(g.edition) })));
  const shown = (listing?.entries ?? []).filter(
    (e) => e.dir || (request.mode !== 'folder' && (!exts || exts.some((x) => e.name.toLowerCase().endsWith('.' + x))))
  );
  const fileName = exts && name && !exts.some((x) => name.toLowerCase().endsWith('.' + x)) ? `${name}.${exts[0]}` : name;
  const title = request.title ?? t(request.mode === 'folder' ? 'browse.titleFolder' : request.mode === 'save' ? 'browse.titleSave' : 'browse.titleFile');

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && done(null)}>
      <div className="panel dialog wide browser" role="dialog" aria-label={title}>
        <div className="panel-head">
          <h2 className="panel-title">{title}</h2>
          <hr className="rule" />
        </div>
        <div className="browser-grid">
          <nav className="browser-places">
            {places.map((p) => (
              <button key={p.path} className="row" aria-selected={listing?.path === p.path} title={p.path} onClick={() => void go(p.path)}>
                <span>
                  <b>{t(PLACE[p.kind])}</b>
                  <small>{p.path}</small>
                </span>
              </button>
            ))}
            {installs.map((p) => (
              <button key={p.path} className="row" aria-selected={listing?.path === p.path} title={p.path} onClick={() => void go(p.path)}>
                <span>
                  <b>{p.label}</b>
                  <small>{p.path}</small>
                </span>
              </button>
            ))}
          </nav>
          <div className="browser-main">
            <div className="browser-path">
              <button className="btn quiet" disabled={!listing?.parent} onClick={() => listing?.parent && void go(listing.parent)}>
                <Chevron dir="left" size={15} />
                {t('browse.up')}
              </button>
              <span className="place">{listing?.path}</span>
            </div>
            <div className="browser-list panel-body">
              {error && <p className="warn">{error}</p>}
              {listing && shown.length === 0 && !error && <p className="empty-note">{t('browse.empty')}</p>}
              {shown.map((e, i) => (
                <button
                  key={e.path}
                  className="row browser-entry"
                  data-pad-default={i === 0 ? '' : undefined}
                  aria-selected={!e.dir && request.mode === 'save' && e.name === name}
                  onClick={() => (e.dir ? void go(e.path) : request.mode === 'save' ? setName(e.name) : done(e.path))}
                >
                  {e.dir ? <FolderOpen size={16} /> : <span className="file-dot" />}
                  <span className="nm">{e.name}</span>
                  {!e.dir && <span className="dim">{new Date(e.modified).toLocaleDateString(lang)}</span>}
                </button>
              ))}
            </div>
            {request.mode === 'save' && (
              <label className="browser-name">
                <span>{t('browse.name')}</span>
                <input className="field text" value={name} spellCheck={false} onChange={(e) => setName(e.target.value)} />
              </label>
            )}
          </div>
        </div>
        <div className="choices">
          {request.mode === 'folder' && (
            <button className="row" aria-selected disabled={!listing} onClick={() => listing && done(listing.path)}>
              {t('browse.chooseFolder')}
            </button>
          )}
          {request.mode === 'save' && (
            <button className="row" aria-selected disabled={!listing || !fileName} onClick={() => listing && done(join(listing.path, fileName))}>
              {t('browse.saveHere')}
            </button>
          )}
          <button className="row" onClick={() => done(null)}>
            {t('browse.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}
