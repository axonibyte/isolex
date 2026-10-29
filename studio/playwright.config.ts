import { defineConfig } from '@playwright/test';

// The interface in a browser, served by Vite, over the studio's core as a
// WebAssembly module (src/lib/core.ts) and the sample library: what the
// window shows and whether it is accessible. The built app's own
// end-to-end test is e2e-app (WebdriverIO and tauri-driver, #73).
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
