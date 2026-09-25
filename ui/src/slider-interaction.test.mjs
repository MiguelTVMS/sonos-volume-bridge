import test from 'node:test';
import assert from 'node:assert/strict';
import { SliderInteraction } from './slider-interaction.ts';
class Slider extends globalThis.EventTarget {
  value = '0';
  setPointerCapture() {}
  send(type, value, key) {
    if (value !== undefined) this.value = value;
    const event = new globalThis.Event(type);
    Object.assign(event, { pointerId: 1, key });
    this.dispatchEvent(event);
  }
}
test('dragging, pausing and releasing commits only the final slider value once', () => {
  const interaction = new SliderInteraction();
  const slider = new Slider();
  const writes = [];
  let revision = 0;
  let preview;
  interaction.bind(
    slider,
    () => revision++,
    () => (preview = slider.value),
    () => writes.push(slider.value),
  );
  slider.send('pointerdown');
  slider.send('input', '2');
  slider.send('input', '8');
  slider.send('change'); // WebViews can send change before pointerup.
  assert.equal(interaction.active, true);
  assert.deepEqual(writes, []);
  assert.equal(preview, '8');
  assert.ok(revision > 0);
  slider.send('pointerup');
  slider.send('change');
  slider.send('lostpointercapture');
  assert.equal(interaction.active, false);
  assert.deepEqual(writes, ['8']);
  slider.send('pointerdown'); // A new drag blocks an older save's refresh.
  assert.equal(interaction.active, true);
  slider.send('pointercancel');
  assert.equal(interaction.active, false);
});
test('keyboard repeats preview immediately and commit on release; accessibility change still works', () => {
  const interaction = new SliderInteraction();
  const slider = new Slider();
  const writes = [];
  interaction.bind(
    slider,
    () => {},
    () => {},
    () => writes.push(slider.value),
  );
  for (const value of ['1', '2', '3']) {
    slider.send('keydown', undefined, 'ArrowRight');
    slider.send('input', value);
    slider.send('change');
  }
  assert.deepEqual(writes, []);
  slider.send('keyup');
  assert.deepEqual(writes, ['3']);
  slider.send('input', '4');
  slider.send('change');
  assert.deepEqual(writes, ['3', '4']);
});

test('device readback updates the display without initiating another write', async () => {
  const { applySpeakerControls } = await import('./speaker-controls.ts');
  const input = new Slider();
  input.dataset = { speakerLevel: 'bass' };
  input.closest = () => null;
  const interaction = new SliderInteraction();
  const writes = [];
  interaction.bind(
    input,
    () => {},
    () => {},
    () => writes.push(input.value),
  );
  const scope = { querySelectorAll: () => [input], querySelector: () => null };
  input.send('pointerdown');
  input.send('input', '4');
  input.send('pointerup');
  assert.deepEqual(writes, ['4']);
  const settings = { bass: 4, capabilities: { bass: 'supported' } };
  applySpeakerControls(scope, settings, null); // Our command's echoed value.
  applySpeakerControls(scope, { ...settings, bass: 7 }, null); // Another controller.
  assert.equal(input.value, '7');
  assert.deepEqual(writes, ['4']); // Neither source creates a feedback write.
});
