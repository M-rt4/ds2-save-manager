import { useEffect, useMemo, useRef, useState } from 'react';
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { api, documentApi, type DocPlace, type DocPreview, type SaveView } from '../api';
import { draftEdit, useStore } from '../store';
import { useT } from '../i18n';
import { CodeEditor, type CodeMark } from './CodeEditor';

/** The line a place in the document is on, read off the text as the editor shows it. */
function locate(lines: string[], p: DocPlace): number | null {
  const find = (re: RegExp, from = 0, to = lines.length) => {
    for (let i = from; i < to; i++) if (re.test(lines[i])) return i;
    return null;
  };
  if (p.slot === null) return find(new RegExp(`^ {2}"${p.field}"\\s*:`));
  const list = find(/^ {2}"characters"\s*:/) ?? 0;
  const at = find(new RegExp(`^\\s*"slot"\\s*:\\s*${p.slot}\\b`), list);
  if (at === null) return list;
  const end = find(/^\s*"slot"\s*:/, at + 1) ?? lines.length;
  if (p.record !== null) return find(new RegExp(`"record"\\s*:\\s*${p.record}\\b`), at, end) ?? at;
  const inventory = find(/^\s*"inventory"\s*:/, at, end);
  const added = /\[new item (\d+)\]/.exec(p.path);
  if (added && inventory !== null) return Math.min(inventory + 1 + Number(added[1]), end - 1);
  return find(new RegExp(`^\\s*"${p.field.split(/[.[]/)[0]}"\\s*:`), at, end) ?? at;
}

