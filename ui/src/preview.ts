// Development-only entry point. Vite's production build includes index.html only.
// All commands are mocked: this preview cannot discover or modify real devices.
import { mockIPC } from '@tauri-apps/api/mocks';

const platform = new URLSearchParams(location.search).get('platform') ?? 'macos';
const agents: Record<string, string> = {
  macos: 'Macintosh; Intel Mac OS X',
  windows: 'Windows NT 10.0',
  linux: 'X11; Linux x86_64',
};
Object.defineProperty(navigator, 'userAgent', { value: agents[platform] ?? 'Preview' });
const snapshot = {
  configuration: {
    schemaVersion: 1,
    selectedSonosId: 'sample-speaker',
    lastKnownSonosAddress: null,
    followDefaultAudioDevice: true,
    fixedAudioDeviceId: null,
    synchronizeMute: true,
    muteSpeakerAtZeroVolume: false,
    twoWaySynchronization: true,
    startAtLogin: false,
    fallbackPolling: true,
    cameraSpeechEnhancementEnabled: false,
    maximumSonosVolume: 70,
    mapping: { type: 'linear' },
  },
  status: 'synchronized',
  sonosName: 'Living Room',
  sonosVolume: 28,
  localVolume: 28,
  muted: false,
};
const speakerSettings = {
  nightSound: false,
  loudness: true,
  statusLight: true,
  speechEnhancement: false,
  treble: 0,
  bass: 0,
};
mockIPC((command, payload) => {
  switch (command) {
    case 'plugin:app|version':
      return 'Preview';
    case 'get_camera_status':
      return {
        available: platform !== 'linux',
        message:
          platform === 'linux'
            ? 'Camera automation is unavailable in this platform build'
            : 'Camera automation is waiting',
        warning: null,
      };
    case 'get_snapshot':
      return snapshot;
    case 'save_configuration':
      Object.assign(snapshot.configuration, (payload as { configuration: object }).configuration);
      return snapshot;
    case 'discover_sonos':
      return [{ id: 'sample-speaker', friendlyName: 'Living Room', location: '' }];
    case 'list_audio_outputs':
      return [{ id: 'sample-output', name: 'Built-in speakers', writableVolume: true }];
    case 'get_speaker_settings':
      return speakerSettings;
    case 'set_speaker_setting': {
      const { setting, enabled } = payload as { setting: string; enabled: boolean };
      Object.assign(speakerSettings, { [setting]: enabled });
      return;
    }
    case 'set_speaker_level': {
      const { setting, value } = payload as { setting: string; value: number };
      Object.assign(speakerSettings, { [setting]: value });
      return;
    }
    case 'diagnostics':
      return { ...snapshot, audioInputFormat: 'Stereo PCM', sanitized: true };
    case 'export_diagnostics':
      return 'Preview only — no file written.';
    case 'reset_configuration':
      return snapshot;
    case 'test_volume':
    case 'use_tv_audio':
      return;
    default:
      throw new Error(`Unexpected preview command: ${command}`);
  }
});
await import('./main');

// Optional appearance overrides affect only this isolated preview document.
const appearance = new URLSearchParams(location.search).get('appearance');
if (appearance === 'light' || appearance === 'dark') {
  for (const sheet of document.styleSheets) {
    for (const rule of sheet.cssRules) {
      if (rule instanceof CSSMediaRule && rule.conditionText === '(prefers-color-scheme: dark)') {
        rule.media.mediaText = appearance === 'dark' ? 'all' : 'not all';
      }
    }
  }
  document.documentElement.style.colorScheme = appearance;
}
