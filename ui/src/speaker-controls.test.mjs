import test from 'node:test';
import assert from 'node:assert/strict';
import { applySpeakerControls } from './speaker-controls.ts';

function view() {
  const names = ['loudness', 'nightSound', 'speechEnhancement', 'statusLight', 'treble', 'bass'];
  const inputs = names.map((name) => ({
    dataset:
      name === 'treble' || name === 'bass' ? { speakerLevel: name } : { speakerSetting: name },
    disabled: false,
    checked: true,
    value: '5',
    title: '',
    closest: () => null,
  }));
  const labels = Object.fromEntries(names.map((name) => [name, { textContent: '' }]));
  const scope = {
    querySelectorAll: () => inputs,
    querySelector: (selector) => labels[selector.match(/"(.*?)"/)[1]],
  };
  return { inputs, labels, scope };
}
const empty = {
  loudness: null,
  nightSound: null,
  speechEnhancement: null,
  statusLight: null,
  treble: null,
  bass: null,
};

test('startup, unavailable refresh, recovery and speaker switch use the same control update', () => {
  const { scope, inputs, labels } = view();
  applySpeakerControls(scope, empty, null);
  assert.ok(inputs.every((input) => input.disabled));
  applySpeakerControls(scope, { ...empty, loudness: false, nightSound: true }, null);
  assert.equal(inputs[0].disabled, false);
  assert.equal(inputs[0].checked, false); // Supported does not mean enabled.
  assert.equal(inputs[1].checked, true);
  applySpeakerControls(scope, { ...empty, capabilities: { nightSound: 'unsupported' } }, null);
  assert.equal(labels.loudness.textContent, 'Temporarily unavailable');
  assert.equal(labels.nightSound.textContent, 'Not supported by this speaker');
  assert.equal(inputs[1].disabled, true);
  applySpeakerControls(
    scope,
    { ...empty, nightSound: false, capabilities: { nightSound: 'supported' } },
    null,
  );
  assert.equal(inputs[1].disabled, false);
  assert.equal(inputs[1].checked, false);
  assert.equal(labels.nightSound.textContent, '');
  applySpeakerControls(scope, empty, null); // New unresolved speaker has no inherited values.
  assert.ok(
    inputs.every((input) => input.disabled && (!input.dataset.speakerSetting || !input.checked)),
  );
});

test('a background speaker update preserves the tone control being edited', () => {
  const { scope, inputs } = view();
  applySpeakerControls(scope, { ...empty, treble: 1, bass: 2 }, inputs[4]);
  assert.equal(inputs[4].value, '5');
  assert.equal(inputs[5].value, '2');
});
