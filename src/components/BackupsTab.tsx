import { useEffect, useState } from 'react';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { api, backupApi, type Backup, type BackupReason, type Edition, type SaveView } from '../api';
import { useHasChanges, useStore } from '../store';
import { useT, type Key } from '../i18n';
import { EditionBadge } from './SaveRail';

const REASONS: Record<BackupReason, Key> = {
  manual: 'backup.reason.manual',
  save: 'backup.reason.save',
  transfer: 'backup.reason.transfer',
  restore: 'backup.reason.restore'
};

/** The release a save file is read by: the game's own file names say it; any other file keeps what it holds now. */
function expectedEdition(doc: SaveView): Edition {
  const name = doc.fileName.toUpperCase();
  return name.startsWith('DS2SOFS') ? 'scholar' : name.startsWith('DARKSII') ? 'original' : doc.sourceEdition;
}

const startOfDay = (ms: number) => new Date(ms).setHours(0, 0, 0, 0);

export function BackupsTab({ doc }: { doc: SaveView }) {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const gameRunning = useStore((s) => s.gameRunning);
  const slot = useStore((s) => s.slot);
  const ask = useStore((s) => s.ask);
  const toast = useStore((s) => s.toast);
  const fail = useStore((s) => s.fail);
  const setDoc = useStore((s) => s.setDoc);
  const selectSlot = useStore((s) => s.selectSlot);
  const refreshSaves = useStore((s) => s.refreshSaves);
  const changes = useHasChanges();
  const [list, setList] = useState<Backup[] | null>(null);
  const [picked, setPicked] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    setList(null);
    setPicked(null);
  }, [doc.path]);

  // A new doc also arrives after saving from here (Ctrl S), which takes a backup.
  useEffect(() => {
    let live = true;
    backupApi
      .list(doc.path)
      .then((l) => live && setList(l))
      .catch((e) => live && (setList([]), fail(e)));
    return () => {
      live = false;
    };
  }, [doc, generation, fail]);

  const time = (ms: number) =>
    new Date(ms).toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' });
  const when = (ms: number) => new Date(ms).toLocaleString(lang, { dateStyle: 'long', timeStyle: 'medium', hourCycle: 'h23' });
  const day = (ms: number) => {
    const ago = Math.round((startOfDay(Date.now()) - startOfDay(ms)) / 86_400_000);
    if (ago === 0) return t('backup.today');
    if (ago === 1) return t('backup.yesterday');
    return new Date(ms).toLocaleDateString(lang, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
  };
  const size = (n: number) =>
    n < 1_048_576
      ? `${Math.max(1, Math.round(n / 1024)).toLocaleString(lang)} KB`
      : `${(n / 1_048_576).toLocaleString(lang, { minimumFractionDigits: 1, maximumFractionDigits: 1 })} MB`;

  const expected = expectedEdition(doc);
  const blocked = (b: Backup): string | null =>
    !b.readable
      ? t('backup.blockedUnreadable')
      : b.edition !== expected
        ? t('backup.blockedEdition', {
            edition: t(b.edition === 'scholar' ? 'edition.scholarShort' : 'edition.originalShort'),
            file: doc.fileName
          })
        : gameRunning
          ? t('backup.blockedGame')
          : null;

  const groups: [number, Backup[]][] = [];
  for (const b of list ?? []) {
    const last = groups.at(-1);
    if (last && last[0] === startOfDay(b.created)) last[1].push(b);
    else groups.push([startOfDay(b.created), [b]]);
  }
  const total = (list ?? []).reduce((sum, b) => sum + b.size, 0);

  const take = async () => {
    setWorking(true);
    try {
      const b = await backupApi.create(doc.path);
      if (b.readable) toast(t('backup.created', { file: b.fileName }));
      else toast(t('backup.createdUnreadable'), 'error');
      setPicked(b.path);
      setGeneration((g) => g + 1);
    } catch (e) {
      fail(e);
    } finally {
      setWorking(false);
    }
  };

  const restore = (b: Backup) => {
    const at = when(b.created);
    const text = t('backup.confirmRestore', { file: doc.fileName, when: at }) + (changes ? ` ${t('backup.confirmRestoreUnsaved')}` : '');
    ask(text, async () => {
      setWorking(true);
      try {
        const report = await backupApi.restore(b.path, doc.path);
        const fresh = await api.getSave(doc.id);
        setDoc(fresh);
        // Re-reading the slot also drops a status draft made against the replaced file.
        const keep = slot !== null && fresh.slots[slot]?.occupied ? slot : fresh.slots.find((s) => s.occupied)?.slot;
        await selectSlot(keep ?? 0);
        toast(t(report.backup ? 'backup.restored' : 'backup.restoredFresh', { when: at }));
        // The new doc above lists the backups again.
        setPicked(null);
        void refreshSaves();
      } catch (e) {
        fail(e);
      } finally {
        setWorking(false);
      }
    });
  };

  const remove = (b: Backup) =>
    ask(t('backup.confirmDelete', { when: when(b.created) }), async () => {
      try {
        await backupApi.remove(b.path);
        toast(t('backup.deleted'));
        setPicked(null);
        setGeneration((g) => g + 1);
      } catch (e) {
        fail(e);
      }
    });

  return (
    <div className="panel-body backups">
      <div className="backups-head">
        <p className="lead">{t('backup.lead')}</p>
        <button className="btn" disabled={working} onClick={() => void take()}>
          {t('backup.take')}
        </button>
      </div>
      {(gameRunning || changes) && (
        <div className="backups-notes">
          {gameRunning && <div className="info">{t('backup.gameNote')}</div>}
          {changes && <div className="info">{t('backup.unsavedNote')}</div>}
        </div>
      )}
      {list && list.length === 0 && <p className="empty-note backups-empty">{t('backup.empty')}</p>}
      {list && list.length > 0 && (
        <div className="backups-count">
          {list.length === 1 ? t('backup.countOne', { size: size(total) }) : t('backup.count', { n: list.length, size: size(total) })}
        </div>
      )}
      <div role="listbox" aria-label={t('backup.tab')}>
        {groups.map(([key, items]) => (
          <div key={key}>
            <div className="group-label">{day(items[0].created)}</div>
            {items.map((b) => {
              const selected = picked === b.path;
              const why = selected ? blocked(b) : null;
              return (
                <div
                  key={b.path}
                  className="row backup-row"
                  role="option"
                  aria-selected={selected}
                  tabIndex={0}
                  title={b.fileName}
                  onClick={() => setPicked(b.path)}
                  onKeyDown={(e) => {
                    if ((e.key === 'Enter' || e.key === ' ') && e.target === e.currentTarget) {
                      e.preventDefault();
                      setPicked(b.path);
                    }
                  }}
                >
                  <span className="when">{time(b.created)}</span>
                  <span className="why">{t(b.reason ? REASONS[b.reason] : 'backup.reason.legacy')}</span>
                  <EditionBadge edition={b.edition} />
                  <span className={`who${b.readable ? '' : ' bad'}`}>
                    {!b.readable
                      ? t('backup.unreadable')
                      : b.characters.length === 0
                        ? t('backup.noCharacters')
                        : b.characters.map((c, i) => (
                            <span key={c.slot}>
                              {i > 0 && ', '}
                              <b>{c.name}</b> {t('slots.level', { n: c.level })}
                            </span>
                          ))}
                  </span>
                  <span className="size">{size(b.size)}</span>
                  {selected && (
                    <div className="acts">
                      <button className="btn ember" disabled={working || !!why} onClick={() => restore(b)}>
                        {t('backup.restore')}
                      </button>
                      <button className="btn quiet" data-external onClick={() => void revealItemInDir(b.path)}>
                        {t('file.reveal')}
                      </button>
                      <button className="btn quiet danger" disabled={working} onClick={() => remove(b)}>
                        {t('backup.delete')}
                      </button>
                      {why && <span className="why-not">{why}</span>}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
