import { expect, test } from '@playwright/test';

const auth = { operator_id: 'operator', csrf_token: 'owner-overview-csrf', expires_at: '2000000000' };
const caps = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] };
// health: 'HEALTHY' is never emitted by the service (crates/arb-storage/src/lib.rs) — sessions with
// no heartbeat yet default to UNKNOWN (F6).
const session = { session_id: 'owner-session-1', network_id: 'base-mainnet', mode: 'OBSERVE', observed_state: 'RUNNING', health: 'UNKNOWN',
  desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'cfg-digest' };
const coverage = { session_id: session.session_id, raw_observations: '0', quoted_candidates: '0', rejected: '0', no_route: '0', data_unavailable: '0',
  unique_opportunity_groups: '0', eligible_attempts: null, reconciled_transactions: null, execution_accounting_available: false,
  collection_completeness: 'UNKNOWN', coverage_window_start_ms: null, coverage_window_end_ms: null };
// Whole-session, un-paginated aggregate (apps/web/src/api/collection.ts): unlike the old 24h
// attempt-page walk this can never be truncated, so Status/Health are sourced from it directly.
const collectionCoverage = { session_id: session.session_id, denominator: 'RECORDED_COLLECTION_ATTEMPTS', collection_completeness: 'UNKNOWN',
  attempts_started: '0', readiness_attempts: '0', research_attempts: '0', in_progress: '0', readiness_completed: '0',
  decisions_recorded: '0', acquisition_failed: '0', evaluation_failed: '0', deadline_exceeded: '0',
  suppressed: '0', worker_cancelled: '0', decision_rows_recorded: '0', window_start_at: null, window_end_at: null };
const emptyPage = { items: [], next_cursor: null };

// A realistic QUOTED decision: route uses real Base USDC/WETH9 addresses (AssetId is always
// `network:address`, never a bare network name — see crates/arb-domain/src/identity.rs), and
// amount_in_minor is 2 USDC at 6 decimals (both networks' research sessions use USDC as the
// starting asset per config/research.example.toml). quoted_output_minor - amount_in_minor ===
// gross_delta_minor, matching the invariant parseDecision enforces client-side.
const oneDecision = {
  trace_id: 't1', recorded_at: new Date(2026, 0, 1).toISOString(),
  trace: {
    schema_version: '1.0.0', observation_id: 'obs-1', session_id: session.session_id, experiment_id: 'e1', generation: '1',
    configuration_digest: 'cfg-digest', calculation_version: 'v1', strategy_id: 'strat', network_id: 'base-mainnet', mode: 'OBSERVE',
    source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'RECORDED_LIVE', observed_at_unix_ms: 1700000000000, input_age_ms: 0,
    capture_refs: [],
    route: [{ pool_id: 'p1', asset_in: 'base-mainnet:0x833589fcd6edb6e08f4c7c32d4f71b54bda02913', asset_out: 'base-mainnet:0x4200000000000000000000000000000000000006', venue_family: 'v' }],
    amount_in_minor: '2000000',
    result: { status: 'QUOTED', quoted_output_minor: '2500000', gross_delta_minor: '500000', included_pool_fees: ['1000', '2000'] },
    grouping: { version: '1', key: 'k', window_ms: 60000, window_start_ms: 1700000000000 },
    diagnostics: [],
  },
};

