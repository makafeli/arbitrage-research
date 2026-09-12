import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';
import { readFileSync } from 'node:fs';

// HTTP stubs exercise UI behavior only. These records are never market evidence.
const opportunity = { ...JSON.parse(readFileSync(new URL('../../../../specs/opportunity.example.json', import.meta.url), 'utf8')), source_kind: 'CAPTURED_MARKET_DATA', opportunity_id: 'browser-record-1', reason_codes: ['NETWORK_COST_UNKNOWN'], simulation_status: 'FAILED' };
const running = { session_id: 'session-base', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'RUNNING', health: 'HEALTHY', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: '2026-09-12T12:00:00Z', configuration_digest: 'sha256:browser-config' };
const capabilities = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [{ configuration_digest: 'sha256:browser-config', mode: 'PAPER', enabled_networks: ['base-mainnet'], strategy_ids: ['usdc-cycle'] }] };
const auth = { operator_id: 'operator', csrf_token: 'browser-csrf', expires_at: '2000000000' };
const pending = { command_id: 'command-stop', session_id: 'session-base', revision: '2', status: 'PENDING', action: 'STOP', accepted_at: '2026-09-12T12:00:00Z', applied_at: null, outstanding_attempts: 0, fence_effective: false, signer_revocation_status: 'NOT_APPLICABLE' };
async function stub(page: Page, override?: (route: Route, path: string) => Promise<boolean>) {
  await page.route('**/v1/**', async route => {
    const path = new URL(route.request().url()).pathname;
    if (override && await override(route, path)) return;
    const data = path === '/v1/auth/session' ? auth : path === '/v1/capabilities' ? capabilities : path === '/v1/sessions' ? { items: [running], next_cursor: null } : path === '/v1/opportunities' ? { items: [], next_cursor: null } : null;
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Unknown stub endpoint' } });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect API', exact: true }).click();
}

test('connected mode shows unsupported capture, rejects synthetic substitution and retains last known records on outage', async ({ page }) => {
  let outage = false;
  await stub(page, async (route, path) => {
    if (outage && path === '/v1/sessions') { await route.fulfill({ status: 503, json: { code: 'UNAVAILABLE', message: 'Database offline' } }); return true; }
    return false;
  });
  await expect(page.getByText('No captured quote records on this page', { exact: true })).toBeVisible();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toHaveCount(0);
  outage = true;
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(page.getByText('STALE / LAST KNOWN', { exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Session session-base' }).getByText('RUNNING', { exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Session session-base' }).getByRole('button', { name: 'Stop', exact: true })).toBeEnabled();
  await expect(page.getByText('No synthetic records have been substituted.')).toBeVisible();
});

