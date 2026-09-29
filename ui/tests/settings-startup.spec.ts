import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} Settings opens and saves without a legacy-app service`, async ({ page }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    await expect(page.getByRole('button', { name: 'Devices', exact: true })).toBeVisible();
    for (const name of ['Speaker', 'Night schedule', 'General', 'Diagnostics', 'About']) {
      await page.getByRole('button', { name, exact: true }).click();
      await expect(page.getByRole('heading', { name, level: 2, exact: true })).toBeVisible();
      await expect(page.locator('#legacy-alert')).toHaveCount(0);
      await expect(page.locator('#diagnostic-legacy')).toHaveCount(0);
    }
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await page.locator('[name="startAtLogin"]').check();
    await expect
      .poll(() =>
        page.evaluate(async () => {
          const host = window as unknown as {
            __TAURI_INTERNALS__: {
              invoke: (command: string) => Promise<{ configuration: { startAtLogin: boolean } }>;
            };
          };
          return (await host.__TAURI_INTERNALS__.invoke('get_snapshot')).configuration.startAtLogin;
        }),
      )
      .toBe(true);
    await expect(page.locator('#notice')).toBeEmpty();
    await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
    await expect(page.locator('#diagnostic-connection')).toBeVisible();
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await expect(page.locator('[name="startAtLogin"]')).toBeChecked();
  });
}
