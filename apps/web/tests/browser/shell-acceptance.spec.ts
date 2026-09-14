import { expect, test } from '@playwright/test';

// This file tests the research shell using local synthetic records and HTTP
// stubs. Passing it does not qualify a provider, dataset or deployed service.
const navigation = ['Overview', 'Opportunities', 'Experiments', 'Runs', 'Strategies', 'System'];

for (const width of [320, 390, 768, 1440]) {
  for (const theme of ['dark', 'light'] as const) {
    test(`equal chain emphasis and explicit demo provenance at ${width}px in ${theme}`, async ({ page }, testInfo) => {
      await page.setViewportSize({ width, height: 1000 });
      const mutations: string[] = [];
      page.on('request', request => {
        if (!['GET', 'HEAD'].includes(request.method())) mutations.push(request.url());
      });
      await page.goto('/');
      if (theme === 'light') await page.getByRole('button', { name: 'Switch to light theme' }).click();
      const solana = page.getByRole('region', { name: 'Solana synthetic observations' });
      const base = page.getByRole('region', { name: 'Base synthetic observations' });
      const a = await solana.boundingBox();
      const b = await base.boundingBox();
      expect(a).not.toBeNull(); expect(b).not.toBeNull();
      expect(Math.abs(a!.width - b!.width)).toBeLessThanOrEqual(1);
      expect(Math.abs(a!.height - b!.height)).toBeLessThanOrEqual(1);
      const tokens = await page.locator('body').evaluate(element => {
        const css = getComputedStyle(element);
        return { accent: css.getPropertyValue('--accent').trim(), background: css.getPropertyValue('--bg').trim() };
      });
      expect(tokens.accent).not.toBe(''); expect(tokens.background).not.toBe('');
      await testInfo.attach(`shell-${width}-${theme}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
      for (const view of navigation) {
        await page.getByRole('navigation').getByRole('button', { name: view, exact: true }).click();
        await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toBeVisible();
        await expect(page.getByText('PAPER MODE DEMO', { exact: true })).toBeVisible();
        await expect(page.getByRole('button', { name: 'Connect API', exact: true })).toBeVisible();
        await expect(page.getByRole('button', { name: 'Start demo', exact: true })).toBeVisible();
        await expect(page.getByRole('combobox', { name: 'View chain' })).toBeVisible();
        await expect(page.getByRole('button', { name: /connect wallet|arm live|import private key/i })).toHaveCount(0);
        await expect(page.locator('input[type="password"]')).toHaveCount(0);
        const overflow = await page.evaluate(() => Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) - document.documentElement.clientWidth);
        expect(overflow).toBeLessThanOrEqual(1);
      }
      await page.getByRole('combobox', { name: 'View chain' }).selectOption('base');
      await expect(page.getByRole('region', { name: 'Local demo controls for both paper sessions' })).toContainText('Solana: STOPPED');
      await expect(page.getByRole('region', { name: 'Local demo controls for both paper sessions' })).toContainText('Base: STOPPED');
      expect(mutations).toEqual([]);
    });
  }
}

for (const width of [320, 390, 768, 1440]) {
 test(`theme and mode labels survive demo/disconnected transitions at ${width}px`, async ({ page }, testInfo) => {
  await page.setViewportSize({ width, height: 1000 });
  await page.route('**/v1/**', route => route.fulfill({ status: 401, json: { code: 'UNAUTHORIZED', message: 'Synthetic unsigned session' } }));
  await page.goto('/');
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await expect(page.locator('body')).toHaveClass(/light/);
  await expect(page.getByRole('banner').getByText('CONNECTED MODE', { exact: true })).toBeVisible();
  await testInfo.attach(`disconnected-${width}-light`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
  await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Open demo', exact: true }).click();
  await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toBeVisible();
  await expect(page.locator('body')).toHaveClass(/light/);
  await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await page.getByRole('button', { name: 'Switch to dark theme' }).click();
  await expect(page.getByRole('banner').getByText('CONNECTED MODE', { exact: true })).toBeVisible();
  await expect(page.locator('body')).not.toHaveClass(/light/);
  await page.getByRole('button', { name: 'Open demo', exact: true }).click();
  await expect(page.locator('body')).not.toHaveClass(/light/);
  await expect(page.getByText('PAPER MODE DEMO', { exact: true })).toBeVisible();
 });
}

for (const width of [320, 390, 768, 1440]) {
 test(`connected mode labels and view-only filtering at ${width}px`, async ({ page }, testInfo) => {
  await page.setViewportSize({ width, height: 1000 });
  const mutations: string[] = [];
  const session = { session_id: 'shell-base', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'STOPPED', health: 'HEALTHY', desired_revision: '0', applied_revision: '0', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:shell-config' };
  await page.route('**/v1/**', async route => {
    const request = route.request();
    if (!['GET', 'HEAD'].includes(request.method())) mutations.push(request.url());
    const path = new URL(request.url()).pathname;
    const data = path === '/v1/auth/session' ? { operator_id: 'shell-test', csrf_token: 'synthetic', expires_at: '2000000000' }
      : path === '/v1/capabilities' ? { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] }
      : path === '/v1/sessions' ? { items: [session, { ...session, session_id: 'shell-solana', network_id: 'solana-mainnet' }], next_cursor: null }
      : path === '/v1/opportunities' ? { items: [], next_cursor: null } : null;
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Not a shell endpoint' } });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await expect(page.getByRole('banner').getByText('CONNECTED MODE', { exact: true })).toBeVisible();
  await testInfo.attach(`connected-${width}-dark`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
  await page.getByRole('combobox', { name: 'View chain' }).selectOption('base-mainnet');
  await expect(page.getByRole('region', { name: 'Session shell-base', exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Session shell-solana', exact: true })).toHaveCount(0);
  await page.getByRole('combobox', { name: 'View chain' }).selectOption('all');
  await expect(page.getByRole('region', { name: 'Session shell-solana', exact: true })).toContainText('STOPPED');
  await expect(page.getByRole('region', { name: 'Session shell-base', exact: true })).toContainText('STOPPED');
  await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toHaveCount(0);
  expect(mutations).toEqual([]);
 });
}
