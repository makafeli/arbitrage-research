import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';
import { readFileSync } from 'node:fs';
import type { StoredDecision } from '../../src/api/research';
import { adapterSupportFixture, configDigest } from '../support.fixture';
import { decision } from '../research.fixture';

// Static contract examples. Screenshots and responses carry no provider qualification.
const fixtures = JSON.parse(readFileSync(new URL('../../../../specs/chain-freshness.example.json', import.meta.url), 'utf8')) as { cases: { name: string; record: StoredDecision }[] };
const session = { session_id: 'fixture-session', network_id: 'base-mainnet', mode: 'OBSERVE', observed_state: 'STOPPED', health: 'UNKNOWN', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: configDigest };
const legacy = decision(); legacy.trace.session_id = session.session_id;
const records = [...fixtures.cases.map(c => c.record), legacy];
async function stub(page: Page, override?: (route: Route, url: URL) => Promise<boolean>) {
  await page.route('**/v1/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname;
    if (override && await override(route, url)) return;
    let data: unknown;
    if (path === '/v1/auth/session') data = { operator_id: 'operator', csrf_token: 'fixture-csrf', expires_at: '2000000000' };
    if (path === '/v1/capabilities') data = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, decision_history: true, collection_telemetry: false, adapter_support: true,
      registered_configurations: [{ configuration_digest: configDigest, mode: 'OBSERVE', enabled_networks: ['base-mainnet'], strategy_ids: ['fixture-strategy'] }] };
    if (path === '/v1/sessions') data = { items: [session], next_cursor: null };
    if (path === '/v1/opportunities') data = { items: [], next_cursor: null };
    if (path === '/v1/adapter-support') data = adapterSupportFixture();
    if (path === '/v1/decisions') data = { items: records, next_cursor: null };
    // Mirror the API router's decoding of the path parameter, including canonical sha256: IDs.
    if (path.startsWith('/v1/decisions/')) {
      const observationId = decodeURIComponent(path.slice('/v1/decisions/'.length));
      data = records.find(item => item.trace.observation_id === observationId);
    }
    if (path === '/v1/decision-groups') data = { items: [], next_cursor: null };
    if (path === '/v1/decision-coverage') data = { session_id: session.session_id, raw_observations: '5', quoted_candidates: '2', rejected: '0', no_route: '0', data_unavailable: '3', unique_opportunity_groups: '2', eligible_attempts: null, reconciled_transactions: null,
      execution_accounting_available: false, collection_completeness: 'UNKNOWN', coverage_window_start_ms: 1789214400000, coverage_window_end_ms: 1789214400000 };
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Unknown fixture endpoint' } });
  });
  await page.goto('/'); await page.getByRole('button', { name: 'Connect API', exact: true }).click(); await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
}
async function navigate(page: Page, name: string) { await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name, exact: true }).click(); }
async function openDecision(page: Page, name: string) {
  await navigate(page, 'Opportunities'); await page.getByLabel('Decision session', { exact: true }).selectOption(session.session_id);
  const id = name === 'legacy' ? legacy.trace.observation_id : fixtures.cases.find(c => c.name === name)!.record.trace.observation_id;
  const responsePromise = page.waitForResponse(response => new URL(response.url()).pathname === '/v1/decisions/' + encodeURIComponent(id));
  await page.getByRole('button', { name: 'Inspect decision ' + id, exact: true }).click();
  const response = await responsePromise;
  expect(response.status()).toBe(200);
  expect((await response.json()).trace.observation_id).toBe(id);
  const dialog = page.getByRole('dialog', { name: 'Decision evidence detail' }); await expect(dialog.getByRole('heading', { name: 'Captured chain age', exact: true })).toBeVisible(); return dialog;
}
for (const width of [320, 390, 1440]) {
  test(`historical chain freshness at ${width}px preserves origin and independent elapsed time`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 1100 }); await stub(page); const dialog = await openDecision(page, 'within-policy');
    const report = dialog.getByRole('region', { name: 'Captured chain age' });
    await expect(report.getByText('Within recorded policy', { exact: true }).first()).toBeVisible();
    await expect(report.getByText('5000 ms', { exact: true })).toBeVisible(); await expect(report.getByText('750 ms', { exact: true }).first()).toBeVisible();
    await expect(report.getByText(/does not measure current provider or service health/)).toBeVisible();
    for (const name of ['Capture 1', 'Capture 2']) {
      const sourceHeading = report.getByRole('heading', { name, exact: true });
      await sourceHeading.scrollIntoViewIfNeeded();
      await expect(sourceHeading).toBeInViewport();
    }
    // Focus the retained viewport image on the new evidence, rather than a tall
    // background page with the report clipped at the bottom of the modal.
    await report.evaluate(element => element.scrollIntoView({ block: 'start', behavior: 'instant' }));
    await expect(report.getByRole('heading', { name: 'Captured chain age', exact: true })).toBeInViewport();
    await expect(report.getByText('5000 ms', { exact: true })).toBeInViewport();
    await expect(report.getByRole('heading', { name: 'Capture 1', exact: true })).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`chain-freshness-${width}.png`), fullPage: false });
  });
  test(`adapter catalog at ${width}px separates local scope and unavailable execution`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 1000 }); await stub(page); await navigate(page, 'System');
    const support = page.getByRole('region', { name: 'Adapter support catalog' });
    await expect(support.getByText('Locally authorized registry', { exact: true })).toBeVisible(); await expect(support.getByText('Registry not loaded', { exact: true })).toBeVisible();
    await expect(support.getByText('Unqualified', { exact: true })).toHaveCount(2);
    await expect(support.getByText(/No chain is marked ready to trade/)).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`adapter-support-${width}.png`), fullPage: true });
  });
}
test('legacy, stale, future and missing timestamps remain distinct historical evidence', async ({ page }) => {
  await stub(page);
  for (const [name, label] of [['legacy', 'Unmeasured legacy record'], ['stale', 'Older than policy'], ['future', 'Future chain time'], ['unknown', 'Unknown chain time']]) {
    const dialog = await openDecision(page, name); await expect(dialog.getByText(label, { exact: true }).first()).toBeVisible();
    if (name === 'legacy') await expect(dialog.getByText(/No chain freshness report was retained/)).toBeVisible();
    if (name === 'stale') await expect(dialog.getByText('5750 ms', { exact: true }).first()).toBeVisible();
    if (name === 'future') await expect(dialog.getByText(/No zero age was substituted/).first()).toBeVisible();
    if (name === 'unknown') await expect(dialog.getByText(/The source supplied no usable chain time/).first()).toBeVisible();
    await dialog.getByRole('button', { name: 'Close evidence detail', exact: true }).click();
  }
});
test('catalog errors are distinct from no registry and refresh retains prior structural evidence', async ({ page }) => {
  let outage = true;
  await stub(page, async (route, url) => { if (url.pathname !== '/v1/adapter-support' || !outage) return false; await route.fulfill({ status: 503, json: { code: 'UNAVAILABLE', message: 'Fixture outage' } }); return true; });
  await navigate(page, 'System'); const support = page.getByRole('region', { name: 'Adapter support catalog' });
  await expect(support.getByText('Adapter catalog unavailable.', { exact: true })).toBeVisible(); await expect(support.getByText('Registry not loaded', { exact: true })).toHaveCount(0);
  outage = false; await support.getByRole('button', { name: 'Refresh adapter catalog' }).click(); await expect(support.getByText('Locally authorized registry', { exact: true })).toBeVisible();
  await page.getByLabel('View chain', { exact: true }).selectOption('solana-mainnet'); await expect(support.getByText('Locally authorized registry', { exact: true })).toHaveCount(0); await expect(support.getByText('Registry not loaded', { exact: true })).toBeVisible();
  outage = true; await support.getByRole('button', { name: 'Refresh adapter catalog' }).click(); await expect(support.getByText('Previous adapter catalog retained.', { exact: true })).toBeVisible(); await expect(support.getByText('Registry not loaded', { exact: true })).toBeVisible();
});
test('navigating away cancels the adapter request without substituting an empty catalog', async ({ page }) => {
  let release = () => {}, started = false, aborted = false;
  page.on('requestfailed', request => { if (request.url().endsWith('/v1/adapter-support')) aborted = true; });
  await stub(page, async (route, url) => {
    if (url.pathname !== '/v1/adapter-support') return false;
    started = true; await new Promise<void>(resolve => { release = resolve; });
    await route.fulfill({ json: adapterSupportFixture() }).catch(() => {}); return true;
  });
  await navigate(page, 'System'); await expect.poll(() => started).toBe(true); await navigate(page, 'Strategies');
  await expect.poll(() => aborted).toBe(true); release();
  await expect(page.getByRole('heading', { name: 'Validated configuration registry', exact: true })).toBeVisible();
  await expect(page.getByText('No immutable configurations are registered in this service.', { exact: true })).toHaveCount(0);
});
