import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { URL } from 'node:url';
import test from 'node:test';

const config = (file) =>
  JSON.parse(readFileSync(new URL(`../../src-tauri/${file}`, import.meta.url), 'utf8'));

test('platform windows preserve the settings window contract and usable resize bounds', () => {
  const base = config('tauri.conf.json').app.windows[0];
  for (const file of [
    'tauri.macos.conf.json',
    'tauri.windows.conf.json',
    'tauri.linux.conf.json',
  ]) {
    const window = config(file).app.windows[0];
    assert.equal(window.label, base.label, `${file}: commands and capabilities target main`);
    assert.equal(window.visible, false, `${file}: startup stays in the tray/menu bar`);
    assert.equal(window.resizable, true);
    assert.ok(window.minWidth <= window.width, `${file}: initial width fits resize bounds`);
    assert.ok(window.minHeight <= window.height, `${file}: initial height fits resize bounds`);
    assert.ok(window.maxWidth == null || window.width <= window.maxWidth);
  }
});

test('Ubuntu keeps a fixed width with native chrome and vertical resizing', () => {
  const window = config('tauri.linux.conf.json').app.windows[0];
  assert.equal(window.width, 1080);
  assert.equal(window.minWidth, window.width);
  assert.equal(window.maxWidth, window.width);
  assert.equal(window.height, 800);
  assert.ok(window.minHeight < window.height);
  assert.notEqual(window.decorations, false);
});

test('Windows supports horizontal resizing and keeps native window chrome', () => {
  const window = config('tauri.windows.conf.json').app.windows[0];
  assert.ok(window.minWidth < window.width, 'compact layout must be reachable');
  assert.ok(window.maxWidth == null, 'Windows Settings layout can expand with the window');
  assert.notEqual(window.decorations, false);
  assert.notEqual(window.titleBarStyle, 'Overlay');
});
