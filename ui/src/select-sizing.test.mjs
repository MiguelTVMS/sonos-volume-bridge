/* global EventTarget, Event */
import assert from 'node:assert/strict';
import test from 'node:test';
import { bindSelectedLabel } from './select-sizing.ts';

test('dropdown sizing follows the selected value initially and after changes', () => {
  const select = new EventTarget();
  select.selectedOptions = [{ label: 'Balanced' }];
  const label = { textContent: null };
  bindSelectedLabel(select, label);
  assert.equal(label.textContent, 'Balanced');
  select.selectedOptions = [{ label: 'Direct' }];
  select.dispatchEvent(new Event('change'));
  assert.equal(label.textContent, 'Direct');
  select.selectedOptions = [{ label: 'A much longer speaker name' }];
  select.dispatchEvent(new Event('change'));
  assert.equal(label.textContent, 'A much longer speaker name');
  select.selectedOptions = [];
  select.dispatchEvent(new Event('change'));
  assert.equal(label.textContent, '');
});
