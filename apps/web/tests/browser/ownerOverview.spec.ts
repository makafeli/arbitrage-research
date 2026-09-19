import { expect, test } from '@playwright/test';

const auth = { operator_id: 'operator', csrf_token: 'owner-overview-csrf', expires_at: '2000000000' };
const caps = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] };
const session = { session_id: 'owner-session-1', network_id: 'base-mainnet', mode: 'OBSERVE', observed_state: 'RUNNING', health: 'HEALTHY',
  desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'cfg-digest' };
const coverage = { session_id: session.session_id, raw_observations: '0', quoted_candidates: '0', rejected: '0', no_route: '0', data_unavailable: '0',
  unique_opportunity_groups: '0', eligible_attempts: null, reconciled_transactions: null, execution_accounting_available: false,
  collection_completeness: 'UNKNOWN', coverage_window_start_ms: null, coverage_window_end_ms: null };
const emptyPage = { items: [], next_cursor: null };

async function mockDemo(page: import('@playwright/test').Page) {
  await page.route('**/v1/**', async route => {
    const p = new URL(route.request().url()).pathname;
    if (p === '/v1/auth/session') { await route.fulfill({ json: auth }); return; }
    if (p === '/v1/capabilities') { await route.fulfill({ json: caps }); return; }
    if (p === '/v1/sessions') { await route.fulfill({ json: { items: [session], next_cursor: null } }); return; }
    if (p === '/v1/decision-coverage') { await route.fulfill({ json: coverage }); return; }
    await route.fulfill({ json: emptyPage });
  });
}

test('owner overview page shows all five plain-language blocks and a working nl/en toggle', async ({ page }) => {
  await mockDemo(page);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Paper trading overview' })).toBeVisible();

  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Eigenaaroverzicht' })).toBeVisible(); // default language is Dutch

  await page.getByLabel('Sessie').selectOption(session.session_id);
  await expect(page.getByRole('heading', { name: '1. Status' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '2. Gezondheid (laatste 24 uur)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '3. Bevindingen' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '4. Wat als (hypothetisch)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '5. Route naar PAPER/live' })).toBeVisible();
  for (const id of ['ARB-035', 'ARB-039', 'ARB-041', 'ARB-042', 'ARB-043', 'ARB-044']) await expect(page.getByText(id, { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'English', exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Owner overview' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '2. Health (last 24h)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '3. Findings' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '4. What if (hypothetical)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '5. Road to PAPER/live' })).toBeVisible();
  await expect(page.getByText('OBSERVE candidates are not executed, not simulated end-to-end, and are not profit.')).toBeVisible();

  // the language choice is remembered across a reload, per the ticket's localStorage requirement.
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Paper trading overview' })).toBeVisible();
  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Owner overview' })).toBeVisible();
});
