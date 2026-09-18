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

test('session creation cannot enable LIVE via a URL parameter or a tampered hidden field', async ({ page }) => {
  let request: unknown;
  await stub(page, async (route, path) => {
    if (path !== '/v1/sessions' || route.request().method() !== 'POST') return false;
    request = route.request().postDataJSON();
    await route.fulfill({ status: 201, json: { ...running, session_id: 'live-guard-session', observed_state: 'RECOVERING', health: 'UNKNOWN', desired_revision: '0', applied_revision: '0' } });
    return true;
  });
  // A URL parameter has no wiring into any component state; this only proves
  // that adding one cannot change what gets submitted.
  await page.goto('/?mode=LIVE&network_id=LIVE');
  await page.getByRole('navigation').getByRole('button', { name: 'Experiments', exact: true }).click();
  const configOptions = await page.getByLabel('Validated configuration', { exact: true }).locator('option').allTextContents();
  expect(configOptions.some(text => text.includes('LIVE'))).toBe(false);
  await page.getByLabel('Validated configuration', { exact: true }).selectOption('sha256:browser-config');
  await page.getByLabel('Session network', { exact: true }).selectOption('base-mainnet');
  await page.getByLabel('Experiment reference', { exact: true }).fill('live-guard-experiment');
  // Tamper: inject a hidden "mode=LIVE" field into the real form. The submit
  // handler builds the request body from React state (the chosen configuration's
  // own mode), never from raw form field values, so this must have no effect.
  await page.evaluate(() => {
    const form = document.querySelector('form.connected-form');
    const hidden = document.createElement('input');
    hidden.type = 'hidden'; hidden.name = 'mode'; hidden.value = 'LIVE';
    form?.appendChild(hidden);
  });
  await page.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Create research session', exact: true }).click();
  await expect(page.getByText('Created live-guard-session · RECOVERING. No start command was sent.', { exact: true })).toBeVisible();
  expect(request).toMatchObject({ mode: 'PAPER' });
  expect((request as { mode: string }).mode).not.toBe('LIVE');
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

test('Resume is only available for a valid PAUSED session; a running session cannot resume or restart', async ({ page }) => {
  const paused = { ...running, session_id: 'session-paused', observed_state: 'PAUSED' };
  await stub(page, async (route, path) => {
    if (path === '/v1/sessions') { await route.fulfill({ json: { items: [running, paused], next_cursor: null } }); return true; }
    return false;
  });
  const runningCard = page.getByRole('region', { name: 'Session session-base' });
  await expect(runningCard.getByRole('button', { name: 'Start', exact: true })).toBeDisabled();
  await expect(runningCard.getByRole('button', { name: 'Pause', exact: true })).toBeEnabled();
  await expect(runningCard.getByRole('button', { name: 'Resume', exact: true })).toBeDisabled();
  await expect(runningCard.getByRole('button', { name: 'Stop', exact: true })).toBeEnabled();
  const pausedCard = page.getByRole('region', { name: 'Session session-paused' });
  await expect(pausedCard.getByRole('button', { name: 'Start', exact: true })).toBeDisabled();
  await expect(pausedCard.getByRole('button', { name: 'Pause', exact: true })).toBeDisabled();
  await expect(pausedCard.getByRole('button', { name: 'Resume', exact: true })).toBeEnabled();
  await expect(pausedCard.getByRole('button', { name: 'Stop', exact: true })).toBeEnabled();
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

test('a synchronous double-click on Stop sends exactly one command with one idempotency key', async ({ page }) => {
  const requests: { key: string }[] = [];
  await stub(page, async (route, path) => {
    if (path !== '/v1/sessions/session-base/commands') return false;
    requests.push({ key: route.request().headers()['idempotency-key'] });
    await route.fulfill({ status: 202, json: pending }); return true;
  });
  const card = page.getByRole('region', { name: 'Session session-base' });
  const stopButton = card.getByRole('button', { name: 'Stop', exact: true });
  // Two `.click()` calls in one evaluate() dispatch synchronously, before React
  // can re-render the disabled attribute: this exercises the in-memory
  // `sending` guard in send(), not just the DOM disabled state.
  await stopButton.evaluate(button => { (button as HTMLButtonElement).click(); (button as HTMLButtonElement).click(); });
  await expect(card.getByText('STOP · PENDING', { exact: true })).toBeVisible();
  expect(requests).toHaveLength(1);
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

test('incomplete simulated evidence fails the initial snapshot without a success badge', async ({ page }) => {
  await stub(page, async (route, path) => {
    if (path !== '/v1/opportunities') return false;
    await route.fulfill({ json: { items: [{ ...opportunity, schema_version: '1.1.0', dataset_origin: 'RECORDED_LIVE',
      net_after_explicit_costs_minor: null, evidence_label: 'SIMULATED', simulation_status: 'PASSED', execution_plan_digest: 'sha256:browser-plan',
      snapshot: { ...opportunity.snapshot, complete: false }, eligibility_checks: { ...opportunity.eligibility_checks,
        atomic_route_supported: true, final_balance_guard_present: true, simulation_matches_exact_plan: true } }], next_cursor: null } });
    return true;
  });
  await expect(page.getByText('Service unavailable or request rejected.', { exact: true })).toBeVisible();
  await expect(page.getByText('No service snapshot available', { exact: true })).toBeVisible();
  await expect(page.getByText('Simulation evidence requires a matching atomic plan and a complete, consistent snapshot.', { exact: true })).toBeVisible();
  await expect(page.getByText('SIMULATED', { exact: true })).toHaveCount(0);
  await expect(page.getByText('API CONNECTED', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Inspect browser-record-1', exact: true })).toHaveCount(0);
});

test('contradictory quote assumptions freeze the last valid snapshot until valid evidence returns', async ({ page }) => {
  let record = opportunity;
  await stub(page, async (route, path) => {
    if (path !== '/v1/opportunities') return false;
    await route.fulfill({ json: { items: [record], next_cursor: null } }); return true;
  });
  await expect(page.getByRole('button', { name: 'Inspect browser-record-1', exact: true })).toBeVisible();
  record = { ...opportunity, opportunity_id: 'contradictory-record', quoted_output_includes_pool_fees_and_price_impact: false };
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(page.getByText('STALE / LAST KNOWN', { exact: true })).toBeVisible();
  await expect(page.getByText('Quoted output must include pool fees and price impact under the retained opportunity contract.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect browser-record-1', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect contradictory-record', exact: true })).toHaveCount(0);
  record = { ...opportunity, opportunity_id: 'recovered-valid-record' };
  await page.getByRole('button', { name: 'Retry connection', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect recovered-valid-record', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect browser-record-1', exact: true })).toHaveCount(0);
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
    if (path === '/v1/auth/sign-in') { expect(route.request().postDataJSON()).toEqual({ email: 'owner@example.test', password: 'browser-only-password' }); loggedIn = true; await route.fulfill({ json: auth }); return true; }
    return false;
  });
  await page.getByLabel('Email address', { exact: true }).fill('owner@example.test'); await page.getByLabel('Password', { exact: true }).fill('browser-only-password');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  expect(await page.evaluate(() => [localStorage.length, sessionStorage.length])).toEqual([0, 0]);
  await expect(page.getByLabel('Password', { exact: true })).toHaveCount(0);
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
