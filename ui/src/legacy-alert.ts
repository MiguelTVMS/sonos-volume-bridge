import { invoke } from './app-commands';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Conflict = { status: 'clear' | 'legacyRunning' | 'unknown'; paused: boolean };
let current: Conflict = { status: 'clear', paused: false };
export function updateLegacyAlert(status: Conflict): void {
  current = status;
  mountLegacyAlert();
}
export function mountLegacyAlert(): void {
  const diagnostic = document.querySelector<HTMLElement>('#diagnostic-legacy');
  if (diagnostic) {
    diagnostic.textContent =
      current.status === 'unknown'
        ? 'Check unavailable'
        : current.status === 'legacyRunning'
          ? 'Old app detected'
          : 'No old app detected';
  }
  const root = document.querySelector<HTMLElement>('#legacy-alert');
  if (!root) return;
  // A failed inspection is not evidence that the user has the old app.
  // Keep an established conflict visible until absence is confirmed.
  root.hidden = !current.paused;
  if (root.hidden) {
    root.replaceChildren();
    return;
  }
  const title =
    current.status === 'unknown'
      ? 'Unable to recheck the previously detected old app'
      : 'Sonos Volume Bridge is still running';
  const message =
    current.status === 'unknown'
      ? 'Speaker Volume Bridge has paused synchronization after detecting Sonos Volume Bridge. Quit the old app, then check again to confirm it has stopped.'
      : 'Speaker Volume Bridge has paused synchronization to prevent conflicting volume changes. Quit Sonos Volume Bridge from its menu bar or system tray. Synchronization will resume automatically.';
  root.innerHTML = `<strong>${title}</strong><p>${message}</p><button type="button" class="secondary" id="legacy-recheck">Check again</button> <a href="https://svb.miguel.ms/upgrade.html" target="_blank" rel="noopener noreferrer">View upgrade instructions</a>`;
  root.querySelector<HTMLAnchorElement>('a')?.addEventListener('click', (event) => {
    if (!isTauri()) return;
    event.preventDefault();
    void invoke('open_legacy_upgrade').catch(() => {
      const guidance = document.createElement('p');
      guidance.textContent =
        'Open https://svb.miguel.ms/upgrade.html in your browser for upgrade instructions.';
      root.append(guidance);
    });
  });
  root.querySelector<HTMLButtonElement>('#legacy-recheck')?.addEventListener('click', async () => {
    const button = root.querySelector<HTMLButtonElement>('#legacy-recheck');
    if (button) button.disabled = true;
    try {
      current = await invoke<Conflict>('recheck_legacy_app');
    } catch {
      current = { status: 'unknown', paused: current.paused };
    }
    mountLegacyAlert();
  });
}
export async function installLegacyAlert(): Promise<void> {
  if (!isTauri()) return;
  try {
    await listen<Conflict>('legacy-app-changed', ({ payload }) => {
      if (payload.status === current.status && payload.paused === current.paused) return;
      current = payload;
      mountLegacyAlert();
    });
    current = await invoke<Conflict>('get_legacy_status');
  } catch {
    current = { status: 'unknown', paused: current.paused };
  }
  mountLegacyAlert();
}
