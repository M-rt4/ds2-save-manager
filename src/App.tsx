import { useEffect } from 'react';
import { draftEdit, useStore } from './store';
import { TitleBar } from './components/TitleBar';
import { SaveRail } from './components/SaveRail';
import { SlotList } from './components/SlotList';
import { Detail } from './components/Detail';
import { Footer } from './components/Footer';
import { Overlays } from './components/Overlays';

export function App() {
  const lang = useStore((s) => s.lang);

  useEffect(() => {
    void useStore.getState().init();
  }, []);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        const s = useStore.getState();
        const pending = s.doc?.dirty || (s.character && s.draft && draftEdit(s.character, s.draft));
        if (pending && !s.gameRunning && !s.busy) void s.saveChanges();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  return (
    <div className="app">
      <div className="backdrop" />
      <TitleBar />
      <main className="main">
        <SaveRail />
        <SlotList />
        <Detail />
      </main>
      <Footer />
      <Overlays />
    </div>
  );
}
