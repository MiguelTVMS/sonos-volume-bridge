import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { connectionLabel } from './connection';
import { diagnosticsDisclosureState } from './diagnostics';
import './style.css';

type MappingPoint = { local: number; sonos: number };
type SettingsPage = 'devices' | 'speaker' | 'volume' | 'general' | 'diagnostics' | 'about';
type SpeakerSettings = {
  loudness: boolean | null;
  nightSound: boolean | null;
  speechEnhancement: boolean | null;
  statusLight: boolean | null;
  treble: number | null;
  bass: number | null;
};
type Configuration = {
  schemaVersion: number;
  selectedSonosId: string | null;
  lastKnownSonosAddress: string | null;
  followDefaultAudioDevice: boolean;
  fixedAudioDeviceId: string | null;
  synchronizeMute: boolean;
  muteSpeakerAtZeroVolume: boolean;
  twoWaySynchronization: boolean;
  startAtLogin: boolean;
  fallbackPolling: boolean;
  maximumSonosVolume: number;
  mapping: {
    type: 'linear' | 'capped_linear' | 'piecewise';
    points?: MappingPoint[];
    maximum?: number;
  };
};
type Snapshot = {
  configuration: Configuration;
  status: string;
  sonosName: string | null;
  sonosVolume: number | null;
  localVolume: number | null;
  muted: boolean | null;
};
type DiscoveredSonos = { id: string; friendlyName: string; location: string };
type AudioOutput = { id: string; name: string; writableVolume: boolean };
type Diagnostics = {
  configurationPresent: boolean;
  sanitized: boolean;
  message: string;
  status: string;
  speakerName: string | null;
  selectedSonosId: string | null;
  lastKnownSonosAddress: string | null;
  sonosVolume: number | null;
  localVolume: number | null;
  muted: boolean | null;
  followsSystemOutput: boolean;
  fixedAudioDeviceId: string | null;
  synchronizeMute: boolean;
  muteSpeakerAtZeroVolume: boolean;
  twoWaySynchronization: boolean;
  fallbackPolling: boolean;
  audioInputFormat: string | null;
};

const root = document.querySelector<HTMLDivElement>('#app');
if (!root) throw new Error('Missing application root.');
const app: HTMLDivElement = root;
let snapshot: Snapshot | null = null;
let discoveredSonos: DiscoveredSonos[] = [];
let audioOutputs: AudioOutput[] = [];
let speakerSettings: SpeakerSettings = {
  loudness: null,
  nightSound: null,
  speechEnhancement: null,
  statusLight: null,
  treble: null,
  bass: null,
};
let activePage: SettingsPage = 'devices';
let discoveryStatus = 'Not checked yet';
let saveTimeout: number | undefined;
let saveRevision = 0;
let statusPoll: number | undefined;
let diagnosticDetailsVisible = false;
let appVersion = 'Loading…';

const repositoryUrl = 'https://github.com/MiguelTVMS/sonos-volume-bridge';
const sonosDisclaimer =
  'Sonos Volume Bridge is an independent, community-developed project. It is not affiliated with, sponsored by, endorsed by, or supported by Sonos. This application contains no Sonos source code. “Sonos” and related product names are trademarks of their respective owners and are used only to identify compatibility with Sonos products.';
