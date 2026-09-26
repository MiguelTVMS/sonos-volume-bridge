import test from 'node:test';
import assert from 'node:assert/strict';
import { LiveStatus } from './live-status.ts';
test('volume push updates immediately and delayed polling cannot move it back', async () => {
  const displayed = [];
  const status = new LiveStatus((value) => displayed.push(value));
  let resolve;
  const pending = status.read(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  status.push({ volume: 42 });
  assert.deepEqual(displayed, [{ volume: 42 }]);
  resolve({ volume: 20 });
  await pending;
  assert.deepEqual(displayed, [{ volume: 42 }]);
  await status.read(async () => ({ volume: 43 }));
  assert.deepEqual(displayed, [{ volume: 42 }, { volume: 43 }]);
});
