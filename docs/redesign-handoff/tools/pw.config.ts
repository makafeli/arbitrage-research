import { defineConfig } from '@playwright/test';
import { resolve } from 'node:path';

// Screenshot runner for docs/redesign-handoff. Run from apps/web:
//   npx playwright test -c ../../docs/redesign-handoff/tools/pw.config.ts
// Needs docs/redesign-handoff/tools/node_modules -> ../../../apps/web/node_modules (see README).
// Port 4174 (not 4173): apps/web's own Playwright suite (apps/web/playwright.config.ts) reuses
// 4173 for its dev preview server; a different port avoids colliding with a suite already running.
export default defineConfig({
  testDir: __dirname,
  testMatch: /shots\.spec\.ts/,
  timeout: 60_000,
  workers: 1,
  reporter: [['list']],
  use: { baseURL: 'http://127.0.0.1:4174', browserName: 'chromium', reducedMotion: 'reduce' },
  webServer: { command: 'npm run preview -- --port 4174', cwd: resolve(__dirname, '../../../apps/web'), url: 'http://127.0.0.1:4174', reuseExistingServer: true, timeout: 30_000 },
});