export function JsonView({ doc }: { doc: SaveView }) {
  const t = useT();
  const toast = useStore((s) => s.toast);
  const fail = useStore((s) => s.fail);
  const ask = useStore((s) => s.ask);
  const setDoc = useStore((s) => s.setDoc);
  const selectSlot = useStore((s) => s.selectSlot);
  const [base, setBase] = useState<string | null>(null);
  const [text, setText] = useState('');
  const [preview, setPreview] = useState<DocPreview | null>(null);
  const [checking, setChecking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [stale, setStale] = useState(false);
  const [jump, setJump] = useState<{ line: number; seq: number } | null>(null);
  const edited = base !== null && text !== base;

  const load = async () => {
    try {
      // A status edit not yet saved goes into the save first, so the document shows it.
      const st = useStore.getState();
      const pending = st.character && st.draft && st.slot !== null ? draftEdit(st.character, st.draft) : null;
      if (pending && st.slot !== null) {
        await api.editCharacter(doc.id, st.slot, pending);
        setDoc(await api.getSave(doc.id));
        await selectSlot(st.slot);
      }
      const fresh = await documentApi.exportText(doc.id);
      setBase(fresh);
      setText(fresh);
      setPreview(null);
      setStale(false);
    } catch (e) {
      fail(e);
    }
  };

  useEffect(() => {
    setBase(null);
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc.id]);

  // The save changed elsewhere (written, discarded, edited in another tab): follow it, unless
  // there are edits here to keep.
  const seen = useRef(doc);
  useEffect(() => {
    if (doc === seen.current) return;
    seen.current = doc;
    if (base === null || busy) return;
    if (text === base) void load();
    else setStale(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc]);

  useEffect(() => {
    if (base === null || text === base) {
      setPreview(null);
      setChecking(false);
      return;
    }
    let live = true;
    setChecking(true);
    const timer = setTimeout(() => {
      documentApi
        .preview(doc.id, text)
        .then((p) => live && setPreview(p))
        .catch((e) => live && fail(e))
        .finally(() => live && setChecking(false));
    }, 450);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [text, base, doc.id, fail]);

  const lines = useMemo(() => text.split('\n'), [text]);
  const applying = preview?.changes.filter((c) => c.applies) ?? [];
  const shownOnly = preview?.changes.filter((c) => !c.applies) ?? [];
  const problems = preview?.problems ?? [];
  const syntax = preview?.syntax ?? null;
  const marks: CodeMark[] = useMemo(() => {
    const out: CodeMark[] = [];
    if (syntax) out.push({ line: syntax.line - 1, message: syntax.message });
    for (const p of problems) {
      const line = locate(lines, p);
      if (line !== null) out.push({ line, message: p.message });
    }
    return out;
  }, [syntax, problems, lines]);
  const canApply = edited && !!preview && !checking && !syntax && problems.length === 0 && applying.length > 0 && !busy;

  const goTo = (line: number | null) => line !== null && setJump({ line, seq: (jump?.seq ?? 0) + 1 });

  const reload = () => (edited ? ask(t('json.confirmReload'), () => void load()) : void load());

  const run = () =>
    ask(t('json.confirmApply', { n: applying.length }), async () => {
      setBusy(true);
      try {
        const changes = await documentApi.apply(doc.id, text);
        setDoc(await api.getSave(doc.id));
        const slot = useStore.getState().slot;
        if (slot !== null) await selectSlot(slot);
        await load();
        toast(t('json.applied', { n: changes.filter((c) => c.applies).length }));
      } catch (e) {
        fail(e);
      } finally {
        setBusy(false);
      }
    });

  const exportFile = async () => {
    const path = await saveDialog({
      defaultPath: `${doc.fileName.replace(/\.sl2$/i, '')}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }]
    });
    if (!path) return;
    try {
      await documentApi.writeFile(path, text);
      toast(t('json.exported', { path }));
    } catch (e) {
      fail(e);
    }
  };

  const importFile = async () => {
    const path = await open({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof path !== 'string') return;
    const go = async () => {
      try {
        setText(await documentApi.readFile(path));
        toast(t('json.imported', { file: path.split(/[\\/]/).pop() ?? path }));
      } catch (e) {
        fail(e);
      }
    };
    if (edited) ask(t('json.confirmReload'), () => void go());
    else void go();
  };

  return (
    <div className="json-view">
      <div className="json-tools">
        <p className="json-help">{t('json.help')}</p>
        <div className="json-actions">
          <button className="btn small" onClick={reload} disabled={busy}>
            {t('json.reload')}
          </button>
          <button className="btn small" onClick={() => void importFile()} disabled={busy}>
            {t('json.import')}
          </button>
          <button className="btn small" onClick={() => void exportFile()} disabled={busy || base === null}>
            {t('json.export')}
          </button>
        </div>
      </div>
      {stale && <div className="warn json-note">{t('json.stale')}</div>}

      {base === null ? (
        <p className="empty-note">{t('json.loading')}</p>
      ) : (
        <CodeEditor value={text} onChange={setText} marks={marks} jump={jump} label={t('json.viewReadable')} />
      )}

      <div className="json-diff">
        <div className="json-state">
          <span>
            {!edited
              ? t('json.unchanged')
              : checking || !preview
                ? t('json.checking')
                : syntax || problems.length
                  ? t('json.hasProblems', { n: (syntax ? 1 : 0) + problems.length })
                  : applying.length
                    ? t('json.willChange', { n: applying.length })
                    : t('json.nothingApplies')}
          </span>
          <button className="btn ember small" disabled={!canApply} onClick={run}>
            {t('json.apply')}
          </button>
        </div>
        {edited && preview && (
          <ul className="json-list">
            {syntax && (
              <li className="bad" onClick={() => goTo(syntax.line - 1)}>
                <b>{t('json.syntaxAt', { line: syntax.line, column: syntax.column })}</b> {syntax.message.replace(/ at line \d+ column \d+$/, '')}
              </li>
            )}
            {problems.map((p, i) => (
              <li key={`p${i}`} className="bad" onClick={() => goTo(locate(lines, p))}>
                <b>{p.path || t('json.document')}</b> {p.message}
              </li>
            ))}
            {applying.map((c, i) => (
              <li key={`c${i}`} onClick={() => goTo(locate(lines, c))}>
                <b>{c.path}</b>{' '}
                {c.stack ? (
                  <span className="to">{t('json.addToStack', { name: c.stack.name, n: c.stack.added, total: c.stack.total })}</span>
                ) : (
                  <>
                    <span className="from">{c.from}</span> → <span className="to">{c.to}</span>
                  </>
                )}
              </li>
            ))}
            {shownOnly.length > 0 && <li className="head">{t('json.shownOnly', { n: shownOnly.length })}</li>}
            {shownOnly.map((c, i) => (
              <li key={`s${i}`} className="dim" onClick={() => goTo(locate(lines, c))}>
                <b>{c.path}</b> <span className="from">{c.from}</span> → <span>{c.to}</span>
              </li>
            ))}
          </ul>
        )}
        <p className="note">{t('json.memoryNote')}</p>
      </div>
    </div>
  );
}
