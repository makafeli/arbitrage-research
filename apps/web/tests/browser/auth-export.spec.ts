import { createCaptureAuditRequest, auditRequestDigest } from '../../src/domain/captureAudit';
import type { CaptureAuditReport, CaptureAuditRequest } from '../../src/domain/captureAudit';
import { expect, test } from '@playwright/test';
import type { Page, Route } from '@playwright/test';
import { coverage, decision, nativeAsset, principalAsset } from '../research.fixture';
import { auditExportFixture, collectionCoverage, frozenExportFixture } from '../exports.fixture';

const session = { session_id: 'session-paper', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'RUNNING', health: 'UNKNOWN', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:paper-fixture-config' };
const capabilities = { modes: ['OBSERVE', 'PAPER', 'REPLAY'], live_execution: false, market_data: false, opportunity_capture: false, decision_history: true, paper_ledger: true, paper_run_creation: true, collection_telemetry: true, session_export: true,
  registered_configurations: [{ configuration_digest: session.configuration_digest, mode: 'PAPER', enabled_networks: ['base-mainnet'], strategy_ids: ['usdc-cycle'], paper_assets: [{ network_id: 'base-mainnet', asset: principalAsset }, { network_id: 'base-mainnet', asset: nativeAsset }] }] };
const auth = { operator_id: 'operator', csrf_token: 'fixture-auth-export-csrf', expires_at: '2000000000' };
async function stub(page: Page, override?: (route: Route, url: URL) => Promise<boolean>) {
  await page.route('**/v1/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname;
    if (override && await override(route, url)) return;
    if (path === '/v1/auth/logout') { await route.fulfill({ status: 204 }); return; }
    const data: unknown = path === '/v1/auth/session' || path === '/v1/auth/login' ? auth
      : path === '/v1/capabilities' ? capabilities
      : path === '/v1/sessions' ? { items: [session], next_cursor: null }
      : path === '/v1/opportunities' ? { items: [], next_cursor: null }
      : path === '/v1/decisions' ? { items: [decision()], next_cursor: null }
      : path === '/v1/decision-coverage' ? coverage()
      : path === '/v1/decision-groups' ? { items: [], next_cursor: null }
      : path === '/v1/sessions/session-paper/export' ? frozenExportFixture()
      : path === '/v1/sessions/session-paper/collection-coverage' ? collectionCoverage()
      : path === '/v1/sessions/session-paper/collection-attempts' ? { items: frozenExportFixture().data.collection_attempts, next_cursor: null }
      : null;
    await route.fulfill({ status: data ? 200 : 404, json: data ?? { code: 'NOT_FOUND', message: 'Fixture endpoint unavailable' } });
  });
  await page.goto('/'); await page.getByRole('button', { name: 'Connect API', exact: true }).click();
  await expect(page.getByText('API CONNECTED', { exact: true })).toBeVisible();
  await navigate(page, 'Opportunities');
  await page.getByLabel('Decision session', { exact: true }).selectOption(session.session_id);
  await expect(page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true })).toBeVisible();
}
async function navigate(page: Page, name: string) { await page.getByRole('navigation', { name: 'Primary navigation' }).getByRole('button', { name, exact: true }).click(); }
const panel = (page: Page) => page.getByRole('region', { name: 'Frozen session export', exact: true });
async function prepare(page: Page) {
  await panel(page).getByRole('button', { name: 'Prepare frozen session export', exact: true }).click();
  await expect(panel(page).getByText('Frozen database snapshot verified', { exact: true })).toBeVisible();
}
async function revoked(page: Page) {
  await expect(page.getByRole('heading', { name: 'Connect to your research service', exact: true })).toBeVisible();
  await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeDisabled();
  await expect(panel(page).getByRole('button', { name: 'Download frozen CSV', exact: true })).toBeDisabled();
  await expect(panel(page).getByText('Frozen database snapshot verified', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toHaveCount(0);
}

test('explicit logout clears the prepared frozen export without changing worker state', async ({ page }) => {
  await stub(page); await prepare(page);
  await page.getByRole('button', { name: 'Sign out', exact: true }).click(); await revoked(page);
  await navigate(page, 'Overview');
  await expect(page.getByRole('region', { name: 'Session session-paper', exact: true }).getByText('RUNNING', { exact: true })).toBeVisible();
});

for (const body of ['<html>fixture expired session</html>', '[]', '']) {
  test('a malformed child 401 promptly revokes an existing frozen export: ' + JSON.stringify(body), async ({ page }) => {
    let expired = false;
    await stub(page, async (route, url) => {
      if (expired && url.pathname === '/v1/decisions') { await route.fulfill({ status: 401, body }); return true; }
      return false;
    });
    await prepare(page); expired = true;
    await page.getByRole('button', { name: 'Refresh evidence', exact: true }).click(); await revoked(page);
  });
}

test('a late frozen-export response cannot restore a bundle after a child 401', async ({ page }) => {
  let expired = false, exports = 0; let held: Route | null = null;
  await stub(page, async (route, url) => {
    if (expired && url.pathname === '/v1/decisions') { await route.fulfill({ status: 401, body: '<html>expired</html>' }); return true; }
    if (url.pathname.endsWith('/export') && ++exports === 2) { held = route; return true; }
    return false;
  });
  await prepare(page);
  await panel(page).getByRole('button', { name: 'Prepare a new frozen snapshot', exact: true }).click();
  await expect.poll(() => held !== null).toBe(true);
  expired = true; await page.getByRole('button', { name: 'Refresh evidence', exact: true }).click(); await revoked(page);
  // The browser can already have cancelled this interception. Either transport
  // cancellation or rejecting the late result is valid; neither may restore it.
  try { await (held as Route | null)!.fulfill({ json: frozenExportFixture() }); } catch { /* already cancelled by revocation */ }
  await page.getByLabel('Operator secret', { exact: true }).fill('fixture-new-operator-secret-00000000');
  expired = false; await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible();
  await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeDisabled();
  await expect(panel(page).getByText('Frozen database snapshot verified', { exact: true })).toHaveCount(0);
});

for (const [status, body] of [[403, '[]'], [404, '<html>unavailable</html>']] as const) {
  test(`export ${status} clears the earlier bundle but does not revoke unrelated operator access`, async ({ page }) => {
    let calls = 0;
    await stub(page, async (route, url) => {
      if (!url.pathname.endsWith('/export') || ++calls === 1) return false;
      await route.fulfill({ status, body }); return true;
    });
    await prepare(page); await panel(page).getByRole('button', { name: 'Prepare a new frozen snapshot', exact: true }).click();
    await expect(panel(page).getByRole('alert')).toBeVisible();
    await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeDisabled();
    await expect(panel(page).getByRole('button', { name: 'Download frozen CSV', exact: true })).toBeDisabled();
    await expect(panel(page).getByText('Frozen database snapshot verified', { exact: true })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible();
  });
}

for (const status of [429, 503]) {
  test(`transient export ${status} preserves the original verified bundle in the same authorization context`, async ({ page }) => {
    let calls = 0;
    await stub(page, async (route, url) => {
      if (!url.pathname.endsWith('/export') || ++calls === 1) return false;
      await route.fulfill({ status, json: { code: 'TEMPORARILY_UNAVAILABLE', message: 'Fixture transient error' } }); return true;
    });
    await prepare(page); await panel(page).getByRole('button', { name: 'Prepare a new frozen snapshot', exact: true }).click();
    await expect(panel(page).getByRole('alert')).toBeVisible();
    await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeEnabled();
    await expect(panel(page).getByRole('button', { name: 'Download frozen CSV', exact: true })).toBeEnabled();
    await expect(panel(page).getByText('export-fixture-one', { exact: false })).toBeVisible();
    expect(calls).toBe(2);
  });
}

test('navigation preserves a bundle within the same login but hidden download controls cannot act', async ({ page }) => {
  await stub(page); await prepare(page); let downloads = 0; page.on('download', () => { downloads += 1; });
  await navigate(page, 'Overview');
  const hiddenButton = page.getByRole('button', { name: 'Download frozen JSON', exact: true, includeHidden: true }).first();
  await expect(hiddenButton).toBeDisabled(); await hiddenButton.evaluate(element => (element as HTMLButtonElement).click());
  await navigate(page, 'Opportunities');
  await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeEnabled();
  await expect(panel(page).getByText('export-fixture-one', { exact: false })).toBeVisible(); expect(downloads).toBe(0);
});

test('revoking the export capability discards a bundle rather than reviving it on a later refresh', async ({ page }) => {
  let available = true;
  await stub(page, async (route, url) => {
    if (url.pathname !== '/v1/capabilities') return false;
    await route.fulfill({ json: { ...capabilities, session_export: available } }); return true;
  });
  await prepare(page); available = false;
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(page.getByText(/Frozen session exports are unavailable in this API version/).first()).toBeVisible();
  available = true; await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeDisabled();
});

test('child authorization loss preserves uncertain command intent and the exact retry key across sign-in', async ({ page }) => {
  let expired = false;
  const requests: { key: string | undefined; body: unknown }[] = [];
  await stub(page, async (route, url) => {
    if (expired && url.pathname === '/v1/decisions') { await route.fulfill({ status: 401, body: 'expired' }); return true; }
    if (!url.pathname.endsWith('/commands')) return false;
    requests.push({ key: route.request().headers()['idempotency-key'], body: route.request().postDataJSON() });
    if (requests.length === 1) await route.abort('failed');
    else await route.fulfill({ status: 202, json: { command_id: 'fixture-command', session_id: session.session_id, revision: '2', status: 'PENDING', action: 'STOP', accepted_at: '2026-09-13T00:00:00Z', applied_at: null, outstanding_attempts: 0, fence_effective: false, signer_revocation_status: 'NOT_APPLICABLE' } });
    return true;
  });
  await prepare(page); await navigate(page, 'Overview');
  const card = page.getByRole('region', { name: 'Session session-paper', exact: true });
  await card.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(card.getByRole('button', { name: 'Retry same request', exact: true })).toBeEnabled();
  await navigate(page, 'Opportunities'); expired = true;
  await page.getByRole('button', { name: 'Refresh evidence', exact: true }).click(); await revoked(page);
  await expect(page.getByRole('button', { name: 'Open demo', exact: true })).toBeDisabled();
  expired = false; await page.getByLabel('Operator secret', { exact: true }).fill('fixture-operator-secret-0000000000');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toBeVisible();
  await navigate(page, 'Overview');
  await card.getByRole('button', { name: 'Retry same request', exact: true }).click();
  await expect(card.getByText('STOP · PENDING', { exact: true })).toBeVisible();
  expect(requests).toHaveLength(2); expect(requests[1]).toEqual(requests[0]); expect(requests[0].key).toBeTruthy();
});

// Operator-supplied report fixtures only. The real local filesystem round-trip
// is exercised separately in captureAudit.test.ts, not by these browser stubs.
async function missingAuditReport(request: CaptureAuditRequest): Promise<CaptureAuditReport> {
  const refs = request.capture_request.captures;
  return { schema_version: 1, kind: 'FROZEN_EXPORT_CAPTURE_AUDIT', source_export: request.source_export,
    request_sha256: await auditRequestDigest(request), audit: { schema_version: 1, kind: 'CAPTURE_DEPENDENCY_AUDIT',
      checked_at_ms: '1789336200000', network: request.capture_request.network, config_digest: request.capture_request.config_digest,
      request_sha256: await auditRequestDigest(request.capture_request), status: 'COMPLETE_WITH_GAPS',
      references_requested: refs.length, references_reported: refs.length, raw_status_counts: { MISSING: refs.length },
      bytes_read_budgeted: '0', max_bytes: '268435456', source_authenticity: 'CALLER_RETAINED_DIGEST_NOT_INDEPENDENTLY_AUTHENTICATED',
      filesystem_snapshot: 'NON_ATOMIC_KEEP_ROOT_QUIESCED', execution_authorized: false,
      dependencies: refs.map(ref => ({ ...ref, raw_artifact_status: 'MISSING', reason: 'CAPTURE_DIRECTORY_OR_INPUT_MISSING',
        expiration_status: 'UNKNOWN', capture_time_status: 'UNKNOWN', origin: 'UNKNOWN', created_at_ms: null, raw_expires_at_ms: null,
        quote_inputs_declared_complete: null, verified_objects: 0, missing_objects: 0, corrupt_objects: 0,
        replay_status: 'NOT_ASSESSED', market_performance_eligible: false })) } };
}
const auditPanel = (page: Page) => page.getByRole('region', { name: 'Local capture audit', exact: true });
async function uploadAudit(page: Page, value: unknown) {
  await auditPanel(page).getByLabel('Import local audit JSON').setInputFiles({ name: 'local-report.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(value)) });
}
async function prepareAudit(page: Page) {
  await prepare(page);
  await expect(auditPanel(page).getByRole('button', { name: 'Download capture audit request', exact: true })).toBeEnabled();
}
for (const width of [320, 390, 1440]) {
  test(`capture audit download/import stays local and readable at ${width}px`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 1000 });
    await stub(page, async (route, url) => {
      if (!url.pathname.endsWith('/export')) return false;
      await route.fulfill({ json: auditExportFixture() }); return true;
    });
    await prepareAudit(page);
    const event = page.waitForEvent('download');
    await auditPanel(page).getByRole('button', { name: 'Download capture audit request', exact: true }).click();
    const download = await event, stream = await download.createReadStream();
    if (!stream) throw new Error('Audit request download missing');
    const chunks: Buffer[] = []; for await (const chunk of stream) chunks.push(Buffer.from(chunk));
    const request = JSON.parse(Buffer.concat(chunks).toString()) as CaptureAuditRequest;
    expect(request).toEqual(await createCaptureAuditRequest(auditExportFixture()));
    const posted: string[] = []; page.on('request', req => { if (req.method() === 'POST') posted.push(req.url()); });
    await uploadAudit(page, await missingAuditReport(request));
    await expect(auditPanel(page).getByText('Imported local audit: COMPLETE_WITH_GAPS', { exact: true })).toBeVisible();
    await expect(auditPanel(page).getByText('MISSING: 2', { exact: true })).toBeVisible();
    await expect(auditPanel(page).getByText('Operator-supplied, not independently authenticated.', { exact: false })).toBeVisible();
    expect(posted).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
    await auditPanel(page).screenshot({ path: info.outputPath(`capture-audit-${width}.png`) });
  });
}
test('mismatched and incomplete local audit results clear the prior imported result', async ({ page }) => {
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/export')) return false;
    await route.fulfill({ json: auditExportFixture() }); return true;
  });
  await prepareAudit(page);
  const valid = await missingAuditReport(await createCaptureAuditRequest(auditExportFixture()));
  await uploadAudit(page, valid); await expect(auditPanel(page).getByText('Imported local audit: COMPLETE_WITH_GAPS', { exact: true })).toBeVisible();
  const wrong = structuredClone(valid); wrong.source_export.export_id = 'another-export';
  await uploadAudit(page, wrong); await expect(auditPanel(page).getByRole('alert')).toBeVisible();
  await expect(auditPanel(page).getByText('Imported local audit:', { exact: false })).toHaveCount(0);
  const incomplete = structuredClone(valid); incomplete.audit.dependencies.pop(); incomplete.audit.references_reported--;
  await uploadAudit(page, incomplete); await expect(auditPanel(page).getByRole('alert')).toBeVisible();
  await expect(auditPanel(page).getByRole('button', { name: 'Download bound audit report', exact: true })).toHaveCount(0);
});
test('preparing a different frozen export invalidates an earlier local audit', async ({ page }) => {
  let calls = 0;
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/export')) return false;
    const source = auditExportFixture(); if (++calls > 1) source.export_id = 'export-new';
    await route.fulfill({ json: source }); return true;
  });
  await prepareAudit(page);
  const old = await missingAuditReport(await createCaptureAuditRequest(auditExportFixture()));
  await uploadAudit(page, old); await expect(auditPanel(page).getByText('Imported local audit: COMPLETE_WITH_GAPS', { exact: true })).toBeVisible();
  await panel(page).getByRole('button', { name: 'Prepare a new frozen snapshot', exact: true }).click();
  await expect(panel(page).getByText('export-new', { exact: false })).toBeVisible();
  await expect(auditPanel(page).getByText('Imported local audit:', { exact: false })).toHaveCount(0);
  await expect(auditPanel(page).getByLabel('Import local audit JSON')).toBeEnabled();
  await uploadAudit(page, old); await expect(auditPanel(page).getByRole('alert')).toBeVisible();
});
test('logout invalidates a pending local file read and prevents its late result returning', async ({ page }) => {
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/export')) return false;
    await route.fulfill({ json: auditExportFixture() }); return true;
  });
  await prepareAudit(page);
  await page.evaluate(() => {
    const original = File.prototype.text;
    File.prototype.text = function() { const file = this; return new Promise<string>(resolve => {
      (window as unknown as { releaseAuditFile: () => void }).releaseAuditFile = () => { void original.call(file).then(resolve); };
    }); };
  });
  await uploadAudit(page, await missingAuditReport(await createCaptureAuditRequest(auditExportFixture())));
  await expect(auditPanel(page).getByText('Checking local report binding...', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Sign out', exact: true }).click(); await revoked(page);
  await page.evaluate(() => (window as unknown as { releaseAuditFile: () => void }).releaseAuditFile());
  await page.getByLabel('Operator secret', { exact: true }).fill('fixture-operator-secret-0000000000');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(panel(page).getByRole('button', { name: 'Download frozen JSON', exact: true })).toBeDisabled();
  await expect(auditPanel(page)).toHaveCount(0);
});
test('hidden capture audit controls cannot download or import after navigation', async ({ page }) => {
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/export')) return false;
    await route.fulfill({ json: auditExportFixture() }); return true;
  });
  await prepareAudit(page); let downloads = 0; page.on('download', () => { downloads++; });
  await navigate(page, 'Overview');
  const button = page.getByRole('button', { name: 'Download capture audit request', exact: true, includeHidden: true });
  await expect(button).toBeDisabled(); await button.evaluate(el => (el as HTMLButtonElement).click());
  expect(downloads).toBe(0);
});
