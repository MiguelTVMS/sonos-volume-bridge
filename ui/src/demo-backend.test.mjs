import assert from 'node:assert/strict';
import test from 'node:test';
import { createDemoBackend } from './demo-backend.ts';

test('demo devices and writable sound settings work without native commands', () => {
  const demo = createDemoBackend();
  assert.equal(demo('discover_sonos').length, 1);
  assert.equal(demo('list_audio_outputs')[0].writableVolume, true);
  demo('set_speaker_setting', { setting: 'speechEnhancement', enabled: true });
  demo('set_speaker_level', { setting: 'bass', value: -4 });
  assert.equal(demo('get_speaker_settings').speechEnhancement, true);
  assert.equal(demo('get_speaker_settings').bass, -4);
  assert.throws(() => demo('unknown_command'), /Unexpected preview command/);
});

test('demo configuration persists within the session, resets, and returns isolated snapshots', () => {
  const demo = createDemoBackend();
  const snapshot = demo('get_snapshot');
  snapshot.configuration.maximumSonosVolume = 42;
  assert.equal(demo('get_snapshot').configuration.maximumSonosVolume, 70);
  demo('save_configuration', { configuration: snapshot.configuration });
  assert.equal(demo('get_snapshot').configuration.maximumSonosVolume, 42);
  demo('set_speaker_level', { setting: 'bass', value: 4 });
  demo('reset_configuration');
  assert.equal(demo('get_snapshot').configuration.maximumSonosVolume, 70);
  assert.equal(demo('get_speaker_settings').bass, 0);
  assert.equal(createDemoBackend()('get_snapshot').configuration.maximumSonosVolume, 70);
});

test('saving a schedule applies the simulated current block and accepts all notification choices', () => {
  const demo = createDemoBackend({ now: () => new Date(2026, 8, 28, 12, 15) });
  const blocks = Array.from({ length: 7 }, () => Array(48).fill(false));
  blocks[0][24] = true;
  demo('save_night_schedule', { blocks });
  assert.equal(demo('get_speaker_settings').nightSound, true);
  for (const mode of ['start', 'end', 'both', 'never']) {
    assert.equal(
      demo('set_schedule_notifications', { mode }).configuration.notifyNightModeScheduleTransitions,
      mode,
    );
  }
  blocks[0][24] = false;
  demo('save_night_schedule', { blocks });
  assert.equal(demo('get_speaker_settings').nightSound, false);
});
