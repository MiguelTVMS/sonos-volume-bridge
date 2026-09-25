import assert from 'node:assert/strict';
import test from 'node:test';
import { adjacentPage, settingsPages } from './settings-navigation.ts';

test('arrows traverse all settings in sidebar order without wrapping', () => {
  assert.equal(adjacentPage('devices', -1), undefined);
  assert.equal(adjacentPage('about', 1), undefined);
  for (let index = 0; index < settingsPages.length - 1; index++) {
    assert.equal(adjacentPage(settingsPages[index], 1), settingsPages[index + 1]);
    assert.equal(adjacentPage(settingsPages[index + 1], -1), settingsPages[index]);
  }
});
