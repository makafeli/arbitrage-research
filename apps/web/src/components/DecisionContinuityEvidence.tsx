import type { ControlApi } from '../api/client';
import type { StoredDecision } from '../api/research';
import { continuityForDecision, continuityLabels, invalidationLabels } from '../api/continuity';
import { useResearchResource } from '../hooks/useResearchResource';
import { ResourceStatus } from './ResearchShared';

export function DecisionContinuityEvidence({ api, record, active }: { api: ControlApi; record: StoredDecision; active: boolean }) {
  const scope = record.trace.session_id + ':' + record.trace.observation_id + ':' + record.trace_id;
  const resource = useResearchResource(active, 'continuity:' + scope, async signal => continuityForDecision(
    await api.decisionContinuity(record.trace.session_id, record.trace.observation_id, signal), record,
  ));
  const data = resource.data;
  return <section className="panel space-top" aria-label="Source continuity">
    <div className="sectionhead"><h3>Source continuity</h3><button disabled={!active || resource.loading} onClick={resource.refresh}>Refresh source status</button></div>
    <p className="tiny">A separate database check of the sources linked to this historical decision. This does not update the captured market price or authorize trading.</p>
    <ResourceStatus resource={resource} />
    {!data && !resource.loading && <p className="notice">Source status unavailable. Missing evidence is not a healthy source.</p>}
    {data && <>
      <p className="notice"><strong>{continuityLabels[data.continuity_status]}</strong></p>
      {(!active || resource.error) && <p role="status">Last received source status only. Current continuity is unknown until a successful refresh.</p>}
      <dl className="research-facts"><dt>Database check received</dt><dd>{data.checked_at}</dd><dt>Bound capture references</dt><dd>{data.bound_count} of {data.capture_count}</dd><dt>Continuity policy</dt><dd>{data.policy_version}</dd></dl>
      {data.invalidation_reasons.map(reason => <p className="notice" key={reason}>{invalidationLabels[reason]} <span className="mono">{reason}</span></p>)}
      {data.continuity_status === 'UNTRACKED' && <p className="tiny">No complete recorded continuity links exist for this decision. Historical data is not retroactively approved.</p>}
      {data.continuity_status === 'UNVERIFIABLE' && <p className="tiny">The available source evidence or this chain is not covered by the Base continuity policy.</p>}
      <p className="tiny">Continuity only: freshness, tick coverage, simulation, costs and profitability require separate evidence. Original decision and capture history remain unchanged.</p>
    </>}
  </section>;
}
