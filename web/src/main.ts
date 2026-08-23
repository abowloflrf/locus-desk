import { mount } from 'svelte';

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

syncViewportHeight();
window.addEventListener('resize', syncViewportHeight);
window.visualViewport?.addEventListener('resize', syncViewportHeight);

mount(App, { target });

if ('serviceWorker' in navigator && import.meta.env.PROD) {
  window.addEventListener('load', () => {
    void navigator.serviceWorker.register(`${import.meta.env.BASE_URL}service-worker.js`);
  });
}
