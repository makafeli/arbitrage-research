import type { AdapterNetwork, AdapterSupport } from '../src/api/support.ts';
export const configDigest = 'sha256:' + 'a'.repeat(64), registryDigest = 'sha256:' + 'b'.repeat(64);
const base = (n: number) => '0x' + n.toString(16).padStart(40, '0');
export function adapterNetwork(network: 'base-mainnet' | 'solana-mainnet' = 'base-mainnet'): AdapterNetwork {
  const isBase = network === 'base-mainnet';
  const assets = isBase ? [network + ':' + base(1), network + ':' + base(2)] : [];
  const pool = network + ':' + base(3), venue = isBase ? 'uniswap-v3' as const : 'orca-whirlpools' as const;
  return { network_id: network, configured_enabled: isBase, declared_scope: { asset_ids: assets, pool_ids: isBase ? [pool] : [], asset_count: assets.length, pool_count: isBase ? 1 : 0, identities_expanded: true, reason_codes: [], registry_digest: isBase ? registryDigest : null },
    registry: { status: isBase ? 'LOADED_AUTHORIZED' : 'NOT_LOADED', loaded_digest: isBase ? registryDigest : null, available_digests: isBase ? [registryDigest] : [], reason_codes: isBase ? [] : ['REGISTRY_NOT_LOADED'],
      pools: isBase ? [{ pool_id: pool, asset_ids: [...assets] as [string, string], venue_family: 'uniswap-v3', programs: [{ role: 'FACTORY', address: base(4), runtime_digest: 'sha256:' + 'c'.repeat(64) }, { role: 'POOL_RUNTIME', address: base(3), runtime_digest: 'sha256:' + 'd'.repeat(64) }] }] : [] },
    chain_freshness: isBase ? { status: 'CONFIGURED', policy: { version: 'finalized-chain-time-v1', max_chain_age_ms: 5000 } } : { status: 'NOT_CONFIGURED', policy: null },
    capability: { network_id: network, venue_family: venue, read_decode_implemented: true, research_math_implemented: true, qualified_quote: false, transaction_build: false,
      full_transaction_simulation: false, submit: false, reason_codes: ['CURRENT_PROTOCOL_EQUIVALENCE_UNQUALIFIED','TOKEN_BEHAVIOR_UNQUALIFIED','FULL_TRANSACTION_SIMULATION_NOT_RUN'] } };
}
export function adapterSupportFixture(): AdapterSupport { return { schema_version: '1.0.0', catalog_version: 'immutable-scope-v1', configurations: [{ configuration_digest: configDigest, networks: [adapterNetwork(), adapterNetwork('solana-mainnet')] }] }; }
