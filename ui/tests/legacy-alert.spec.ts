import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} preserves the conflict warning across Settings navigation and rechecks`, async ({
    page,
  }) => {
    await page.goto(`/preview.html?platform=${platform}&legacy=running`);
    const warning = page.getByRole('alert');
    await expect(warning).toContainText('Sonos Volume Bridge is still running');
    await expect(warning).toContainText('has paused synchronization');
    await page.getByRole('button', { name: 'About', exact: true }).click();
    await expect(warning).toBeVisible();
    await expect(warning.getByRole('link')).toHaveAttribute(
      'href',
      'https://svb.miguel.ms/upgrade.html',
    );
    await page.evaluate(() => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      };
      const invoke = host.__TAURI_INTERNALS__.invoke;
      let checks = 0;
      host.__TAURI_INTERNALS__.invoke = async (command, args) => {
        if (command === 'recheck_legacy_app') {
          checks += 1;
          return checks === 1
            ? { status: 'unknown', paused: true }
            : { status: 'clear', paused: false };
        }
        return invoke(command, args);
      };
    });
    await warning.getByRole('button', { name: 'Check again' }).click();
    await expect(warning).toBeVisible();
    await expect(warning).toContainText('has paused synchronization');
    await warning.getByRole('button', { name: 'Check again' }).click();
    await expect(warning).toBeHidden();
  });
}
test('initial inspection failure provides guidance without claiming a conflict', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=macos&legacy=unknown');
  await expect(page.getByRole('alert')).toContainText('Unable to check');
  await expect(page.getByRole('alert')).not.toContainText('has paused synchronization');
});
