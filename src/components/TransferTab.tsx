import { useEffect, useState } from 'react';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { api, type ConversionMode, type ConversionPlan, type Edition, type SaveView, type SlotView, type WriteReport } from '../api';
import { useStore } from '../store';
import { errorText, useT } from '../i18n';
import { editionName } from '../game';
import { Bonfire } from './icons';

const other = (e: Edition): Edition => (e === 'scholar' ? 'original' : 'scholar');
const norm = (p: string) => p.replaceAll('/', '\\').toLowerCase();
/** File name over its account folder; the full path goes into the tooltip. */
function ShortPath({ path }: { path: string }) {
  const parts = path.split(/[\\/]/);
  return (
    <>
      <b style={{ fontWeight: 500, color: 'var(--bone)', fontSize: 15 }}>{parts.at(-1)}</b>
      <br />
      {parts.at(-2)}
    </>
  );
}

export function TransferTab({ doc }: { doc: SaveView }) {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const store = useStore();
  const [kind, setKind] = useState<'whole' | 'single'>('whole');
  const [targetEdition, setTargetEdition] = useState<Edition>(other(doc.sourceEdition));
  const [targetPath, setTargetPath] = useState('');
  const [mode, setMode] = useState<ConversionMode>('merge');
  const [plan, setPlan] = useState<ConversionPlan | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [fromSlot, setFromSlot] = useState<number | null>(store.slot ?? doc.slots.find((s) => s.occupied)?.slot ?? null);
  const [toSlot, setToSlot] = useState<number | null>(null);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<WriteReport | null>(null);
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    setTargetEdition(other(doc.sourceEdition));
    setResult(null);
  }, [doc.path, doc.sourceEdition]);

  useEffect(() => {
    void api.defaultTarget(doc.path, targetEdition).then(setTargetPath);
  }, [doc.path, targetEdition]);

  const exists = plan?.targetExists ?? false;
  const effectiveMode: ConversionMode = exists ? mode : 'replace';

  useEffect(() => {
    if (!targetPath) return;
    let live = true;
    setPlanError(null);
    api
      .planConversion({ sourceId: doc.id, targetEdition, targetPath, mode: effectiveMode })
      .then((p) => live && setPlan(p))
      .catch((e) => live && (setPlan(null), setPlanError(errorText(lang, e))));
    return () => {
      live = false;
    };
  }, [doc.id, targetEdition, targetPath, effectiveMode, generation, lang]);

  const sameFile = !!targetPath && norm(targetPath) === norm(doc.path);
  const compat = plan?.compatibility;
  const targetOk = exists && !plan?.targetError && (compat?.compatible ?? false);
  const fromOk = fromSlot !== null && doc.slots[fromSlot]?.occupied;

  const ready =
    !!plan &&
    !running &&
    !store.gameRunning &&
    (kind === 'whole'
      ? !sameFile && (effectiveMode === 'replace' || targetOk)
      : targetOk && fromOk && toSlot !== null && !(sameFile && fromSlot === toSlot));

  const targetName = targetPath.split(/[\\/]/).pop() ?? targetPath;

  const choose = async () => {
    const picked = await saveDialog({
      defaultPath: targetPath,
      filters: [{ name: 'Dark Souls II save', extensions: ['sl2'] }]
    });
    if (picked) setTargetPath(picked);
  };

  const afterWrite = async (report: WriteReport) => {
    setResult(report);
    setGeneration((g) => g + 1);
    await store.refreshSaves();
    if (norm(report.path) === norm(doc.path)) {
      store.setDoc(await api.getSave(doc.id));
      if (store.slot !== null) await store.selectSlot(store.slot);
    }
  };

  const runWhole = () =>
    store.ask(t('confirm.transfer', { file: targetName }), async () => {
      setRunning(true);
      try {
        await afterWrite(await api.runConversion({ sourceId: doc.id, targetEdition, targetPath, mode: effectiveMode }));
      } catch (e) {
        store.fail(e);
      } finally {
        setRunning(false);
      }
    });

  const runSingle = () => {
    if (fromSlot === null || toSlot === null) return;
    const name = doc.slots[fromSlot].name;
    store.ask(t('confirm.single', { name, slot: toSlot + 1, file: targetName }), async () => {
      setRunning(true);
      let opened: string | null = null;
      try {
        const targetId = sameFile ? doc.id : (opened = (await api.openSave(targetPath)).id);
        await api.transferCharacter(doc.id, fromSlot, targetId, toSlot);
        await afterWrite(await api.writeSave(targetId));
      } catch (e) {
        store.fail(e);
      } finally {
        if (opened) await api.closeSave(opened).catch(() => undefined);
        setRunning(false);
      }
    });
  };

  return (
    <div className="panel-body">
      <div className="transfer">
        <p className="lead">{t('transfer.lead')}</p>

        <div className="choice">
          <button className="row" aria-pressed={kind === 'whole'} onClick={() => setKind('whole')}>
            <b>{t('transfer.whole')}</b>
            <span>{t('transfer.wholeHint')}</span>
          </button>
          <button className="row" aria-pressed={kind === 'single'} onClick={() => setKind('single')}>
            <b>{t('transfer.single')}</b>
            <span>{t('transfer.singleHint')}</span>
          </button>
        </div>

        <div className="journey">
          <div className="side">
            <h4>{t('transfer.from')}</h4>
            <div className={`edition ${doc.sourceEdition}`}>{editionName(doc.sourceEdition)}</div>
            <div className="path" title={doc.path}>
              <ShortPath path={doc.path} />
            </div>
            <MiniSlots
              slots={doc.slots}
              pick={kind === 'single' ? fromSlot : null}
              onPick={kind === 'single' ? setFromSlot : undefined}
              onlyOccupied={kind === 'single'}
              emptyText={t('slots.empty')}
            />
          </div>

          <div className={`fire${ready ? '' : ' cold'}`}>
            <Bonfire size={72} lit={ready} className="flame" />
          </div>

          <div className="side">
            <h4>{t('transfer.to')}</h4>
            <div className="seg" role="group">
              {(['original', 'scholar'] as Edition[]).map((e) => (
                <button key={e} aria-pressed={targetEdition === e} onClick={() => setTargetEdition(e)}>
                  {e === 'scholar' ? t('edition.scholarShort') : t('edition.originalShort')}
                </button>
              ))}
            </div>
            <div className="path" title={targetPath}>
              <ShortPath path={targetPath} />{' '}
              <button className="btn quiet" style={{ padding: '0 6px', fontSize: 14 }} onClick={() => void choose()}>
                {t('transfer.choose')}
              </button>
            </div>
            {plan && (exists ? <p className="note">{t('transfer.exists')}</p> : <p className="note">{t('transfer.new')}</p>)}
            {plan?.targetSlots && (
              <MiniSlots
                slots={plan.targetSlots}
                pick={kind === 'single' ? toSlot : null}
                onPick={kind === 'single' ? setToSlot : undefined}
                emptyText={t('transfer.intoEmpty')}
              />
            )}
          </div>
        </div>

        {kind === 'whole' && exists && (
          <div className="modes">
            <button className="row" aria-pressed={mode === 'merge'} onClick={() => setMode('merge')}>
              <span className="radio" />
              <span>
                <b>{t('transfer.modeMerge')}</b>
                <span>{t('transfer.modeMergeHint')}</span>
              </span>
            </button>
            <button className="row" aria-pressed={mode === 'replace'} onClick={() => setMode('replace')}>
              <span className="radio" />
              <span>
                <b>{t('transfer.modeReplace')}</b>
                <span>{t('transfer.modeReplaceHint')}</span>
              </span>
            </button>
          </div>
        )}

        <div className="notes">
          {planError && <div className="warn" style={{ margin: 0 }}>{planError}</div>}
          {kind === 'whole' && sameFile && <div className="warn" style={{ margin: 0 }}>{t('transfer.sameFile')}</div>}
          {kind === 'single' && plan && !exists && <div className="warn" style={{ margin: 0 }}>{t('transfer.needsTarget')}</div>}
          {plan?.targetError && (
            <div className="warn" style={{ margin: 0 }}>{t('transfer.targetUnreadable', { msg: errorText(lang, plan.targetError) })}</div>
          )}
          {compat && !compat.compatible && (kind === 'single' || effectiveMode === 'merge') && (
            <div className="warn" style={{ margin: 0 }}>
              {t('transfer.incompatible')} {compat.problems.join(' ')}
            </div>
          )}
          {compat && compat.versionNotes.length > 0 && (
            <div className="info">
              {t('transfer.versionNotes')}
              <div style={{ marginTop: 6, fontSize: 14, color: 'var(--dim)' }}>
                {compat.versionNotes.map((v) => (
                  <span key={`${v.entry}-${v.chunk}`} className="chunk">
                    #{v.entry}/{v.chunk.toString(16)}: {v.source.toString(16)} → {v.target.toString(16)}
                  </span>
                ))}
              </div>
            </div>
          )}
          {compat && compat.systemSizeDifferences.length > 0 && effectiveMode === 'merge' && <div className="info">{t('transfer.systemSize')}</div>}
          {plan?.accountChange && kind === 'whole' && <div className="info">{t('transfer.account', { to: plan.accountChange[1] })}</div>}
          {plan?.targetOpenAndDirty && <div className="warn" style={{ margin: 0 }}>{t('transfer.dirtyTarget')}</div>}
        </div>

        <div className="go">
          <button className="btn ember" disabled={!ready} onClick={kind === 'whole' ? runWhole : runSingle}>
            {kind === 'whole' ? t('transfer.runWhole', { file: targetName }) : t('transfer.run')}
          </button>
          {result && (
            <span className="note" style={{ margin: 0 }}>
              {t('transfer.done', { path: result.path })}
              {result.backup && (
                <>
                  <br />
                  {t('transfer.backup', { path: result.backup })}
                </>
              )}
            </span>
          )}
        </div>

        <p className="note" style={{ marginTop: 22, maxWidth: '72ch' }}>
          {t('transfer.tip')}
        </p>
      </div>
    </div>
  );
}

function MiniSlots({
  slots,
  pick,
  onPick,
  onlyOccupied = false,
  emptyText
}: {
  slots: SlotView[];
  pick: number | null;
  onPick?: (slot: number) => void;
  onlyOccupied?: boolean;
  emptyText: string;
}) {
  const list = onlyOccupied ? slots.filter((s) => s.occupied) : slots;
  return (
    <div className="mini-slots">
      {list.map((s) => (
        <div
          key={s.slot}
          className={`ms${onPick ? ' pick' : ''}`}
          aria-selected={pick === s.slot}
          role={onPick ? 'option' : undefined}
          tabIndex={onPick ? 0 : undefined}
          onClick={() => onPick?.(s.slot)}
          onKeyDown={(e) => onPick && (e.key === 'Enter' || e.key === ' ') && onPick(s.slot)}
        >
          <span className="n">{s.slot + 1}</span>
          {s.occupied ? (
            <>
              <b>{s.name}</b>
              <span>Lv {s.level}</span>
            </>
          ) : (
            <span style={{ fontStyle: 'normal' }}>{emptyText}</span>
          )}
        </div>
      ))}
    </div>
  );
}
