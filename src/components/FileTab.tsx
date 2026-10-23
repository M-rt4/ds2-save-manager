import { useState } from 'react';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { api, type ConversionPlan, type ConversionReport, type ConversionRequest, type SaveView } from '../api';
import { gameDirFor, useStore } from '../store';
import { errorText, useT } from '../i18n';
import { editionName } from '../game';
import { ChangeList } from './ChangeList';

export function FileTab({ doc }: { doc: SaveView }) {
  const t = useT();
  const setDoc = useStore((s) => s.setDoc);
  const fail = useStore((s) => s.fail);
  const mismatch = !!doc.account && !!doc.folderAccount && doc.account !== doc.folderAccount;

  const sign = async () => {
    try {
      setDoc(await api.setAccount(doc.id, doc.folderAccount!));
    } catch (e) {
      fail(e);
    }
  };

  return (
    <div className="panel-body padded">
      <div className="kv">
        <div className="k">{t('file.path')}</div>
        <div className="v">
          {doc.path}{' '}
          <button className="btn quiet" data-external style={{ padding: '0 6px', fontSize: 14 }} onClick={() => void revealItemInDir(doc.path)}>
            {t('file.reveal')}
          </button>
        </div>
        <div className="k">{t('file.edition')}</div>
        <div className="v">{editionName(doc.sourceEdition)}</div>
        <div className="k">{t('file.account')}</div>
        <div className="v">{doc.account ?? '—'}</div>
        <div className="k">{t('file.folderAccount')}</div>
        <div className="v">{doc.folderAccount ?? doc.folder}</div>
        <div className="k">{t('file.checksums')}</div>
        <div className="v">
          {doc.checksumFailures.length === 0
            ? t('file.checksumsOk', { n: doc.entries.length })
            : t('file.checksumsBad', { list: doc.checksumFailures.join(', ') })}
        </div>
      </div>

      {mismatch && (
        <div className="warn" style={{ margin: '14px 0 0' }}>
          {t('file.accountMismatch')}{' '}
          <button className="btn" style={{ marginTop: 8 }} onClick={() => void sign()}>
            {t('file.setAccount')}
          </button>
        </div>
      )}

      {/* Keyed by file: a plan checked for one save never shows for another. */}
      <FitToGame key={doc.id} doc={doc} />

      <h3 style={{ margin: '22px 0 2px', fontWeight: 400, fontStyle: 'normal', color: 'var(--ash)', fontSize: 15 }}>
        {t('file.entries')}
      </h3>
      <p className="note" style={{ marginTop: 0 }}>
        {t('file.entriesNote')}
      </p>
      <table className="blocks">
        <thead>
          <tr>
            <th>#</th>
            <th>BND4</th>
            <th>{t('file.size', { n: '' }).trim()}</th>
            <th>type / version / size</th>
          </tr>
        </thead>
        <tbody>
          {doc.entries.map((e) => (
            <tr key={e.index}>
              <td>{e.index}</td>
              <td>{e.name}</td>
              <td>{e.size.toLocaleString('en-US')}</td>
              <td>
                {e.chunks.length === 0 ? (
                  <span className="chunk">
                    <i>—</i>
                  </span>
                ) : (
                  e.chunks.map((c) => (
                    <span key={c.offset} className="chunk">
                      {c.kind.toString(16)}
                      <i>/</i>
                      {c.version.toString(16)}
                      <i>/</i>
                      {c.size}
                    </span>
                  ))
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * Writes the save over itself in the installed game's format and for its folder's account:
 * the repair for a save that came from another player or another conversion tool.
 */
function FitToGame({ doc }: { doc: SaveView }) {
  const t = useT();
  const store = useStore();
  const lang = useStore((s) => s.lang);
  const gameDir = useStore((s) => gameDirFor(s.games, doc.sourceEdition));
  const [plan, setPlan] = useState<ConversionPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const request: ConversionRequest = {
    sourceId: doc.id,
    slot: null,
    targetEdition: doc.sourceEdition,
    targetPath: doc.path,
    account: null,
    gameDir
  };
  const changes = plan?.changes.filter((c) => c.kind !== 'keptTargetSystem') ?? [];

  const check = async () => {
    try {
      setPlan(await api.planConversion(request));
    } catch (e) {
      store.fail(e);
    }
  };
  const run = () =>
    store.ask(t('confirm.transfer', { file: doc.fileName, folder: doc.folder }), async () => {
      setBusy(true);
      let report: ConversionReport;
      try {
        report = await api.runConversion(request);
      } catch (e) {
        store.fail(e);
        setBusy(false);
        return;
      }
      // The file is written: say so first, so a failed reload below never hides that it worked.
      if (useStore.getState().settings.showTransferReport) store.showReport({ path: report.path, changes: report.changes });
      else store.toast(t('transfer.done', { path: report.path }));
      try {
        store.setDoc(await api.getSave(doc.id));
        if (store.slot !== null) await store.selectSlot(store.slot);
        await store.refreshSaves();
        // Check the written file again: it now reads "already matches" instead of the old plan.
        setPlan(await api.planConversion(request));
      } catch (e) {
        store.fail(e);
      } finally {
        setBusy(false);
      }
    });

  return (
    <div className="fit">
      <h3 className="step">{t('file.fitTitle')}</h3>
      <p className="note" style={{ marginTop: 0 }}>
        {t('file.fitLead', { edition: editionName(doc.sourceEdition) })}
      </p>
      {doc.dirty ? (
        <p className="note">{t('file.fitDirty')}</p>
      ) : (
        <button className="btn" onClick={() => void check()}>
          {t('file.fitCheck')}
        </button>
      )}
      {plan &&
        (plan.blocked ? (
          <div className="warn" style={{ margin: '10px 0 0' }}>
            {errorText(lang, plan.blocked)}
          </div>
        ) : changes.length === 0 ? (
          <p className="note">{t('file.fitOk')}</p>
        ) : (
          <>
            <ChangeList changes={changes} extra={[t('transfer.backupFirst', { file: doc.fileName })]} />
            <div className="go">
              <button className="btn ember" disabled={busy || store.gameRunning} onClick={run}>
                {t('file.fitRun', { file: doc.fileName })}
              </button>
            </div>
          </>
        ))}
    </div>
  );
}
