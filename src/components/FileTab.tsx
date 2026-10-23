import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { api, type SaveView } from '../api';
import { useStore } from '../store';
import { useT } from '../i18n';
import { editionName } from '../game';

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
          <button className="btn quiet" style={{ padding: '0 6px', fontSize: 14 }} onClick={() => void revealItemInDir(doc.path)}>
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
