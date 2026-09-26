import {
  scheduleMarkup,
  setSystemHour12,
  captureScheduleView,
  mountSchedule,
  updateScheduleView,
  scheduleDraft,
  emptyBlocks,
  type NightSchedule,
  type ScheduleNotifications,
  type ScheduleStatus,
} from './night-schedule';
import { getVersion } from '@tauri-apps/api/app';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { bindWindowFocus } from './window-appearance';
import { connectionLabel } from './connection';
import { diagnosticsDisclosureState } from './diagnostics';
import { desktopPlatform } from './platform';
import { settingCaption } from './setting-caption';
import { sizeSelectedControls } from './select-sizing';
import { canApplyRefresh } from './refresh-state';
import { adjacentPage, type SettingsPage } from './settings-navigation';
import { applySpeakerControls, type SpeakerSettings } from './speaker-controls';
import { SliderInteraction } from './slider-interaction';
import { LiveStatus } from './live-status';
import { UserWrites } from './user-writes';
import './style.css';
import './platform.css';
import './windows.css';

const platform = desktopPlatform(navigator.userAgent);
document.documentElement.dataset.platform = platform;

if (document.documentElement.dataset.platform === 'macos') {
  const applyFocus = (focused: boolean): void => {
    document.documentElement.dataset.windowActive = String(focused);
  };
  applyFocus(document.hasFocus());
  if (isTauri()) {
    const nativeWindow = getCurrentWindow();
    void bindWindowFocus(
      {
        listen: (update) => nativeWindow.onFocusChanged(({ payload }) => update(payload)),
        current: () => nativeWindow.isFocused(),
      },
      applyFocus,
    ).catch(() => {
      window.addEventListener('focus', () => applyFocus(true));
      window.addEventListener('blur', () => applyFocus(false));
      applyFocus(document.hasFocus());
    });
  } else {
    window.addEventListener('focus', () => applyFocus(true));
    window.addEventListener('blur', () => applyFocus(false));
  }
}

