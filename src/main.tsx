import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource/eb-garamond/400.css';
import '@fontsource/eb-garamond/500.css';
import '@fontsource/cinzel/600.css';
import './styles.css';
import { App } from './App';
import { initPad } from './pad';

initPad();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
