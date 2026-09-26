// Shared in-memory device simulator for browser previews and opt-in desktop UI demos.
// No native commands, device discovery, files, or audio APIs are used here.
export function createDemoBackend(options: { hour12?: boolean | null; now?: () => Date } = {}) {
  const snapshot = {
    configuration: {
      schemaVersion: 1,
      nightModeSchedule: {
        enabled: false,
        blocks: Array.from({ length: 7 }, () => Array<boolean>(48).fill(false)),
      },
      notifyNightModeScheduleTransitions: 'never',
      selectedSonosId: 'sample-speaker',
      lastKnownSonosAddress: null,
      followDefaultAudioDevice: true,
      fixedAudioDeviceId: null,
      synchronizeMute: true,
      muteSpeakerAtZeroVolume: false,
      twoWaySynchronization: true,
      startAtLogin: false,
      fallbackPolling: true,
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
  const initialSnapshot = structuredClone(snapshot);
  const initialSpeakerSettings = structuredClone(speakerSettings);
  const dispatch = (command: string, payload?: Record<string, unknown>): unknown => {
    switch (command) {
      case 'plugin:app|version':
        return 'Preview';
      case 'get_schedule_status':
        return {
          active: false,
          supported: true,
          message: snapshot.configuration.nightModeSchedule.enabled
            ? 'Outside scheduled hours. Manual control is available.'
            : 'Schedule disabled.',
          nextTransition: null,
          timeZone: 'Europe/Lisbon',
          notificationsBlocked: false,
        };
      case 'save_night_schedule': {
        snapshot.configuration.nightModeSchedule.blocks = (
          payload as { blocks: boolean[][] }
        ).blocks;
        const now = options.now?.() ?? new Date();
        speakerSettings.nightSound =
          snapshot.configuration.nightModeSchedule.blocks[(now.getDay() + 6) % 7][
            now.getHours() * 2 + Math.floor(now.getMinutes() / 30)
          ];
        return snapshot;
      }
      case 'enable_night_schedule':
        snapshot.configuration.nightModeSchedule.enabled = (
          payload as { enabled: boolean }
        ).enabled;
        return snapshot;
      case 'set_schedule_notifications':
        snapshot.configuration.notifyNightModeScheduleTransitions = (
          payload as { mode: string }
        ).mode;
        return snapshot;
      case 'get_system_hour12':
        return options.hour12 ?? null;
      case 'get_snapshot':
        return snapshot;
      case 'save_configuration':
        Object.assign(snapshot.configuration, (payload as { configuration: object }).configuration);
        snapshot.status = snapshot.configuration.selectedSonosId
          ? 'synchronized'
          : 'configuration_required';
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
        return {
          ...snapshot,
          configurationPresent: true,
          speakerName: snapshot.sonosName,
          message: 'Simulated devices — no hardware connected.',
          audioInputFormat: 'Stereo PCM (simulated)',
          sanitized: true,
        };
      case 'export_diagnostics':
        return 'Preview only — no file written.';
      case 'reset_configuration':
        Object.assign(snapshot, structuredClone(initialSnapshot));
        Object.assign(speakerSettings, initialSpeakerSettings);
        return snapshot;
      case 'test_volume':
        snapshot.localVolume = Math.min(snapshot.configuration.maximumSonosVolume, 30);
        snapshot.sonosVolume = snapshot.localVolume;
        return;
      case 'use_tv_audio':
        return;
      default:
        throw new Error(`Unexpected preview command: ${command}`);
    }
  };
  return (command: string, payload?: Record<string, unknown>): unknown =>
    structuredClone(dispatch(command, structuredClone(payload)));
}
