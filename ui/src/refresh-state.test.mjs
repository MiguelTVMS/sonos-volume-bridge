import assert from 'node:assert/strict';
import test from 'node:test';
import { canApplyRefresh } from './refresh-state.ts';

test('background refresh never overwrites edits, pending writes, or newer reads', () => {
  assert.equal(canApplyRefresh(1, 1, 0, 0, false), true);
  assert.equal(canApplyRefresh(1, 2, 0, 0, false), false);
  assert.equal(canApplyRefresh(1, 1, 0, 1, false), false);
  assert.equal(canApplyRefresh(1, 1, 0, 0, true), false);
});
