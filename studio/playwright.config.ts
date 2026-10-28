import { defineConfig } from '@playwright/test';

// The interface in a browser, over the sample library (src/lib/fake.ts),
// served by Vite: what the window shows and whether it is accessible.
// The shell's own end-to-end test comes with tauri-driver (#73).
export default defineConfig({
  testDir: 'e2e',
  retries: 0,
  timeout: 30_000,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:5173',
    viewport: { width: 1400, height: 900 },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  outputDir: 'test-results',
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1',
    url: 'http://127.0.0.1:5173',
    reuseExistingServer: !process.env.CI,
  },
});