async function mockDemo(page: import('@playwright/test').Page, options: { decisions?: unknown[]; failDecisions?: boolean; sessionOverrides?: Partial<typeof session>; collectionCoverageOverrides?: Partial<typeof collectionCoverage> } = {}) {
  const decisions = options.decisions ?? [];
  const activeSession = { ...session, ...options.sessionOverrides };
  const activeCollectionCoverage = { ...collectionCoverage, ...options.collectionCoverageOverrides, session_id: activeSession.session_id };
  await page.route('**/v1/**', async route => {
    const p = new URL(route.request().url()).pathname;
    if (p === '/v1/auth/session') { await route.fulfill({ json: auth }); return; }
    if (p === '/v1/capabilities') { await route.fulfill({ json: caps }); return; }
    if (p === '/v1/sessions') { await route.fulfill({ json: { items: [activeSession], next_cursor: null } }); return; }
    if (p === '/v1/decision-coverage') { await route.fulfill({ json: coverage }); return; }
    if (p.endsWith('/collection-coverage')) { await route.fulfill({ json: activeCollectionCoverage }); return; }
    if (p === '/v1/decisions') {
      if (options.failDecisions) { await route.fulfill({ status: 500, json: { code: 'INTERNAL', message: 'decisions unavailable' } }); return; }
      await route.fulfill({ json: { items: decisions, next_cursor: null } }); return;
    }
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
  await expect(page.getByRole('heading', { name: '2. Gezondheid (sinds start sessie)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '3. Bevindingen' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '4. Wat als (hypothetisch)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '5. Route naar PAPER/live' })).toBeVisible();
  for (const id of ['ARB-035', 'ARB-039', 'ARB-041', 'ARB-042', 'ARB-043', 'ARB-044']) await expect(page.getByText(id, { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'English', exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Owner overview' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '2. Health (since session start)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '3. Findings' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '4. What if (hypothetical)' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '5. Road to PAPER/live' })).toBeVisible();
  await expect(page.getByText('OBSERVE candidates are not executed, not simulated end-to-end, and are not profit.')).toBeVisible();

  // The language choice is remembered across a reload via localStorage. This is a UX nicety this
  // implementation chose, not a requirement stated in ticket #182 (F9).
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Paper trading overview' })).toBeVisible();
  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Owner overview' })).toBeVisible();
});

test('what-if models one real decision at the default stake (which equals its own amount_in_minor, so gross scales 1:1)', async ({ page }) => {
  await mockDemo(page, { decisions: [oneDecision] });
  await page.goto('/');
  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await page.getByLabel('Sessie').selectOption(session.session_id);
  await page.getByRole('button', { name: 'English', exact: true }).click();

  await expect(page.getByText('Top candidates by modeled edge')).toBeVisible();
  await expect(page.getByRole('cell', { name: 'obs-1' }).first()).toBeVisible();
  await expect(page.getByText('Modeled gross edge at this stake')).toBeVisible();
  await expect(page.locator('.research-exact', { hasText: '500000' }).first()).toBeVisible();
  await expect(page.getByText('no cost assessment in the first 25 stored assessments')).toBeVisible();
  await expect(page.getByText('These numbers scale linearly with your stake')).toBeVisible();
});

// Regression for a CodeRabbit finding: only the attempts resource rendered a ResourceStatus, so a
// failed decisions request silently showed an honest-looking empty candidate list instead of
// reporting that decision data was unavailable. Every research resource must surface its own status.
test('a failed decisions request is reported, not shown as an honest-looking empty list', async ({ page }) => {
  await mockDemo(page, { failDecisions: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await page.getByLabel('Sessie').selectOption(session.session_id);
  await page.getByRole('button', { name: 'English', exact: true }).click();

  await expect(page.getByText('Research records unavailable.')).toBeVisible();
});

// A session whose worker lease has expired (health UNREACHABLE) while still nominally RUNNING must
// show red with the correct Dutch reason text — no attempt-window truncation, no sleeps, just a
// deterministic route mock (H1/H2).
test('a session with an unreachable worker while running shows the red Dutch health pill and reason', async ({ page }) => {
  await mockDemo(page, { sessionOverrides: { health: 'UNREACHABLE', observed_state: 'RUNNING' } });
  await page.goto('/');
  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await page.getByLabel('Sessie').selectOption(session.session_id);

  await expect(page.getByText('rood', { exact: true })).toBeVisible();
  await expect(page.getByText('de werker is niet bereikbaar')).toBeVisible();
  await expect(page.getByText('niet bereikbaar', { exact: true })).toBeVisible();
});

// F1 regression (round-2's H1/H3 fix re-broken at 97d373c): collection-coverage is fetched once
// per session selection, not re-polled, while the parent's session poll ticks the component's own
// re-renders every 5s. If freshness were judged against a live clock instead of the coverage
// resource's own fetch time, a healthy session would drift from green to amber to red purely from
// the page sitting open, with no new data. Uses Playwright's clock API instead of real sleeps.
test('a healthy session stays green after 6 minutes with the page open (coverage read once, not by a live clock)', async ({ page }) => {
  const t0 = Date.parse('2026-01-01T00:00:00.000Z');
  await page.clock.install({ time: t0 });
  let coverageCalls = 0;
  let sessionCalls = 0;
  await page.route('**/v1/**', async route => {
    const p = new URL(route.request().url()).pathname;
    if (p === '/v1/auth/session') { await route.fulfill({ json: auth }); return; }
    if (p === '/v1/capabilities') { await route.fulfill({ json: caps }); return; }
    if (p === '/v1/sessions') {
      sessionCalls += 1;
      // The worker is alive and collecting: every poll returns a fresh heartbeat.
      const activeSession = { ...session, health: 'DEGRADED', last_heartbeat_at: new Date(t0 + sessionCalls * 5_000).toISOString() };
      await route.fulfill({ json: { items: [activeSession], next_cursor: null } }); return;
    }
    if (p === '/v1/decision-coverage') { await route.fulfill({ json: coverage }); return; }
    if (p.endsWith('/collection-coverage')) {
      coverageCalls += 1;
      const fresh = { ...collectionCoverage, attempts_started: '5', research_attempts: '5', decisions_recorded: '5',
        window_start_at: new Date(t0 - 60_000).toISOString(), window_end_at: new Date(t0).toISOString() };
      await route.fulfill({ json: fresh }); return;
    }
    await route.fulfill({ json: emptyPage });
  });

  await page.goto('/');
  await page.getByRole('button', { name: 'Owner overview', exact: true }).click();
  await page.getByLabel('Sessie').selectOption(session.session_id);
  await page.getByRole('button', { name: 'English', exact: true }).click();
  await expect(page.getByText('green', { exact: true })).toBeVisible();
  await expect(page.getByText('is caught up (recently updated)')).toBeVisible();
  const callsAfterLoad = coverageCalls;

  // Six minutes pass with the page open; the parent keeps polling sessions every 5s.
  await page.clock.fastForward('06:00');
  await expect(page.getByText('green', { exact: true })).toBeVisible();
  await expect(page.getByText('is caught up (recently updated)')).toBeVisible();
  expect(coverageCalls).toBe(callsAfterLoad); // collection-coverage was not re-fetched.
});
