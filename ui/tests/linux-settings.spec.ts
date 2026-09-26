import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';

const linuxWindow = JSON.parse(
  readFileSync(new URL('../../src-tauri/tauri.linux.conf.json', import.meta.url), 'utf8'),
).app.windows[0];

test('Ubuntu Night schedule fits the default window after saving without scrolling', async ({
  page,
}) => {
  await page.setViewportSize({ width: linuxWindow.width, height: linuxWindow.height });
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
  await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
  await expect(page.locator('#notice')).toContainText('saved and applied');
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
  await expect(page.locator('#notice')).toHaveText('Volume control test requested.');
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
  await expect(page.locator('#notice')).toContainText('saved and applied');
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
