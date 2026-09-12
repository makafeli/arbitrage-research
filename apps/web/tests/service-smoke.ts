// Run only against an isolated local control API with its disposable database.
// The actual browser client is exercised through a Node cookie/Origin shim;
// this verifies DTO and auth interoperability, not browser cookie enforcement.
import assert from 'node:assert/strict';
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
  assert.ok(caps.registered_configurations.every(config => config.enabled_networks.length === 0), 'Smoke service must use disabled research configurations.');
  assert.equal(sessions.items.length, 0, 'Smoke service must use an isolated empty database.');
  assert.equal(opportunities.items.length, 0);
  await assert.rejects(api.create({ network_id: 'base-mainnet', mode: 'PAPER', configuration_digest: 'unregistered-smoke-config', experiment_id: 'denied-smoke-experiment', strategy_ids: ['not-enabled'] }, crypto.randomUUID()), e => e instanceof ApiError && [400, 403].includes(e.status));
  await api.logout();
  await assert.rejects(api.auth(), e => e instanceof ApiError && e.status === 401);
  process.stdout.write('UI client / control API interoperability: PASS (auth, CSRF session, capabilities, empty records, rejected configuration, logout).\n');
} finally {
  // Best-effort cleanup if an assertion failed before the explicit logout.
  if (cookie && cookie !== 'arb_session=') await api.logout().catch(() => {});
  api.clearAuth();
}