const mitLicense = `MIT License

Copyright (c) 2026 João Miguel Tabosa Vaz Marques Silva

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;

void getVersion()
  .then((version) => {
    appVersion = version;
    if (snapshot) render(snapshot);
  })
  .catch(() => {
    appVersion = 'Unavailable';
    if (snapshot) render(snapshot);
  });
function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

function option(value: string, label: string, selected = false): string {
  return `<option value="${escapeHtml(value)}"${selected ? ' selected' : ''}>${escapeHtml(label)}</option>`;
}

function mappingOptions(configuration: Configuration): string {
  return [
    option('piecewise', 'Balanced', configuration.mapping.type === 'piecewise'),
    option('linear', 'Direct', configuration.mapping.type === 'linear'),
    option('capped_linear', 'Scaled', configuration.mapping.type === 'capped_linear'),
  ].join('');
}

function sonosOptions(configuration: Configuration): string {
  const selected = configuration.selectedSonosId;
  const devices = [...discoveredSonos];
  if (selected && !devices.some((device) => device.id === selected)) {
    devices.unshift({
      id: selected,
      friendlyName: 'Previously chosen speaker (not nearby)',
      location: configuration.lastKnownSonosAddress ?? '',
    });
  }
  return [
    option('', 'Select a Sonos speaker', !selected),
    ...devices.map((device) => option(device.id, device.friendlyName, device.id === selected)),
  ].join('');
}

function outputLabel(name: string): string {
  if (name.includes('Media Renderer') && name.includes('RINCON')) {
    return name.split(' - ')[0].trim();
  }
  return name;
}

function outputOptions(configuration: Configuration): string {
  const selected = configuration.followDefaultAudioDevice
    ? 'default'
    : (configuration.fixedAudioDeviceId ?? 'default');
  const writableOutputs = audioOutputs.filter((output) => output.writableVolume);
  if (selected !== 'default' && !writableOutputs.some((output) => output.id === selected)) {
    writableOutputs.unshift({
      id: selected,
      name: 'Previously chosen output (not available)',
      writableVolume: true,
    });
  }
  return [
    option('default', 'Follow system output', selected === 'default'),
    ...writableOutputs.map((output) =>
      option(output.id, outputLabel(output.name), output.id === selected),
    ),
  ].join('');
}

function selectedOutputName(configuration: Configuration): string {
  if (configuration.followDefaultAudioDevice) return 'Follow system output';
  const selected = audioOutputs.find((output) => output.id === configuration.fixedAudioDeviceId);
  return selected ? outputLabel(selected.name) : 'Selected output is unavailable';
}

function volumeText(volume: number | null): string {
  return volume === null ? '—' : `${volume}%`;
}

function muteText(muted: boolean | null): string {
  return muted === null ? '—' : muted ? 'Muted' : 'On';
}

function knownSonosAddress(configuration: Configuration): string {
  const selected = discoveredSonos.find((device) => device.id === configuration.selectedSonosId);
  return selected?.location ?? configuration.lastKnownSonosAddress ?? '';
}

function pageButton(page: SettingsPage, label: string): string {
  return `<button class="page-button${activePage === page ? ' active' : ''}" type="button" data-page="${page}"${activePage === page ? ' aria-current="page"' : ''}>${label}</button>`;
}

function panel(page: SettingsPage, content: string): string {
  return `<section class="panel" data-panel="${page}"${activePage === page ? '' : ' hidden'}>${content}</section>`;
}

function render(nextSnapshot: Snapshot): void {
  snapshot = nextSnapshot;
  const c = nextSnapshot.configuration;
  const speakerName = nextSnapshot.sonosName ?? 'No speaker selected';
  const status = connectionLabel(nextSnapshot.status);
  app.innerHTML = `
    <div class="settings-shell">
      <aside class="sidebar">
        <div class="app-heading"><h1><span class="sonos-name">SONOS</span><span>Volume Bridge</span></h1><p class="status" id="runtime-status">${escapeHtml(status)}</p></div>
        <nav aria-label="Settings sections">
          ${pageButton('devices', 'Devices')}
          ${pageButton('speaker', 'Speaker')}
          ${pageButton('volume', 'Volume')}
          ${pageButton('general', 'General')}
          ${pageButton('diagnostics', 'Diagnostics')}
          ${pageButton('about', 'About')}
        </nav>
        <div class="sidebar-speaker"><span>Speaker</span><b id="runtime-speaker">${escapeHtml(speakerName)}</b></div>
      </aside>
      <form id="settings" class="content">
        ${panel(
          'devices',
          `<div class="panel-heading"><h2>Devices</h2><p>Choose the speaker and audio output to keep in step.</p></div>
          <div class="settings-group">
            <div class="control-field"><label for="sonos-device">Sonos speaker</label><div class="field-row"><select name="selectedSonosId" id="sonos-device">${sonosOptions(c)}</select><button class="secondary icon-button" type="button" id="discover" title="Refresh Sonos speakers" aria-label="Refresh Sonos speakers">↻</button></div></div>
            <input type="hidden" name="lastKnownSonosAddress" value="${escapeHtml(knownSonosAddress(c))}" />
            <div class="control-field"><label for="audio-output">Follow</label><div class="field-row"><select name="audioOutputMode" id="audio-output">${outputOptions(c)}</select><button class="secondary icon-button" type="button" id="outputs" title="Refresh local outputs" aria-label="Refresh local outputs">↻</button></div></div>
            <label class="toggle"><span>Synchronize mute</span><input type="checkbox" role="switch" name="synchronizeMute" ${c.synchronizeMute ? 'checked' : ''}/></label>
          </div>`,
        )}
        ${panel(
          'speaker',
          `<div class="panel-heading"><h2>Speaker</h2><p>Adjust sound settings available on the selected Sonos speaker.</p></div><div class="settings-group"><label class="toggle"><span>Night sound</span><input type="checkbox" role="switch" data-speaker-setting="nightSound"${speakerSettings.nightSound ? ' checked' : ''}${speakerSettings.nightSound === null ? ' disabled' : ''}/></label><label class="toggle"><span>Loudness</span><input type="checkbox" role="switch" data-speaker-setting="loudness"${speakerSettings.loudness ? ' checked' : ''}${speakerSettings.loudness === null ? ' disabled' : ''}/></label><label class="toggle"><span>Status light</span><input type="checkbox" role="switch" data-speaker-setting="statusLight"${speakerSettings.statusLight ? ' checked' : ''}${speakerSettings.statusLight === null ? ' disabled' : ''}/></label><label class="toggle"><span>Speech enhancement</span><input type="checkbox" role="switch" data-speaker-setting="speechEnhancement"${speakerSettings.speechEnhancement ? ' checked' : ''}${speakerSettings.speechEnhancement === null ? ' disabled' : ''}/></label><label class="speaker-level"><span>Treble <output>${speakerSettings.treble ?? 'Unavailable'}</output></span><input type="range" min="-10" max="10" value="${speakerSettings.treble ?? 0}" data-speaker-level="treble"${speakerSettings.treble === null ? ' disabled' : ''}/></label><label class="speaker-level"><span>Bass <output>${speakerSettings.bass ?? 'Unavailable'}</output></span><input type="range" min="-10" max="10" value="${speakerSettings.bass ?? 0}" data-speaker-level="bass"${speakerSettings.bass === null ? ' disabled' : ''}/></label><div class="speaker-settings-footer"><p class="setting-note">Unavailable settings are shown in gray.</p><div class="speaker-settings-actions"><button class="secondary" type="button" id="use-tv-audio">Use TV audio</button><button class="secondary icon-button" type="button" id="refresh-speaker-settings" title="Refresh speaker settings" aria-label="Refresh speaker settings">↻</button></div></div></div>`,
        )}
        ${panel(
          'volume',
          `<div class="panel-heading"><h2>Volume</h2><p>Control how your computer volume changes the speaker.</p></div>
          <div class="settings-group">
            <label class="toggle"><span>Two-way synchronization</span><input type="checkbox" role="switch" name="twoWaySynchronization" ${c.twoWaySynchronization ? 'checked' : ''}/></label><label class="toggle"><span>Mute speaker at zero volume</span><input type="checkbox" role="switch" name="muteSpeakerAtZeroVolume" ${c.muteSpeakerAtZeroVolume ? 'checked' : ''}/></label>
            <label>Highest speaker volume <output class="range-value" id="maximum-value">${c.maximumSonosVolume}%</output><input name="maximumSonosVolume" id="maximum-volume" type="range" min="0" max="100" step="1" value="${c.maximumSonosVolume}" /></label>
            <label>Volume feel<select name="mapping">${mappingOptions(c)}</select></label>
            <details class="help"><summary>What do these options mean?</summary><dl><div><dt>Balanced</dt><dd>Gives you more control at lower volumes and rises more gently.</dd></div><div><dt>Direct</dt><dd>Keeps the speaker volume closely matched to your computer volume.</dd></div><div><dt>Scaled</dt><dd>Scales the full system volume range to the highest speaker volume you chose.</dd></div></dl></details>
            <button class="secondary test-button" type="button" id="test">Test speaker volume</button>
          </div>`,
        )}
        ${panel(
          'general',
          `<div class="panel-heading"><h2>General</h2><p>Choose how the app behaves in the background.</p></div>
          <div class="settings-group">
            <label class="toggle"><span>Start at login</span><input type="checkbox" role="switch" name="startAtLogin" ${c.startAtLogin ? 'checked' : ''}/></label>
            <label class="toggle"><span>Keep checking if updates are missed</span><input type="checkbox" role="switch" name="fallbackPolling" ${c.fallbackPolling ? 'checked' : ''}/></label>
          </div>`,
        )}
        ${panel(
          'diagnostics',
          `<div class="panel-heading"><h2>Diagnostics</h2><p>Live information about the speaker and audio output.</p></div>
          <dl class="status-list"><div><dt>Connection</dt><dd id="diagnostic-connection">${escapeHtml(status)}</dd></div><div><dt>Speaker</dt><dd id="diagnostic-speaker">${escapeHtml(speakerName)}</dd></div><div><dt>Speaker volume</dt><dd id="diagnostic-sonos-volume">${volumeText(nextSnapshot.sonosVolume)}</dd></div><div><dt>Speaker input format</dt><dd id="diagnostic-audio-input">Unavailable</dd></div><div><dt>Selected output</dt><dd id="diagnostic-output">${escapeHtml(selectedOutputName(c))}</dd></div><div><dt>Output volume</dt><dd id="diagnostic-local-volume">${volumeText(nextSnapshot.localVolume)}</dd></div><div><dt>Mute</dt><dd id="diagnostic-mute">${muteText(nextSnapshot.muted)}</dd></div><div><dt>Speaker search</dt><dd>${escapeHtml(discoveryStatus)}</dd></div></dl>
          <details class="technical-details" id="technical-details"${diagnosticDetailsVisible ? ' open' : ''}><summary>Speaker technical details</summary><p>Shows the saved speaker identity and local endpoint for troubleshooting.</p><pre id="diagnostic-payload">${diagnosticDetailsVisible ? 'Loading…' : ''}</pre></details>
          <div class="diagnostics-actions"><button class="secondary" type="button" id="export">Export diagnostics</button><button class="danger" type="button" id="reset">Reset settings</button></div>`,
        )}
        ${panel(
          'about',
          `<div class="panel-heading"><h2>About</h2><p>Version, licensing, and project information.</p></div>
          <dl class="status-list about-list"><div><dt>Version</dt><dd>${escapeHtml(appVersion)}</dd></div><div><dt>Source code</dt><dd><a href="${repositoryUrl}" target="_blank" rel="noopener noreferrer">github.com/MiguelTVMS/sonos-volume-bridge</a></dd></div><div><dt>License</dt><dd>MIT License © 2026 João Miguel Tabosa Vaz Marques Silva</dd></div></dl>
          <section class="settings-group about-disclaimer" aria-labelledby="sonos-notice-title"><h3 id="sonos-notice-title">Sonos trademark and independence notice</h3><p>${escapeHtml(sonosDisclaimer)}</p></section><details class="technical-details about-license"><summary>Read the MIT License</summary><pre>${escapeHtml(mitLicense)}</pre></details>`,
        )}
        <output id="notice" aria-live="polite"></output>
      </form>
    </div>`;
  const form = document.querySelector<HTMLFormElement>('#settings');
  const scheduleConfigurationSave = (event: Event): void => {
    if (
      !(event.target instanceof HTMLElement) ||
      (!event.target.dataset.speakerSetting && !event.target.dataset.speakerLevel)
    )
      scheduleSave();
  };
  form?.addEventListener('input', scheduleConfigurationSave);
  form?.addEventListener('change', scheduleConfigurationSave);
  document.querySelectorAll<HTMLInputElement>('[data-speaker-setting]').forEach((input) => {
    input.addEventListener('change', () => void updateSpeakerSetting(input));
  });
  document.querySelectorAll<HTMLInputElement>('[data-speaker-level]').forEach((input) => {
    input.addEventListener('input', () => {
      const label = input.closest('label')?.querySelector<HTMLOutputElement>('output');
      if (label) label.value = input.value;
    });
    input.addEventListener('change', () => void updateSpeakerLevel(input));
  });

  document.querySelectorAll<HTMLButtonElement>('[data-page]').forEach((button) => {
    button.addEventListener('click', () => activatePage(button.dataset.page as SettingsPage));
  });
  document.querySelector('#test')?.addEventListener('click', testVolume);
  document.querySelector('#use-tv-audio')?.addEventListener('click', () => void useTvAudio());
  document
    .querySelector('#refresh-speaker-settings')
    ?.addEventListener('click', () => void refreshSpeakerSettings());
  document.querySelector('#technical-details')?.addEventListener('toggle', refreshDiagnostics);
  document.querySelector('#export')?.addEventListener('click', exportDiagnostics);
  document.querySelector('#reset')?.addEventListener('click', reset);
  document.querySelector('#discover')?.addEventListener('click', discoverSonos);
  document.querySelector('#outputs')?.addEventListener('click', refreshAudioOutputs);
  document
    .querySelector<HTMLSelectElement>('#sonos-device')
    ?.addEventListener('change', syncSelectedSonosAddress);
  document
    .querySelector<HTMLInputElement>('#maximum-volume')
    ?.addEventListener('input', updateMaximumValue);
}

function activatePage(page: SettingsPage): void {
  activePage = page;
  if (page === 'diagnostics') void refreshAudioInputFormat();
  document.querySelectorAll<HTMLElement>('[data-panel]').forEach((element) => {
    element.hidden = element.dataset.panel !== page;
  });
  document.querySelectorAll<HTMLButtonElement>('[data-page]').forEach((button) => {
    const active = button.dataset.page === page;
    button.classList.toggle('active', active);
    button.toggleAttribute('aria-current', active);
  });
}

function refreshRuntimeStatus(nextSnapshot: Snapshot): void {
  snapshot = nextSnapshot;
  const status = connectionLabel(nextSnapshot.status);
  const speaker = nextSnapshot.sonosName ?? 'No speaker selected';
  const targets: Array<[string, string]> = [
    ['#runtime-status', status],
    ['#runtime-speaker', speaker],
    ['#diagnostic-connection', status],
    ['#diagnostic-speaker', speaker],
    ['#diagnostic-sonos-volume', volumeText(nextSnapshot.sonosVolume)],
    ['#diagnostic-local-volume', volumeText(nextSnapshot.localVolume)],
    ['#diagnostic-mute', muteText(nextSnapshot.muted)],
  ];
  for (const [selector, value] of targets) {
    const element = document.querySelector<HTMLElement>(selector);
    if (element) element.textContent = value;
  }
}

function startStatusPolling(): void {
  if (statusPoll !== undefined) return;
  statusPoll = window.setInterval(() => {
    void invoke<Snapshot>('get_snapshot')
      .then(refreshRuntimeStatus)
      .catch(() => undefined);
  }, 1_000);
}

function syncSelectedSonosAddress(): void {
  const select = document.querySelector<HTMLSelectElement>('#sonos-device');
  const selected = discoveredSonos.find((device) => device.id === select?.value);
  const address = select?.value
    ? (selected?.location ?? snapshot?.configuration.lastKnownSonosAddress ?? '')
    : '';
  const hidden = document.querySelector<HTMLInputElement>('input[name="lastKnownSonosAddress"]');
  if (hidden) hidden.value = address;
}

function updateMaximumValue(event: Event): void {
  const input = event.currentTarget as HTMLInputElement;
  const output = document.querySelector<HTMLOutputElement>('#maximum-value');
  if (output) output.value = `${input.value}%`;
}

async function refreshAudioOutputs(): Promise<void> {
  try {
    audioOutputs = await invoke<AudioOutput[]>('list_audio_outputs');
    if (snapshot) render(snapshot);
  } catch (error) {
    notice(String(error));
  }
}

async function discoverSonos(): Promise<void> {
  try {
    discoveredSonos = await invoke<DiscoveredSonos[]>('discover_sonos');
    if (discoveredSonos.length === 0) {
      discoveryStatus = 'No speakers found';
      if (snapshot) render(snapshot);
      return;
    }
    discoveryStatus = `Found ${discoveredSonos.length} speaker${discoveredSonos.length === 1 ? '' : 's'}`;
    if (snapshot) {
      render(snapshot);
      syncSelectedSonosAddress();
    }
  } catch {
    discoveryStatus = 'Unable to search this network';
    if (snapshot) render(snapshot);
  }
}

async function refreshSpeakerSettings(): Promise<void> {
  speakerSettings = await invoke<SpeakerSettings>('get_speaker_settings').catch(() => ({
    loudness: null,
    nightSound: null,
    speechEnhancement: null,
    statusLight: null,
    treble: null,
    bass: null,
  }));
  if (snapshot) render(snapshot);
}

async function useTvAudio(): Promise<void> {
  try {
    await invoke('use_tv_audio');
    notice('TV audio selected.');
    void refreshAudioInputFormat();
  } catch (error) {
    notice(String(error));
  }
}
async function updateSpeakerSetting(input: HTMLInputElement): Promise<void> {
  try {
    await invoke('set_speaker_setting', {
      setting: input.dataset.speakerSetting,
      enabled: input.checked,
    });
    speakerSettings[
      input.dataset.speakerSetting as
        'loudness' | 'nightSound' | 'speechEnhancement' | 'statusLight'
    ] = input.checked;
    notice('Saved.');
  } catch (error) {
    input.checked = !input.checked;
    notice(String(error));
  }
}

async function updateSpeakerLevel(input: HTMLInputElement): Promise<void> {
  try {
    const setting = input.dataset.speakerLevel as 'treble' | 'bass';
    const value = Number(input.value);
    await invoke('set_speaker_level', { setting, value });
    speakerSettings[setting] = value;
    notice('Saved.');
  } catch (error) {
    notice(String(error));
  }
}
function notice(value: string): void {
  const output = document.querySelector<HTMLOutputElement>('#notice');
  if (output) output.value = value;
}

function formConfiguration(form: HTMLFormElement): Configuration {
  const values = new FormData(form);
  const mapping = String(values.get('mapping')) as Configuration['mapping']['type'];
  const output = String(values.get('audioOutputMode') ?? 'default');
  return {
    schemaVersion: 1,
    selectedSonosId: String(values.get('selectedSonosId') ?? '') || null,
    lastKnownSonosAddress: String(values.get('lastKnownSonosAddress') ?? '') || null,
    followDefaultAudioDevice: output === 'default',
    fixedAudioDeviceId: output === 'default' ? null : output,
    synchronizeMute: values.has('synchronizeMute'),
    muteSpeakerAtZeroVolume: values.has('muteSpeakerAtZeroVolume'),
    twoWaySynchronization: values.has('twoWaySynchronization'),
    startAtLogin: values.has('startAtLogin'),
    fallbackPolling: values.has('fallbackPolling'),
    maximumSonosVolume: Number(values.get('maximumSonosVolume')),
    mapping:
      mapping === 'piecewise'
        ? {
            type: mapping,
            points: [
              { local: 0, sonos: 0 },
              { local: 20, sonos: 5 },
              { local: 40, sonos: 12 },
              { local: 60, sonos: 23 },
              { local: 80, sonos: 40 },
              { local: 100, sonos: 55 },
            ],
          }
        : mapping === 'linear'
          ? { type: mapping }
          : { type: mapping, maximum: Number(values.get('maximumSonosVolume')) },
  };
}

function scheduleSave(): void {
  if (saveTimeout !== undefined) window.clearTimeout(saveTimeout);
  const revision = ++saveRevision;
  saveTimeout = window.setTimeout(() => {
    const form = document.querySelector<HTMLFormElement>('#settings');
    if (form) void saveConfiguration(formConfiguration(form), revision);
  }, 350);
}

async function saveConfiguration(configuration: Configuration, revision: number): Promise<void> {
  try {
    const nextSnapshot = await invoke<Snapshot>('save_configuration', { configuration });
    if (revision !== saveRevision) return;
    render(nextSnapshot);
    notice('Saved.');
  } catch (error) {
    if (revision === saveRevision) notice(`Could not save: ${String(error)}`);
  }
}

async function testVolume(): Promise<void> {
  try {
    await invoke('test_volume');
    notice('Volume control test requested.');
  } catch (error) {
    notice(String(error));
  }
}
async function refreshAudioInputFormat(): Promise<void> {
  try {
    const diagnostics = await invoke<Diagnostics>('diagnostics');
    const audioInput = document.querySelector<HTMLElement>('#diagnostic-audio-input');
    if (audioInput) audioInput.textContent = diagnostics.audioInputFormat ?? 'Unavailable';
  } catch {
    /* Diagnostics remain usable when the speaker is unavailable. */
  }
}
async function refreshDiagnostics(event: Event): Promise<void> {
  const details = event.currentTarget as HTMLDetailsElement;
  const disclosureState = diagnosticsDisclosureState(details.open);
  diagnosticDetailsVisible = disclosureState.visible;
  if (!disclosureState.shouldRefresh) return;

  try {
    const diagnostics = await invoke<Diagnostics>('diagnostics');
    const audioInput = document.querySelector<HTMLElement>('#diagnostic-audio-input');
    if (audioInput) audioInput.textContent = diagnostics.audioInputFormat ?? 'Unavailable';
    const payload = document.querySelector<HTMLPreElement>('#diagnostic-payload');
    if (payload) payload.textContent = JSON.stringify(diagnostics, null, 2);
  } catch (error) {
    notice(`Could not load diagnostics: ${String(error)}`);
  }
}
async function exportDiagnostics(): Promise<void> {
  notice(await invoke<string>('export_diagnostics'));
}
async function reset(): Promise<void> {
  render(await invoke<Snapshot>('reset_configuration'));
  notice('Settings reset.');
}

invoke<Snapshot>('get_snapshot')
  .then((nextSnapshot) => {
    render(nextSnapshot);
    startStatusPolling();
    void refreshAudioOutputs();
    void refreshSpeakerSettings();
    discoveryStatus = 'Searching…';
    void discoverSonos();
  })
  .catch((error: unknown) => {
    app.textContent = `Unable to load settings: ${String(error)}`;
  });
