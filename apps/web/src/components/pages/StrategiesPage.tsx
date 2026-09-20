import type { Capabilities } from '../../api/client';
import { Empty, names } from '../SessionCard';

export function StrategiesPage({ capabilities }: { capabilities: Capabilities }) {
  return <section className="panel"><h2>Validated configuration registry</h2><p className="muted space-top">Configurations are immutable server references. A changed experiment requires a newly validated configuration.</p>{capabilities.registered_configurations.length ? capabilities.registered_configurations.map(config => <div className="listrow" key={config.configuration_digest}><div><h3>{config.mode} · {config.enabled_networks.map(n => names[n]).join(', ') || 'No enabled networks'}</h3><p className="mono">{config.configuration_digest}</p><p>Strategies: {config.strategy_ids.join(', ') || 'None registered'}</p></div><span className="pill">IMMUTABLE</span></div>) : <Empty title="No configurations registered" text="An operator must validate and register a research configuration before a session can be created." />}</section>;
}
