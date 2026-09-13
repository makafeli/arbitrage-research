import assert from 'node:assert/strict';
import test from 'node:test';
import { ApiError, ControlApi } from '../src/api/client.ts';
import { parseAdapterSupport } from '../src/api/support.ts';
import { adapterSupportFixture, registryDigest } from './support.fixture.ts';

test('catalog preserves local configuration and registry relationships without promoting quote or execution', () => {
  const input = adapterSupportFixture(), parsed = parseAdapterSupport(input); assert.deepEqual(parsed, input);
  assert.equal(parsed.configurations[0].networks[0].registry.status, 'LOADED_AUTHORIZED');
  assert.equal(parsed.configurations[0].networks[0].capability.qualified_quote, false);
  assert.equal(parsed.configurations[0].networks[1].registry.status, 'NOT_LOADED');
  const privateExtra = { ...input, raw_registry_path: '/secrets/provider/config.toml' };
  assert.equal(JSON.stringify(parseAdapterSupport(privateExtra)).includes('/secrets'), false);
});
test('catalog rejects disabled authorization, unsupported execution, wrong scope and ambiguous registry selection', () => {
  for (const mutate of [
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks[0].configured_enabled = false,
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks[0].registry.loaded_digest = 'sha256:' + 'e'.repeat(64),
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks[0].registry.pools[0].asset_ids[0] = 'base-mainnet:0x' + 'e'.repeat(40),
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks[0].capability.network_id = 'solana-mainnet',
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks[0].capability.reason_codes.pop(),
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks.push(v.configurations[0].networks[0]),
    (v: ReturnType<typeof adapterSupportFixture>) => v.configurations[0].networks[0].registry.pools[0].programs[0].runtime_digest = null,
  ]) { const value = adapterSupportFixture(); mutate(value); assert.throws(() => parseAdapterSupport(value)); }
  const promoted = adapterSupportFixture() as unknown as { configurations: { networks: { capability: Record<string, unknown> }[] }[] };
  promoted.configurations[0].networks[0].capability.qualified_quote = true; assert.throws(() => parseAdapterSupport(promoted), /cannot promote/);
});
test('large declared scopes retain actual counts and authorized concrete pools without inventing truncated allowlists', () => {
  const value = adapterSupportFixture(), scope = value.configurations[0].networks[0].declared_scope;
  scope.asset_count = 100; scope.pool_count = 50; scope.asset_ids = []; scope.pool_ids = []; scope.identities_expanded = false; scope.reason_codes = ['DECLARED_SCOPE_NOT_EXPANDED'];
  assert.equal(parseAdapterSupport(value).configurations[0].networks[0].registry.pools.length, 1);
  assert.equal(parseAdapterSupport(value).configurations[0].networks[0].declared_scope.pool_count, 50);
  const falseExpansion = structuredClone(value); falseExpansion.configurations[0].networks[0].declared_scope.identities_expanded = true; assert.throws(() => parseAdapterSupport(falseExpansion));
  const partial = structuredClone(value); partial.configurations[0].networks[0].declared_scope.pool_ids = ['base-mainnet:0x' + 'e'.repeat(40)]; assert.throws(() => parseAdapterSupport(partial));
});
test('registry absence, disabled scope and digest mismatch retain distinct reasons', () => {
  const value = adapterSupportFixture(), n = value.configurations[0].networks[0]; n.registry.status = 'LOADED_BLOCKED'; n.registry.pools = [];
  n.configured_enabled = false; n.registry.reason_codes = ['NETWORK_DISABLED']; assert.equal(parseAdapterSupport(value).configurations[0].networks[0].registry.loaded_digest, registryDigest);
  n.configured_enabled = true; n.declared_scope.registry_digest = 'sha256:' + 'e'.repeat(64); n.registry.loaded_digest = null; n.registry.reason_codes = ['REGISTRY_DIGEST_MISMATCH']; assert.doesNotThrow(() => parseAdapterSupport(value));
  n.registry.reason_codes = ['REGISTRY_NOT_LOADED']; assert.throws(() => parseAdapterSupport(value));
});
test('catalog request uses authenticated same-origin transport, cancellation and explicit errors', async () => {
  let path = '', options: RequestInit | undefined;
  const api = new ControlApi(async (url, init) => { path = String(url); options = init; return Response.json(adapterSupportFixture()); });
  const controller = new AbortController(); await api.adapterSupport(controller.signal);
  assert.equal(path, '/v1/adapter-support'); assert.equal(options?.credentials, 'same-origin'); assert.equal(options?.signal?.aborted, false);
  const unavailable = new ControlApi(async () => Response.json({ code: 'UNAVAILABLE', message: 'Fixture outage' }, { status: 503 }));
  await assert.rejects(unavailable.adapterSupport(), e => e instanceof ApiError && e.status === 503);
  const cancelling = new ControlApi(async (_url, init) => new Promise((_resolve, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')), { once: true })));
  const pending = cancelling.adapterSupport(controller.signal); controller.abort(); assert.equal(options?.signal?.aborted, true); await assert.rejects(pending, /Aborted/);
});
