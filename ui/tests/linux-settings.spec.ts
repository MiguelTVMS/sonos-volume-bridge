import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';

const linuxWindow = JSON.parse(
  readFileSync(new URL('../../src-tauri/tauri.linux.conf.json', import.meta.url), 'utf8'),
).app.windows[0];

test('Ubuntu schedule status stays in one row below notifications through save and navigation', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  const row = page.locator('.schedule-status-row');
  await expect(row).toBeVisible();
  await expect(row.locator(':scope > span')).toHaveText('Status');
  const labelFont = await page
    .locator('label[for="schedule-notifications"]')
    .evaluate((label) => getComputedStyle(label).font);
  await expect(row.locator(':scope > span')).toHaveCSS('font', labelFont);
  await expect(row.locator('#schedule-status')).toHaveText('Schedule disabled.');
  const notifications = (await page.locator('#schedule-notifications').boundingBox())!;
  const status = (await row.locator('#schedule-status').boundingBox())!;
  expect(Math.abs(status.x + status.width - (notifications.x + notifications.width))).toBeLessThan(
    2,
  );
  await expect(row.locator('#schedule-status')).toHaveCSS('text-align', 'right');
  await expect(row.locator('..').locator(':scope > :nth-child(4)')).toHaveClass(
    'schedule-status-row',
  );
  await page.locator('#schedule-enabled').check();
  await expect(row).toContainText('Outside scheduled hours. Manual control is available.');
  await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
  await expect(row.locator('#schedule-feedback')).toBeEmpty();
  await expect(page.locator('#notice')).toBeEmpty();
  await expect(page.locator('#night-schedule > #schedule-status')).toHaveCount(0);
  await page.getByRole('button', { name: 'Devices', exact: true }).click();
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await expect(row).toContainText('Outside scheduled hours. Manual control is available.');
  await expect(
    page.getByText('Schedule saved and applied to the selected speaker.', { exact: true }),
  ).toHaveCount(0);
});

test('Ubuntu schedule shows save errors and clears them on a successful retry', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    let fail = true;
    host.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === 'save_night_schedule' && fail) {
        fail = false;
        return Promise.reject(new Error('Could not apply schedule.'));
      }
      return invoke(command, args);
    };
  });
  const save = page.getByRole('button', { name: 'Save schedule', exact: true });
  await save.click();
  await expect(page.locator('.schedule-status-row #schedule-feedback')).toContainText(
    'Could not apply schedule.',
  );
  await expect(page.locator('#notice')).toBeEmpty();
  const previous = await save.elementHandle();
  await save.click();
  await expect.poll(() => previous!.evaluate((element) => element.isConnected)).toBe(false);
  await expect(page.locator('#schedule-feedback')).toBeEmpty();
  await expect(page.locator('#notice')).toBeEmpty();
});

test('Ubuntu Night schedule fits the default window after saving without scrolling', async ({
  page,
}) => {
  await page.setViewportSize({ width: linuxWindow.width, height: linuxWindow.height });
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
  await expect(page.locator('#schedule-feedback')).toBeEmpty();
  for (const selector of ['.content', '.schedule-scroll']) {
    const size = await page.locator(selector).evaluate((element) => ({
      height: element.clientHeight,
      content: element.scrollHeight,
    }));
    expect(size.content, `${selector} needs no vertical scrolling`).toBeLessThanOrEqual(
      size.height,
    );
  }
});

const sections = [
  'Devices',
  'Speaker',
  'Night schedule',
  'Volume',
  'General',
  'Diagnostics',
  'About',
];

test('Ubuntu volume test action is inset inside its card and works by keyboard', async ({
  page,
}) => {
  await page.setViewportSize({ width: linuxWindow.width, height: linuxWindow.height });
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Volume', exact: true }).click();
  const button = page.getByRole('button', { name: 'Test speaker volume', exact: true });
  const inset = await button.evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    const card = element.closest('.settings-group')!.getBoundingClientRect();
    return { left: bounds.left - card.left, bottom: card.bottom - bounds.bottom };
  });
  expect(inset.left).toBeGreaterThanOrEqual(16);
  expect(inset.bottom).toBeGreaterThanOrEqual(16);
  await expect(button).toHaveCSS('height', '34px');
  await button.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#notice')).toBeEmpty();
});

test('Ubuntu schedule keeps square cells through navigation, selection and save', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  const cell = page.locator('.schedule-cell').first();
  await expect(cell).toHaveCSS('border-radius', '0px');
  await cell.focus();
  await page.keyboard.press('Space');
  await expect(cell).toHaveAttribute('aria-selected', 'true');
  await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
  await expect(page.locator('#schedule-feedback')).toBeEmpty();
  await page.getByRole('button', { name: 'Devices', exact: true }).click();
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await expect(cell).toHaveCSS('border-radius', '0px');
  await expect(cell).toHaveAttribute('aria-selected', 'true');
});

for (const colorScheme of ['light', 'dark'] as const) {
  for (const viewport of [
    { width: linuxWindow.width, height: linuxWindow.height },
    { width: linuxWindow.width, height: linuxWindow.minHeight },
  ]) {
    test(`Ubuntu controls fit ${viewport.width}x${viewport.height} in ${colorScheme}`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await page.emulateMedia({ colorScheme });
      await page.goto('/preview.html?platform=linux');
      const mute = page.getByRole('switch', { name: 'Synchronize mute', exact: true });
      await expect(mute).toHaveCSS('width', '48px');
      await expect(mute).toHaveCSS('height', '26px');
      await mute.press('Space');
      await expect(mute).not.toBeChecked();
      for (const name of sections) {
        await page.getByRole('button', { name, exact: true }).click();
        await expect(page.getByRole('heading', { name, level: 2, exact: true })).toBeVisible();
        const overflow = await page.locator('.panel:visible').evaluate((panel) =>
          Array.from(panel.querySelectorAll('button, select, input, dd')).some((element) => {
            const rect = element.getBoundingClientRect();
            return rect.width > 0 && (rect.left < 0 || rect.right > innerWidth + 1);
          }),
        );
        expect(overflow, `${name} fits horizontally`).toBe(false);
        if (['Night schedule', 'Volume'].includes(name) && viewport.height === linuxWindow.height) {
          await page.screenshot({
            path: test.info().outputPath(`ubuntu-${name}-${colorScheme}.png`),
          });
        }
      }
      await page.getByRole('button', { name: 'Volume', exact: true }).click();
      const slider = page.getByRole('slider');
      await slider.press('ArrowRight');
      await expect(slider).toHaveValue('71');
      await expect(page.locator('#maximum-value')).toHaveText('71%');
      await expect(slider).toHaveCSS('--range-progress', '71%');
      await page.getByRole('button', { name: 'Devices', exact: true }).click();
      await expect(mute).not.toBeChecked();
    });
  }
}
