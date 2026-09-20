import type { Capabilities } from '../api/client';

const gateKeys = [
  'live_execution', 'market_data', 'opportunity_capture', 'decision_history', 'paper_ledger',
  'paper_run_creation', 'collection_telemetry', 'session_export', 'cost_assessments', 'adapter_support',
] as const;

export function CapabilitiesPanel({ capabilities }: { capabilities: Capabilities }) {
  return <>
    <section className="panel"><h2>Capabilities and data quality</h2><div className="facts"><div className="fact"><span className="metriclabel">API process market collection</span><strong>{capabilities.market_data ? 'Available' : 'Unavailable'}</strong></div><div className="fact"><span className="metriclabel">API process opportunity capture</span><strong>{capabilities.opportunity_capture ? 'Available' : 'Unavailable'}</strong></div><div className="fact"><span className="metriclabel">Coverage gaps</span><strong>Unknown</strong><p className="tiny">Stored observation bounds are available in the decision explorer. Continuous collection completeness remains unknown.</p></div><div className="fact"><span className="metriclabel">Research modes</span><strong>{capabilities.modes.join(', ') || 'None'}</strong></div></div><div className="notice">Worker heartbeat and data age are reported per record or session. HTTP connectivity cannot establish their freshness. No live controls are offered in this research interface.</div></section>
    <section className="panel space-top"><h3>Capability gates</h3><p className="tiny">Modes: {capabilities.modes.join(', ')}</p><div className="research-scroll" tabIndex={0} aria-label="Capability gates table scroll area"><table className="research-table"><caption>Feature flags reported by /v1/capabilities</caption><thead><tr><th>Capability</th><th>Status</th></tr></thead><tbody>{gateKeys.map(key => <tr key={key}><td className="mono">{key}</td><td><span className="pill">{capabilities[key] === true ? 'Available' : 'Unavailable'}</span></td></tr>)}</tbody></table></div></section>
  </>;
}
