import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
test.use({ locale: 'en-GB' });

test('Night schedule fits the default macOS window without vertical scrolling', async ({
  page,
}) => {
  const config = JSON.parse(
    readFileSync(new URL('../../src-tauri/tauri.macos.conf.json', import.meta.url), 'utf8'),
  );
  const { width, height, minWidth, maxWidth } = config.app.windows[0];
  expect(minWidth).toBe(width);
  expect(maxWidth).toBe(width);
  await page.setViewportSize({ width, height });
  await page.goto('/preview.html?platform=macos');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
  await expect(page.locator('#notice')).toContainText('saved and applied');
  const dimensions = await page.locator('.content').evaluate((element) => ({
    content: element.scrollHeight,
    viewport: element.clientHeight,
  }));
  expect(dimensions.content).toBeLessThanOrEqual(dimensions.viewport);
});

test('macOS notification dropdown fits the saved option after rerendering', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 650 });
  await page.goto('/preview.html?platform=macos');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  for (const mode of ['start', 'end', 'both', 'never']) {
    const previous = await page.locator('#schedule-notifications').elementHandle();
    await page.locator('#schedule-notifications').selectOption(mode);
    await expect.poll(() => previous!.evaluate((element) => element.isConnected)).toBe(false);
    const select = page.locator('#schedule-notifications');
    await expect(select).toHaveValue(mode);
    const label = select.locator('..').locator('.selected-control-label');
    await expect(label).toHaveText(await select.locator('option:checked').innerText());
    const dimensions = await select.evaluate((element) => {
      const style = getComputedStyle(element);
      const text = document.createElement('span');
      text.style.font = style.font;
      text.style.whiteSpace = 'nowrap';
      text.textContent = element.selectedOptions[0].label;
      document.body.append(text);
      const required = text.getBoundingClientRect().width;
      text.remove();
      return {
        available:
          element.getBoundingClientRect().width -
          parseFloat(style.paddingLeft) -
          parseFloat(style.paddingRight),
        required,
      };
    });
    expect(dimensions.available, mode).toBeGreaterThanOrEqual(dimensions.required);
  }
});

for (const appearance of ['light', 'dark'] as const) {
  test(`Windows schedule tooltip is opaque in ${appearance} mode`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: appearance });
    await page.goto('/preview.html?platform=windows');
    await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
    await page.locator('[data-day="3"][data-slot="24"]').hover();
    const tooltip = page.getByRole('tooltip');
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toHaveText('Thu 12:00–12:30');
    await expect(tooltip).toHaveCSS(
      'background-color',
      appearance === 'light' ? 'rgb(255, 255, 255)' : 'rgb(53, 53, 53)',
    );
    await expect(tooltip).toHaveCSS('opacity', '1');
    await page.getByRole('heading', { name: 'Night schedule', exact: true }).hover();
    await expect(tooltip).toBeHidden();
  });
}

