import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { ApiError, ControlApi, availableActions, parseOpportunity, parseReceipt, parseSession } from '../src/api/client.ts';

const session = { session_id: 'test-session', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'RUNNING', health: 'HEALTHY', desired_revision: '9007199254740999', applied_revision: '9007199254740999', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:test' };
const receipt = { command_id: 'test-command', session_id: 'test-session', revision: '9007199254741000', action: 'STOP', status: 'PENDING', accepted_at: '2026-09-12T12:00:00Z', applied_at: null, outstanding_attempts: 0, fence_effective: false, signer_revocation_status: 'NOT_APPLICABLE' };
const fixture = JSON.parse(readFileSync(new URL('../../../specs/opportunity.example.json', import.meta.url), 'utf8'));

test('session revisions remain exact strings and lifecycle controls exclude LIVE', () => {
  const parsed = parseSession(session);
  assert.equal(parsed.desired_revision, '9007199254740999');
  assert.deepEqual(availableActions(parsed), ['PAUSE', 'STOP']);
  assert.deepEqual(availableActions({ ...parsed, mode: 'LIVE' }), []);
  assert.throws(() => parseSession({ ...session, desired_revision: 9007199254740999 }), ApiError);
});
test('a 202 receipt is pending; APPLIED stop without a fence is invalid', () => {
  assert.equal(parseReceipt(receipt).status, 'PENDING');
  assert.throws(() => parseReceipt({ ...receipt, status: 'APPLIED', applied_at: '2026-09-12T12:00:01Z' }), ApiError);
  assert.equal(parseReceipt({ ...receipt, status: 'APPLIED', applied_at: '2026-09-12T12:00:01Z', fence_effective: true }).status, 'APPLIED');
});
test('connected data parser refuses synthetic records and impossible success evidence', () => {
  assert.throws(() => parseOpportunity(fixture), /Synthetic records/);
  const captured = { ...fixture, source_kind: 'CAPTURED_MARKET_DATA' };
  assert.equal(parseOpportunity(captured).evidence_label, 'CANDIDATE');
  assert.throws(() => parseOpportunity({ ...captured, evidence_label: 'SIMULATED', simulation_status: 'FAILED' }), ApiError);
  assert.throws(() => parseOpportunity({ ...captured, evidence_label: 'REALIZED' }), ApiError);
  assert.throws(() => parseOpportunity({ ...captured, costs: [{ ...captured.costs[0], in_start_asset_minor: 0 }] }), ApiError);
});
for (const schema_version of ['1.0.0', '1.1.0']) {
  const captured = { ...fixture, schema_version, source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'RECORDED_LIVE',
    net_after_explicit_costs_minor: schema_version === '1.1.0' ? null : fixture.net_after_explicit_costs_minor };
  const simulated = { ...captured, evidence_label: 'SIMULATED', simulation_status: 'PASSED', execution_plan_digest: 'sha256:retained-plan',
    eligibility_checks: { ...captured.eligibility_checks, simulation_matches_exact_plan: true, atomic_route_supported: true, final_balance_guard_present: true } };
  test(`schema ${schema_version} simulated evidence requires a complete consistent snapshot`, () => {
    assert.equal(parseOpportunity(simulated).evidence_label, 'SIMULATED');
    for (const complete of [true, false]) for (const consistent of [true, false]) {
      if (complete && consistent) continue;
      assert.throws(() => parseOpportunity({ ...simulated, snapshot: { ...simulated.snapshot, complete, consistent } }), ApiError);
    }
    // Incomplete candidate observations remain useful rejection/data-quality evidence.
    assert.equal(parseOpportunity({ ...captured, snapshot: { ...captured.snapshot, complete: false, consistent: false },
      eligibility_checks: { ...captured.eligibility_checks, state_fresh_and_coherent: false } }).evidence_label, 'CANDIDATE');
  });
  test(`schema ${schema_version} quote assumptions cannot contradict the displayed fee inclusion`, () => {
    assert.equal(parseOpportunity(captured).quoted_output_minor, fixture.quoted_output_minor);
    for (const quoted_output_includes_pool_fees_and_price_impact of [false, null, undefined, 'true']) {
      assert.throws(() => parseOpportunity({ ...captured, quoted_output_includes_pool_fees_and_price_impact }), ApiError);
    }
  });
  test(`schema ${schema_version} research records cannot claim transaction finality`, () => {
    for (const mode of ['OBSERVE', 'PAPER', 'REPLAY']) {
      assert.equal(parseOpportunity({ ...captured, mode }).finality_status, 'NOT_APPLICABLE');
      assert.throws(() => parseOpportunity({ ...captured, mode, transaction_id: 'claimed-transaction' }), ApiError);
      for (const finality_status of ['PROVISIONAL', 'FINALIZED']) {
        assert.throws(() => parseOpportunity({ ...captured, mode, finality_status }), ApiError);
      }
    }
    assert.equal(parseOpportunity({ ...captured, mode: 'LIVE', evidence_label: 'REALIZED', transaction_id: 'retained-transaction', finality_status: 'FINALIZED' }).evidence_label, 'REALIZED');
  });
}
test('authentication is same-origin, CSRF is attached to commands, retries preserve caller key and payload', async () => {
  const seen: { url: string; options: RequestInit }[] = [];
  const api = new ControlApi(async (url, options) => {
    seen.push({ url: String(url), options: options! });
    return Response.json(String(url).endsWith('/auth/login') ? { operator_id: 'operator', csrf_token: 'csrf-test', expires_at: '2000000000' } : { ...receipt, session_id: 'session/with/slash' }, { status: String(url).endsWith('/auth/login') ? 200 : 202 });
  });
  await api.login('operator-test-secret');
  const body = { action: 'STOP' as const, expected_revision: '9007199254740999' };
  assert.equal((await api.command('session/with/slash', body, 'same-key')).status, 'PENDING');
  await api.command('session/with/slash', body, 'same-key');
  assert.equal(seen[0].options.credentials, 'same-origin');
  assert.equal(seen[1].url, '/v1/sessions/session%2Fwith%2Fslash/commands');
  assert.equal((seen[1].options.headers as Record<string, string>)['X-CSRF-Token'], 'csrf-test');
  assert.equal((seen[1].options.headers as Record<string, string>)['Idempotency-Key'], 'same-key');
  assert.equal(seen[1].options.body, seen[2].options.body);
});
test('rejection and invalid service responses propagate rather than returning fixtures', async () => {
  const rejected = new ControlApi(async () => Response.json({ code: 'REVISION_CONFLICT', message: 'Revision changed' }, { status: 409 }));
  await assert.rejects(rejected.command('s', { action: 'STOP', expected_revision: '1' }, 'key'), e => e instanceof ApiError && e.status === 409 && e.code === 'REVISION_CONFLICT');
  const invalid = new ControlApi(async () => new Response('<html>vite</html>', { status: 200 }));
  await assert.rejects(invalid.sessions(), /did not return JSON/);
  const synthetic = new ControlApi(async () => Response.json({ items: [fixture], next_cursor: null }));
  await assert.rejects(synthetic.opportunities(), /Synthetic records/);
});
test('unauthorized responses remove the in-memory CSRF token', async () => {
  let calls = 0; let finalHeaders: Record<string, string> = {};
  const api = new ControlApi(async (_url, options) => {
    calls += 1;
    if (calls === 1) return Response.json({ operator_id: 'operator', csrf_token: 'csrf-test', expires_at: '2000000000' });
    if (calls === 2) return Response.json({ code: 'AUTH_REQUIRED', message: 'Expired' }, { status: 401 });
    finalHeaders = options!.headers as Record<string, string>; return Response.json({ ...receipt, session_id: 's' }, { status: 202 });
  });
  await api.auth(); await assert.rejects(api.sessions(), ApiError);
  await api.command('s', { action: 'STOP', expected_revision: '1' }, 'key');
  assert.equal(finalHeaders['X-CSRF-Token'], undefined);
});

test('default transport invokes native fetch with its global receiver', async () => {
  const original = globalThis.fetch;
  let called = false;
  globalThis.fetch = async function (this: unknown) {
    // Browser fetch is a Web API method; Node fetch does not enforce its
    // receiver, so ordinary injected transport tests do not catch this bug.
    assert.equal(this, globalThis, 'Native fetch lost its Window/global receiver');
    called = true;
    return Response.json({ operator_id: 'operator', csrf_token: 'csrf-test', expires_at: '2000000000' });
  };
  try { await new ControlApi().auth(); assert.equal(called, true); }
  finally { globalThis.fetch = original; }
});
