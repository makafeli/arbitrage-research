import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { coverage, decision, huge, journal, nativeAsset, paperRun, principalAsset, reservation } from '../research.fixture';
import { collectionCoverage, frozenExportFixture } from '../exports.fixture';

// HTTP fixtures validate view behavior; they are never production market data.
const session = { session_id: 'session-paper', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'STOPPED', health: 'UNKNOWN', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:paper-fixture-config' };
const capabilities = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, decision_history: true, paper_ledger: true, paper_run_creation: true, collection_telemetry: true, session_export: true,
  registered_configurations: [{ configuration_digest: session.configuration_digest, mode: 'PAPER', enabled_networks: ['base-mainnet'], strategy_ids: ['usdc-cycle'], paper_assets: [{ network_id: 'base-mainnet', asset: principalAsset }, { network_id: 'base-mainnet', asset: nativeAsset }] }] };
const auth = { operator_id: 'operator', csrf_token: 'research-test-csrf', expires_at: '2000000000' };
const records = [decision(), { ...decision('RECORDED_LIVE', 'observation-rejected'), trace: { ...decision('RECORDED_LIVE', 'observation-rejected').trace, result: { status: 'REJECTED', reason_codes: ['STALE_INPUT'] } } }, { ...decision('MANUALLY_CONSTRUCTED', 'observation-unavailable'), trace: { ...decision('MANUALLY_CONSTRUCTED', 'observation-unavailable').trace, amount_in_minor: null, input_age_ms: null, route: [], result: { status: 'DATA_UNAVAILABLE', reason_codes: ['NO_CAPTURE_INPUTS'] } } }];
const group = { grouping_version: 'group-v1', grouping_key: 'group-SYNTHETIC', window_start_ms: 1789214400000, dataset_origin: 'SYNTHETIC', source_kind: 'SYNTHETIC_FIXTURE', raw_observations: '1', quoted_candidates: '1', rejected: '0', no_route: '0', data_unavailable: '0' };
async function stub(page: Page, override?: (route: Route, url: URL) => Promise<boolean>) {
  await page.route('**/v1/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname;
    if (override && await override(route, url)) return;
    let data: unknown = null;
    if (path === '/v1/auth/session') data = auth;
    if (path === '/v1/capabilities') data = capabilities;
    if (path === '/v1/sessions') data = { items: [session], next_cursor: null };
    if (path === '/v1/opportunities') { expect(url.searchParams.get('source_kind')).toBe('CAPTURED_MARKET_DATA'); data = { items: [], next_cursor: null }; }
    if (path === '/v1/decisions') { expect(url.searchParams.get('session_id')).toBe('session-paper'); expect(url.searchParams.get('limit')).toBe('25'); data = { items: records, next_cursor: null }; }
    if (path.startsWith('/v1/decisions/')) data = records.find(item => item.trace.observation_id === path.split('/').at(-1));
    if (path === '/v1/decision-coverage') data = coverage();
    if (path === '/v1/decision-groups') data = { items: [group], next_cursor: null };
    if (path === '/v1/sessions/session-paper/collection-coverage') data = collectionCoverage();
    if (path === '/v1/sessions/session-paper/collection-attempts') data = { items: frozenExportFixture().data.collection_attempts, next_cursor: null };
    if (path === '/v1/sessions/session-paper/export') data = frozenExportFixture();
    if (path === '/v1/sessions/session-paper/paper-runs') data = { items: [paperRun()], next_cursor: null };
    if (path === '/v1/paper-runs/run-original') data = paperRun();
    if (path === '/v1/paper-runs/run-original/journal') data = { items: [journal()], next_cursor: null };
    if (path === '/v1/paper-runs/run-original/reservations') data = { items: [reservation], next_cursor: null };
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Unknown contract fixture endpoint' } });
  });
  await page.goto('/');

  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
}
async function openDecisions(page: Page) {
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Opportunities', exact: true }).click();
  await page.getByLabel('Decision session', { exact: true }).selectOption('session-paper');
  await expect(page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true })).toBeVisible();
}
async function openPaper(page: Page) {
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Runs', exact: true }).click();
  await page.getByLabel('Paper session', { exact: true }).selectOption('session-paper');
  await expect(page.getByRole('button', { name: 'Inspect paper run run-original', exact: true })).toBeVisible();
}
test('decision origins, raw counts, grouped candidates and unknown execution accounting stay distinct', async ({ page }) => {
  await stub(page); await openDecisions(page);
  const counts = page.getByRole('region', { name: 'Session evidence counts' });
  await expect(counts.getByText('Raw observations', { exact: true })).toBeVisible();
  await expect(counts.getByText('Unknown', { exact: true })).toHaveCount(2);
  await expect(page.getByText('CANDIDATE · GROSS QUOTE', { exact: true })).toBeVisible();
  await expect(page.getByText('STALE_INPUT', { exact: true })).toBeVisible();
  await expect(page.getByText('NO_CAPTURE_INPUTS', { exact: true })).toBeVisible();
  await expect(page.getByText('-10', { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Captured', exact: true }).click();
  await expect(page.getByText('No captured quote records on this page', { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Decisions', exact: true }).click();
  await page.getByLabel('Origin on this page', { exact: true }).selectOption('RECORDED_LIVE');
  await expect(page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Inspect decision observation-rejected', exact: true })).toBeVisible();
  await expect(counts.getByText('3', { exact: true })).toBeVisible();
  await expect(page.getByText(/Aggregate counts include all origins in this session/)).toBeVisible();
});
test('trace inspector exports only the selected persisted wrapper and restores keyboard focus', async ({ page }) => {
  await stub(page); await openDecisions(page);
  const inspect = page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true });
  await inspect.click();
  const modal = page.getByRole('dialog', { name: 'Decision evidence detail' });
  await expect(modal.getByText('sha256:manifest-fixture', { exact: true }).first()).toBeVisible();
  await expect(modal.getByText('Unknown — external costs incomplete', { exact: true })).toBeVisible();
  await expect(modal.getByText('Unknown — references do not prove raw artifacts remain available', { exact: true })).toBeVisible();
  await expect(modal.getByText('15 ms', { exact: true })).toBeVisible();
  const downloadEvent = page.waitForEvent('download');
  await modal.getByRole('button', { name: 'Export selected trace JSON', exact: true }).click();
  const download = await downloadEvent, path = await download.path(); expect(path).not.toBeNull();
  const exported = JSON.parse(readFileSync(path!, 'utf8'));
  expect(exported.data.trace_id).toBe('trace-observation-fixture');
  expect(exported.data.trace.dataset_origin).toBe('SYNTHETIC');
  expect(exported.data.trace.result.gross_delta_minor).toBe('-10');
  expect(exported.data.trace.input_age_ms).toBe(15);
  await modal.getByRole('button', { name: 'Close evidence detail' }).focus();
  await page.keyboard.press('Shift+Tab');
  await expect(modal.getByRole('button', { name: 'Refresh selected trace' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(modal.getByRole('button', { name: 'Close evidence detail' })).toBeFocused();
  await page.keyboard.press('Escape'); await expect(inspect).toBeFocused();
});
test('bounded decision pagination retains the same page on outage without synthetic fallback', async ({ page }) => {
  let outage = false;
  await stub(page, async (route, url) => {
    if (url.pathname !== '/v1/decisions') return false;
    if (outage) { await route.fulfill({ status: 503, json: { code: 'UNAVAILABLE', message: 'Fixture outage' } }); return true; }
    const next = url.searchParams.has('cursor');
    await route.fulfill({ json: { items: [decision('SYNTHETIC', next ? 'observation-next' : 'observation-fixture')], next_cursor: next ? null : 'page-two' } }); return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Next observations', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Inspect decision observation-next', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true })).toHaveCount(0);
  outage = true; await page.getByRole('button', { name: 'Refresh evidence', exact: true }).click();
  await expect(page.getByText('Stale snapshot retained.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect decision observation-next', exact: true })).toBeVisible();
  await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toHaveCount(0);
});
test('paper balances preserve huge units and distinguish original budgets from reserved inventory', async ({ page }) => {
  await stub(page, async (route, url) => {
    if (url.pathname !== '/v1/paper-runs/run-original/journal') return false;
    const next = url.searchParams.has('cursor');
    await route.fulfill({ json: { items: [{ ...journal(), event_id: next ? 'journal-second' : 'journal-fixture', event: { ...journal().event, sequence: next ? '9007199254740994' : '9007199254740993' } }], next_cursor: next ? null : 'journal-two' } }); return true;
  });
  await openPaper(page);
  await page.getByRole('button', { name: 'Inspect paper run run-original', exact: true }).click();
  const balances = page.getByRole('table', { name: 'Current free, reserved and total hypothetical inventory' });
  await expect(balances.getByText(huge, { exact: true })).toBeVisible();
  await expect(balances.getByText('9', { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Reservations', exact: true }).click();
  await expect(page.getByText('Original native fee budget', { exact: true })).toBeVisible();
  await expect(page.getByRole('table', { name: 'Original reservation requests and reconciliation states' }).getByText('UNKNOWN', { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Ledger', exact: true }).click();
  await expect(page.getByText(/Run comparisons are unavailable/)).toBeVisible();
  await page.getByRole('tab', { name: 'Journal', exact: true }).click();
  await page.getByRole('button', { name: 'Next journal entries', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Journal event 9007199254740994' })).toBeVisible();
  const downloadEvent = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export journal page JSON', exact: true }).click();
  const download = await downloadEvent, path = await download.path(); expect(path).not.toBeNull();
  const exported = JSON.parse(readFileSync(path!, 'utf8'));
  expect(exported.data.items).toHaveLength(1);
  expect(exported.data.items[0].event.sequence).toBe('9007199254740994');
  expect(exported.data.items[0].event.command).toEqual({ kind: 'INITIALIZE' });
  expect(exported.data.items[0].event.postings[0].amount).toBe(huge);
  await expect(page.getByRole('button', { name: /settle|reset ledger/i })).toHaveCount(0);
});
test('an unsupported token or pool cannot be selected for paper creation', async ({ page }) => {
  await stub(page); await openPaper(page);
  const tokenSelect = page.getByLabel('Validated principal asset', { exact: true });
  await expect(tokenSelect.locator('option')).toHaveCount(2);
  await expect(tokenSelect.locator('option', { hasText: principalAsset.identity })).toHaveCount(1);
  await expect(tokenSelect.locator('option', { hasText: 'unsupported-pool' })).toHaveCount(0);
});
test('uncertain paper creation locks immutable scope and retries exact request after navigation', async ({ page }) => {
  const requests: { key: string; body: { initial_balances: { asset: typeof principalAsset | typeof nativeAsset; amount: string }[] }; path: string }[] = [];
  let created: ReturnType<typeof paperRun> | null = null;
  await stub(page, async (route, url) => {
    if (url.pathname === '/v1/sessions' && requests.length === 1) { await route.fulfill({ json: { items: [], next_cursor: null } }); return true; }
    if (url.pathname === '/v1/sessions/session-paper/paper-runs' && route.request().method() === 'POST') {
      expect(route.request().headers()['x-csrf-token']).toBe('research-test-csrf');
      requests.push({ key: route.request().headers()['idempotency-key'], body: route.request().postDataJSON(), path: url.pathname });
      if (requests.length === 1) { await route.fulfill({ status: 504, json: { code: 'REQUEST_TIMEOUT', message: 'Outcome uncertain' } }); return true; }
      if (requests.length === 2) { await route.fulfill({ status: 429, json: { code: 'RATE_LIMITED', message: 'Retry later' } }); return true; }
      const balances = requests[0].body.initial_balances;
      created = { ...paperRun(), run_id: 'run-created', revision: '0', outstanding_reservations: 0, initial_balances: balances, balances: balances.map(value => ({ asset: value.asset, free: value.amount, reserved: '0', total: value.amount })) };
      await route.fulfill({ status: 201, json: created }); return true;
    }
    if (url.pathname === '/v1/sessions/session-paper/paper-runs' && created) { await route.fulfill({ json: { items: [created, paperRun()], next_cursor: null } }); return true; }
    if (url.pathname === '/v1/paper-runs/run-created' && created) { await route.fulfill({ json: created }); return true; }
    if (url.pathname.startsWith('/v1/paper-runs/run-created/')) { await route.fulfill({ json: { items: [], next_cursor: null } }); return true; }
    return false;
  });
  await openPaper(page);
  await expect(page.getByRole('button', { name: 'Create hypothetical paper run', exact: true })).toBeDisabled();
  await page.getByLabel('Validated principal asset', { exact: true }).selectOption(principalAsset.identity);
  await page.getByLabel('Initial token principal · exact minor units', { exact: true }).fill(huge);
  await page.getByLabel('Initial native fee reserve · exact minor units', { exact: true }).fill('0');
  await page.getByRole('checkbox', { name: 'I reviewed these hypothetical amounts and the frozen configuration.' }).check();
  await page.getByRole('button', { name: 'Create hypothetical paper run', exact: true }).click();
  await expect(page.getByText(/Creation delivery is uncertain/)).toBeVisible();
  await expect(page.getByLabel('Paper session', { exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Real trading', exact: true })).toBeDisabled();
  await expect(page.getByLabel('Initial token principal · exact minor units', { exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(page.getByText('No sessions in this view', { exact: true })).toBeVisible();
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Opportunities', exact: true }).click();
  await page.getByLabel('View chain', { exact: true }).selectOption('solana-mainnet');
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'Runs', exact: true }).click();
  await expect(page.getByLabel('Paper session', { exact: true })).toHaveValue('session-paper');
  await page.getByRole('button', { name: 'Retry same paper creation', exact: true }).click();
  await expect(page.getByText(/Creation delivery is uncertain/)).toBeVisible();
  await expect(page.getByLabel('Paper session', { exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Retry same paper creation', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Paper run created', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Real trading', exact: true })).toBeEnabled();
  expect(requests).toHaveLength(3); expect(requests[0]).toEqual(requests[1]); expect(requests[1]).toEqual(requests[2]);
  await expect(page.getByRole('button', { name: 'Inspect paper run run-original', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Inspect paper run run-created', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Prepare another new run', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Create hypothetical paper run', exact: true })).toBeDisabled();
  expect(requests).toHaveLength(3);
});
for (const width of [320, 390, 1440]) {
  test('persisted research views remain usable at ' + width + 'px', async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await stub(page); await openDecisions(page);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
    await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
    await expect(page.getByRole('dialog').getByRole('button', { name: 'Close evidence detail' })).toBeVisible();
    await page.keyboard.press('Escape');
    await openPaper(page); await page.getByRole('button', { name: 'Inspect paper run run-original', exact: true }).click();
    await expect(page.getByRole('table', { name: 'Current free, reserved and total hypothetical inventory' })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
    for (const name of ['Refresh paper records', 'Export selected run JSON']) {
      const bounds = await page.getByRole('button', { name, exact: true }).boundingBox();
      expect(bounds!.width).toBeGreaterThanOrEqual(44); expect(bounds!.height).toBeGreaterThanOrEqual(44);
    }
    await page.getByRole('tab', { name: 'Sessions', exact: true }).click();
    {
      const bounds = await page.getByRole('button', { name: 'Create hypothetical paper run', exact: true }).boundingBox();
      expect(bounds!.width).toBeGreaterThanOrEqual(44); expect(bounds!.height).toBeGreaterThanOrEqual(44);
    }
    await testInfo.attach('paper-evidence-' + width, { body: await page.screenshot({ path: testInfo.outputPath('research-dashboard-' + width + '.png'), fullPage: true, scale: 'css' }), contentType: 'image/png' });
  });
}

test('stopped paper sessions with a rejected command and revision gap can request initialization', async ({ page }) => {
  let creationRequests = 0;
  await stub(page, async (route, url) => {
    if (url.pathname === '/v1/sessions') {
      await route.fulfill({ json: { items: [{ ...session, desired_revision: '2', applied_revision: '1' }], next_cursor: null } }); return true;
    }
    if (url.pathname === '/v1/sessions/session-paper/commands') {
      await route.fulfill({ status: 409, json: { code: 'REVISION_CONFLICT', message: 'Command was rejected' } }); return true;
    }
    if (url.pathname === '/v1/sessions/session-paper/paper-runs' && route.request().method() === 'POST') {
      creationRequests += 1;
      const body = route.request().postDataJSON() as { initial_balances: ReturnType<typeof paperRun>['initial_balances'] };
      await route.fulfill({ status: 201, json: { ...paperRun(), initial_balances: body.initial_balances, balances: body.initial_balances.map(value => ({ asset: value.asset, free: value.amount, reserved: '0', total: value.amount })), outstanding_reservations: 0 } }); return true;
    }
    return false;
  });
  const card = page.getByRole('region', { name: 'Session session-paper' });
  await card.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(card.getByText('Command rejected', { exact: true })).toBeVisible();
  await expect(card.getByRole('button', { name: 'Start', exact: true })).toBeEnabled();
  await expect(card.getByText('Worker acknowledgement pending.', { exact: false })).toHaveCount(0);
  await openPaper(page);
  await page.getByLabel('Validated principal asset', { exact: true }).selectOption(principalAsset.identity);
  await page.getByLabel('Initial token principal · exact minor units', { exact: true }).fill('25');
  await page.getByLabel('Initial native fee reserve · exact minor units', { exact: true }).fill('0');
  await page.getByRole('checkbox', { name: 'I reviewed these hypothetical amounts and the frozen configuration.' }).check();
  await expect(page.getByRole('button', { name: 'Create hypothetical paper run', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Create hypothetical paper run', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Paper run created', exact: true })).toBeDisabled();
  expect(creationRequests).toBe(1);
  const creation = page.getByRole('region', { name: 'Initialize a new hypothetical run' });
  await expect(creation.getByLabel('Validated principal asset', { exact: true })).toBeDisabled();
  await expect(creation.getByLabel('Initial token principal · exact minor units', { exact: true })).toBeDisabled();
  await expect(creation.getByLabel('Initial native fee reserve · exact minor units', { exact: true })).toBeDisabled();
  await creation.locator('form').evaluate(form => (form as HTMLFormElement).requestSubmit());
  expect(creationRequests).toBe(1);
});

test('JSON and CSV downloads reuse one verified frozen snapshot while live pages remain independent', async ({ page }) => {
  let exports = 0;
  await stub(page, async (route, url) => {
    if (url.pathname !== '/v1/sessions/session-paper/export') return false;
    exports++; await route.fulfill({ json: frozenExportFixture() }); return true;
  });
  await openDecisions(page);
  await page.getByRole('tab', { name: 'Exports', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Frozen session export', exact: true });
  await expect(panel.getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeDisabled();
  await panel.getByRole('button', { name: 'Prepare frozen session export', exact: true }).click();
  await expect(panel.getByText('Frozen database snapshot verified', { exact: true })).toBeVisible();
  await expect(panel.getByText(/Scheduled collection completeness: UNKNOWN/)).toBeVisible();
  await panel.getByText('Capture dependency availability:', { exact: false }).click();
  await expect(panel.getByText(/This bundle alone cannot reproduce capture-based calculations/)).toBeVisible();
  const jsonEvent = page.waitForEvent('download'); await panel.getByRole('button', { name: 'Download frozen JSON', exact: true }).click();
  const jsonDownload = await jsonEvent, json = JSON.parse(readFileSync((await jsonDownload.path())!, 'utf8'));
  await page.getByRole('button', { name: 'Refresh evidence', exact: true }).click();
  const csvEvent = page.waitForEvent('download'); await panel.getByRole('button', { name: 'Download frozen CSV', exact: true }).click();
  const csvDownload = await csvEvent, csv = readFileSync((await csvDownload.path())!, 'utf8');
  expect(json.export_id).toBe('export-fixture-one'); expect(json.snapshot.source_counts.decisions).toBe('1');
  expect(json.data.paper_runs[0].run.balances[0].total).toBe(huge);
  expect(json.data.paper_runs[0].journal[0].event.command.balances[0].amount).toBe(huge);
  expect(csv).toContain(json.content_sha256); expect(csv).toContain(huge); expect(csv).toContain('"PAPER_JOURNAL_EVENT"');
  expect(exports).toBe(1); expect(csvDownload.suggestedFilename()).toBe(jsonDownload.suggestedFilename().replace('.json', '.csv'));
});

test('over-limit or tampered refresh keeps the earlier frozen bundle explicitly available', async ({ page }) => {
  let calls = 0;
  await stub(page, async (route, url) => {
    if (url.pathname !== '/v1/sessions/session-paper/export') return false;
    calls++;
    if (calls === 2) await route.fulfill({ status: 413, json: { code: 'EXPORT_LIMIT_EXCEEDED', message: 'Too large' } });
    else if (calls === 3) await route.fulfill({ json: { ...frozenExportFixture(), content_sha256: 'sha256:' + 'b'.repeat(64) } });
    else await route.fulfill({ json: frozenExportFixture() });
    return true;
  });
  await openDecisions(page);
  await page.getByRole('tab', { name: 'Exports', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Frozen session export', exact: true });
  await panel.getByRole('button', { name: 'Prepare frozen session export', exact: true }).click();
  await expect(panel.getByText('Frozen database snapshot verified', { exact: true })).toBeVisible();
  await panel.getByRole('button', { name: 'Prepare a new frozen snapshot', exact: true }).click();
  await expect(panel.getByRole('alert')).toContainText('No partial export was prepared');
  await expect(panel.getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeEnabled();
  await panel.getByRole('button', { name: 'Prepare a new frozen snapshot', exact: true }).click();
  await expect(panel.getByRole('alert')).toContainText('content digest could not be verified');
  await expect(panel.getByText('export-fixture-one', { exact: false })).toBeVisible();
});

test('System distinguishes provider failures, control suppression and interrupted work without claiming schedule coverage', async ({ page }) => {
  let outage = false;
  await stub(page, async (route, url) => {
    if (!outage || !url.pathname.endsWith('/collection-attempts')) return false;
    await route.fulfill({ status: 503, json: { code: 'UNAVAILABLE', message: 'Fixture outage' } }); return true;
  });
  await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'System', exact: true }).click();
  await page.getByRole('tab', { name: 'Collection', exact: true }).click();
  await page.getByLabel('Collection health session', { exact: true }).selectOption('session-paper');
  const counts = page.getByRole('region', { name: 'Recorded collection attempt counts', exact: true });
  await expect(counts.getByText('3', { exact: true }).first()).toBeVisible();
  await expect(counts.getByText(/Scheduled collection completeness: UNKNOWN/)).toBeVisible();
  const attempts = page.getByRole('table', { name: 'Recorded batch attempts and suggested checks', exact: true });
  await expect(attempts.getByText('PROVIDER_UNAVAILABLE', { exact: true })).toBeVisible();
  await expect(attempts.getByText('GENERATION_FENCED', { exact: true })).toBeVisible();
  await expect(attempts.getByText('NO TERMINAL OUTCOME', { exact: true })).toBeVisible();
  await expect(attempts.getByText(/may still be running or may have been interrupted/)).toBeVisible();
  await expect(page.getByText(/Attempt origin is not retained/)).toBeVisible();
  outage = true; await page.getByRole('button', { name: 'Refresh collection health', exact: true }).click();
  await expect(page.getByText('Stale snapshot retained.', { exact: true })).toBeVisible();
  await expect(attempts.getByText('PROVIDER_UNAVAILABLE', { exact: true })).toBeVisible();
});

for (const width of [320, 390, 1440]) {
  test('frozen exports and collection diagnostics remain contained at ' + width + 'px', async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 }); await stub(page); await openDecisions(page);
    await page.getByRole('tab', { name: 'Exports', exact: true }).click();
    const panel = page.getByRole('region', { name: 'Frozen session export', exact: true });
    await panel.getByRole('button', { name: 'Prepare frozen session export', exact: true }).click();
    await expect(panel.getByText('Frozen database snapshot verified', { exact: true })).toBeVisible();
    await panel.getByText('Capture dependency availability:', { exact: false }).click();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
    for (const name of ['Prepare a new frozen snapshot', 'Download frozen JSON', 'Download frozen CSV']) {
      const bounds = await panel.getByRole('button', { name, exact: true }).boundingBox();
      expect(bounds!.width).toBeGreaterThanOrEqual(44); expect(bounds!.height).toBeGreaterThanOrEqual(44);
    }
    await testInfo.attach('frozen-export-' + width, { body: await page.screenshot({ path: testInfo.outputPath('frozen-export-' + width + '.png'), fullPage: true, scale: 'css' }), contentType: 'image/png' });
    await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name: 'System', exact: true }).click();
    await page.getByRole('tab', { name: 'Collection', exact: true }).click();
    await page.getByLabel('Collection health session', { exact: true }).selectOption('session-paper');
    await expect(page.getByText('NO TERMINAL OUTCOME', { exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
    await testInfo.attach('collection-health-' + width, { body: await page.screenshot({ path: testInfo.outputPath('collection-health-' + width + '.png'), fullPage: true, scale: 'css' }), contentType: 'image/png' });
  });
}

function continuityFixture(status = 'NO_KNOWN_INVALIDATION') {
  const record = records[0];
  return { schema_version: '1.0.0', assessment_kind: 'CONTINUITY_ONLY', authorizes_execution: false,
    trace_id: record.trace_id, session_id: record.trace.session_id, observation_id: record.trace.observation_id,
    network_id: record.trace.network_id, checked_at: '2026-09-17T08:00:00.000Z', policy_version: 'base-capture-continuity-v1',
    continuity_status: status, capture_count: String(record.trace.capture_refs.length),
    bound_count: status === 'UNTRACKED' ? '0' : String(record.trace.capture_refs.length),
    invalidation_reasons: status === 'INVALIDATED' ? ['CONTINUITY_LOST'] : [] };
}
test('source invalidation is visible without changing historical quote evidence', async ({ page }) => {
  let status = 'NO_KNOWN_INVALIDATION';
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/continuity')) return false;
    await route.fulfill({ json: continuityFixture(status) }); return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Source continuity', exact: true });
  await expect(panel.getByText('No known source invalidation', { exact: true })).toBeVisible();
  status = 'INVALIDATED';
  await panel.getByRole('button', { name: 'Refresh source status' }).click();
  await expect(panel.getByText('Source invalidated', { exact: true })).toBeVisible();
  await expect(panel.getByText('CONTINUITY_LOST', { exact: true })).toBeVisible();
  const modal = page.getByRole('dialog', { name: 'Decision evidence detail' });
  await expect(modal.getByText('CANDIDATE · GROSS QUOTE', { exact: true })).toBeVisible();
  await expect(modal.getByText('Unknown — external costs incomplete', { exact: true })).toBeVisible();
});
test('failed source refresh preserves a labelled last-known timestamp and status', async ({ page }) => {
  let failed = false;
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/continuity')) return false;
    if (failed) await route.fulfill({ status: 503, json: { code: 'DEPENDENCY_UNAVAILABLE' } });
    else await route.fulfill({ json: continuityFixture('INVALIDATED') });
    return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Source continuity', exact: true });
  await expect(panel.getByText('Source invalidated', { exact: true })).toBeVisible();
  failed = true; await panel.getByRole('button', { name: 'Refresh source status' }).click();
  await expect(panel.getByText(/Last received source status only/)).toBeVisible();
  await expect(panel.getByText('2026-09-17T08:00:00.000Z', { exact: true })).toBeVisible();
  await expect(panel.getByText('Source invalidated', { exact: true })).toBeVisible();
  await expect(panel.getByText('No known source invalidation', { exact: true })).toHaveCount(0);
});
test('missing source links and wrong-session responses never appear as healthy', async ({ page }) => {
  let wrong = false;
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/continuity')) return false;
    await route.fulfill({ json: wrong ? { ...continuityFixture(), session_id: 'other-session' } : continuityFixture('UNTRACKED') }); return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Source continuity', exact: true });
  await expect(panel.getByText('Source continuity not tracked', { exact: true })).toBeVisible();
  wrong = true; await panel.getByRole('button', { name: 'Refresh source status' }).click();
  await expect(panel.getByText(/Last received source status only/)).toBeVisible();
  await expect(panel.getByText('No known source invalidation', { exact: true })).toHaveCount(0);
});
