import assert from 'node:assert/strict';
import test from 'node:test';
import { ApiError, ControlApi } from '../src/api/client.ts';

// Deterministic transport fixtures: no live identity, credential or network call.
const auth = (token = 'fixture-current-csrf') => ({ operator_id: 'operator', csrf_token: token, expires_at: '2000000000' });
const caps = { modes: ['OBSERVE'], live_execution: false, market_data: false, opportunity_capture: false, registered_configurations: [] };
const receipt = { command_id: 'fixture-command', session_id: 'fixture-session', revision: '2', status: 'PENDING', action: 'STOP', accepted_at: '2026-09-13T00:00:00Z', applied_at: null, outstanding_attempts: 0, fence_effective: false, signer_revocation_status: 'NOT_APPLICABLE' };
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes; }); return { promise, resolve }; }
function apiError(status: number, code?: string) { return (error: unknown) => error instanceof ApiError && error.status === status && (code === undefined || error.code === code); }
async function checkCsrf(api: ControlApi) { await api.command('fixture-session', { action: 'STOP', expected_revision: '1' }, 'fixture-idempotency-key'); }

for (const [label, body] of [
  ['object', '{"code":"SECRET_TOKEN","message":"private error text"}'],
  ['html', '<html>private token</html>'], ['empty', ''], ['array', '[]'],
  ['null', 'null'], ['number', '42'], ['string', '"private token"'],
] as const) {
  test(`HTTP 401 ${label} revokes authorization before parsing and never reuses CSRF`, async () => {
    let sentCsrf: string | null = null; let notices = 0;
    const api = new ControlApi(async (url, init) => {
      if (String(url).endsWith('/auth/session')) return Response.json(auth());
      if (String(url).endsWith('/commands')) { sentCsrf = new Headers(init?.headers).get('X-CSRF-Token'); return Response.json(receipt); }
      return new Response(body, { status: 401 });
    });
    await api.auth(); const before = api.authorizationVersion();
    const unsubscribe = api.subscribeAuth(() => { notices += 1; assert.equal(api.isAuthorized(), false); });
    await assert.rejects(api.capabilities(), apiError(401, 'UNAUTHENTICATED'));
    assert.equal(api.isAuthorized(), false); assert.ok(api.authorizationVersion() > before); assert.equal(notices, 1);
    await checkCsrf(api); assert.equal(sentCsrf, null);
    unsubscribe(); api.clearAuth(); assert.equal(notices, 1);
  });
}

test('401 with a never-ending body or cancellation still promptly revokes authorization', { timeout: 1000 }, async () => {
  let cancelled = false;
  const api = new ControlApi(async url => String(url).endsWith('/auth/session') ? Response.json(auth()) : new Response(new ReadableStream({ cancel() { cancelled = true; return new Promise(() => {}); } }), { status: 401 }));
  await api.auth();
  await assert.rejects(api.capabilities(), apiError(401, 'UNAUTHENTICATED'));
  assert.equal(api.isAuthorized(), false); assert.equal(cancelled, true);
});

for (const status of [403, 404]) {
  for (const body of ['[]', 'null', '<html>private scope error</html>']) {
    test(`scope failure ${status}/${body} preserves HTTP status without globally revoking authorization`, async () => {
      const api = new ControlApi(async url => String(url).endsWith('/auth/session') ? Response.json(auth()) : new Response(body, { status }));
      await api.auth(); const before = api.authorizationVersion();
      await assert.rejects(api.capabilities(), apiError(status));
      assert.equal(api.authorizationVersion(), before); assert.equal(api.isAuthorized(), true);
    });
  }
}

for (const status of [200, 401, 403]) {
  test(`old ${status} response cannot revoke or replace a newer login`, async () => {
    const held = deferred<Response>(); let csrf: string | null = null;
    const api = new ControlApi(async (url, init) => {
      if (String(url).endsWith('/auth/session')) return Response.json(auth('old-token'));
      if (String(url).endsWith('/auth/login')) return Response.json(auth('new-token'));
      if (String(url).endsWith('/commands')) { csrf = new Headers(init?.headers).get('X-CSRF-Token'); return Response.json(receipt); }
      return held.promise;
    });
    await api.auth(); const old = assert.rejects(api.capabilities(), apiError(0, 'AUTH_CONTEXT_CHANGED'));
    await api.login('fixture-operator-secret'); const current = api.authorizationVersion();
    held.resolve(Response.json(status === 200 ? caps : { message: 'old error' }, { status })); await old;
    assert.equal(api.isAuthorized(), true); assert.equal(api.authorizationVersion(), current);
    await checkCsrf(api); assert.equal(csrf, 'new-token');
  });
}

test('older session restoration cannot overwrite a later explicit login', async () => {
  const held = deferred<Response>(); let csrf: string | null = null;
  const api = new ControlApi(async (url, init) => {
    if (String(url).endsWith('/auth/session')) return held.promise;
    if (String(url).endsWith('/auth/login')) return Response.json(auth('new-token'));
    csrf = new Headers(init?.headers).get('X-CSRF-Token'); return Response.json(receipt);
  });
  const old = assert.rejects(api.auth(), apiError(0, 'AUTH_CONTEXT_CHANGED'));
  await api.login('fixture-operator-secret'); held.resolve(Response.json(auth('old-token'))); await old;
  await checkCsrf(api); assert.equal(csrf, 'new-token');
});

