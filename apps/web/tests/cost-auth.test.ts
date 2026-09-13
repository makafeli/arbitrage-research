import assert from 'node:assert/strict';
import test from 'node:test';
import type { TestContext } from 'node:test';
import { ApiError, ControlApi } from '../src/api/client.ts';
import { costAssessmentFixture } from './costs.fixture.ts';

// Synthetic fixtures and a held real digest exercise the complete client path.
// The cryptographic implementation, parsers and scope checks are not replaced.
const operations = ['list', 'detail', 'create'] as const;
type Operation = typeof operations[number];
function gate() {
  let release!: () => void;
  const promise = new Promise<void>(resolve => { release = resolve; });
  return { promise, release };
}
function holdDigest(t: TestContext, nth: number) {
  const entered = gate(), held = gate();
  const original = crypto.subtle.digest.bind(crypto.subtle);
  let calls = 0;
  t.mock.method(crypto.subtle, 'digest', async (...args: Parameters<typeof crypto.subtle.digest>) => {
    if (++calls === nth) { entered.release(); await held.promise; }
    return original(...args);
  });
  t.after(() => { held.release(); });
  return { entered: entered.promise, release: held.release };
}
function fixture(operation: Operation) {
  const record = costAssessmentFixture();
  const body = { observation_id: record.assessment.binding.observation_id, scenario: record.assessment.scenario };
  const key = 'fixture-original-cost-write-key';
  const requests: { body: string | undefined; key: string | null }[] = [];
  const api = new ControlApi(async (input, init) => {
    if (String(input).includes('/auth/')) return Response.json({ operator_id: 'operator', csrf_token: 'fixture-csrf', expires_at: '2000000000' });
    requests.push({ body: init?.body === undefined ? undefined : String(init.body), key: new Headers(init?.headers).get('Idempotency-Key') });
    return Response.json(operation === 'list' ? { items: [record], next_cursor: null } : record);
  });
  const invoke = (signal?: AbortSignal) => operation === 'list' ? api.costAssessments(record.assessment.binding.session_id, undefined, signal)
    : operation === 'detail' ? api.costAssessment(record.assessment.binding.session_id, record.record_id, signal)
      : api.createCostAssessment(record.assessment.binding.session_id, body, key);
  return { api, invoke, requests, record, body, key };
}
function changed(error: unknown) {
  return error instanceof ApiError && error.status === 0 && error.code === 'AUTH_CONTEXT_CHANGED';
}

for (const operation of operations) {
  for (const digest of [1, 2]) {
    for (const transition of ['revoke', 'new-login']) {
      test(`${operation}: ${transition} during digest ${digest} rejects old verified results`, { timeout: 5000 }, async t => {
        const f = fixture(operation); await f.api.auth();
        const before = f.api.authorizationVersion(), held = holdDigest(t, digest);
        const rejected = assert.rejects(f.invoke(), changed);
        await held.entered;
        if (transition === 'revoke') f.api.clearAuth();
        else await f.api.login('fixture-new-login-secret');
        const current = f.api.authorizationVersion(); assert.ok(current > before);
        held.release(); await rejected;
        assert.equal(f.api.authorizationVersion(), current);
        assert.equal(f.api.isAuthorized(), transition === 'new-login');
        assert.equal(f.requests.length, 1, 'verification rejection never dispatches a new write');
        if (operation === 'create') {
          if (!f.api.isAuthorized()) await f.api.login('fixture-retry-login-secret');
          await f.invoke();
          assert.equal(f.requests.length, 2);
          assert.deepEqual(f.requests[1], f.requests[0]);
          assert.equal(f.requests[0].key, f.key);
          assert.equal(f.requests[0].body, JSON.stringify(f.body));
        }
      });
    }
  }
  test(`${operation}: unchanged authorization preserves verified result and exact amounts`, async () => {
    const f = fixture(operation); await f.api.auth();
    const result = await f.invoke();
    assert.deepEqual(result, operation === 'list' ? { items: [f.record], next_cursor: null } : f.record);
    assert.equal(f.requests.length, 1);
  });
}
for (const operation of ['list', 'detail'] as const) {
  for (const digest of [1, 2]) {
    test(`${operation}: abort during digest ${digest} cannot return a verified late read`, { timeout: 5000 }, async t => {
      const f = fixture(operation); await f.api.auth();
      const epoch = f.api.authorizationVersion(), held = holdDigest(t, digest), controller = new AbortController();
      const rejected = assert.rejects(f.invoke(controller.signal), error => error instanceof DOMException && error.name === 'AbortError');
      await held.entered; controller.abort(); held.release(); await rejected;
      assert.equal(f.api.authorizationVersion(), epoch);
      assert.equal(f.api.isAuthorized(), true, 'cancelling a read is not global logout');
      assert.equal(f.requests.length, 1);
    });
  }
}
