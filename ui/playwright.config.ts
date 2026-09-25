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
    command: 'pnpm run dev --host 127.0.0.1 --port 5174 --strictPort',
    url: 'http://127.0.0.1:5174/preview.html',
    reuseExistingServer: false,
  },
});
