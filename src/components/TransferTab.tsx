import { useEffect, useState } from 'react';
import { choosePath } from '../browse';
import { api, type ConversionPlan, type ConversionRequest, type Edition, type SaveView, type SlotView } from '../api';
import { gameDirFor, useStore } from '../store';
import { errorText, useT } from '../i18n';
import { editionName } from '../game';
import { ChangeList } from './ChangeList';
import { Bonfire } from './icons';

const other = (e: Edition): Edition => (e === 'scholar' ? 'original' : 'scholar');
const norm = (p: string) => p.replaceAll('/', '\\').toLowerCase();
const fileOf = (p: string) => p.split(/[\\/]/).pop() ?? p;
const folderOf = (p: string) => p.split(/[\\/]/).slice(-2, -1)[0] ?? '';

/** File name over its account folder; the full path goes into the tooltip. */
function ShortPath({ path }: { path: string }) {
  return (
    <div className="path" title={path}>
      <b>{fileOf(path)}</b>
      <br />
      {folderOf(path)}
    </div>
  );
}

export function TransferTab({ doc }: { doc: SaveView }) {
  const t = useT();
  const lang = useStore((s) => s.lang);
  const store = useStore();
  const [kind, setKind] = useState<'whole' | 'single'>('whole');
  const [targetEdition, setTargetEdition] = useState<Edition>(other(doc.sourceEdition));
  const [targetPath, setTargetPath] = useState('');
  const [plan, setPlan] = useState<ConversionPlan | null>(null);
  const [fromSlot, setFromSlot] = useState<number | null>(store.slot ?? doc.slots.find((s) => s.occupied)?.slot ?? null);
  const [toSlot, setToSlot] = useState<number | null>(null);
  const [account, setAccount] = useState('');
  const [editingAccount, setEditingAccount] = useState(false);
  const [running, setRunning] = useState(false);
  const [generation, setGeneration] = useState(0);

  useEffect(() => setTargetEdition(other(doc.sourceEdition)), [doc.path, doc.sourceEdition]);
  useEffect(() => {
    void api.defaultTarget(doc.path, targetEdition).then(setTargetPath);
  }, [doc.path, targetEdition]);

  const gameDir = useStore((s) => gameDirFor(s.games, targetEdition));
  const request: ConversionRequest = {
    sourceId: doc.id,
    slot: kind === 'single' && fromSlot !== null && toSlot !== null ? [fromSlot, toSlot] : null,
    targetEdition,
    targetPath,
    account: editingAccount && account.trim() ? account.trim() : null,
    gameDir
  };
  const key = JSON.stringify(request);

  useEffect(() => {
    if (!targetPath || (kind === 'single' && request.slot === null)) return;
    // The plan on screen is for the request it was made for: never offer to run it for another.
    setPlan(null);
    let live = true;
    api
      .planConversion(request)
      .then((p) => live && setPlan(p))
      .catch((e) => live && store.fail(e));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, generation]);

  // A single character needs a target first: plan the whole move just to read the target.
  useEffect(() => {
    if (kind !== 'single' || request.slot !== null || !targetPath) return;
    void api
      .planConversion({ ...request, slot: null })
      .then(setPlan)
      .catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind, targetPath, targetEdition, request.slot === null, generation]);

  const singleReady = kind === 'single' && request.slot !== null;
  const blocked = plan?.blocked ?? null;
  const ready = !!plan && !running && !store.gameRunning && !store.gameUnknown && !blocked && (kind === 'whole' || singleReady);

  const chooseTarget = async () => {
    const picked = await choosePath({ mode: 'save', defaultPath: targetPath, extensions: ['sl2'], filterName: 'Dark Souls II save' });
    if (picked) setTargetPath(picked);
  };
  const chooseGameDir = async () => {
    const dir = await choosePath({ mode: 'folder', title: editionName(targetEdition) });
    if (dir) await store.addGameFolder(dir);
  };

  const run = () => {
    const question =
      kind === 'whole'
        ? t('confirm.transfer', { file: fileOf(targetPath), folder: folderOf(targetPath) })
        : t('confirm.single', { name: doc.slots[fromSlot!].name, slot: toSlot! + 1, file: fileOf(targetPath) });
    store.ask(question, async () => {
      setRunning(true);
      try {
        // The character moves as it stands here, Status edits not yet saved included. Not when
        // copying within this same file: the backend refuses a target with unsaved edits.
        if (norm(targetPath) !== norm(doc.path)) await store.commitDraft();
        const report = await api.runConversion(request);
        setGeneration((g) => g + 1);
        await store.refreshSaves();
        if (norm(report.path) === norm(doc.path)) {
          store.setDoc(await api.getSave(doc.id));
          if (store.slot !== null) await store.selectSlot(store.slot);
        }
        if (useStore.getState().settings.showTransferReport) store.showReport({ path: report.path, changes: report.changes });
        else store.toast(t('transfer.done', { path: report.path }));
      } catch (e) {
        store.fail(e);
      } finally {
        setRunning(false);
      }
    });
  };

  const accounts = plan?.accounts;
  const sourceReg = plan?.sourceRegulation;

  return (
    <div className="panel-body">
      <div className="transfer">
        <p className="lead">{t('transfer.lead')}</p>

        <h3 className="step">{t('transfer.stepWhat')}</h3>
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

        <h3 className="step">{t('transfer.stepWhere')}</h3>
        <div className="journey">
          <div className="side">
            <h4>{t('transfer.from')}</h4>
            <div className={`edition ${doc.sourceEdition}`}>{editionName(doc.sourceEdition)}</div>
            <ShortPath path={doc.path} />
            {sourceReg && <div className="meta-line">{t('transfer.gameData', { game: sourceReg.gameVersion, version: sourceReg.version })}</div>}
            {kind === 'single' && <p className="note">{t('transfer.pickFrom')}</p>}
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
            <ShortPath path={targetPath} />
            <button className="btn quiet small" onClick={() => void chooseTarget()}>
              {t('transfer.choose')}
            </button>
            {plan && (
              <p className="note">
                {plan.targetExists ? t('transfer.exists') : t('transfer.new')}
              </p>
            )}
            {plan?.targetSlots && (
              <>
                {kind === 'single' && <p className="note">{t('transfer.pickTo')}</p>}
                <MiniSlots
                  slots={plan.targetSlots}
                  pick={kind === 'single' ? toSlot : null}
                  onPick={kind === 'single' ? setToSlot : undefined}
                  emptyText={t('transfer.intoEmpty')}
                />
              </>
            )}
          </div>
        </div>

        <h3 className="step">{t('transfer.stepGame')}</h3>
        <div className="info">
          {plan?.format ? (
            <>
              {t('transfer.formatLine', {
                edition: editionName(targetEdition),
                game: plan.format.regulation.gameVersion,
                version: plan.format.regulation.version
              })}
              <br />
              <span className="dim">
                {plan.format.kind === 'install'
                  ? t('transfer.formatInstall', { path: plan.format.path })
                  : plan.format.kind === 'targetFile'
                    ? t('transfer.formatTargetFile', { file: fileOf(plan.format.path) })
                    : t('transfer.formatOtherSave', { path: plan.format.path })}
              </span>
            </>
          ) : (
            t('transfer.formatUnknown', { edition: editionName(targetEdition) })
          )}
          <div style={{ marginTop: 8 }}>
            <button className="btn small" onClick={() => void chooseGameDir()}>
              {gameDir ? t('transfer.changeGameDir') : t('transfer.chooseGameDir', { edition: editionName(targetEdition) })}
            </button>
            {gameDir && <span className="dim" style={{ marginLeft: 10 }}>{gameDir}</span>}
          </div>
        </div>

        <h3 className="step">{t('transfer.stepAccount')}</h3>
        <div className="info">
          {accounts && (
            <div className="kv compact">
              <div className="k">{t('transfer.accountSource')}</div>
              <div className="v">{accounts.source ?? '—'}</div>
              <div className="k">{t('transfer.accountFolder')}</div>
              <div className="v">{accounts.targetFolder ?? t('transfer.accountNoFolder')}</div>
              <div className="k">{t('transfer.accountResult')}</div>
              <div className="v strong">
                {accounts.result ?? '—'}{' '}
                <span className="dim">{t(`transfer.accountOrigin.${accounts.origin}` as never)}</span>
              </div>
            </div>
          )}
          {accounts?.origin === 'manual' && accounts.targetFolder && accounts.result !== accounts.targetFolder && (
            <div className="warn" style={{ margin: '10px 0 0' }}>
              {t('transfer.accountMismatch', { folder: accounts.targetFolder })}
            </div>
          )}
          <p className="note">{t('transfer.accountWhy')}</p>
          {editingAccount ? (
            <div className="account-edit">
              <input
                className="field text"
                inputMode="numeric"
                maxLength={17}
                autoFocus
                placeholder="7656119…"
                value={account}
                onChange={(e) => setAccount(e.target.value.replace(/\D/g, '').slice(0, 17))}
              />
              {accounts?.local.map((a) => (
                <button key={a.id} className="btn quiet small" title={a.folder} onClick={() => setAccount(a.id)}>
                  {a.id}
                </button>
              ))}
              <button className="btn quiet small" onClick={() => (setEditingAccount(false), setAccount(''))}>
                {t('transfer.accountUseFolder')}
              </button>
            </div>
          ) : (
            <button className="btn small" onClick={() => (setEditingAccount(true), setAccount(accounts?.result ?? ''))}>
              {t('transfer.accountEdit')}
            </button>
          )}
        </div>

        <h3 className="step">{t('transfer.stepWhatHappens')}</h3>
        {kind === 'single' && !singleReady ? (
          <p className="note">{t('transfer.pickBoth')}</p>
        ) : blocked ? (
          <div className="warn" style={{ margin: 0 }}>
            {errorText(lang, blocked)}
          </div>
        ) : plan ? (
          <ChangeList
            changes={plan.changes}
            extra={[
              t('transfer.sourceUntouched', { file: fileOf(doc.path) }),
              ...(plan.targetExists ? [t('transfer.backupFirst', { file: fileOf(targetPath) })] : [])
            ]}
          />
        ) : null}
        {plan?.compatibility && plan.compatibility.problems.length > 0 && (
          <div className="warn" style={{ margin: '10px 0 0' }}>
            {t('transfer.incompatible')} {plan.compatibility.problems.join(' ')}
          </div>
        )}

        <div className="go">
          <button className="btn ember" disabled={!ready} onClick={run}>
            {kind === 'whole' ? t('transfer.runWhole', { file: fileOf(targetPath) }) : t('transfer.run')}
          </button>
          {(store.gameRunning || store.gameUnknown) && (
            <span className="note" style={{ margin: 0 }}>
              {t(store.gameRunning ? 'footer.gameRunning' : 'footer.gameUnknown')}
            </span>
          )}
        </div>
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
            <span className="dim">{emptyText}</span>
          )}
        </div>
      ))}
    </div>
  );
}
