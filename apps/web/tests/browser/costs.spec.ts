import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { coverage, decision, huge } from '../research.fixture';
import { costAssessmentFixture } from '../costs.fixture';
import { frozenExportFixture, sealExport } from '../exports.fixture';
import type { CostAssessmentRequest, StoredCostAssessment } from '../../src/api/costs';

const session = { session_id: 'session-paper', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'STOPPED', health: 'UNKNOWN', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:paper-fixture-config' };
const caps = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, decision_history: true, cost_assessments: true, session_export: true, registered_configurations: [] };
async function setup(page: Page, override?: (route: Route, url: URL) => Promise<boolean>) {
  await page.route('**/v1/**', async route => {
    const url = new URL(route.request().url()); if (override && await override(route, url)) return;
    const path = url.pathname; let data: unknown = null;
    if (path === '/v1/auth/session') data = { operator_id: 'operator', csrf_token: 'cost-test-csrf', expires_at: '2000000000' };
    if (path === '/v1/capabilities') data = caps;
    if (path === '/v1/sessions') data = { items: [session], next_cursor: null };
    if (path === '/v1/opportunities' || path === '/v1/decision-groups' || path.endsWith('/cost-assessments')) data = { items: [], next_cursor: null };
    if (path === '/v1/decisions') data = { items: [decision()], next_cursor: null };
    if (path.startsWith('/v1/decisions/')) data = decision();
    if (path === '/v1/decision-coverage') data = coverage();
    if (path.endsWith('/export')) data = frozenExportFixture();
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Unknown fixture endpoint' } });
  });
  await page.goto('/');
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Opportunities', exact: true }).click();
  await page.getByLabel('Decision session', { exact: true }).selectOption('session-paper');
  await page.getByRole('button', { name: 'Assess hypothetical costs for observation-fixture', exact: true }).click();
}
async function basic(page: Page) {
  await page.getByLabel('Scenario identifier', { exact: true }).fill('manual-browser-scenario');
  await page.getByLabel('Manual assumption reference', { exact: true }).fill('browser-fixture-note');
}
async function complete(page: Page, large = false) {
  await basic(page);
  for (const name of ['Network execution', 'Base L1 data', 'Relay tip', 'Funding cost', 'Account setup', 'Other transaction costs']) {
    await page.getByLabel(name + ' input', { exact: true }).selectOption('KNOWN');
    await page.getByLabel(name + ' base units', { exact: true }).fill(large && name === 'Other transaction costs' ? huge : '0');
  }
  await page.getByLabel('Start-asset base units · numerator', { exact: true }).fill('2');
  await page.getByLabel('Native base units · denominator', { exact: true }).fill('1');
  await page.getByLabel('Funding assumption', { exact: true }).selectOption('OWN_VIRTUAL_CAPITAL');
}
async function reviewed(page: Page) { await page.getByRole('checkbox', { name: 'I reviewed the exact units, historical valuation and hypothetical assumptions.' }).check(); }
const workspace = (page: Page) => page.getByRole('region', { name: 'Hypothetical cost research', exact: true });

