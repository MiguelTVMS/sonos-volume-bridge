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
    await expect(page.locator('#notice')).toBeEmpty();
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(speech).toBeChecked();
    await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
    await page.locator('.schedule-cell').first().click();
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect(page.locator('.schedule-cell').first()).toHaveAttribute('aria-selected', 'true');
    await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
    await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
    await expect(page.locator('#notice')).toBeEmpty();
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(speech).not.toBeChecked();
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
  });
}

test('reset cancels a pending settings autosave', async ({ page }) => {
  await page.goto('/preview.html?platform=macos');
  await expect(page.locator('#runtime-status')).toBeVisible();
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await page.locator('[name="startAtLogin"]').check();
  await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
  await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
  await expect(page.locator('#notice')).toBeEmpty();
  await page.clock.fastForward(1000);
  await expect(page.locator('#notice')).toBeEmpty();
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
});

test('reset follows an autosave already in flight', async ({ page }) => {
  await page.goto('/preview.html?platform=macos');
  await expect(page.locator('#runtime-status')).toBeVisible();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      releaseSave: () => void;
      saveStarted: boolean;
      writes: string[];
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    const gate = new Promise<void>((resolve) => {
      host.releaseSave = resolve;
    });
    host.writes = [];
    host.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'save_configuration') {
        host.saveStarted = true;
        await gate;
      }
      if (['save_configuration', 'reset_configuration'].includes(command))
        host.writes.push(command);
      return invoke(command, args);
    };
  });
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await page.locator('[name="startAtLogin"]').check();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { saveStarted: boolean }).saveStarted))
    .toBe(true);
  await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
  await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
  await page.evaluate(() => (window as unknown as { releaseSave: () => void }).releaseSave());
  await expect(page.locator('#notice')).toBeEmpty();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { writes: string[] }).writes))
    .toEqual(['save_configuration', 'reset_configuration']);
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
});
