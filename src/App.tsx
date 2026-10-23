import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { hasChanges, useStore } from './store';
import { TitleBar } from './components/TitleBar';
import { SaveRail } from './components/SaveRail';
import { SlotList } from './components/SlotList';
import { Detail } from './components/Detail';
import { Footer } from './components/Footer';
import { Overlays } from './components/Overlays';
import { FileBrowser } from './components/FileBrowser';
import { PadKeyboard } from './components/PadKeyboard';

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
        if (hasChanges(s) && !s.gameRunning && !s.busy) void s.saveChanges();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // However the window is closed (the title bar's X, Alt+F4, the taskbar, Settings → Quit),
  // unsaved changes are asked about first; with none it closes at once.
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onCloseRequested((e) => {
      const s = useStore.getState();
      if (!hasChanges(s)) return;
      e.preventDefault();
      s.ask(s.t('window.quitUnsaved'), () => void win.destroy());
    });
    return () => void unlisten.then((stop) => stop());
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
      <FileBrowser />
      <PadKeyboard />
    </div>
  );
}