test('global half-hour grid supports painting, saving, keyboard editing and notifications', async ({
  page,
}) => {
  await page.setViewportSize({ width: 600, height: 650 });
  await page.goto('/preview.html?platform=macos');
  await page.getByRole('button', { name: 'Speaker', exact: true }).click();
  await expect(page.locator('#night-schedule')).toBeHidden();
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await expect(page.locator('#schedule-target')).toHaveCount(0);
  await expect(page.locator('#schedule-status')).not.toContainText('Europe/Lisbon');
  await expect(page.getByRole('heading', { name: 'Night schedule', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Night Mode schedule', exact: true })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole('combobox', { name: 'Night schedule notifications', exact: true }),
  ).toHaveValue('never');
  await expect(page.locator('#schedule-notifications option')).toHaveText([
    'On start',
    'On end',
    'On start and end',
    'Never',
  ]);
  await expect(page.locator('#schedule-status')).toBeEmpty();
  const cells = page.locator('.schedule-cell');
  await expect(cells).toHaveCount(336);
  await expect(cells.first()).toHaveAttribute('aria-label', 'Mon 00:00–00:30');
  await expect(cells.last()).toHaveAttribute('aria-label', 'Sun 23:30–24:00');
  await expect(page.locator('.schedule-hours [role="columnheader"]')).toHaveText([
    '00',
    '03',
    '06',
    '09',
    '12',
    '15',
    '18',
    '21',
  ]);
  await expect(cells.first()).toHaveAttribute('data-tooltip', 'Mon 00:00–00:30');
  await expect(cells.last()).toHaveAttribute('data-tooltip', 'Sun 23:30–24:00');
  expect((await cells.first().boundingBox())!.height).toBeLessThanOrEqual(18);
  await expect(page.locator('.schedule-row [role="rowheader"]')).toHaveText([
    'Mon',
    'Tue',
    'Wed',
    'Thu',
    'Fri',
    'Sat',
    'Sun',
  ]);
  await cells.first().hover();
  await expect(page.getByRole('tooltip')).toBeVisible({ timeout: 1000 });
  await expect(page.getByRole('tooltip')).toHaveText('Mon 00:00–00:30');
  const first = cells.nth(0);
  const second = cells.nth(1);
  await first.click();
  await expect(first).toHaveAttribute('aria-selected', 'true');
  await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
  await expect(first).toHaveAttribute('aria-selected', 'true');
  await first.focus();
  await page.keyboard.press('ArrowDown');
  await expect(cells.nth(48)).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Space');
  await expect(second).toHaveAttribute('aria-selected', 'true');
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await expect(second).toHaveAttribute('aria-selected', 'true');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(second).toHaveAttribute('aria-selected', 'false');
  await expect(first).toHaveAttribute('aria-selected', 'true');
  await page.locator('#schedule-enabled').check();
  await expect(page.locator('#schedule-enabled')).toBeChecked();
  await expect(page.locator('#schedule-status')).toBeEmpty();
  await expect(page.locator('#toolbar-section-title')).toHaveText('Night schedule');
  for (const mode of ['start', 'end', 'never', 'both']) {
    await page.locator('#schedule-notifications').selectOption(mode);
    await expect(page.locator('#schedule-notifications')).toHaveValue(mode);
  }
  await expect(page.locator('#schedule-notifications')).toHaveValue('both');
  await page.locator('#night-schedule').screenshot({ path: '/tmp/sonos-night-schedule.png' });
});

test('diagonal drag selects a rectangle and reversing shrinks without trails', async ({ page }) => {
  await page.goto('/preview.html?platform=macos');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  const cell = (day: number, slot: number) =>
    page.locator(`[data-day="${day}"][data-slot="${slot}"]`);
  const move = async (day: number, slot: number) => {
    const rect = (await cell(day, slot).boundingBox())!;
    await page.mouse.move(rect.x + rect.width / 2, rect.y + rect.height / 2, { steps: 10 });
  };
  await cell(0, 0).scrollIntoViewIfNeeded();
  await move(0, 0);
  await page.mouse.down();
  await move(3, 6);
  await expect(page.locator('.schedule-cell[aria-selected="true"]')).toHaveCount(28);
  await expect(cell(0, 6)).toHaveAttribute('aria-selected', 'true');
  await expect(cell(3, 0)).toHaveAttribute('aria-selected', 'true');
  await move(1, 2);
  await page.mouse.up();
  await expect(page.locator('.schedule-cell[aria-selected="true"]')).toHaveCount(6);
  await expect(cell(3, 6)).toHaveAttribute('aria-selected', 'false');
  await move(1, 2);
  await page.mouse.down();
  await move(0, 0);
  await page.mouse.up();
  await expect(page.locator('.schedule-cell[aria-selected="true"]')).toHaveCount(0);
});

test('Save stays available for repeatable application even without grid edits', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=macos');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  const save = page.getByRole('button', { name: 'Save schedule', exact: true });
  await expect(save).toBeEnabled();
  await save.click();
  await expect(page.locator('#notice')).toContainText('saved and applied');
  await expect(save).toBeEnabled();
  await save.click();
  await expect(save).toBeEnabled();
  await expect(page.locator('#schedule-enabled')).not.toBeChecked();
});

test('native 24-hour clock overrides an American WebView locale', async ({ browser }) => {
  const context = await browser.newContext({ locale: 'en-US' });
  const page = await context.newPage();
  await page.goto('/preview.html?platform=macos&hour12=false');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await expect(page.locator('[data-day="0"][data-slot="26"]')).toHaveAttribute(
    'aria-label',
    'Mon 13:00–13:30',
  );
  await expect(page.locator('.schedule-editor.settings-group')).toBeVisible();
  await context.close();
});

for (const platform of ['windows', 'linux']) {
  test(`${platform} schedule uses rectangle selection and notification modes`, async ({ page }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
    await expect(page.locator('.schedule-cell')).toHaveCount(336);
    await page.locator('#schedule-notifications').selectOption('start');
    await expect(page.locator('#schedule-notifications')).toHaveValue('start');
    const first = page.locator('[data-day="0"][data-slot="0"]');
    const last = page.locator('[data-day="2"][data-slot="4"]');
    const a = (await first.boundingBox())!;
    const b = (await last.boundingBox())!;
    await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
    await page.mouse.down();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 5 });
    await page.mouse.up();
    await expect(page.locator('.schedule-cell[aria-selected="true"]')).toHaveCount(15);
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect(page.locator('.schedule-cell[aria-selected="true"]')).toHaveCount(15);
  });
}
