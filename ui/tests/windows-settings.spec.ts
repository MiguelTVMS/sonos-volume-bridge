import { expect, test } from '@playwright/test';

const pages = ['Devices', 'Speaker', 'Night schedule', 'Volume', 'General', 'Diagnostics', 'About'];

for (const colorScheme of ['light', 'dark'] as const) {
  for (const viewport of [
    { width: 960, height: 760 },
    { width: 760, height: 460 },
  ]) {
    test(`all Windows pages fit ${viewport.width}x${viewport.height} in ${colorScheme}`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await page.emulateMedia({ colorScheme });
      await page.goto('/preview.html?platform=windows');
      await expect(page.locator('html')).toHaveAttribute('data-platform', 'windows');
      await expect(page.locator('.setting-caption').first()).toBeVisible();
      await expect(page.getByRole('switch', { name: /^Synchronize mute/ })).toHaveCSS(
        'width',
        '40px',
      );
      for (const name of pages) {
        await page.getByRole('button', { name, exact: true }).click();
        await expect(page.getByRole('heading', { name, level: 2, exact: true })).toBeVisible();
        await expect(page.getByRole('button', { name, exact: true })).toHaveAttribute(
          'aria-current',
          'page',
        );
        expect(await page.locator('.panel:visible').count()).toBe(1);
        const clipped = await page.locator('.panel:visible').evaluate((panel) =>
          Array.from(panel.querySelectorAll('button, select, input, dd'))
            .filter((element) => {
              const rect = element.getBoundingClientRect();
              return rect.width > 0 && (rect.left < 0 || rect.right > innerWidth + 1);
            })
            .map((element) => element.outerHTML),
        );
        expect(clipped, `${name} controls fit horizontally`).toEqual([]);
      }
      // A short window must keep the speaker footer reachable, not silently clip it.
      const speaker = page.locator('.sidebar-speaker');
      const bounds = await speaker.boundingBox();
      expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(viewport.height);
      // Guard against the generic card rule adding a second card to each notice child.
      const notice = page.getByRole('heading', { name: 'Sonos trademark and independence notice' });
      await expect(notice).toHaveCSS('border-top-width', '0px');
      await expect(notice).toHaveCSS('padding-top', '0px');
    });
  }
}

test('Windows captions preserve keyboard switches and sliders, and select changes through rerenders', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=windows');
  const mute = page.getByRole('switch', { name: /^Synchronize mute/ });
  await expect(mute).toBeChecked();
  await mute.focus();
  await page.keyboard.press('Space');
  await expect(mute).not.toBeChecked();
  await expect(page.locator('#notice')).toHaveText('Saved.');
  const output = page.getByRole('combobox', { name: /^Follow/ });
  // Native popup keyboard behavior belongs to the host OS, even in Windows styling.
  // Exercise the select's change/save path portably; verify popup keys on Windows.
  await output.selectOption('sample-output');
  await expect(output).toHaveValue('sample-output');
  await page.getByRole('button', { name: 'Volume', exact: true }).click();
  const slider = page.getByRole('slider');
  await slider.focus();
  await page.keyboard.press('ArrowRight');
  await expect(slider).toHaveValue('71');
  await expect(page.locator('#maximum-value')).toHaveText('71%');
  await page.getByRole('button', { name: 'Devices', exact: true }).click();
  await expect(mute).not.toBeChecked();
  await expect(output).toHaveValue('sample-output');
  await page.getByRole('button', { name: 'Volume', exact: true }).click();
  await expect(slider).toHaveValue('71');
});

test('Windows honors forced colors, reduced motion, and keyboard focus', async ({ page }) => {
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await page.goto('/preview.html?platform=windows');
  const mute = page.getByRole('switch', { name: /^Synchronize mute/ });
  await expect(mute).toHaveCSS('appearance', 'auto');
  await expect(mute).toHaveCSS('transition-duration', '0s');
  await mute.focus();
  await page.keyboard.press('Space');
  await expect(mute).not.toBeChecked();
  await expect(mute).toBeFocused();
  await expect(mute).toHaveCSS('outline-style', 'solid');
});

for (const platform of ['macos', 'linux']) {
  test(`${platform} retains its presentation and shared controls`, async ({ page }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    await expect(page.locator('html')).toHaveAttribute('data-platform', platform);
    await expect(page.locator('.setting-caption')).toHaveCount(0);
    await expect(page.getByRole('combobox', { name: 'Sonos speaker', exact: true })).toBeVisible();
    const mute = page.getByRole('switch', { name: 'Synchronize mute', exact: true });
    await expect(mute).toHaveCSS('width', platform === 'macos' ? '36px' : '48px');
    await mute.press('Space');
    await expect(mute).not.toBeChecked();
  });
}