type MappingPoint = { local: number; sonos: number };
type Configuration = {
  schemaVersion: number;
  nightModeSchedule?: NightSchedule;
  notifyNightModeScheduleTransitions?: ScheduleNotifications;
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
let scheduleStatus: ScheduleStatus = {
  active: false,
  supported: false,
  message: '',
  nextTransition: null,
  timeZone: '',
  notificationsBlocked: false,
};
let activePage: SettingsPage = 'devices';
let discoveryStatus = 'Not checked yet';
let saveTimeout: number | undefined;
let saveRevision = 0;
let statusPoll: number | undefined;
let diagnosticDetailsVisible = false;
let speakerReadRequest = 0;
let pushRefreshTimeout: number | undefined;
let lastFallbackRefresh = 0;
let refreshRequest = 0;
let editRevision = 0;
let pendingWrites = 0;
const sliders = new SliderInteraction();
const userWrites = new UserWrites();
const liveStatus = new LiveStatus<Snapshot>(refreshRuntimeStatus);
let refreshRunning = false;
let refreshAgain = false;
let currentNotice = '';
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
      friendlyName: 'Speaker unavailable',
      location: configuration.lastKnownSonosAddress ?? '',
    });
  }
  return [
    option('', 'Select speaker', !selected),
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
      name: 'Output unavailable',
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

const pageIcons: Record<SettingsPage, string> = {
  devices:
    '<rect x="3" y="4" width="12" height="10" rx="2"/><path d="M6 18h6m-3-4v4"/><rect x="17" y="8" width="4" height="12" rx="1"/>',
  speaker:
    '<rect x="6" y="2" width="12" height="20" rx="3"/><circle cx="12" cy="14" r="4"/><circle cx="12" cy="6" r="1"/>',
  schedule:
    '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M7 3v4m10-4v4M3 10h18m-14 4h3m4 0h3m-10 4h3"/>',
  volume: '<path d="M11 4 6 8H3v8h3l5 4V4Zm4 4a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14"/>',
  general:
    '<path d="M4 6h16M4 12h16M4 18h16"/><circle cx="8" cy="6" r="2"/><circle cx="16" cy="12" r="2"/><circle cx="10" cy="18" r="2"/>',
  diagnostics: '<path d="M3 12h4l3-7 4 14 3-7h4"/>',
  about: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-11v1"/>',
};

function pageButton(page: SettingsPage, label: string): string {
  return `<button class="page-button${activePage === page ? ' active' : ''}" type="button" data-page="${page}"${activePage === page ? ' aria-current="page"' : ''}><span class="nav-icon nav-icon-${page}" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${pageIcons[page]}</svg></span><span>${label}</span></button>`;
}

function panel(page: SettingsPage, content: string): string {
  return `<section class="panel" data-panel="${page}"${activePage === page ? '' : ' hidden'}>${content}</section>`;
}

function render(nextSnapshot: Snapshot): void {
  if (sliders.active || scheduleDraft.painting !== null) return;
  captureScheduleView(app);
  snapshot = nextSnapshot;
  refreshScheduleView();
  const c = nextSnapshot.configuration;
  const speakerName = nextSnapshot.sonosName ?? 'No speaker selected';
  const status = connectionLabel(nextSnapshot.status);
  app.innerHTML = `
    <div class="settings-shell">
      <div class="macos-titlebar" data-tauri-drag-region>
        <div class="section-navigation" role="group" aria-label="Navigate settings sections">
          <button type="button" id="previous-section" aria-label="Previous settings section" title="Previous settings section"${adjacentPage(activePage, -1) ? '' : ' disabled'}><svg aria-hidden="true" viewBox="0 0 16 16"><path d="m10 2-6 6 6 6"/></svg></button>
          <button type="button" id="next-section" aria-label="Next settings section" title="Next settings section"${adjacentPage(activePage, 1) ? '' : ' disabled'}><svg aria-hidden="true" viewBox="0 0 16 16"><path d="m6 2 6 6-6 6"/></svg></button>
        </div>
        <span id="toolbar-section-title" data-tauri-drag-region>${activePage === 'schedule' ? 'Night schedule' : activePage[0].toUpperCase() + activePage.slice(1)}</span>
      </div>
      <aside class="sidebar">
        <div class="app-heading"><h1><span class="sonos-name">SONOS</span><span>Volume Bridge</span></h1><p class="status" id="runtime-status">${escapeHtml(status)}</p></div>
        <nav aria-label="Settings sections">
          ${pageButton('devices', 'Devices')}
          ${pageButton('speaker', 'Speaker')}
          ${pageButton('schedule', 'Night schedule')}
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
            <div class="control-field"><label for="sonos-device">${settingCaption(platform, 'Sonos speaker', 'Choose the speaker to control from this computer.', 'speaker')}</label><div class="field-row"><select name="selectedSonosId" id="sonos-device">${sonosOptions(c)}</select><button class="secondary icon-button" type="button" id="discover" title="Refresh Sonos speakers" aria-label="Refresh Sonos speakers">↻</button></div></div>
            <input type="hidden" name="lastKnownSonosAddress" value="${escapeHtml(knownSonosAddress(c))}" />
            <div class="control-field"><label for="audio-output">${settingCaption(platform, 'Follow', 'Choose which computer audio output to follow.', 'output')}</label><div class="field-row"><select name="audioOutputMode" id="audio-output">${outputOptions(c)}</select><button class="secondary icon-button" type="button" id="outputs" title="Refresh local outputs" aria-label="Refresh local outputs">↻</button></div></div>
            <label class="toggle"><span>${settingCaption(platform, 'Synchronize mute', 'Keep the computer and speaker mute states in step.', 'mute')}</span><input type="checkbox" role="switch" name="synchronizeMute" ${c.synchronizeMute ? 'checked' : ''}/></label>
          </div>`,
        )}
        ${panel(
          'speaker',
          `<div class="panel-heading"><h2>Speaker</h2><p>Adjust sound settings available on the selected Sonos speaker.</p></div><div class="settings-group"><label class="toggle"><span>${settingCaption(platform, 'Night sound', 'Reduce loud sounds for quieter listening.', 'moon')}<small class="feature-status" data-feature-status="nightSound"></small></span><input type="checkbox" role="switch" data-speaker-setting="nightSound"${speakerSettings.nightSound ? ' checked' : ''}${speakerSettings.nightSound === null ? ' disabled' : ''}/></label><label class="toggle"><span>${settingCaption(platform, 'Loudness', 'Enhance bass and treble at lower volumes.', 'sound')}<small class="feature-status" data-feature-status="loudness"></small></span><input type="checkbox" role="switch" data-speaker-setting="loudness"${speakerSettings.loudness ? ' checked' : ''}${speakerSettings.loudness === null ? ' disabled' : ''}/></label><label class="toggle"><span>${settingCaption(platform, 'Status light', 'Show the indicator light on the speaker.', 'light')}<small class="feature-status" data-feature-status="statusLight"></small></span><input type="checkbox" role="switch" data-speaker-setting="statusLight"${speakerSettings.statusLight ? ' checked' : ''}${speakerSettings.statusLight === null ? ' disabled' : ''}/></label><label class="toggle"><span>${settingCaption(platform, 'Speech enhancement', 'Make voices easier to hear.', 'speech')}<small class="feature-status" data-feature-status="speechEnhancement"></small></span><input type="checkbox" role="switch" data-speaker-setting="speechEnhancement"${speakerSettings.speechEnhancement ? ' checked' : ''}${speakerSettings.speechEnhancement === null ? ' disabled' : ''}/></label><label class="speaker-level"><span>${settingCaption(platform, 'Treble', 'Adjust the higher frequencies.', 'tone')} <output>${speakerSettings.treble ?? 'Unavailable'}</output><small class="feature-status" data-feature-status="treble"></small></span><input type="range" min="-10" max="10" value="${speakerSettings.treble ?? 0}" data-speaker-level="treble"${speakerSettings.treble === null ? ' disabled' : ''}/></label><label class="speaker-level"><span>${settingCaption(platform, 'Bass', 'Adjust the lower frequencies.', 'tone')} <output>${speakerSettings.bass ?? 'Unavailable'}</output><small class="feature-status" data-feature-status="bass"></small></span><input type="range" min="-10" max="10" value="${speakerSettings.bass ?? 0}" data-speaker-level="bass"${speakerSettings.bass === null ? ' disabled' : ''}/></label><div class="speaker-settings-footer"><p class="setting-note">Unavailable settings are checked again on refresh.</p><div class="speaker-settings-actions"><button class="secondary" type="button" id="use-tv-audio">Use TV audio</button><button class="secondary icon-button" type="button" id="refresh-speaker-settings" title="Refresh speaker settings" aria-label="Refresh speaker settings">↻</button></div></div></div>`,
        )}
        ${panel(
          'schedule',
          `<div class="panel-heading"><h2 id="night-schedule-title">Night schedule</h2><p>Set weekly Night Mode hours for the selected speaker. This schedule applies to your selected speaker when it supports Night Mode.</p></div>${scheduleMarkup()}`,
        )}
        ${panel(
          'volume',
          `<div class="panel-heading"><h2>Volume</h2><p>Control how your computer volume changes the speaker.</p></div>
          <div class="settings-group">
            <label class="toggle"><span>${settingCaption(platform, 'Two-way synchronization', 'Also follow volume changes made on the speaker.', 'sync')}</span><input type="checkbox" role="switch" name="twoWaySynchronization" ${c.twoWaySynchronization ? 'checked' : ''}/></label><label class="toggle"><span>${settingCaption(platform, 'Mute speaker at zero volume', 'Mute Sonos when computer volume reaches zero.', 'mute')}</span><input type="checkbox" role="switch" name="muteSpeakerAtZeroVolume" ${c.muteSpeakerAtZeroVolume ? 'checked' : ''}/></label>
            <label class="volume-limit"><span>${settingCaption(platform, 'Highest speaker volume', 'Limit how loud the speaker can become.', 'sound')} <output class="range-value" id="maximum-value">${c.maximumSonosVolume}%</output></span><input name="maximumSonosVolume" id="maximum-volume" type="range" min="0" max="100" step="1" value="${c.maximumSonosVolume}" /></label>
            <label class="select-setting"><span>${settingCaption(platform, 'Volume feel', 'Choose how computer volume maps to the speaker.', 'tone')}</span><select name="mapping">${mappingOptions(c)}</select></label>
            <details class="help"><summary>What do these options mean?</summary><dl><div><dt>Balanced</dt><dd>Gives you more control at lower volumes and rises more gently.</dd></div><div><dt>Direct</dt><dd>Keeps the speaker volume closely matched to your computer volume.</dd></div><div><dt>Scaled</dt><dd>Scales the full system volume range to the highest speaker volume you chose.</dd></div></dl></details>
            <button class="secondary test-button" type="button" id="test">Test speaker volume</button>
          </div>`,
        )}
        ${panel(
          'general',
          `<div class="panel-heading"><h2>General</h2><p>Choose how the app behaves in the background.</p></div>
          <div class="settings-group">
            <label class="toggle"><span>${settingCaption(platform, 'Start at login', 'Run Volume Bridge when you sign in.', 'power')}</span><input type="checkbox" role="switch" name="startAtLogin" ${c.startAtLogin ? 'checked' : ''}/></label>
            <label class="toggle"><span>${settingCaption(platform, 'Keep checking if updates are missed', 'Recover speaker updates when notifications are interrupted.', 'sync')}</span><input type="checkbox" role="switch" name="fallbackPolling" ${c.fallbackPolling ? 'checked' : ''}/></label>
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
        <output id="notice" aria-live="polite">${escapeHtml(currentNotice)}</output>
      </form>
    </div>`;
  applySpeakerControls(app, speakerSettings, document.activeElement);
  refreshScheduleView();
  if (document.documentElement.dataset.platform === 'macos') sizeSelectedControls(app);
  document.querySelector('#previous-section')?.addEventListener('click', () => {
    const page = adjacentPage(activePage, -1);
    if (page) activatePage(page);
  });
  document.querySelector('#next-section')?.addEventListener('click', () => {
    const page = adjacentPage(activePage, 1);
    if (page) activatePage(page);
  });
  mountSchedule(
    app,
    snapshot?.configuration.nightModeSchedule ?? { enabled: false, blocks: emptyBlocks() },
    snapshot?.configuration.notifyNightModeScheduleTransitions ?? 'never',
    {
      save: async (blocks) => {
        const revision = scheduleDraft.revision;
        await writeSchedule('save_night_schedule', { blocks }, (next) => {
          if (revision === scheduleDraft.revision)
            scheduleDraft.reset(next.configuration.nightModeSchedule!.blocks);
        });
        notice('Schedule saved and applied to the selected speaker.');
      },
      enable: async (enabled) => writeSchedule('enable_night_schedule', { enabled }),
      notify: async (mode) => writeSchedule('set_schedule_notifications', { mode }),
      error: notice,
    },
  );
  refreshScheduleView();
  const form = document.querySelector<HTMLFormElement>('#settings');
  const scheduleConfigurationSave = (event: Event): void => {
    if (event.target instanceof Element && event.target.closest('[data-schedule]')) return;
    if (
      !(event.target instanceof HTMLElement) ||
      (!event.target.dataset.speakerSetting &&
        !event.target.dataset.speakerLevel &&
        !(event.target instanceof HTMLInputElement && event.target.type === 'range'))
    )
      scheduleSave();
  };
  form?.addEventListener('input', scheduleConfigurationSave);
  form?.addEventListener('change', scheduleConfigurationSave);
  document.querySelectorAll<HTMLInputElement>('[data-speaker-setting]').forEach((input) => {
    input.addEventListener('change', () => void updateSpeakerSetting(input));
  });
  document.querySelectorAll<HTMLInputElement>('input[type="range"]').forEach((input) => {
    sliders.bind(
      input,
      () => {
        editRevision++;
        saveRevision++;
      },
      () => {
        const label = input.closest('label')?.querySelector<HTMLOutputElement>('output');
        if (label) label.value = input.id === 'maximum-volume' ? `${input.value}%` : input.value;
      },
      () => {
        if (input.dataset.speakerLevel) void updateSpeakerLevel(input);
        else scheduleSave();
      },
    );
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
  const title = document.querySelector('#toolbar-section-title');
  if (title)
    title.textContent =
      page === 'schedule' ? 'Night schedule' : page[0].toUpperCase() + page.slice(1);
  const previous = document.querySelector<HTMLButtonElement>('#previous-section');
  const next = document.querySelector<HTMLButtonElement>('#next-section');
  if (previous) previous.disabled = !adjacentPage(page, -1);
  if (next) next.disabled = !adjacentPage(page, 1);
  document.querySelector('.content')?.scrollTo(0, 0);
  void refreshAllSettings();
  document.querySelectorAll<HTMLElement>('[data-panel]').forEach((element) => {
    element.hidden = element.dataset.panel !== page;
  });
  document.querySelectorAll<HTMLButtonElement>('[data-page]').forEach((button) => {
    const active = button.dataset.page === page;
    button.classList.toggle('active', active);
    if (active) button.setAttribute('aria-current', 'page');
    else button.removeAttribute('aria-current');
  });
}

function refreshRuntimeStatus(nextSnapshot: Snapshot): void {
  snapshot = nextSnapshot;
  refreshScheduleView();
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
    void liveStatus
      .read(() => invoke<Snapshot>('get_snapshot'))
      .then(() => {
        const next = snapshot;
        if (!next) return;
        if (document.visibilityState === 'visible' && Date.now() - lastFallbackRefresh > 5000) {
          lastFallbackRefresh = Date.now();
          schedulePushRefresh();
        }
      })
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
  if (!snapshot || sliders.active || pendingWrites > 0 || saveTimeout !== undefined) return;
  const request = ++speakerReadRequest;
  const revision = editRevision;
  const speaker = await invoke<SpeakerSettings>('get_speaker_settings').catch(() => ({
    loudness: null,
    nightSound: null,
    speechEnhancement: null,
    statusLight: null,
    treble: null,
    bass: null,
  }));
  if (
    !canApplyRefresh(
      request,
      speakerReadRequest,
      revision,
      editRevision,
      sliders.active || pendingWrites > 0 || saveTimeout !== undefined,
    )
  )
    return;
  speakerSettings = speaker;
  applySpeakerControls(app, speakerSettings, document.activeElement);
  refreshScheduleView();
}

function schedulePushRefresh(): void {
  editRevision++;
  if (pushRefreshTimeout !== undefined) window.clearTimeout(pushRefreshTimeout);
  pushRefreshTimeout = window.setTimeout(() => {
    pushRefreshTimeout = undefined;
    void refreshSpeakerSettings();
  }, 150);
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
  editRevision++;
  pendingWrites++;
  try {
    const setting = input.dataset.speakerSetting;
    const enabled = input.checked;
    await userWrites.run(() => invoke('set_speaker_setting', { setting, enabled }));
    notice('Saved.');
  } catch (error) {
    notice(String(error));
  } finally {
    pendingWrites--;
    void refreshAllSettings();
  }
}

async function updateSpeakerLevel(input: HTMLInputElement): Promise<void> {
  editRevision++;
  pendingWrites++;
  try {
    const setting = input.dataset.speakerLevel as 'treble' | 'bass';
    const value = Number(input.value);
    await userWrites.run(() => invoke('set_speaker_level', { setting, value }));
    speakerSettings[setting] = value;
    notice('Saved.');
  } catch (error) {
    notice(String(error));
  } finally {
    pendingWrites--;
    void refreshAllSettings();
  }
}
function notice(value: string): void {
  currentNotice = value;
  const output = document.querySelector<HTMLOutputElement>('#notice');
  if (output) output.value = value;
}

function formConfiguration(form: HTMLFormElement): Configuration {
  const values = new FormData(form);
  const mapping = String(values.get('mapping')) as Configuration['mapping']['type'];
  const output = String(values.get('audioOutputMode') ?? 'default');
  return {
    schemaVersion: 1,
    nightModeSchedule: snapshot?.configuration.nightModeSchedule,
    notifyNightModeScheduleTransitions: snapshot?.configuration.notifyNightModeScheduleTransitions,
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
  editRevision++;
  if (saveTimeout !== undefined) window.clearTimeout(saveTimeout);
  const revision = ++saveRevision;
  saveTimeout = window.setTimeout(() => {
    saveTimeout = undefined;
    if (sliders.active) {
      scheduleSave();
      return;
    }
    const form = document.querySelector<HTMLFormElement>('#settings');
    if (form) void saveConfiguration(formConfiguration(form), revision);
  }, 350);
}

async function saveConfiguration(configuration: Configuration, revision: number): Promise<void> {
  pendingWrites++;
  let saved = false;
  try {
    const nextSnapshot = await invoke<Snapshot>('save_configuration', { configuration });
    if (revision !== saveRevision) return;
    render(nextSnapshot);
    notice('Saved.');
    saved = true;
  } catch (error) {
    if (revision === saveRevision) notice(`Could not save: ${String(error)}`);
  } finally {
    pendingWrites--;
    if (saved) void refreshAllSettings();
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

Promise.all([
  invoke<Snapshot>('get_snapshot'),
  invoke<boolean | null>('get_system_hour12').catch(() => null),
])
  .then(([nextSnapshot, hour12]) => {
    setSystemHour12(hour12);
    render(nextSnapshot);
    startStatusPolling();
    void refreshAllSettings();
  })
  .catch((error: unknown) => {
    app.textContent = `Unable to load settings: ${String(error)}`;
  });

async function refreshAllSettings(): Promise<void> {
  if (!snapshot) return;
  if (refreshRunning) {
    refreshAgain = true;
    return;
  }
  if (
    scheduleDraft.painting !== null ||
    sliders.active ||
    saveTimeout !== undefined ||
    pendingWrites > 0
  )
    return;
  refreshRunning = true;
  const request = ++refreshRequest;
  const revision = editRevision;
  try {
    const [next, speaker, outputs, discovered, diagnostics, hour12] = await Promise.all([
      invoke<Snapshot>('get_snapshot'),
      invoke<SpeakerSettings>('get_speaker_settings'),
      invoke<AudioOutput[]>('list_audio_outputs').catch(() => null),
      invoke<DiscoveredSonos[]>('discover_sonos').catch(() => null),
      invoke<Diagnostics>('diagnostics').catch(() => null),
      invoke<boolean | null>('get_system_hour12').catch(() => null),
    ]);
    if (
      !canApplyRefresh(
        request,
        refreshRequest,
        revision,
        editRevision,
        sliders.active || pendingWrites > 0 || saveTimeout !== undefined,
      )
    )
      return;
    setSystemHour12(hour12);
    speakerSettings = speaker;
    if (outputs) audioOutputs = outputs;
    if (discovered) {
      discoveredSonos = discovered;
      discoveryStatus = `Found ${discovered.length} speaker${discovered.length === 1 ? '' : 's'}`;
    }
    render({
      ...next,
      status: snapshot.status,
      sonosName: snapshot.sonosName,
      sonosVolume: snapshot.sonosVolume,
      localVolume: snapshot.localVolume,
      muted: snapshot.muted,
    });
    const audioInput = document.querySelector('#diagnostic-audio-input');
    if (audioInput) audioInput.textContent = diagnostics?.audioInputFormat ?? 'Unavailable';
    const payload = document.querySelector('#diagnostic-payload');
    if (payload && diagnosticDetailsVisible && diagnostics)
      payload.textContent = JSON.stringify(diagnostics, null, 2);
  } catch {
    notice('Could not refresh settings. Check the speaker connection and try again.');
  } finally {
    refreshRunning = false;
    if (refreshAgain) {
      refreshAgain = false;
      void refreshAllSettings();
    }
  }
}

if (isTauri()) {
  void listen<Snapshot>('runtime-status-changed', ({ payload }) => liveStatus.push(payload)).catch(
    () => undefined,
  );
  void getCurrentWindow()
    .onFocusChanged(({ payload: focused }) => {
      if (focused) void refreshAllSettings();
    })
    .catch(() => {
      window.addEventListener('focus', () => void refreshAllSettings());
    });
} else {
  window.addEventListener('focus', () => void refreshAllSettings());
}

if (isTauri()) {
  void listen('speaker-settings-changed', schedulePushRefresh).catch(() => {
    notice('Live speaker updates are unavailable. Reopen settings to refresh.');
  });
}

function refreshScheduleView(): void {
  updateScheduleView(
    app,
    scheduleStatus,
    speakerSettings.capabilities?.nightSound === 'supported' ||
      (speakerSettings.capabilities?.nightSound === undefined &&
        speakerSettings.nightSound !== null),
    snapshot?.configuration.nightModeSchedule?.enabled ?? false,
  );
}
if (isTauri()) {
  void listen<ScheduleStatus>('night-schedule-changed', ({ payload }) => {
    scheduleStatus = payload;
    refreshScheduleView();
  });
  void invoke<ScheduleStatus>('get_schedule_status').then((status) => {
    scheduleStatus = status;
    refreshScheduleView();
  });
  void listen('open-night-schedule', () => {
    activatePage('schedule');
  });
}

async function writeSchedule(
  command: string,
  args: Record<string, unknown>,
  accepted?: (next: Snapshot) => void,
): Promise<void> {
  editRevision++;
  pendingWrites++;
  try {
    const next = await userWrites.run(() => invoke<Snapshot>(command, args));
    accepted?.(next);
    scheduleStatus = await invoke<ScheduleStatus>('get_schedule_status');
    render(next);
  } finally {
    pendingWrites--;
    void refreshAllSettings();
  }
}
