import type { Network } from './client.ts';
import { base58Address } from './freshness.ts';

export interface AdapterProgram { role: 'FACTORY' | 'POOL_RUNTIME' | 'VENUE_PROGRAM' | 'PROGRAM_DATA'; address: string; runtime_digest: string | null }
export interface AdapterPool { pool_id: string; asset_ids: [string, string]; venue_family: 'uniswap-v3' | 'orca-whirlpools'; programs: AdapterProgram[] }
export interface AdapterNetwork {
  network_id: Network; configured_enabled: boolean;
  declared_scope: { asset_ids: string[]; pool_ids: string[]; asset_count: number; pool_count: number; identities_expanded: boolean; reason_codes: string[]; registry_digest: string | null };
  registry: { status: 'NOT_LOADED' | 'LOADED_AUTHORIZED' | 'LOADED_BLOCKED'; loaded_digest: string | null; available_digests: string[]; reason_codes: string[]; pools: AdapterPool[] };
  chain_freshness: { status: 'NOT_CONFIGURED' | 'CONFIGURED'; policy: { version: 'finalized-chain-time-v1'; max_chain_age_ms: number } | null };
  capability: { network_id: Network; venue_family: AdapterPool['venue_family']; read_decode_implemented: true; research_math_implemented: true;
    qualified_quote: false; transaction_build: false; full_transaction_simulation: false; submit: false; reason_codes: string[] };
}
export interface AdapterSupport { schema_version: '1.0.0'; catalog_version: 'immutable-scope-v1'; configurations: { configuration_digest: string; networks: AdapterNetwork[] }[] }
function valid(value: unknown, message = 'Adapter catalog does not match the structural support contract.'): asserts value { if (!value) throw new Error(message); }
function obj(v: unknown): Record<string, unknown> { valid(v && typeof v === 'object' && !Array.isArray(v)); return v as Record<string, unknown>; }
function digest(v: unknown): v is string { return typeof v === 'string' && /^sha256:[0-9a-f]{64}$/.test(v); }
function address(v: unknown, network: Network): v is string { return network === 'base-mainnet' ? typeof v === 'string' && /^0x[0-9a-f]{40}$/.test(v) : base58Address(v); }
function id(v: unknown, network: Network): v is string { return typeof v === 'string' && v.startsWith(network + ':') && address(v.slice(network.length + 1), network); }
function list<T>(v: unknown, max: number, parse: (v: unknown) => T): T[] { valid(Array.isArray(v) && v.length <= max); return v.map(parse); }
function unique(v: string[]) { valid(new Set(v).size === v.length, 'Adapter catalog repeats a scoped identity.'); return v; }
const reasonCodes = ['CURRENT_PROTOCOL_EQUIVALENCE_UNQUALIFIED', 'TOKEN_BEHAVIOR_UNQUALIFIED', 'FULL_TRANSACTION_SIMULATION_NOT_RUN'];
function parseNetwork(value: unknown): AdapterNetwork {
  const v = obj(value); valid(v.network_id === 'base-mainnet' || v.network_id === 'solana-mainnet'); const network = v.network_id;
  valid(typeof v.configured_enabled === 'boolean'); const d = obj(v.declared_scope), r = obj(v.registry), f = obj(v.chain_freshness), c = obj(v.capability);
  const ids = (raw: unknown, max: number) => unique(list(raw, max, value => { valid(id(value, network)); return value; }));
  const asset_ids = ids(d.asset_ids, 16), pool_ids = ids(d.pool_ids, 8);
  valid(typeof d.asset_count === 'number' && Number.isSafeInteger(d.asset_count) && d.asset_count >= 0 && typeof d.pool_count === 'number' && Number.isSafeInteger(d.pool_count) && d.pool_count >= 0 && typeof d.identities_expanded === 'boolean');
  const expanded = d.asset_count <= 16 && d.pool_count <= 8;
  valid(d.identities_expanded === expanded && Array.isArray(d.reason_codes) && (expanded ? d.asset_count === asset_ids.length && d.pool_count === pool_ids.length && d.reason_codes.length === 0 : !asset_ids.length && !pool_ids.length && d.reason_codes.length === 1 && d.reason_codes[0] === 'DECLARED_SCOPE_NOT_EXPANDED')); valid(d.registry_digest === null || digest(d.registry_digest));
  const available_digests = unique(list(r.available_digests, 16, value => { valid(digest(value)); return value; }));
  valid(r.loaded_digest === null || digest(r.loaded_digest) && available_digests.includes(r.loaded_digest));
  const venue: AdapterPool['venue_family'] = network === 'base-mainnet' ? 'uniswap-v3' : 'orca-whirlpools';
  const pools = list(r.pools, 8, value => {
    const p = obj(value); valid(id(p.pool_id, network) && (!expanded || pool_ids.includes(p.pool_id)) && p.venue_family === venue); const assets = ids(p.asset_ids, 2);
    valid(assets.length === 2 && (!expanded || assets.every(asset => asset_ids.includes(asset))));
    const roles = network === 'base-mainnet' ? ['FACTORY', 'POOL_RUNTIME'] : ['VENUE_PROGRAM', 'PROGRAM_DATA'];
    const programs = list(p.programs, 2, (value): AdapterProgram => { const program = obj(value);
      valid(typeof program.role === 'string' && roles.includes(program.role) && address(program.address, network));
      valid(program.role === 'VENUE_PROGRAM' ? program.runtime_digest === null : digest(program.runtime_digest));
      return { role: program.role as AdapterProgram['role'], address: program.address, runtime_digest: program.runtime_digest as string | null };
    });
    valid(programs.length === 2 && roles.every((role, i) => programs[i].role === role));
    return { pool_id: p.pool_id, asset_ids: assets as [string, string], venue_family: venue, programs };
  });
  unique(pools.map(p => p.pool_id));
  valid(r.status === 'NOT_LOADED' || r.status === 'LOADED_AUTHORIZED' || r.status === 'LOADED_BLOCKED');
  const registryReasons = list(r.reason_codes, 1, value => { valid(typeof value === 'string' && ['REGISTRY_NOT_LOADED','NETWORK_DISABLED','REGISTRY_DIGEST_MISMATCH','REGISTRY_SCOPE_MISMATCH'].includes(value)); return value; });
  const matching = d.registry_digest !== null && available_digests.includes(d.registry_digest);
  if (r.status === 'NOT_LOADED') valid(!available_digests.length && r.loaded_digest === null && !pools.length && registryReasons[0] === 'REGISTRY_NOT_LOADED');
  else {
    valid(available_digests.length > 0 && r.loaded_digest === (matching ? d.registry_digest : null));
    if (r.status === 'LOADED_AUTHORIZED') valid(v.configured_enabled && matching && !registryReasons.length && pools.length > 0, 'Loaded registry is not authorized by this immutable configuration.');
    else valid(!pools.length && registryReasons[0] === (!v.configured_enabled ? 'NETWORK_DISABLED' : !matching ? 'REGISTRY_DIGEST_MISMATCH' : 'REGISTRY_SCOPE_MISMATCH'));
  }
  let policy: AdapterNetwork['chain_freshness']['policy'] = null;
  if (f.status === 'CONFIGURED') { const p = obj(f.policy); valid(p.version === 'finalized-chain-time-v1' && typeof p.max_chain_age_ms === 'number' && Number.isSafeInteger(p.max_chain_age_ms) && p.max_chain_age_ms >= 1 && p.max_chain_age_ms <= 86400000); policy = { version: p.version, max_chain_age_ms: p.max_chain_age_ms }; }
  else valid(f.status === 'NOT_CONFIGURED' && f.policy === null);
  valid(c.network_id === network && c.venue_family === venue && c.read_decode_implemented === true && c.research_math_implemented === true
    && c.qualified_quote === false && c.transaction_build === false && c.full_transaction_simulation === false && c.submit === false, 'Adapter catalog cannot promote code support to qualified execution.');
  const reasons = unique(list(c.reason_codes, 3, value => { valid(typeof value === 'string' && reasonCodes.includes(value)); return value; })); valid(reasons.length === 3);
  return { network_id: network, configured_enabled: v.configured_enabled, declared_scope: { asset_ids, pool_ids, asset_count: d.asset_count, pool_count: d.pool_count, identities_expanded: expanded, reason_codes: d.reason_codes as string[], registry_digest: d.registry_digest },
    registry: { status: r.status, loaded_digest: r.loaded_digest as string | null, available_digests, reason_codes: registryReasons, pools },
    chain_freshness: { status: f.status as 'CONFIGURED' | 'NOT_CONFIGURED', policy },
    capability: { network_id: network, venue_family: venue, read_decode_implemented: true, research_math_implemented: true, qualified_quote: false,
      transaction_build: false, full_transaction_simulation: false, submit: false, reason_codes: reasons } };
}
export function parseAdapterSupport(value: unknown): AdapterSupport {
  const v = obj(value); valid(v.schema_version === '1.0.0' && v.catalog_version === 'immutable-scope-v1');
  const configurations = list(v.configurations, 16, value => { const c = obj(value); valid(digest(c.configuration_digest)); const networks = list(c.networks, 2, parseNetwork);
    valid(networks.length === 2 && new Set(networks.map(n => n.network_id)).size === 2); return { configuration_digest: c.configuration_digest, networks }; });
  unique(configurations.map(c => c.configuration_digest));
  return { schema_version: '1.0.0', catalog_version: 'immutable-scope-v1', configurations };
}