test('STOP stays pending until API ACK and then exposes DRAINING instead of inventing STOPPED', async ({ page }) => {
  let posted = false; let applied = false; let posts = 0;
  await stub(page, async (route, path) => {
    if (path === '/v1/sessions/session-base/commands') {
      posts += 1; posted = true;
      expect(route.request().postDataJSON()).toEqual({ action: 'STOP', expected_revision: '1' });
      expect(route.request().headers()['x-csrf-token']).toBe('browser-csrf');
      await route.fulfill({ status: 202, json: pending }); return true;
    }
    if (path === '/v1/commands/command-stop') { await route.fulfill({ json: applied ? { ...pending, status: 'APPLIED', applied_at: '2026-09-12T12:00:03Z', fence_effective: true, outstanding_attempts: 1 } : pending }); return true; }
    if (path === '/v1/sessions' && posted) { await route.fulfill({ json: { items: [{ ...running, desired_revision: '2', applied_revision: applied ? '2' : '1', observed_state: applied ? 'DRAINING' : 'PAUSING', outstanding_attempts: applied ? 1 : 0 }], next_cursor: null } }); return true; }
    return false;
  });
  const card = page.getByRole('region', { name: 'Session session-base' });
  await card.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(card.getByText('STOP · PENDING', { exact: true })).toBeVisible();
  await expect(card.getByRole('button', { name: 'Stop', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(card.getByText('STOP · PENDING', { exact: true })).toBeVisible();
  await expect(card.getByText('STOPPED', { exact: true })).toHaveCount(0);
  expect(posts).toBe(1);
  applied = true;
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(card.getByText('STOP · APPLIED', { exact: true })).toBeVisible();
  await expect(card.getByText('DRAINING', { exact: true })).toBeVisible();
  await expect(card.getByText(/A stop cannot recall an emitted transaction/)).toBeVisible();
});

test('transport failure retries the exact idempotent command without a new receipt claim', async ({ page }) => {
  const requests: { key: string; body: unknown }[] = [];
  await stub(page, async (route, path) => {
    if (path !== '/v1/sessions/session-base/commands') return false;
    requests.push({ key: route.request().headers()['idempotency-key'], body: route.request().postDataJSON() });
    if (requests.length === 1) await route.abort('failed');
    else if (requests.length === 2) await route.fulfill({ status: 429, json: { code: 'RATE_LIMITED', message: 'Retry later' } });
    else await route.fulfill({ status: 202, json: pending });
    return true;
  });
  const card = page.getByRole('region', { name: 'Session session-base' });
  await card.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(card.getByText('Delivery uncertain', { exact: true })).toBeVisible();
  await expect(card.getByText('STOP · APPLIED', { exact: true })).toHaveCount(0);
  await card.getByRole('button', { name: 'Retry same request', exact: true }).click();
  await expect(card.getByText('Delivery uncertain', { exact: true })).toBeVisible();
  await card.getByRole('button', { name: 'Retry same request', exact: true }).click();
  await expect(card.getByText('STOP · PENDING', { exact: true })).toBeVisible();
  expect(requests).toHaveLength(3); expect(requests[0]).toEqual(requests[1]); expect(requests[1]).toEqual(requests[2]);
});

test('view filter does not change stop-all scope and partial rejection stays on its session', async ({ page }) => {
  const sent: string[] = [];
  await stub(page, async (route, path) => {
    if (path === '/v1/sessions') { await route.fulfill({ json: { items: [running, { ...running, session_id: 'session-solana', network_id: 'solana-mainnet' }], next_cursor: null } }); return true; }
    if (path.endsWith('/commands')) { sent.push(path); await route.fulfill(path.includes('session-solana') ? { status: 409, json: { code: 'REVISION_CONFLICT', message: 'Session revision changed' } } : { status: 202, json: pending }); return true; }
    return false;
  });
  await expect(page.getByRole('region', { name: 'Session session-solana' })).toBeVisible();
  await page.getByLabel('View chain', { exact: true }).selectOption('base-mainnet');
  await page.getByRole('button', { name: 'Stop all loaded sessions (2)', exact: true }).click();
  await expect.poll(() => sent.length).toBe(2);
  await page.getByLabel('View chain', { exact: true }).selectOption('all');
  await expect(page.getByRole('region', { name: 'Session session-solana' }).getByText('Command rejected', { exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Session session-base' }).getByText('STOP · PENDING', { exact: true })).toBeVisible();
});

test('captured rejected record exposes provenance and unknown costs, modal focus returns', async ({ page }) => {
  await stub(page, async (route, path) => {
    if (path !== '/v1/opportunities') return false;
    await route.fulfill({ json: { items: [opportunity], next_cursor: null } }); return true;
  });
  await expect(page.getByText('REJECTED · simulation failed', { exact: true })).toBeVisible();
  await expect(page.getByText('Unknown — costs incomplete', { exact: true })).toBeVisible();
  const inspect = page.getByRole('button', { name: 'Inspect browser-record-1', exact: true });
  await inspect.click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText('Provenance', { exact: true })).toBeVisible();
  await expect(dialog.getByText('Unknown: costs incomplete', { exact: true })).toBeVisible();
  await page.keyboard.press('Tab'); await expect(dialog.getByRole('button', { name: 'Close record detail' })).toBeFocused();
  await page.keyboard.press('Escape'); await expect(inspect).toBeFocused();
});

test('experiment creation is validated reference only and remains RECOVERING until worker evidence', async ({ page }) => {
  let request: unknown;
  await stub(page, async (route, path) => {
    if (path !== '/v1/sessions' || route.request().method() !== 'POST') return false;
    request = route.request().postDataJSON();
    await route.fulfill({ status: 201, json: { ...running, session_id: 'created-session', observed_state: 'RECOVERING', health: 'UNKNOWN', desired_revision: '0', applied_revision: '0' } }); return true;
  });
  await page.getByRole('navigation').getByRole('button', { name: 'Experiments', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Create research session', exact: true })).toBeDisabled();
  await page.getByLabel('Validated configuration', { exact: true }).selectOption('sha256:browser-config');
  await page.getByLabel('Session network', { exact: true }).selectOption('base-mainnet');
  await page.getByLabel('Experiment reference', { exact: true }).fill('reviewed-experiment');
  await page.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Create research session', exact: true }).click();
  await expect(page.getByText('Created created-session · RECOVERING. No start command was sent.', { exact: true })).toBeVisible();
  expect(request).toEqual({ configuration_digest: 'sha256:browser-config', mode: 'PAPER', network_id: 'base-mainnet', experiment_id: 'reviewed-experiment', strategy_ids: ['usdc-cycle'] });
  await expect(page.getByRole('button', { name: 'Session created', exact: true })).toBeDisabled();
});

test('login clears password and does not save credentials in browser storage', async ({ page }) => {
  let loggedIn = false;
  await stub(page, async (route, path) => {
    if (path === '/v1/auth/session' && !loggedIn) { await route.fulfill({ status: 401, json: { code: 'AUTH_REQUIRED', message: 'Sign in required' } }); return true; }
    if (path === '/v1/auth/login') { expect(route.request().postDataJSON()).toEqual({ operator_secret: 'browser-only-password' }); loggedIn = true; await route.fulfill({ json: auth }); return true; }
    return false;
  });
  await page.getByLabel('Operator secret', { exact: true }).fill('browser-only-password');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  expect(await page.evaluate(() => [localStorage.length, sessionStorage.length])).toEqual([0, 0]);
  await expect(page.getByLabel('Operator secret', { exact: true })).toHaveCount(0);
});

for (const width of [320, 390, 1440]) {
  test(`connected dashboard remains usable at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await stub(page);
    await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
    for (const name of ['Overview', 'Opportunities', 'Experiments', 'Runs', 'Strategies', 'System']) {
      await page.getByRole('navigation').getByRole('button', { name, exact: true }).click();
      expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
    }
    await page.getByRole('navigation').getByRole('button', { name: 'Overview', exact: true }).click();
    const session = page.getByRole('region', { name: 'Session session-base' });
    for (const action of ['Start', 'Pause', 'Resume', 'Stop']) {
      const geometry = await session.getByRole('button', { name: action, exact: true }).evaluate(button => {
        const text = document.createRange();
        text.selectNodeContents(button);
        const lines = Array.from(text.getClientRects()).filter(rect => rect.width > 0 && rect.height > 0);
        const bounds = button.getBoundingClientRect();
        return { lineTops: [...new Set(lines.map(rect => Math.round(rect.top)))], width: bounds.width, height: bounds.height,
          clipped: lines.some(rect => rect.left < bounds.left || rect.right > bounds.right) };
      });
      expect(geometry.lineTops, `${action} label wraps at ${width}px`).toHaveLength(1);
      expect(geometry.clipped, `${action} label is clipped at ${width}px`).toBe(false);
      expect(geometry.width, `${action} target width at ${width}px`).toBeGreaterThanOrEqual(44);
      expect(geometry.height, `${action} target height at ${width}px`).toBeGreaterThanOrEqual(44);
    }
    await testInfo.attach(`connected-${width}`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
  });
}

test('uncertain session creation retains the same key after a retry is rate limited', async ({ page }) => {
  const requests: { key: string; body: unknown }[] = [];
  await stub(page, async (route, path) => {
    if (path !== '/v1/sessions' || route.request().method() !== 'POST') return false;
    requests.push({ key: route.request().headers()['idempotency-key'], body: route.request().postDataJSON() });
    if (requests.length === 1) await route.fulfill({ status: 504, json: { code: 'REQUEST_TIMEOUT', message: 'Outcome uncertain' } });
    else if (requests.length === 2) await route.fulfill({ status: 429, json: { code: 'RATE_LIMITED', message: 'Retry later' } });
    else await route.fulfill({ status: 201, json: { ...running, session_id: 'created-after-retry', observed_state: 'RECOVERING', health: 'UNKNOWN', desired_revision: '0', applied_revision: '0' } });
    return true;
  });
  await page.getByRole('navigation').getByRole('button', { name: 'Experiments', exact: true }).click();
  await page.getByLabel('Validated configuration', { exact: true }).selectOption('sha256:browser-config');
  await page.getByLabel('Session network', { exact: true }).selectOption('base-mainnet');
  await page.getByLabel('Experiment reference', { exact: true }).fill('retry-experiment');
  await page.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Create research session', exact: true }).click();
  await page.getByRole('button', { name: 'Retry same creation request', exact: true }).click();
  await expect(page.getByLabel('Experiment reference', { exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Retry same creation request', exact: true }).click();
  await expect(page.getByText('Created created-after-retry · RECOVERING. No start command was sent.', { exact: true })).toBeVisible();
  expect(requests).toHaveLength(3); expect(requests[0]).toEqual(requests[1]); expect(requests[1]).toEqual(requests[2]);
});
