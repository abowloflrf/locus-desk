import { mount } from 'svelte';

import '@fontsource-variable/atkinson-hyperlegible-mono/wght.css';
import '@fontsource-variable/atkinson-hyperlegible-next/wght.css';
import '@fontsource-variable/atkinson-hyperlegible-next/wght-italic.css';
import '@fontsource-variable/ibm-plex-sans/wght.css';
import '@fontsource-variable/ibm-plex-sans/wght-italic.css';
import '@fontsource/ibm-plex-mono/latin-400.css';
import '@fontsource/ibm-plex-mono/latin-500.css';
import '@fontsource/ibm-plex-mono/latin-600.css';
import App from './App.svelte';
import './styles.css';

const target = document.getElementById('app');

if (!target) {
  throw new Error('Unable to find the application mount point');
}

function syncViewportHeight(): void {
  const viewport = window.visualViewport;
  if (viewport && viewport.scale !== 1) return;
  document.documentElement.style.setProperty(
    '--app-height',
    `${Math.round(viewport?.height ?? window.innerHeight)}px`,
  );
}

const standalone =
  window.matchMedia('(display-mode: standalone)').matches ||
  ('standalone' in navigator && navigator.standalone === true);

// WebKit can report a stale visual viewport height after keyboard transitions in standalone PWAs.
if (!standalone) {
  syncViewportHeight();
  window.addEventListener('resize', syncViewportHeight);
  window.visualViewport?.addEventListener('resize', syncViewportHeight);
}

mount(App, { target });

if ('serviceWorker' in navigator && import.meta.env.PROD) {
  window.addEventListener('load', () => {
    void navigator.serviceWorker.register(`${import.meta.env.BASE_URL}service-worker.js`);
  });
}
