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
  const root = document.querySelector<HTMLElement>('#legacy-alert');
  if (!root) return;
  root.hidden = current.status === 'clear' && !current.paused;
  if (root.hidden) {
    root.replaceChildren();
    return;
  }
  const title = current.paused
    ? 'Sonos Volume Bridge is still running'
    : 'Unable to check whether the old app is running';
  const message = current.paused
    ? 'Speaker Volume Bridge has paused synchronization to prevent conflicting volume changes. Quit Sonos Volume Bridge from its menu bar or system tray. Synchronization will resume automatically.'
    : 'Quit Sonos Volume Bridge before using Speaker Volume Bridge, then check again.';
  root.innerHTML = `<strong>${title}</strong><p>${message}</p><button type="button" class="secondary" id="legacy-recheck">Check again</button> <a href="https://svb.miguel.ms/upgrade.html" target="_blank" rel="noopener noreferrer">View upgrade instructions</a>`;
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
