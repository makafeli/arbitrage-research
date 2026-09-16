import { expect, test } from '@playwright/test';
const auth = { operator_id: 'operator', csrf_token: 'account-fixture-csrf', expires_at: '2000000000' };
const caps = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] };

test('production opens on login with no demo, public registration or operator-secret field', async ({ page }) => {
  const posts: string[] = []; const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.route('**/v1/**', route => { if (route.request().method() === 'POST') posts.push(route.request().url()); return route.fulfill({ status: 401, json: { code: 'AUTH_REQUIRED', message: 'Sign in' } }); });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible();
  await expect(page.getByLabel('Email address', { exact: true })).toBeVisible();
  await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /demo|connect api|register/i })).toHaveCount(0);
  await expect(page.getByLabel('Operator secret')).toHaveCount(0);
  await expect(page.getByRole('navigation', { name: 'Trading mode' })).not.toBeVisible();
  expect(posts).toEqual([]); expect(errors).toEqual([]);
});

test('invalid login stays private; successful email/password login restores the real application', async ({ page }) => {
  let logged = false; let deny = true; const requests: unknown[] = [];
  await page.route('**/v1/**', async route => {
    const p = new URL(route.request().url()).pathname;
    if (p === '/v1/auth/sign-in') { requests.push(route.request().postDataJSON()); if (deny) { await route.fulfill({ status: 401, json: { code: 'AUTHENTICATION_FAILED' } }); return; } logged = true; await route.fulfill({ json: auth }); return; }
    if (!logged) { await route.fulfill({ status: 401, json: {} }); return; }
    await route.fulfill({ json: p === '/v1/auth/session' ? auth : p === '/v1/capabilities' ? caps : { items: [], next_cursor: null } });
  });
  await page.goto('/');
  await page.getByLabel('Email address', { exact: true }).fill('owner@example.test'); await page.getByLabel('Password', { exact: true }).fill('first-wrong-password');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible(); await expect(page.getByLabel('Password', { exact: true })).toHaveValue('');
  deny = false; await page.getByLabel('Password', { exact: true }).fill('correct-long-password'); await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Paper trading overview' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Paper trading', exact: true })).toHaveAttribute('aria-pressed', 'true');
  expect(requests).toHaveLength(2); expect(await page.evaluate(() => [localStorage.length, sessionStorage.length])).toEqual([0, 0]);
});

test('one-time activation token is removed from the address and never stored; password confirmation is enforced', async ({ page }) => {
  const token = 'a'.repeat(64); const activations: unknown[] = [];
  await page.route('**/v1/**', async route => {
    if (route.request().url().endsWith('/auth/activate')) { activations.push(route.request().postDataJSON()); await route.fulfill({ status: 204 }); }
    else await route.fulfill({ status: 401, json: {} });
  });
  await page.goto('/#activate=' + token);
  await expect(page.getByRole('heading', { name: 'Set your password' })).toBeVisible();
  await expect(page).toHaveURL(/\/$/);
  await page.getByLabel('Email address', { exact: true }).fill('owner@example.test');
  await page.getByLabel('New password', { exact: true }).fill('a very good passphrase');
  await page.getByLabel('Confirm password', { exact: true }).fill('a different passphrase');
  await page.getByRole('button', { name: 'Save password' }).click();
  await expect(page.getByRole('alert')).toHaveText('The passwords do not match.'); expect(activations).toHaveLength(0);
  await page.getByLabel('Confirm password', { exact: true }).fill('a very good passphrase'); await page.getByRole('button', { name: 'Save password' }).click();
  await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible();
  expect(activations).toEqual([{ email: 'owner@example.test', password: 'a very good passphrase', token }]);
  expect(await page.evaluate(() => [localStorage.length, sessionStorage.length])).toEqual([0, 0]);
});

test('Real trading is a disabled execution workspace, never a mode mutation or fake live activity', async ({ page }) => {
  const writes: string[] = [];
  await page.route('**/v1/**', async route => {
    if (route.request().method() !== 'GET') writes.push(route.request().url());
    const p = new URL(route.request().url()).pathname;
    await route.fulfill({ json: p === '/v1/auth/session' ? auth : p === '/v1/capabilities' ? caps : { items: [], next_cursor: null } });
  });
  await page.goto('/'); await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Real trading', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Real trading is not available' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Start real trading' })).toBeDisabled();
  await expect(page.getByRole('heading', { name: 'Paper trading overview' })).not.toBeVisible();
  await page.getByRole('button', { name: 'Paper trading', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Paper trading overview' })).toBeVisible(); expect(writes).toEqual([]);
});

test('change password uses current password and CSRF, then requires a new sign in', async ({ page }) => {
  let logged = true; let body: unknown; let csrf: string | undefined;
  await page.route('**/v1/**', async route => {
    const p = new URL(route.request().url()).pathname;
    if (p === '/v1/auth/password') { body = route.request().postDataJSON(); csrf = route.request().headers()['x-csrf-token']; logged = false; await route.fulfill({ status: 204 }); return; }
    if (!logged) { await route.fulfill({ status: 401, json: {} }); return; }
    await route.fulfill({ json: p === '/v1/auth/session' ? auth : p === '/v1/capabilities' ? caps : { items: [], next_cursor: null } });
  });
  await page.goto('/'); await page.getByRole('button', { name: 'Change password', exact: true }).click();
  await page.getByLabel('Current password', { exact: true }).fill('old-complete-password');
  await page.getByLabel('New password', { exact: true }).fill('new-complete-password');
  await page.getByLabel('Confirm new password', { exact: true }).fill('new-complete-password');
  await page.getByRole('button', { name: 'Save password', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible();
  expect(csrf).toBe(auth.csrf_token); expect(body).toEqual({ current_password: 'old-complete-password', new_password: 'new-complete-password' });
});
