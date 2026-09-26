import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows']) {
  test(`${platform} camera opt-in persists through a settings rerender`, async ({ page }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    const option = page.locator('input[name="cameraSpeechEnhancementEnabled"]');
    await expect(option).toBeEnabled();
    await option.check();
    await expect(option).toBeChecked();
    await expect(page.locator('#camera-status')).toHaveText('Camera automation is waiting');
    // Let the production save/refresh cycle complete, then check the rendered setting again.
    await page.waitForTimeout(600);
    await page.getByRole('button', { name: 'Devices', exact: true }).click();
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(option).toBeChecked();
    await option.uncheck();
    await expect(option).not.toBeChecked();
  });
}

test('Linux clearly leaves the deferred feature unavailable', async ({ page }) => {
  await page.goto('/preview.html?platform=linux');
  await page.getByRole('button', { name: 'Speaker', exact: true }).click();
  await expect(page.locator('input[name="cameraSpeechEnhancementEnabled"]')).toBeDisabled();
  await expect(page.locator('#camera-status')).toContainText('unavailable');
});
