import { useEffect, useRef } from 'react';
import { useStore, type Tab } from '../store';
import { useT, type Key } from '../i18n';
import { CLASSES, COVENANTS, journeyLabel } from '../game';
import { StatusTab } from './StatusTab';
import { InventoryTab } from './InventoryTab';
import { TransferTab } from './TransferTab';
import { FileTab } from './FileTab';
import { BackupsTab } from './BackupsTab';
import { RawTab } from './RawTab';
import { ErrorBoundary } from './ErrorBoundary';
import { PanelChip } from './PadHints';

const TABS: { id: Tab; key: Key }[] = [
  { id: 'status', key: 'tab.status' },
  { id: 'inventory', key: 'tab.inventory' },
  { id: 'transfer', key: 'tab.transfer' },
  { id: 'backups', key: 'backup.tab' },
  { id: 'file', key: 'tab.file' },
  { id: 'raw', key: 'raw.tab' }
];

export function Detail() {
  const t = useT();
  const doc = useStore((s) => s.doc);
  const tab = useStore((s) => s.tab);
  const setTab = useStore((s) => s.setTab);
  const character = useStore((s) => s.character);
  const draft = useStore((s) => s.draft);
  const strip = useRef<HTMLElement>(null);

  // Should the strip ever scroll, the open tab stays in sight (LB / RB included).
  useEffect(() => {
    strip.current?.querySelector('[aria-selected=true]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [tab]);

  const needsCharacter = tab === 'status' || tab === 'inventory';

  return (
    <section className="panel view-panel">
      <PanelChip index={2} label={t(TABS.find((x) => x.id === tab)!.key)} />
      <nav className="tabs" role="tablist" ref={strip}>
        {TABS.map((x) => (
          <button key={x.id} className="tab" role="tab" aria-selected={tab === x.id} onClick={() => setTab(x.id)}>
            {t(x.key)}
          </button>
        ))}
      </nav>
      <hr className="rule" style={{ margin: 0 }} />
      {!doc ? (
        <p className="empty-note">{t('slots.noSave')}</p>
      ) : needsCharacter && !character ? (
        <p className="empty-note">{t('status.emptySlot')}</p>
      ) : (
        <>
          {needsCharacter && character && (
            <div className="char-head">
              <div className="portrait-name">{draft?.name.trim() || character.name}</div>
              <div className="facts">
                {[
                  CLASSES[character.class],
                  character.covenant ? COVENANTS[character.covenant] : null,
                  journeyLabel(character.journey)
                ]
                  .filter(Boolean)
                  .join(', ')}
              </div>
            </div>
          )}
          {needsCharacter && character && !character.layoutOk && <div className="warn">{t('status.layout')}</div>}
          <ErrorBoundary resetKey={`${tab}:${doc.id}:${character?.slot}`}>
            {tab === 'status' && character && <StatusTab character={character} />}
            {tab === 'inventory' && character && <InventoryTab character={character} />}
            {tab === 'transfer' && <TransferTab doc={doc} />}
            {tab === 'file' && <FileTab doc={doc} />}
            {tab === 'backups' && <BackupsTab doc={doc} />}
            {tab === 'raw' && <RawTab doc={doc} />}
          </ErrorBoundary>
        </>
      )}
    </section>
  );
}
