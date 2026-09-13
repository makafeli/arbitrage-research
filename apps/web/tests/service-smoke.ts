// Run only against an isolated local control API with its disposable database.
// The actual browser client is exercised through a Node cookie/Origin shim;
// this verifies DTO and auth interoperability, not browser cookie enforcement.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ApiError, ControlApi } from '../src/api/client.ts';

const service = new URL(process.env.ARB_API_SMOKE_URL ?? 'http://127.0.0.1:8080');
if (service.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(service.hostname) || service.username || service.password) {
  throw new Error('Smoke tests require a credential-free loopback HTTP test service.');
}
const secret = process.env.ARB_OPERATOR_SECRET;
if (!secret) throw new Error('ARB_OPERATOR_SECRET must be provisioned for the isolated test service.');
const origin = process.env.ARB_PUBLIC_ORIGIN ?? 'http://127.0.0.1:5173';
let cookie = '';
const api = new ControlApi(async (path, options) => {
  const headers = new Headers(options?.headers);
  headers.set('Origin', origin);
  if (cookie) headers.set('Cookie', cookie);
  const response = await fetch(new URL(String(path), service), { ...options, headers });
  const setCookie = response.headers.get('set-cookie');
  if (setCookie) cookie = setCookie.split(';')[0];
  return response;
});
await assert.rejects(api.auth(), e => e instanceof ApiError && e.status === 401);
try {
  const login = await api.login(secret);
  assert.equal(login.operator_id, 'operator');
  assert.match(login.expires_at, /^\d+$/);
  assert.ok(login.csrf_token.length > 20);
  const session = await api.auth();
  assert.equal(session.csrf_token, login.csrf_token);
  const [caps, sessions, opportunities] = await Promise.all([api.capabilities(), api.sessions(), api.opportunities()]);
  assert.equal(caps.live_execution, false);
  assert.equal(caps.opportunity_capture, false);
  assert.ok(caps.registered_configurations.length > 0);
  const researchSmoke = process.env.ARB_HTTP_SMOKE_RESEARCH === 'true';
  const enabled = caps.registered_configurations.filter(config => config.enabled_networks.length > 0);
  if (researchSmoke) {
    assert.equal(enabled.length, 1, 'Research smoke requires exactly one dedicated enabled configuration.');
    assert.equal(enabled[0].mode, 'OBSERVE'); assert.deepEqual(enabled[0].enabled_networks, ['base-mainnet']);
    assert.equal(caps.collection_telemetry, true); assert.equal(caps.session_export, true); assert.equal(caps.cost_assessments, true);
  } else assert.equal(enabled.length, 0, 'Smoke service must use disabled research configurations.');
  assert.equal(sessions.items.length, 0, 'Smoke service must use an isolated empty database.');
  assert.equal(opportunities.items.length, 0);
  await assert.rejects(api.create({ network_id: 'base-mainnet', mode: 'PAPER', configuration_digest: 'unregistered-smoke-config', experiment_id: 'denied-smoke-experiment', strategy_ids: ['not-enabled'] }, crypto.randomUUID()), e => e instanceof ApiError && [400, 403].includes(e.status));
  if (researchSmoke) {
    const created = await api.create({ network_id: 'base-mainnet', mode: 'OBSERVE', configuration_digest: enabled[0].configuration_digest,
      experiment_id: 'isolated-http-smoke', strategy_ids: enabled[0].strategy_ids }, crypto.randomUUID());
    assert.equal(created.observed_state, 'RECOVERING'); assert.equal(created.execution_authorized, false);
    const [coverage, attempts] = await Promise.all([api.collectionCoverage(created.session_id), api.collectionAttempts(created.session_id)]);
    assert.equal(coverage.session_id, created.session_id); assert.equal(coverage.attempts_started, '0');
    assert.equal(coverage.collection_completeness, 'UNKNOWN'); assert.equal(coverage.denominator, 'RECORDED_COLLECTION_ATTEMPTS');
    assert.deepEqual(attempts, { items: [], next_cursor: null });
    const costFixture = JSON.parse(readFileSync(new URL('../../../specs/cost-assessment.example.json', import.meta.url), 'utf8'));
    assert.deepEqual(await api.costAssessments(created.session_id), { items: [], next_cursor: null });
    await assert.rejects(api.createCostAssessment(created.session_id, costFixture.request, crypto.randomUUID()), e => e instanceof ApiError && e.status === 404);
    // Successful assessment creation is covered by the real Rust HTTP/PG tests;
    // this isolated service deliberately has no worker or seeded market evidence.
    // This uses the shipped client parser and browser-compatible SHA-256 verifier.
    const frozen = await api.frozenExport(created.session_id), second = await api.frozenExport(created.session_id);
    assert.equal(frozen.data.session.session_id, created.session_id); assert.equal(frozen.data.experiment_id, 'isolated-http-smoke');
    assert.deepEqual(frozen.snapshot.source_counts, { decisions: '0', paper_runs: '0', paper_journal_events: '0', capture_catalog_entries: '0', collection_attempts: '0', cost_assessments: '0' });
    assert.equal(frozen.snapshot.collection_completeness, 'UNKNOWN'); assert.equal(frozen.content_sha256, second.content_sha256);
    assert.notEqual(frozen.export_id, second.export_id); assert.equal(frozen.data.decision_coverage.raw_observations, '0');
    assert.equal(frozen.schema_version, '1.1.0'); assert.deepEqual(frozen.data.cost_assessments, []); assert.deepEqual(frozen.data.collection_attempts, []); assert.deepEqual(frozen.data.capture_dependencies, []);
    assert.equal(frozen.methodology.configuration_snapshot, 'DIGEST_ONLY'); assert.equal(frozen.methodology.execution_authorized, false);
    await assert.rejects(api.frozenExport('missing-smoke-session'), e => e instanceof ApiError && e.status === 404);
  }
  await api.logout();
  await assert.rejects(api.auth(), e => e instanceof ApiError && e.status === 401);
  process.stdout.write('UI client / control API interoperability: PASS (auth, CSRF session, capabilities, empty records, rejected configuration' + (researchSmoke ? ', isolated OBSERVE creation without worker start, scoped collection telemetry, empty cost history and missing-source rejection, six-dataset frozen export counts and repeated content digest' : '') + ', logout).\n');
} finally {
  // Best-effort cleanup if an assertion failed before the explicit logout.
  if (cookie && cookie !== 'arb_session=') await api.logout().catch(() => {});
  api.clearAuth();
}
