import assert from 'node:assert/strict';
import test from 'node:test';
import { createCommandBridge } from './command-bridge.ts';

for (const demo of [false, true]) {
  test(`${demo ? 'demo' : 'normal'} desktop commands use native IPC`, async () => {
    const calls = [];
    const native = async (command) => {
      calls.push(command);
      return command === 'ui_demo_enabled' ? demo : 'native result';
    };
    const bridge = await createCommandBridge(native);
    assert.equal(bridge.demo, demo);
    for (const command of [
      'discover_sonos',
      'save_night_schedule',
      'enable_night_schedule',
      'set_speaker_setting',
    ]) {
      assert.equal(await bridge.invoke(command), 'native result');
    }
    assert.deepEqual(calls, [
      'ui_demo_enabled',
      'discover_sonos',
      'save_night_schedule',
      'enable_night_schedule',
      'set_speaker_setting',
    ]);
  });
}

test('desktop demo waits for startup then keeps normal native commands', async () => {
  let answer;
  const pending = new Promise((resolve) => {
    answer = resolve;
  });
  const calls = [];
  const bridgePromise = createCommandBridge(async (command) => {
    calls.push(command);
    return command === 'ui_demo_enabled' ? pending : 'native result';
  });
  assert.deepEqual(calls, ['ui_demo_enabled']);
  answer(true);
  const bridge = await bridgePromise;
  assert.equal(bridge.demo, true);
  assert.equal(await bridge.invoke('discover_sonos'), 'native result');
  assert.deepEqual(calls, ['ui_demo_enabled', 'discover_sonos']);
});

test('a failed handshake cannot silently fall back to simulated devices', async () => {
  await assert.rejects(
    createCommandBridge(async () => {
      throw new Error('IPC unavailable');
    }),
    /IPC unavailable/,
  );
});
