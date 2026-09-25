import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  workers: 2,
  retries: 0,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:5174',
    browserName: 'chromium',
    viewport: { width: 960, height: 760 },
  },
  webServer: {
    // Launch Vite directly so Playwright owns its process group and can stop it.
    // On Linux, `pnpm run dev` with pnpm 11.27.1 and 12.6.0 leaves Vite in a
    // separate process group: all tests pass, but Playwright hangs in teardown.
    // Direct Node startup was verified with Node 26 / pnpm 12: all eight browser
    // tests pass and the runner exits cleanly. Keep this command free of nested
    // package-manager scripts; successful assertions alone do not verify cleanup.
    command: 'node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 5174 --strictPort',
    url: 'http://127.0.0.1:5174/preview.html',
    reuseExistingServer: false,
  },
});
