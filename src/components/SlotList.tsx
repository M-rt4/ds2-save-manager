import { useStore } from '../store';
import { useT } from '../i18n';
import { journeyLabel, playTime } from '../game';

export function SlotList() {
  const t = useT();
  const doc = useStore((s) => s.doc);
  const slot = useStore((s) => s.slot);
  const select = useStore((s) => s.selectSlot);

  return (
    <section className="panel">
      <div className="panel-head">
        <h2 className="panel-title">{t('slots.title')}</h2>
        <hr className="rule" />
      </div>
      <div className="panel-body">
        {!doc && <p className="empty-note">{t('slots.noSave')}</p>}
        {doc?.slots.map((s) => {
          const shown = s.occupied;
          return (
            <button
              key={s.slot}
              className={`row slot${shown ? '' : ' empty'}`}
              aria-selected={slot === s.slot}
              onClick={() => void select(s.slot)}
            >
              <span className="no">{s.slot + 1}</span>
              <span className="name">{shown ? s.name : s.ghost ? `${s.name}` : t('slots.empty')}</span>
              <span className="lv">{shown ? t('slots.level', { n: s.level }) : ''}</span>
              <span className="sub">
                {shown ? (
                  <>
                    <span className="grow">{s.area ?? s.mapLabel}</span>
                    {doc.lastSlot === s.slot && <span className="continue">{t('slots.last')}</span>}
                    {s.journey > 1 && <span>{journeyLabel(s.journey)}</span>}
                    <span>{playTime(s.playTime)}</span>
                  </>
                ) : (
                  s.ghost && <span className="grow">{t('slots.ghost')}</span>
                )}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
