import { expect, test } from '@playwright/test';

for (const platform of ['windows', 'macos', 'linux']) {
  test(`${platform} saves silently but shows errors and clears them after retry`, async ({
    page,
  }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
    await page.locator('.schedule-cell').first().click();
    const beforeSave = await page.locator('#schedule-save').elementHandle();
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect.poll(() => beforeSave!.evaluate((element) => element.isConnected)).toBe(false);
    await expect(page.locator('#notice')).toBeEmpty();

    await page.evaluate(() => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      };
      const invoke = host.__TAURI_INTERNALS__.invoke;
      let fail = true;
      host.__TAURI_INTERNALS__.invoke = async (command, args) => {
        if (command === 'save_night_schedule' && fail) {
          fail = false;
          throw new Error('Could not save schedule.');
        }
        return invoke(command, args);
      };
    });
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect(page.locator('#notice')).toContainText('Could not save schedule.');
    await expect(page.locator('#notice')).toBeVisible();
    await expect(page.locator('#notice')).toHaveAttribute('aria-live', 'polite');
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect(page.locator('#notice')).toBeEmpty();

    await page.getByRole('button', { name: 'General', exact: true }).click();
    const beforeSetting = await page.locator('[name="startAtLogin"]').elementHandle();
    await page.locator('[name="startAtLogin"]').check();
    await expect.poll(() => beforeSetting!.evaluate((element) => element.isConnected)).toBe(false);
    await expect(page.locator('#notice')).toBeEmpty();
  });
}
