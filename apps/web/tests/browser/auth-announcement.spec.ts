import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';

const auth = { operator_id: 'operator', csrf_token: 'fixture-announcement-csrf', expires_at: '2000000000' };
const capabilities = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] };
const announcement = (page: Page) => page.locator('[role="status"][aria-live="polite"][aria-atomic="true"]');
async function connect(page: Page, expired: () => boolean = () => false) {
  await page.route('**/v1/**', async route => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/v1/auth/session' || path === '/v1/auth/login') { await route.fulfill({ json: auth }); return; }
    if (expired()) { await route.fulfill({ status: 401, body: '<html>fixture expired session</html>' }); return; }
    await route.fulfill({ json: path === '/v1/capabilities' ? capabilities : { items: [], next_cursor: null } });
  });
  await page.goto('/'); await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
}

test('restored cookie authorization replaces the temporary unauthorized live-region announcement', async ({ page }) => {
  await connect(page);
  await expect(announcement(page)).toHaveText('Authenticated. Loading service records.');
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Connect to your research service', exact: true })).toHaveCount(0);
});

test('revocation and subsequent login announce their actual authorization state', async ({ page }) => {
  let expired = false; await connect(page, () => expired);
  expired = true; await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(announcement(page)).toHaveText('Authorization unavailable. Sign in again; unresolved requests are retained.');
  await expect(page.getByRole('heading', { name: 'Connect to your research service', exact: true })).toBeVisible();
  expired = false; await page.getByLabel('Operator secret', { exact: true }).fill('fixture-operator-secret-0000000000');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(announcement(page)).toHaveText('Authenticated. Loading service records.');
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible();
});
