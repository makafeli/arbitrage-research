import { expect, test } from '@playwright/test';
const views = ['Overview', 'Opportunities', 'Experiments', 'Runs', 'Strategies', 'System'];
for (const width of [320, 390, 768, 1440]) {
 for (const light of [false, true]) {
  test(`login and paper/real shell fit ${width}px with light=${light}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 1000 });
    let signed = false; const writes: string[] = [];
    const session = { session_id: 'shell-base', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'STOPPED', health: 'HEALTHY', desired_revision: '0', applied_revision: '0', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:shell-config' };
    await page.route('**/v1/**', async route => {
      const p = new URL(route.request().url()).pathname;
      const auth = { operator_id: 'operator', csrf_token: 'fixture', expires_at: '2000000000' };
      if (p === '/v1/auth/sign-in') { signed = true; writes.push(p); await route.fulfill({ json: auth }); return; }
      if (!signed) { await route.fulfill({ status: 401, json: {} }); return; }
      if (route.request().method() !== 'GET') writes.push(p);
      await route.fulfill({ json: p === '/v1/auth/session' ? auth : p === '/v1/capabilities' ? { modes: ['PAPER'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] } : p === '/v1/sessions' ? { items: [session, { ...session, session_id: 'shell-solana', network_id: 'solana-mainnet' }], next_cursor: null } : { items: [], next_cursor: null } });
    });
    await page.goto('/'); await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible();
    if (light) await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await testInfo.attach(`login-${width}-${light}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
    await page.getByLabel('Email address', { exact: true }).fill('owner@example.test'); await page.getByLabel('Password', { exact: true }).fill('fixture-password'); await page.getByRole('button', { name: 'Sign in', exact: true }).click();
    await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
    for (const view of views) {
      await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: view, exact: true }).click();
      const overflow = await page.evaluate(() => Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) - document.documentElement.clientWidth);
      expect(overflow).toBeLessThanOrEqual(1);
      await expect(page.getByRole('button', { name: /demo|Connect API/i })).toHaveCount(0);
    }
    await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Overview', exact: true }).click();
    await page.getByRole('combobox', { name: 'View chain' }).selectOption('base-mainnet');
    await expect(page.getByRole('region', { name: 'Session shell-solana', exact: true })).toHaveCount(0);
    await page.getByRole('combobox', { name: 'View chain' }).selectOption('all');
    await expect(page.getByRole('region', { name: 'Session shell-solana', exact: true })).toContainText('STOPPED');
    await page.getByRole('button', { name: 'Real trading', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Start real trading', exact: true })).toBeDisabled();
    await testInfo.attach(`real-${width}-${light}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
    await page.getByRole('button', { name: 'Paper trading', exact: true }).click();
    expect(await page.locator('body').evaluate(el => el.classList.contains('light'))).toBe(light);
    expect(writes).toEqual(['/v1/auth/sign-in']);
  });
 }
}
