import { invoke } from '@tauri-apps/api/core';
import './style.css';

type MappingPoint = { local: number; sonos: number };
type Configuration = { schemaVersion: number; selectedSonosId: string | null; lastKnownSonosAddress: string | null; followDefaultAudioDevice: boolean; fixedAudioDeviceId: string | null; synchronizeMute: boolean; startAtLogin: boolean; fallbackPolling: boolean; maximumSonosVolume: number; mapping: { type: 'linear' | 'capped_linear' | 'piecewise'; points?: MappingPoint[]; maximum?: number } };
type Snapshot = { configuration: Configuration; status: string; sonosName: string | null; sonosVolume: number | null; localVolume: number | null; muted: boolean | null };

const app = document.querySelector<HTMLDivElement>('#app');
if (!app) throw new Error('Missing application root.');

function input(label: string, name: string, value: string, type = 'text'): string { return `<label>${label}<input name="${name}" type="${type}" value="${value}" /></label>`; }
function render(snapshot: Snapshot): void {
  const c = snapshot.configuration;
  app.innerHTML = `<header><h1>SonosVolumeBridge</h1><p class="status">${snapshot.status}</p></header><section class="summary"><span>Sonos: ${snapshot.sonosName ?? 'Not selected'} (${snapshot.sonosVolume ?? '—'}%)</span><span>Local: ${snapshot.localVolume ?? '—'}%</span><span>Mute: ${snapshot.muted ? 'On' : 'Off'}</span></section><form id="settings"><h2>Connection</h2>${input('Sonos UDN', 'selectedSonosId', c.selectedSonosId ?? '')}${input('Cached Sonos address', 'lastKnownSonosAddress', c.lastKnownSonosAddress ?? '')}<button type="button" id="discover">Discover Sonos devices</button><h2>Audio</h2><label><input type="checkbox" name="followDefaultAudioDevice" ${c.followDefaultAudioDevice ? 'checked' : ''}/> Follow default output device</label>${input('Fixed output device ID', 'fixedAudioDeviceId', c.fixedAudioDeviceId ?? '')}<label><input type="checkbox" name="synchronizeMute" ${c.synchronizeMute ? 'checked' : ''}/> Synchronize mute</label><h2>Volume safety</h2>${input('Maximum Sonos volume', 'maximumSonosVolume', String(c.maximumSonosVolume), 'number')}<label>Mapping<select name="mapping"><option value="piecewise" selected>Piecewise (default)</option><option value="linear">Linear</option><option value="capped_linear">Capped linear</option></select></label><p class="hint">Default curve: 0→0, 20→5, 40→12, 60→23, 80→40, 100→55.</p><h2>Runtime</h2><label><input type="checkbox" name="startAtLogin" ${c.startAtLogin ? 'checked' : ''}/> Start at login</label><label><input type="checkbox" name="fallbackPolling" ${c.fallbackPolling ? 'checked' : ''}/> Enable fallback polling</label><footer><button type="submit">Save settings</button><button type="button" id="test">Test volume control</button><button type="button" id="diagnostics">Diagnostics</button><button type="button" id="export">Export diagnostics</button><button type="button" id="reset">Reset configuration</button></footer><output id="notice"></output></form>`;
  document.querySelector<HTMLFormElement>('#settings')?.addEventListener('submit', save);
  document.querySelector('#test')?.addEventListener('click', testVolume);
  document.querySelector('#diagnostics')?.addEventListener('click', showDiagnostics);
  document.querySelector('#export')?.addEventListener('click', exportDiagnostics);
  document.querySelector('#reset')?.addEventListener('click', reset);
  document.querySelector('#discover')?.addEventListener('click', () => notice('Discovery is available after the runtime composition starts.'));
}
function notice(value: string): void { const output = document.querySelector<HTMLOutputElement>('#notice'); if (output) output.value = value; }
function formConfiguration(form: HTMLFormElement): Configuration {
  const values = new FormData(form); const mapping = String(values.get('mapping')) as Configuration['mapping']['type'];
  return { schemaVersion: 1, selectedSonosId: String(values.get('selectedSonosId') ?? '') || null, lastKnownSonosAddress: String(values.get('lastKnownSonosAddress') ?? '') || null, followDefaultAudioDevice: values.has('followDefaultAudioDevice'), fixedAudioDeviceId: String(values.get('fixedAudioDeviceId') ?? '') || null, synchronizeMute: values.has('synchronizeMute'), startAtLogin: values.has('startAtLogin'), fallbackPolling: values.has('fallbackPolling'), maximumSonosVolume: Number(values.get('maximumSonosVolume')), mapping: mapping === 'piecewise' ? { type: mapping, points: [{ local: 0, sonos: 0 }, { local: 20, sonos: 5 }, { local: 40, sonos: 12 }, { local: 60, sonos: 23 }, { local: 80, sonos: 40 }, { local: 100, sonos: 55 }] } : mapping === 'linear' ? { type: mapping } : { type: mapping, maximum: Number(values.get('maximumSonosVolume')) } };
}
async function save(event: SubmitEvent): Promise<void> { event.preventDefault(); const snapshot = await invoke<Snapshot>('save_configuration', { configuration: formConfiguration(event.currentTarget as HTMLFormElement) }); render(snapshot); notice('Settings saved.'); }
async function testVolume(): Promise<void> { try { await invoke('test_volume'); notice('Volume control test requested.'); } catch (error) { notice(String(error)); } }
async function showDiagnostics(): Promise<void> { notice(JSON.stringify(await invoke('diagnostics'))); }
async function exportDiagnostics(): Promise<void> { notice(await invoke<string>('export_diagnostics')); }
async function reset(): Promise<void> { render(await invoke<Snapshot>('reset_configuration')); notice('Configuration reset.'); }
invoke<Snapshot>('get_snapshot').then(render).catch((error: unknown) => { app.textContent = `Unable to load settings: ${String(error)}`; });
