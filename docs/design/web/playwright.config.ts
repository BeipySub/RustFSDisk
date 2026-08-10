import { existsSync } from 'node:fs';
import { join } from 'node:path';

import { defineConfig, devices } from 'playwright/test';

const cachedChromium = process.env.LOCALAPPDATA
  ? join(
      process.env.LOCALAPPDATA,
      'ms-playwright',
      'chromium-1217',
      'chrome-win64',
      'chrome.exe',
    )
  : '';
const reuseExistingServer = process.env.FUSTFS_PLAYWRIGHT_REUSE_SERVER === '1';

export default defineConfig({
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false,
  outputDir: '../../tmp/web-playwright',
  reporter: [['list']],
  testDir: './tests/e2e',
  timeout: 30_000,
  use: {
    baseURL: 'http://127.0.0.1:4314',
    colorScheme: 'dark',
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  webServer: {
    command:
      'pnpm exec vite preview --config vite.config.ts --configLoader runner --mode production --host 127.0.0.1 --port 4314',
    cwd: 'apps/web-antd',
    reuseExistingServer,
    timeout: 120_000,
    url: 'http://127.0.0.1:4314',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: existsSync(cachedChromium)
          ? { executablePath: cachedChromium }
          : {},
      },
    },
  ],
});
