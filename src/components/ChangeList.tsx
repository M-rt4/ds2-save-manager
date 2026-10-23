import type { Change, FormatSource } from '../api';
import { useT } from '../i18n';
import { editionName } from '../game';

type T = ReturnType<typeof useT>;

const fileOf = (p: string) => p.split(/[\\/]/).pop() ?? p;
const folderOf = (p: string) => p.split(/[\\/]/).slice(-2, -1)[0] ?? '';
const mb = (n: number) => `${(n / 1048576).toLocaleString('en-US', { maximumFractionDigits: 2 })} MB`;

function sourceText(t: T, s: FormatSource): string {
  if (s.kind === 'install') return t('change.fromInstall', { path: s.path });
  if (s.kind === 'targetFile') return t('change.fromTargetFile', { file: fileOf(s.path) });
  return t('change.fromOtherSave', { path: s.path });
}

/** One sentence per change, naming the file and the values on both sides. */
export function changeText(t: T, c: Change): string {
  switch (c.kind) {
    case 'key':
      return t('change.key', { from: editionName(c.from), to: editionName(c.to) });
    case 'regulation':
      return t('change.regulation', {
        from: c.from ? t('change.regValue', { version: c.from.version, game: c.from.gameVersion }) : t('change.none'),
        to: t('change.regValue', { version: c.to.version, game: c.to.gameVersion }),
        source: sourceText(t, c.source)
      });
    case 'titleVersion':
      return t('change.titleVersion', { from: `0x${c.from.toString(16)}`, to: `0x${c.to.toString(16)}` });
    case 'entrySize':
      return t('change.entrySize', { from: mb(c.from), to: mb(c.to) });
    case 'account':
      return t(c.origin === 'manual' ? 'change.accountManual' : c.origin === 'folder' ? 'change.accountFolder' : 'change.accountFile', {
        from: c.from ?? t('change.none'),
        to: c.to
      });
    case 'slot':
      if (c.before && c.after && c.before === c.after) return t('change.slotUpdate', { slot: c.slot + 1, name: c.after });
      if (c.before && c.after) return t('change.slotReplace', { slot: c.slot + 1, before: c.before, after: c.after });
      if (c.after) return t('change.slotFill', { slot: c.slot + 1, after: c.after });
      return t('change.slotClear', { slot: c.slot + 1, before: c.before ?? '' });
    case 'keptTargetSystem':
      return t('change.keptTarget', { file: fileOf(c.path) });
    case 'newFile':
      return t('change.newFile', { file: fileOf(c.path), folder: folderOf(c.path) });
    case 'backup':
      return t('change.backup', { path: c.path });
  }
}

export function ChangeList({ changes, extra = [] }: { changes: Change[]; extra?: string[] }) {
  const t = useT();
  return (
    <ul className="changes">
      {changes.map((c, i) => (
        <li key={i} className={`change-${c.kind}`}>
          {changeText(t, c)}
        </li>
      ))}
      {extra.map((x) => (
        <li key={x}>{x}</li>
      ))}
    </ul>
  );
}
