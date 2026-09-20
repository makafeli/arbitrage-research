import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { coverage, decision, journal, nativeAsset, paperRun, principalAsset, reservation } from '../../../apps/web/tests/research.fixture';
import { collectionCoverage, frozenExportFixture } from '../../../apps/web/tests/exports.fixture';
import { adapterSupportFixture } from '../../../apps/web/tests/support.fixture';
import { costAssessmentFixture } from '../../../apps/web/tests/costs.fixture';

// Synthetic stub data for redesign screenshots only. Never market evidence.
const ROOT = resolve(__dirname, '../../..');
const OUT = resolve(__dirname, '../screenshots') + '/';
const opportunity = { ...JSON.parse(readFileSync(resolve(ROOT, 'specs/opportunity.example.json'), 'utf8')), source_kind: 'CAPTURED_MARKET_DATA', opportunity_id: 'record-1', reason_codes: ['NETWORK_COST_UNKNOWN'], simulation_status: 'FAILED' };
const paper = { session_id: 'session-paper', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'STOPPED', health: 'UNKNOWN', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:paper-fixture-config' };
const running = { ...paper, session_id: 'session-base-running', mode: 'OBSERVE', observed_state: 'RUNNING', health: 'HEALTHY', desired_revision: '2', applied_revision: '1', last_heartbeat_at: '2026-09-19T09:00:00Z' };
const draining = { ...paper, session_id: 'session-solana-draining', network_id: 'solana-mainnet', observed_state: 'DRAINING', health: 'DEGRADED', outstanding_attempts: 2, last_heartbeat_at: '2026-09-19T08:55:00Z' };
const capabilities = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, decision_history: true, paper_ledger: true, paper_run_creation: true, collection_telemetry: true, session_export: true, cost_assessments: true, adapter_support: true,
  registered_configurations: [{ configuration_digest: paper.configuration_digest, mode: 'PAPER', enabled_networks: ['base-mainnet'], strategy_ids: ['usdc-cycle'], paper_assets: [{ network_id: 'base-mainnet', asset: principalAsset }, { network_id: 'base-mainnet', asset: nativeAsset }] }, { configuration_digest: 'sha256:observe-fixture-config', mode: 'OBSERVE', enabled_networks: ['base-mainnet', 'solana-mainnet'], strategy_ids: ['usdc-cycle', 'weth-cycle'] }] };
const auth = { operator_id: 'operator', csrf_token: 'csrf', expires_at: '2000000000' };
const records = [decision(), { ...decision('RECORDED_LIVE', 'observation-rejected'), trace: { ...decision('RECORDED_LIVE', 'observation-rejected').trace, result: { status: 'REJECTED', reason_codes: ['STALE_INPUT'] } } }];
const group = { grouping_version: 'group-v1', grouping_key: 'group-SYNTHETIC', window_start_ms: 1789214400000, dataset_origin: 'SYNTHETIC', source_kind: 'SYNTHETIC_FIXTURE', raw_observations: '1', quoted_candidates: '1', rejected: '0', no_route: '0', data_unavailable: '0' };
const continuity = { schema_version: '1.0.0', assessment_kind: 'CONTINUITY_ONLY', authorizes_execution: false, trace_id: records[0].trace_id, session_id: records[0].trace.session_id, observation_id: records[0].trace.observation_id, network_id: records[0].trace.network_id, checked_at: '2026-09-17T08:00:00.000Z', policy_version: 'base-capture-continuity-v1', continuity_status: 'NO_KNOWN_INVALIDATION', capture_count: String(records[0].trace.capture_refs.length), bound_count: String(records[0].trace.capture_refs.length), invalidation_reasons: [] };

async function stub(page: Page, signedIn = true) {
  await page.route('**/v1/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname;
    let data: unknown = null;
    if (path === '/v1/auth/session') { if (!signedIn) { await route.fulfill({ status: 401, json: { code: 'UNAUTHENTICATED', message: 'No session' } }); return; } data = auth; }
    if (path === '/v1/capabilities') data = capabilities;
    if (path === '/v1/sessions') data = { items: [paper, running, draining], next_cursor: null };
    if (path === '/v1/opportunities') data = { items: [opportunity], next_cursor: null };
    if (path === '/v1/decisions') data = { items: records, next_cursor: null };
    if (path.startsWith('/v1/decisions/')) data = records.find(item => item.trace.observation_id === decodeURIComponent(path.split('/').at(-1)!));
    if (path.endsWith('/continuity')) data = continuity;
    if (path === '/v1/decision-coverage') data = coverage();
    if (path === '/v1/decision-groups') data = { items: [group], next_cursor: null };
    if (path === '/v1/sessions/session-paper/collection-coverage') data = collectionCoverage();
    if (path === '/v1/sessions/session-paper/collection-attempts') data = { items: frozenExportFixture().data.collection_attempts, next_cursor: null };
    if (path === '/v1/sessions/session-paper/export') data = frozenExportFixture();
    if (path === '/v1/sessions/session-paper/cost-assessments') data = { items: [costAssessmentFixture()], next_cursor: null };
    if (path === '/v1/sessions/session-paper/paper-runs') data = { items: [paperRun()], next_cursor: null };
    if (path === '/v1/paper-runs/run-original') data = paperRun();
    if (path === '/v1/paper-runs/run-original/journal') data = { items: [journal()], next_cursor: null };
    if (path === '/v1/paper-runs/run-original/reservations') data = { items: [reservation], next_cursor: null };
    if (path === '/v1/adapter-support') data = adapterSupportFixture();
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'stub' } });
  });
  await page.goto('/');
  if (signedIn) await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
}
const nav = (page: Page, name: string) => page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name, exact: true }).click();
const tab = (page: Page, name: string) => page.getByRole('tab', { name, exact: true }).click();
const shot = (page: Page, name: string) => page.screenshot({ path: OUT + name + '.png', fullPage: true });

