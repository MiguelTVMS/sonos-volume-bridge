import assert from 'node:assert/strict';
import test from 'node:test';
import { cameraAutomationPresentation } from './camera-automation.ts';

test('unavailable automation remains possible to disable and keeps its explanation', () => {
  const status = { available: false, message: 'Unavailable in this platform build' };
  assert.equal(cameraAutomationPresentation(status, false, true).disabled, true);
  assert.equal(cameraAutomationPresentation(status, true, false).disabled, false);
  assert.equal(cameraAutomationPresentation(status, false, true).message, status.message);
});

test('speaker capability gates opt-in independently and recovery enables it', () => {
  const status = { available: true, message: 'Camera automation is off' };
  assert.equal(cameraAutomationPresentation(status, false, false).disabled, true);
  assert.equal(cameraAutomationPresentation(status, false, true).disabled, false);
});

test('cleanup warning remains visible alongside current automation status', () => {
  const presentation = cameraAutomationPresentation(
    {
      available: true,
      message: 'Waiting',
      warning: 'Previous speaker may remain enabled',
    },
    true,
    true,
  );
  assert.match(presentation.message, /Previous speaker may remain enabled/);
});
