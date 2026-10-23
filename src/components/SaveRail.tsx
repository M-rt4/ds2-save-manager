import { open } from '@tauri-apps/plugin-dialog';
import type { FoundSave } from '../api';
import { useStore } from '../store';
import { useT } from '../i18n';
import { FolderOpen, Refresh } from './icons';

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

export function SaveRail() {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const saves = useStore((s) => s.saves);
  const loaded = useStore((s) => s.savesLoaded);
  const doc = useStore((s) => s.doc);
  const openSave = useStore((s) => s.openSave);
  const refresh = useStore((s) => s.refreshSaves);

  const groups = new Map<string, FoundSave[]>();
  for (const s of saves) groups.set(s.folder, [...(groups.get(s.folder) ?? []), s]);

  const pick = async () => {
    const path = await open({ multiple: false, filters: [{ name: 'Dark Souls II save', extensions: ['sl2'] }] });
    if (typeof path === 'string') await openSave(path);
  };

  const outside = doc && !saves.some((s) => s.path.toLowerCase() === doc.path.toLowerCase());

  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="panel-title">{t('rail.title')}</h2>
        <hr className="rule" />
      </div>
      <div className="panel-body">
        {loaded && saves.length === 0 && <p className="empty-note">{t('rail.none')}</p>}
        {[...groups.entries()].map(([folder, list]) => (
          <div key={folder}>
            <div className="group-label">
              {list[0].account ? t('rail.account', { id: list[0].account }) : folder || t('rail.otherFolder')}
            </div>
            {list.map((s) => (
              <SaveRow key={s.path} save={s} selected={doc?.path.toLowerCase() === s.path.toLowerCase()} lang={lang} />
            ))}
          </div>
        ))}
        {outside && doc && (
          <div>
            <div className="group-label">{t('rail.otherFolder')}</div>
            <button className="row save-row" aria-selected>
              <span className="file">
                <b>{doc.fileName}</b>
                <EditionBadge edition={doc.sourceEdition} />
              </span>
              <span className="meta">{doc.folder}</span>
            </button>
          </div>
        )}
      </div>
      <div className="rail-actions">
        <button className="row" onClick={() => void pick()}>
          <FolderOpen size={17} />
          {t('rail.open')}
        </button>
        <button className="row" onClick={() => void refresh()}>
          <Refresh size={17} />
          {t('rail.rescan')}
        </button>
      </div>
    </section>
  );
}

function SaveRow({ save, selected, lang }: { save: FoundSave; selected: boolean; lang: string }) {
  const t = useT();
  const openSave = useStore((s) => s.openSave);
  return (
    <button className="row save-row" aria-selected={selected} onClick={() => void openSave(save.path)}>
      <span className="file">
        <b>{save.fileName}</b>
        <EditionBadge edition={save.edition} />
      </span>
      <span className="meta">{t('rail.modified', { when: relative(save.modified, lang) })}</span>
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
