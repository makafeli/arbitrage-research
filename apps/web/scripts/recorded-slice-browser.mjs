// Actual local API/browser evidence. No HTTP mocks, provider keys or live controls.
import { chromium, expect } from '@playwright/test';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const secret = process.env.ARB_SLICE_BROWSER_SECRET;
const session = process.env.ARB_SLICE_SESSION;
const output = process.env.ARB_SLICE_EVIDENCE_DIR;
if (!secret || !session || !output) throw new Error('Explicit disposable browser context required');
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, reducedMotion: 'reduce' });
  const apiMutations = [];
  page.on('request', request => {
    const url = new URL(request.url());
    if (url.pathname.startsWith('/v1/') && !['GET', 'HEAD'].includes(request.method())) {
      apiMutations.push(url.pathname);
    }
  });
  let loaded = false;
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try { await page.goto('http://127.0.0.1:5173', { timeout: 1500 }); loaded = true; break; }
    catch { await new Promise(resolve => setTimeout(resolve, 200)); }
  }
  if (!loaded) throw new Error('Local dashboard unavailable');
  await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await page.getByLabel('Operator secret', { exact: true }).fill(secret);
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible({ timeout: 15000 });
  await expect(page.getByRole('region', { name: `Session ${session}`, exact: true })).toContainText('STOPPED');
  await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toHaveCount(0);
  const observed = await page.evaluate(async sid => {
    const response = await fetch(`/v1/decisions?session_id=${encodeURIComponent(sid)}&limit=100`);
    if (!response.ok) throw new Error('Actual decision API failed');
    return response.json();
  }, session);
  if (!observed.items.length || observed.items.some(row => row.trace.dataset_origin !== 'RECORDED_LIVE')) {
    throw new Error('Browser did not receive actual recorded decisions');
  }
  await page.screenshot({ path: join(output, 'recorded-overview.png'), fullPage: true });
  await page.getByRole('navigation').getByRole('button', { name: 'Opportunities', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Opportunities', exact: true })).toBeVisible();
  await page.getByLabel('Decision session', { exact: true }).selectOption(session);
  const id = observed.items[0].trace.observation_id;
  const inspect = page.getByRole('button', { name: `Inspect decision ${id}`, exact: true });
  await expect(inspect).toBeVisible({ timeout: 15000 });
  await inspect.click();
  const detail = page.getByRole('dialog', { name: 'Decision evidence detail', exact: true });
  await expect(detail).toBeVisible();
  await expect(detail).toContainText(id);
  await expect(detail).toContainText(observed.items[0].trace.configuration_digest);
  await expect(detail).toContainText('Capture references');
  for (const ref of observed.items[0].trace.capture_refs) await expect(detail).toContainText(ref.manifest_digest);
  // Only the login POST is permitted; viewing captured evidence cannot start work.
  if (apiMutations.some(path => path !== '/v1/auth/login')) throw new Error('Unexpected browser mutation');
  await page.screenshot({ path: join(output, 'recorded-decisions.png'), fullPage: true });
  await writeFile(join(output, 'browser.json'), JSON.stringify({
    session_id: session, source: 'ACTUAL_LOCAL_AUTHENTICATED_API', http_mocks: false,
    received_observations: observed.items.map(row => row.trace.observation_id),
    dataset_origins: [...new Set(observed.items.map(row => row.trace.dataset_origin))],
    visible_inspected_observation: id, visible_configuration: observed.items[0].trace.configuration_digest,
    state: 'STOPPED', api_mutations: apiMutations, execution_authorized: false,
  }, null, 2) + '\n');
} finally {
  await browser.close();
}
