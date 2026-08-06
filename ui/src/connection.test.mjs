import assert from 'node:assert/strict';
import test from 'node:test';
import { connectionLabel } from './connection.ts';

test('normal runtime activity remains connected', () => {
  for (const status of [
    'synchronized',
    'waitingForSonosConfirmation',
    'subscriptionDegraded',
    'pollingFallback',
  ]) {
    assert.equal(connectionLabel(status), 'Connected');
  }
});

test('setup and unavailable runtime states are disconnected', () => {
  for (const status of [
    'discovering',
    'connecting',
    'sonosUnavailable',
    'localAudioUnavailable',
    'unsupportedLocalDevice',
    'configurationRequired',
    'error',
    'unknown',
  ]) {
    assert.equal(connectionLabel(status), 'Disconnected');
  }
});
