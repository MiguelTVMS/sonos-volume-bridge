import { initializeCommands } from './app-commands';

export const settingsReady = initializeCommands()
  .then(() => import('./main'))
  .catch(() => {
    const root = document.querySelector('#app');
    if (root) root.textContent = 'Could not start Settings. Please reopen the app.';
  });
