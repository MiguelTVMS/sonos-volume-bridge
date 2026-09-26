import assert from 'node:assert/strict';
import test from 'node:test';
import { settingCaption } from './setting-caption.ts';

test('Windows captions keep text safe and icons out of the accessibility tree', () => {
  const caption = settingCaption('windows', '<Speaker>', 'A & B "output"', 'speaker');
  assert.ok(caption.includes('&lt;Speaker&gt;'));
  assert.ok(caption.includes('A &amp; B &quot;output&quot;'));
  assert.ok(caption.includes('aria-hidden="true"'));
  assert.ok(!caption.includes('<Speaker>'));
  assert.ok(!caption.includes('<input'), 'caption does not duplicate interactive controls');
});

test('Windows descriptions and icons do not leak into other platform presentations', () => {
  for (const platform of ['macos', 'linux', 'generic']) {
    assert.equal(settingCaption(platform, 'Volume', 'Description', 'sound'), 'Volume');
  }
});
