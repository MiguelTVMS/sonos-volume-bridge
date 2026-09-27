import test from 'node:test';
import assert from 'node:assert/strict';
import { ScheduleDraft, emptyBlocks, timeLabel, setSystemHour12 } from './night-schedule.ts';

test('painting uses the first cell state and never toggles revisited cells', () => {
  const draft = new ScheduleDraft();
  draft.paint(0, 0);
  draft.paint(0, 1);
  draft.paint(0, 0);
  draft.paint(1, 0);
  draft.end();
  assert.equal(draft.blocks[0][0], true);
  assert.equal(draft.blocks[1][0], true);
  draft.paint(0, 0);
  draft.paint(0, 1);
  draft.paint(0, 0);
  draft.end();
  assert.equal(draft.blocks[0][0], false);
  assert.equal(draft.blocks[0][1], false);
});
test('global drafts survive refresh and selection changes until cancel or save', () => {
  const draft = new ScheduleDraft();
  draft.paint(6, 47);
  draft.end();
  draft.refresh(emptyBlocks());
  assert.equal(draft.blocks[6][47], true);
  draft.reset(emptyBlocks());
  assert.equal(draft.dirty, false);
  assert.equal(draft.blocks[6][47], false);
  draft.clear();
  assert.equal(draft.dirty, true);
});
test('half-hour labels include the final midnight boundary', () => {
  assert.equal(timeLabel(0, 'en-GB'), '00:00');
  assert.equal(timeLabel(47, 'en-GB'), '23:30');
  assert.equal(timeLabel(48, 'en-GB'), '24:00');
});

test('half-hour edits preserve the neighboring half and existing selections', () => {
  const saved = emptyBlocks();
  saved[0][3] = true;
  const draft = new ScheduleDraft();
  draft.refresh(saved);
  assert.deepEqual(draft.blocks[0].slice(2, 4), [false, true]);
  draft.paint(0, 2);
  draft.end();
  assert.deepEqual(draft.blocks[0].slice(2, 4), [true, true]);
  draft.paint(0, 3);
  draft.end();
  assert.deepEqual(draft.blocks[0].slice(2, 4), [true, false]);
});

test('schedule times use locale hour cycles in tooltips and axis labels', () => {
  assert.equal(timeLabel(26, 'en-US'), '01:00 PM');
  assert.equal(timeLabel(26, 'pt-PT'), '13:00');
  assert.equal(timeLabel(12, 'en-US', true), '6 AM');
  assert.equal(timeLabel(48, 'en-US'), '12:00 AM');
});

test('machine clock override wins over language defaults', () => {
  try {
    setSystemHour12(false);
    assert.equal(timeLabel(26, 'en-US'), '13:00');
    assert.equal(timeLabel(36, 'en-US', true), '18');
    setSystemHour12(true);
    assert.match(timeLabel(26, 'pt-PT'), /^01:00/);
  } finally {
    setSystemHour12(null);
  }
});

test('rectangle drag shrinks without trails and restores preexisting cells', () => {
  const draft = new ScheduleDraft();
  draft.blocks[1][2] = true;
  draft.paint(0, 0);
  draft.paint(3, 4);
  assert.equal(draft.blocks.flat().filter(Boolean).length, 20);
  draft.paint(1, 1);
  assert.equal(draft.blocks.flat().filter(Boolean).length, 5);
  assert.equal(draft.blocks[1][2], true);
  assert.equal(draft.blocks[3][4], false);
  draft.end();
  draft.paint(1, 2);
  draft.paint(0, 0);
  assert.equal(draft.blocks.flat().filter(Boolean).length, 0);
  draft.end();
});