test('missing manual costs remain unknown and saving retains the original gross quote and separate history', async ({ page }) => {
  let saved: StoredCostAssessment | null = null;
  await setup(page, async (route, url) => {
    if (!url.pathname.endsWith('/cost-assessments')) return false;
    if (route.request().method() === 'POST') {
      const body = route.request().postDataJSON() as CostAssessmentRequest;
      expect(Object.keys(body).sort()).toEqual(['observation_id', 'scenario']); expect(body.scenario.origin).toBe('MANUALLY_CONSTRUCTED');
      expect(body.scenario.expenses).toHaveLength(6); expect(body.scenario.expenses.every(e => e.amount.status === 'MISSING')).toBe(true);
      saved = costAssessmentFixture(body.scenario); await route.fulfill({ status: 201, json: saved });
    } else await route.fulfill({ json: { items: saved ? [saved] : [], next_cursor: null } }); return true;
  });
  await basic(page); await reviewed(page); await page.getByRole('button', { name: 'Save hypothetical cost assessment', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Cost assessment saved', exact: true })).toBeDisabled();
  const result = workspace(page).getByRole('region', { name: /^Cost result/ }).first();
  await expect(result.getByText('Net remains unknown', { exact: true })).toBeVisible();
  await expect(result.getByText('Unknown', { exact: true })).toHaveCount(8);
  await page.getByRole('tab', { name: 'Decisions', exact: true }).click();
  await expect(page.getByRole('table', { name: 'Stored decision observations · exact start asset minor units' }).getByText('-10', { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Costs', exact: true }).click();
  await expect(workspace(page).getByText('manual-browser-scenario · v1 · 2026-09-12T12:03:00Z', { exact: true })).toBeVisible();
  expect(saved!.assessment.report.transaction_net).toBeNull();
});

test('explicit zero, huge negative results and allocation survive immutable history and frozen downloads', async ({ page }) => {
  let saved: StoredCostAssessment | null = null;
  await setup(page, async (route, url) => {
    if (url.pathname.endsWith('/cost-assessments')) {
      if (route.request().method() === 'POST') { saved = costAssessmentFixture((route.request().postDataJSON() as CostAssessmentRequest).scenario); await route.fulfill({ status: 201, json: saved }); }
      else await route.fulfill({ json: { items: saved ? [saved] : [], next_cursor: null } }); return true;
    }
    if (url.pathname.endsWith('/export') && saved) { const bundle = frozenExportFixture(); bundle.data.cost_assessments = [saved]; bundle.snapshot.source_counts.cost_assessments = '1'; await route.fulfill({ json: sealExport(bundle) }); return true; }
    return false;
  });
  await complete(page, true); await page.getByLabel('Shared operating overhead', { exact: true }).selectOption('ALLOCATED');
  await page.getByLabel('Overhead allocation · start-asset base units', { exact: true }).fill('2');
  await page.getByLabel('Allocation method identifier', { exact: true }).fill('per-observation'); await reviewed(page);
  await page.getByRole('button', { name: 'Save hypothetical cost assessment', exact: true }).click();
  await expect(workspace(page).getByRole('region', { name: /^Cost result/ }).first().getByText((-12n - BigInt(huge)).toString(), { exact: true })).toBeVisible();
  await expect(page.getByLabel('Other transaction costs base units', { exact: true })).toBeDisabled();
  await page.getByRole('tab', { name: 'Exports', exact: true }).click();
  const exporter = page.getByRole('region', { name: 'Frozen session export', exact: true });
  await exporter.getByRole('button', { name: 'Prepare frozen session export', exact: true }).click();
  await expect(exporter.getByText('Frozen database snapshot verified', { exact: true })).toBeVisible();
  const event = page.waitForEvent('download'); await exporter.getByRole('button', { name: 'Download frozen JSON', exact: true }).click();
  const download = await event; const bundle = JSON.parse(readFileSync((await download.path())!, 'utf8'));
  expect(bundle.schema_version).toBe('1.1.0'); expect(bundle.snapshot.source_counts.cost_assessments).toBe('1');
  expect(bundle.data.cost_assessments[0].assessment.report.transaction_net).toBe((-10n - BigInt(huge)).toString());
  expect(bundle.data.decisions[0].trace.result.gross_delta_minor).toBe('-10');
});

test('uncertain cost delivery locks scope through navigation and retries the identical request after throttling', async ({ page }) => {
  const requests: { path: string; key: string; body: CostAssessmentRequest }[] = [];
  await setup(page, async (route, url) => {
    if (!url.pathname.endsWith('/cost-assessments') || route.request().method() !== 'POST') return false;
    requests.push({ path: url.pathname, key: route.request().headers()['idempotency-key'], body: route.request().postDataJSON() });
    expect(route.request().headers()['x-csrf-token']).toBe('cost-test-csrf');
    if (requests.length < 3) await route.fulfill({ status: requests.length === 1 ? 504 : 429, json: { code: 'REQUEST_TIMEOUT', message: 'Fixture uncertain delivery' } });
    else await route.fulfill({ status: 201, json: costAssessmentFixture(requests[0].body.scenario) }); return true;
  });
  await basic(page); await reviewed(page); await page.getByRole('button', { name: 'Save hypothetical cost assessment', exact: true }).click();
  await expect(workspace(page).getByRole('alert')).toContainText('delivery is uncertain');
  await expect(page.getByLabel('Decision session', { exact: true })).toBeDisabled(); await expect(page.getByRole('button', { name: 'Real trading', exact: true })).toBeDisabled();
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Runs', exact: true }).click();
  await page.getByLabel('View chain', { exact: true }).selectOption('solana-mainnet');
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Opportunities', exact: true }).click();
  await expect(page.getByLabel('Decision session', { exact: true })).toHaveValue('session-paper');
  await page.getByRole('button', { name: 'Retry identical cost assessment', exact: true }).click();
  await expect(workspace(page).getByRole('alert')).toContainText('delivery is uncertain');
  await page.getByRole('button', { name: 'Retry identical cost assessment', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Cost assessment saved', exact: true })).toBeDisabled();
  expect(requests).toHaveLength(3); expect(requests[0]).toEqual(requests[1]); expect(requests[1]).toEqual(requests[2]);
  await expect(page.getByLabel('Decision session', { exact: true })).toBeEnabled(); await expect(page.getByRole('button', { name: 'Real trading', exact: true })).toBeEnabled();
});

test('cost history rejects cross-session data and retains the prior received snapshot during outage', async ({ page }) => {
  let phase = 0;
  await setup(page, async (route, url) => {
    if (!url.pathname.endsWith('/cost-assessments')) return false;
    const fixture = costAssessmentFixture();
    if (phase === 1) fixture.assessment.binding.session_id = 'other-session';
    await route.fulfill(phase === 2 ? { status: 503, json: { code: 'UNAVAILABLE', message: 'Fixture outage' } } : { json: { items: [fixture], next_cursor: null } }); return true;
  });
  const panel = workspace(page); await expect(panel.getByText('manual-fixture · v1 · 2026-09-12T12:03:00Z', { exact: true })).toBeVisible();
  phase = 1; await panel.getByRole('button', { name: 'Refresh cost history', exact: true }).click();
  await expect(panel.getByText('Stale snapshot retained.', { exact: true })).toBeVisible();
  await expect(panel.getByText('other-session', { exact: true })).toHaveCount(0);
  phase = 2; await panel.getByRole('button', { name: 'Refresh cost history', exact: true }).click();
  await expect(panel.getByText('Stale snapshot retained.', { exact: true })).toBeVisible();
  await page.getByLabel('Decision session', { exact: true }).selectOption(''); await expect(panel).toHaveCount(0);
});

for (const width of [320, 390, 1440]) {
  test('manual cost form and retained results remain usable at ' + width + 'px', async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await setup(page, async (route, url) => {
      if (!url.pathname.endsWith('/cost-assessments') || route.request().method() !== 'POST') return false;
      await route.fulfill({ status: 201, json: costAssessmentFixture((route.request().postDataJSON() as CostAssessmentRequest).scenario) }); return true;
    });
    await complete(page); await reviewed(page); await page.getByRole('button', { name: 'Save hypothetical cost assessment', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Cost assessment saved', exact: true })).toBeDisabled();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
    const panel = workspace(page);
    for (const name of ['Cost assessment saved', 'Prepare another cost scenario', 'Refresh cost history']) {
      const box = await panel.getByRole('button', { name, exact: true }).boundingBox(); expect(box!.width).toBeGreaterThanOrEqual(44); expect(box!.height).toBeGreaterThanOrEqual(44);
    }
    await testInfo.attach('cost-assessment-' + width, { body: await page.screenshot({ path: testInfo.outputPath('cost-assessment-' + width + '.png'), fullPage: true, scale: 'css' }), contentType: 'image/png' });
  });
}
