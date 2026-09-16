import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ControlApi } from '../src/api/client.ts';
const auth = { operator_id: 'operator', csrf_token: 'csrf-account', expires_at: '2000000000' };
test('account login sends credentials only as JSON and keeps existing CSRF contract', async () => {
  const calls: [string, RequestInit | undefined][] = [];
  const api = new ControlApi(async (url, init) => { calls.push([String(url), init]); return Response.json(auth); });
  await api.signIn('owner@example.test', 'long-password-for-test');
  assert.equal(api.isAuthorized(), true); assert.equal(calls[0][0], '/v1/auth/sign-in');
  assert.deepEqual(JSON.parse(String(calls[0][1]?.body)), { email: 'owner@example.test', password: 'long-password-for-test' });
  assert.equal(calls[0][1]?.credentials, 'same-origin'); assert.equal(calls[0][1]?.cache, 'no-store');
});
test('activation does not claim an authenticated session and never sends token in URL', async () => {
  let target = ''; let sent: unknown;
  const api = new ControlApi(async (url, init) => { target = String(url); sent = JSON.parse(String(init?.body)); return new Response(null, { status: 204 }); });
  await api.activateAccount('owner@example.test', 'long-new-password', 'a'.repeat(64));
  assert.equal(target, '/v1/auth/activate'); assert.equal(api.isAuthorized(), false);
  assert.deepEqual(sent, { email: 'owner@example.test', password: 'long-new-password', token: 'a'.repeat(64) });
});
test('password update sends CSRF and clears authorization only on success', async () => {
  let headers: Headers | undefined;
  const api = new ControlApi(async (url, init) => {
    if (String(url).endsWith('sign-in')) return Response.json(auth);
    headers = new Headers(init?.headers); return new Response(null, { status: 204 });
  });
  await api.signIn('owner@example.test', 'old-long-password');
  await api.changePassword('old-long-password', 'new-long-password');
  assert.equal(headers?.get('x-csrf-token'), auth.csrf_token); assert.equal(api.isAuthorized(), false);
});
test('production App imports no demo and the UI cannot start real trading', () => {
  const app = readFileSync(new URL('../src/App.tsx', import.meta.url), 'utf8');
  assert.doesNotMatch(app, /import.*DemoApp/);
  const workspace = readFileSync(new URL('../src/ConnectedApp.tsx', import.meta.url), 'utf8');
  assert.doesNotMatch(workspace, /onDemo|>Connect API<|>Open demo</);
  assert.match(workspace, /disabled>Start real trading</);
});
