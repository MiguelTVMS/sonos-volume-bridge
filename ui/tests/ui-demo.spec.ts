import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} demo boots and supports hardware-free settings edits and reset`, async ({
    page,
  }) => {
    await page.goto(`/preview.html?platform=macos&ui=${platform}`);
    await expect(page.locator('html')).toHaveAttribute('data-platform', platform);
    await expect(page.getByText('UI demo · simulated devices', { exact: true })).toBeVisible();
    await expect(page.getByRole('combobox', { name: /^Sonos speaker/ })).toHaveValue(
      'sample-speaker',
    );
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    const speech = page.locator('[data-speaker-setting="speechEnhancement"]');
    await speech.check();
    await expect(speech).toBeChecked();
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await page.locator('[name="startAtLogin"]').check();
    await expect(page.locator('#notice')).toHaveText('Saved.');
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(speech).toBeChecked();
    await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
    await page.locator('.schedule-cell').first().click();
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect(page.locator('.schedule-cell').first()).toHaveAttribute('aria-selected', 'true');
    await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
    await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
    await expect(page.locator('#notice')).toHaveText('Settings reset.');
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(speech).not.toBeChecked();
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
  });
}
