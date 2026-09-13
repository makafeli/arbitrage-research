import { expect, test } from '@playwright/test';

// Exercise cookie-session restoration before any navigation event can replace
// the live-region message. All responses are synthetic same-origin fixtures.
test('restored authorization replaces the temporary unauthorized live announcement', async ({ page }) => {
  const auth = { operator_id: 'operator', csrf_token: 'fixture-restored-csrf', expires_at: '2000000000' };
  const capabilities = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] };
  await page.route('**/v1/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const data = path === '/v1/auth/session' ? auth
      : path === '/v1/capabilities' ? capabilities
      : path === '/v1/sessions' || path === '/v1/opportunities' ? { items: [], next_cursor: null }
      : null;
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Fixture endpoint unavailable' } });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await expect(page.locator('.sr[role="status"][aria-live="polite"]')).toHaveText('Authenticated. Loading service records.');
  await expect(page.getByRole('heading', { name: 'Connect to your research service', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible();
});
