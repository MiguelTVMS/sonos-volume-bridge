import assert from 'node:assert/strict';
import test from 'node:test';
import { diagnosticsDisclosureState } from './diagnostics.ts';

test('expanding speaker details requests a refresh', () => {
  assert.deepEqual(diagnosticsDisclosureState(true), {
    visible: true,
    shouldRefresh: true,
  });
});

test('collapsing speaker details does not request a refresh', () => {
  assert.deepEqual(diagnosticsDisclosureState(false), {
    visible: false,
    shouldRefresh: false,
  });
});
