import test from 'node:test';
import assert from 'node:assert/strict';
import { UserWrites } from './user-writes.ts';
test('rapid user commits stay ordered and a failed write does not block the next one', async () => {
  const writes = new UserWrites();
  const sent = [];
  let release;
  const first = writes.run(() => {
    sent.push(2);
    return new Promise((resolve) => {
      release = resolve;
    });
  });
  const second = writes.run(async () => {
    sent.push(8);
  });
  await Promise.resolve();
  assert.deepEqual(sent, [2]);
  release();
  await Promise.all([first, second]);
  assert.deepEqual(sent, [2, 8]);
  await assert.rejects(
    writes.run(async () => {
      throw new Error('unavailable');
    }),
  );
  await writes.run(async () => {
    sent.push(4);
  });
  assert.deepEqual(sent, [2, 8, 4]);
});
