import assert from 'node:assert/strict';
import test from 'node:test';
import { bindWindowFocus } from './window-appearance.ts';

test('initially inactive window gains and loses accent appearance with focus', async () => {
  let update;
  const states = [];
  await bindWindowFocus(
    {
      listen: async (handler) => {
        update = handler;
        return () => {};
      },
      current: async () => false,
    },
    (focused) => states.push(focused),
  );
  update(true);
  update(false);
  update(true);
  assert.deepEqual(states, [false, true, false, true]);
});

test('a delayed initial focus read cannot override a newer blur event', async () => {
  const states = [];
  let update;
  await bindWindowFocus(
    {
      listen: async (handler) => {
        update = handler;
        return () => {};
      },
      current: async () => {
        update(false);
        return true;
      },
    },
    (focused) => states.push(focused),
  );
  assert.deepEqual(states, [false]);
});
