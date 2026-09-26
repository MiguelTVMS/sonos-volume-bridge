import assert from 'node:assert/strict';
import test from 'node:test';
import { createCommandBridge } from './command-bridge.ts';

test('a normal build routes commands to native IPC and never loads the simulator', async () => {
  const calls = [];
  const native = async (command) => {
    calls.push(command);
    return command === 'ui_demo_enabled' ? false : 'native result';
  };
  const bridge = await createCommandBridge(native, () => assert.fail('demo must not load'));
  assert.equal(bridge.demo, false);
  assert.equal(await bridge.invoke('discover_sonos'), 'native result');
  assert.deepEqual(calls, ['ui_demo_enabled', 'discover_sonos']);
});

test('demo startup completes its handshake before any simulated command', async () => {
  let answer;
  const pending = new Promise((resolve) => {
    answer = resolve;
  });
  let loaded = false;
  const bridgePromise = createCommandBridge(
    async (command) => {
      assert.equal(command, 'ui_demo_enabled');
      return pending;
    },
    async () => {
      loaded = true;
      return async () => 'simulated';
    },
  );
  assert.equal(loaded, false);
  answer(true);
  const bridge = await bridgePromise;
  assert.equal(bridge.demo, true);
  assert.equal(await bridge.invoke('discover_sonos'), 'simulated');
});

test('a failed handshake cannot silently fall back to simulated devices', async () => {
  await assert.rejects(
    createCommandBridge(
      async () => {
        throw new Error('IPC unavailable');
      },
      () => assert.fail('demo must not load'),
    ),
    /IPC unavailable/,
  );
});