for (const [tag, viewport] of [['desktop', { width: 1440, height: 1000 }], ['mobile', { width: 390, height: 844 }]] as const) {
  test.describe(tag, () => {
    test.use({ viewport });
    test('sign in', async ({ page }) => { await stub(page, false); await shot(page, `00-signin-${tag}`); await page.getByRole('button', { name: 'First time here or forgot your password?' }).click(); await shot(page, `00-signin-help-${tag}`); });
    test('overview', async ({ page }) => {
      // Light is the v3 default theme; dark is the toggled state (inverted from the pre-v3 shell).
      await stub(page); await shot(page, `01-overview-${tag}`);
      await page.getByRole('button', { name: 'Dark theme' }).click(); await shot(page, `01-overview-dark-${tag}`);
    });
    test('record dialog', async ({ page }) => { await stub(page); await page.getByRole('button', { name: 'Inspect record-1' }).click(); await page.screenshot({ path: OUT + `01b-record-dialog-${tag}.png` }); });
    test('opportunities', async ({ page }) => {
      await stub(page); await nav(page, 'Opportunities');
      // Opportunities sections: Captured / Decisions (default) / Costs / Exports.
      await page.getByLabel('Decision session', { exact: true }).selectOption('session-paper');
      await expect(page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true })).toBeVisible();
      await shot(page, `02b-opp-decisions-${tag}`);
      await tab(page, 'Captured');
      await shot(page, `02a-opp-captured-${tag}`);
      await tab(page, 'Decisions');
      await page.getByRole('button', { name: 'Assess hypothetical costs for observation-fixture' }).click();
      await expect(page.getByText('Hypothetical cost research')).toBeVisible();
      await shot(page, `02c-opp-costs-${tag}`);
      await tab(page, 'Exports');
      await page.getByRole('button', { name: 'Prepare frozen session export' }).click();
      await expect(page.getByText('Frozen database snapshot verified')).toBeVisible();
      await shot(page, `02d-opp-exports-${tag}`);
      await tab(page, 'Decisions');
      await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
      await expect(page.getByText('Decision evidence detail')).toBeVisible();
      await page.waitForTimeout(500);
      await page.screenshot({ path: OUT + `02e-decision-drawer-${tag}.png` });
    });
    test('experiments', async ({ page }) => {
      await stub(page); await nav(page, 'Experiments');
      await page.getByLabel('Validated configuration', { exact: true }).selectOption(paper.configuration_digest);
      await page.getByLabel('Session network', { exact: true }).selectOption('base-mainnet');
      await page.getByLabel('Experiment reference', { exact: true }).fill('redesign-example');
      await shot(page, `03-experiments-${tag}`);
    });
    test('runs', async ({ page }) => {
      await stub(page); await nav(page, 'Runs');
      // Runs sections: Sessions (default) / Ledger / Journal / Reservations.
      await page.getByLabel('Paper session', { exact: true }).selectOption('session-paper');
      await expect(page.getByRole('button', { name: 'Inspect paper run run-original' })).toBeVisible();
      await shot(page, `04a-runs-sessions-${tag}`);
      await page.getByRole('button', { name: 'Inspect paper run run-original' }).click();
      await expect(page.getByText('Hypothetical balances')).toBeVisible();
      await shot(page, `04b-runs-ledger-${tag}`);
      await tab(page, 'Journal');
      await expect(page.getByRole('heading', { name: 'Paper journal' })).toBeVisible();
      await shot(page, `04c-runs-journal-${tag}`);
      await tab(page, 'Reservations');
      await expect(page.getByText('Reservation history')).toBeVisible();
      await shot(page, `04d-runs-reservations-${tag}`);
    });
    test('strategies', async ({ page }) => { await stub(page); await nav(page, 'Strategies'); await shot(page, `05-strategies-${tag}`); });
    test('owner overview', async ({ page }) => {
      await stub(page); await nav(page, 'Owner overview');
      await page.getByLabel('Sessie', { exact: true }).selectOption('session-paper');
      await expect(page.getByRole('heading', { name: '3. Bevindingen' })).toBeVisible();
      await page.waitForTimeout(500);
      await shot(page, `09-owner-overview-${tag}`);
    });
    test('system', async ({ page }) => {
      await stub(page); await nav(page, 'System');
      // System sections: Adapters (default) / Collection / Capabilities.
      await expect(page.getByRole('heading', { name: 'Adapter support catalog' })).toBeVisible();
      await shot(page, `06a-system-adapters-${tag}`);
      await tab(page, 'Collection');
      await page.getByLabel('Collection health session', { exact: true }).selectOption('session-paper');
      await expect(page.getByText('Batch outcomes · all recorded attempts')).toBeVisible();
      await page.waitForTimeout(500);
      await shot(page, `06b-system-collection-${tag}`);
      await tab(page, 'Capabilities');
      await expect(page.getByText('Capability gates')).toBeVisible();
      await shot(page, `06c-system-capabilities-${tag}`);
    });
    test('real trading + password', async ({ page }) => {
      await stub(page);
      await page.getByRole('button', { name: 'Real trading' }).click(); await shot(page, `07-real-trading-${tag}`);
      await page.getByRole('button', { name: 'Paper trading' }).click();
      await page.getByRole('button', { name: 'Change password' }).click(); await page.screenshot({ path: OUT + `08-change-password-${tag}.png` });
    });
  });
}
