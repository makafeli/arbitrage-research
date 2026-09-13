import type { ControlApi, Network } from '../api/client';
import type { AdapterNetwork } from '../api/support';
import { useResearchResource } from '../hooks/useResearchResource';

const registryLabels = { NOT_LOADED: 'Registry not loaded', LOADED_AUTHORIZED: 'Locally authorized registry', LOADED_BLOCKED: 'Loaded registry blocked' };
const registryGuidance: Record<string, string> = {
  REGISTRY_NOT_LOADED: 'No registry for this chain was loaded by this API process.', NETWORK_DISABLED: 'This immutable configuration disables this chain.',
  REGISTRY_DIGEST_MISMATCH: 'No loaded registry matches the digest declared by this configuration.', REGISTRY_SCOPE_MISMATCH: 'The selected registry does not satisfy this configuration’s pool or asset scope.',
};
function NetworkCard({ network }: { network: AdapterNetwork }) {
  const { registry, declared_scope: scope, capability, chain_freshness: freshness } = network;
  return <article className="support-card" aria-label={(network.network_id === 'base-mainnet' ? 'Base' : 'Solana') + ' adapter support'}><div className="sectionhead"><div><h4>{network.network_id === 'base-mainnet' ? 'Base' : 'Solana'}</h4><p className="tiny">{capability.venue_family}</p></div><span className={'pill ' + (network.configured_enabled ? 'blue' : 'amber')}>{network.configured_enabled ? 'Configured enabled' : 'Configured disabled'}</span></div>
    <p className="space-top"><strong>{registryLabels[registry.status]}</strong></p>{registry.reason_codes.map(reason => <p className="tiny space-top" key={reason}>{registryGuidance[reason]}</p>)}
    <dl className="support-capabilities"><dt>Read / decode code</dt><dd>Implemented</dd><dt>Research quote math</dt><dd>Implemented</dd><dt>Qualified quote</dt><dd>Unqualified</dd><dt>Transaction build</dt><dd>Unavailable</dd><dt>Full transaction simulation</dt><dd>Unavailable</dd><dt>Submission</dt><dd>Unavailable</dd></dl>
    <p className="notice">Protocol equivalence and token behavior remain unqualified. Full transaction simulation has not run.</p>
    <dl className="research-facts"><dt>Declared asset identities</dt><dd>{scope.asset_count}</dd><dt>Declared pool identities</dt><dd>{scope.pool_count}</dd><dt>Loaded authorized pool relations</dt><dd>{registry.pools.length}</dd><dt>Chain age assumption</dt><dd>{freshness.policy ? freshness.policy.max_chain_age_ms + ' ms maximum' : 'Not configured'}</dd></dl>
    <p className="tiny">These counts describe configuration scope and local registry structure. They do not measure provider coverage, available markets or executable routes.</p>
    <details className="support-scope"><summary>Inspect immutable identities</summary><p className="tiny mono">Declared registry: {scope.registry_digest ?? 'None declared'}</p><p className="tiny mono">Selected loaded registry: {registry.loaded_digest ?? 'No matching selection'}</p>
      {!scope.identities_expanded && <p className="notice">The declared scope exceeds this response’s identity expansion limit. Counts retain the full scope; no partial identity list is presented.</p>}
      {scope.identities_expanded && <><p className="tiny space-top">Declared assets</p>{scope.asset_ids.length ? <ul>{scope.asset_ids.map(id => <li className="mono" key={id}>{id}</li>)}</ul> : <p className="tiny">None declared</p>}<p className="tiny space-top">Declared pools</p>{scope.pool_ids.length ? <ul>{scope.pool_ids.map(id => <li className="mono" key={id}>{id}</li>)}</ul> : <p className="tiny">None declared</p>}</>}
      {registry.pools.map(pool => <div className="notice" key={pool.pool_id}><p className="mono">{pool.pool_id}</p><p className="tiny">{pool.asset_ids.join(' ↔ ')}</p>{pool.programs.map(program => <p className="tiny mono space-top" key={program.role}>{program.role.replaceAll('_', ' ')}: {program.address}<br />Code digest: {program.runtime_digest ?? 'Not supplied for this program role'}</p>)}</div>)}
    </details>
  </article>;
}
export function AdapterSupport({ active, available, api, filter }: { active: boolean; available: boolean; api: ControlApi; filter: Network | 'all' }) {
  const resource = useResearchResource(active && available, 'adapter-support', signal => api.adapterSupport(signal));
  return <section className="adapter-support research-workspace section-spacer" aria-labelledby="adapter-support-title"><div className="sectionhead"><div><p className="eyebrow">Code and immutable scope</p><h2 id="adapter-support-title">Adapter support catalog</h2><p>Inspect what this service implements and which loaded registry each configuration authorizes.</p></div><button disabled={!active || !available || resource.loading} onClick={resource.refresh}>Refresh adapter catalog</button></div>
    {!available && <p className="notice">Adapter support catalog is unavailable in this API version. Configuration counts do not establish adapter coverage.</p>}
    {resource.loading && <p className="notice" role="status">Loading adapter support catalog…</p>}{resource.error && <div className="notice error-notice" role="alert"><strong>{resource.data ? 'Previous adapter catalog retained.' : 'Adapter catalog unavailable.'}</strong><p>The service request failed or returned an unsupported catalog. This is a transport or contract error, not a statement that registries are absent.</p></div>}
    {resource.at !== null && <p className="tiny space-top">Catalog received {new Date(resource.at).toISOString()}. This snapshot describes the API process’s locally loaded registries; worker deployment state and provider health remain separate.</p>}
    {resource.data && <><p className="notice">Local structural authorization does not qualify current program code, providers, quotes or execution. No chain is marked ready to trade by this catalog.</p>{!resource.data.configurations.length && <p className="notice">No immutable configurations are registered in this service.</p>}{resource.data.configurations.map(config => <section className="panel support-config" key={config.configuration_digest} aria-label={'Adapter configuration ' + config.configuration_digest}><h3>Immutable configuration</h3><p className="tiny mono space-top">{config.configuration_digest}</p><div className="support-networks space-top">{config.networks.filter(n => filter === 'all' || n.network_id === filter).map(network => <NetworkCard key={network.network_id} network={network} />)}</div></section>)}</>}
  </section>;
}