test('overlapping logins install only the most recently started local authentication context', async () => {
  const first = deferred<Response>(); let logins = 0; let csrf: string | null = null;
  const api = new ControlApi(async (url, init) => {
    if (String(url).endsWith('/auth/login')) return ++logins === 1 ? first.promise : Response.json(auth('second-token'));
    csrf = new Headers(init?.headers).get('X-CSRF-Token'); return Response.json(receipt);
  });
  const old = assert.rejects(api.login('first-fixture-secret'), apiError(0, 'AUTH_CONTEXT_CHANGED'));
  await api.login('second-fixture-secret'); first.resolve(Response.json(auth('first-token'))); await old;
  await checkCsrf(api); assert.equal(csrf, 'second-token');
});

for (const operation of ['auth', 'login'] as const) {
  test(`${operation} cannot resurrect local credentials after explicit invalidation`, async () => {
    const held = deferred<Response>(); const api = new ControlApi(async () => held.promise);
    const old = assert.rejects(operation === 'auth' ? api.auth() : api.login('fixture-secret'), apiError(0, 'AUTH_CONTEXT_CHANGED'));
    api.clearAuth(); held.resolve(Response.json(auth())); await old;
    assert.equal(api.isAuthorized(), false);
  });
}

test('authorization change while JSON is being decoded rejects the stale result', async () => {
  const body = deferred<unknown>(), reading = deferred<void>();
  const api = new ControlApi(async url => {
    if (String(url).endsWith('/auth/session')) return Response.json(auth());
    const response = Response.json(caps);
    response.json = () => { reading.resolve(); return body.promise; };
    return response;
  });
  await api.auth(); const old = assert.rejects(api.capabilities(), apiError(0, 'AUTH_CONTEXT_CHANGED'));
  await reading.promise; api.clearAuth(); body.resolve(caps); await old;
});

test('aborted session restoration cannot install a token after JSON completion', async () => {
  const body = deferred<unknown>(), reading = deferred<void>(); const controller = new AbortController();
  const api = new ControlApi(async () => {
    const response = Response.json(auth()); response.json = () => { reading.resolve(); return body.promise; }; return response;
  });
  const pending = assert.rejects(api.auth(controller.signal), error => error instanceof DOMException && error.name === 'AbortError');
  await reading.promise; controller.abort(); body.resolve(auth()); await pending;
  assert.equal(api.isAuthorized(), false);
});

test('stale logout completion cannot clear a newer login', async () => {
  const held = deferred<Response>();
  const api = new ControlApi(async url => String(url).endsWith('/auth/logout') ? held.promise : Response.json(auth()));
  await api.auth(); const old = assert.rejects(api.logout(), apiError(0, 'AUTH_CONTEXT_CHANGED'));
  await api.login('new-fixture-secret'); const epoch = api.authorizationVersion();
  held.resolve(new Response(null, { status: 204 })); await old;
  assert.equal(api.isAuthorized(), true); assert.equal(api.authorizationVersion(), epoch);
});

test('logout failure invalidates local credentials without claiming server-side revocation', async () => {
  const api = new ControlApi(async url => String(url).endsWith('/auth/logout') ? new Response('unavailable', { status: 503 }) : Response.json(auth()));
  await api.auth(); await assert.rejects(api.logout(), apiError(503)); assert.equal(api.isAuthorized(), false);
});

test('observer failure cannot block credential revocation or another observer', async () => {
  const api = new ControlApi(async () => Response.json(auth())); await api.auth();
  let notified = false; api.subscribeAuth(() => { throw new Error('fixture observer failure'); });
  api.subscribeAuth(() => { notified = true; }); api.clearAuth();
  assert.equal(api.isAuthorized(), false); assert.equal(notified, true);
});

test('a stale mutation response remains uncertain and its retry payload and key stay caller-owned', async () => {
  const held = deferred<Response>(); const sent: { body: string; key: string | null; csrf: string | null }[] = [];
  const api = new ControlApi(async (url, init) => {
    if (String(url).includes('/auth/')) return Response.json(auth());
    sent.push({ body: String(init?.body), key: new Headers(init?.headers).get('Idempotency-Key'), csrf: new Headers(init?.headers).get('X-CSRF-Token') });
    return sent.length === 1 ? held.promise : Response.json(receipt);
  });
  await api.auth(); const request = { action: 'STOP' as const, expected_revision: '1' }; const key = 'same-fixture-idempotency-key';
  const old = assert.rejects(api.command('fixture-session', request, key), apiError(0, 'AUTH_CONTEXT_CHANGED'));
  api.clearAuth(); held.resolve(Response.json(receipt)); await old;
  await api.login('fixture-secret'); await api.command('fixture-session', request, key);
  assert.equal(sent.length, 2); assert.equal(sent[0].body, sent[1].body); assert.equal(sent[0].key, sent[1].key);
});
