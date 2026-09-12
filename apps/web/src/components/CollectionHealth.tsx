import { useState } from 'react';
import type { Capabilities, ControlApi, Network, Session } from '../api/client';
import type { CollectionAttempt, CollectionReason } from '../api/collection';
import { useResearchPagination, useResearchResource } from '../hooks/useResearchResource';
import { EmptyResearch, Exact, Pagination, ResourceStatus } from './ResearchShared';

const guidance: Record<CollectionReason, string> = {
  PROVIDER_UNAVAILABLE: 'Check the configured provider availability and credentials in the service environment, then inspect the next recorded attempt.',
  INPUT_VALIDATION_FAILED: 'Review the pool registry and the frozen configuration for unsupported or inconsistent inputs.',
  CAPTURE_STORAGE_UNAVAILABLE: 'Check the capture store and database availability before starting another research attempt.',
  RESOURCE_LIMIT: 'Review the configured pool count and capture bounds. Reduce the workload before retrying.',
  ACQUISITION_UNAVAILABLE: 'Inspect provider availability and the configured pool inputs before retrying collection.',
  ACQUISITION_DEADLINE: 'Check provider latency and request limits. This attempt exceeded its acquisition budget.',
  EVALUATION_REJECTED: 'Review supported route inputs and the frozen strategy configuration.',
  EVALUATION_DEADLINE: 'Review evaluation workload and resource pressure. This attempt exceeded its evaluation budget.',
  GENERATION_FENCED: 'Compare this generation with the current session command. A stopped or superseded generation cannot admit decisions.',
  WORKER_SHUTDOWN: 'Check the intended worker lifecycle and restart status. Shutdown does not establish a completed research batch.',
  TASK_FAILED: 'Inspect the worker health and restricted service diagnostics, then verify a later attempt completes.',
};
function advice(attempt: CollectionAttempt) {
  if (attempt.outcome === 'IN_PROGRESS') return 'No terminal outcome recorded. The batch may still be running or may have been interrupted. Compare its start time with the worker heartbeat; do not count it as success or failure.';
  if (attempt.reason) return guidance[attempt.reason];
  if (attempt.outcome === 'READINESS_COMPLETED') return 'Readiness capture completed. This batch does not establish research decisions or continuous market coverage.';
  return 'Decisions were durably admitted for this batch. Their quote, rejection and data quality evidence remains separate from executable results.';
}
interface Props { active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all' }
export function CollectionHealth({ active, api, capabilities, sessions, filter }: Props) {
  const [sessionId, setSessionId] = useState('');
  const selected = sessions.find(session => session.session_id === sessionId);
  const enabled = active && capabilities.collection_telemetry === true && Boolean(selected);
  const pages = useResearchPagination(sessionId);
  const coverage = useResearchResource(enabled, 'collection-coverage:' + sessionId, signal => api.collectionCoverage(sessionId, signal));
  const attempts = useResearchResource(enabled, 'collection-attempts:' + sessionId + ':' + (pages.cursor ?? ''), signal => api.collectionAttempts(sessionId, pages.cursor, signal));
  const options = sessions.filter(session => filter === 'all' || session.network_id === filter || session.session_id === sessionId);
  return <section className="research-workspace section-spacer" aria-labelledby="collection-health-title"><div className="sectionhead"><div><h2 id="collection-health-title">Collection attempt health</h2><p>Durable batch outcomes from the worker, including failures before any decision exists.</p></div><span className="pill blue">RECORDED ATTEMPTS</span></div>
    {capabilities.collection_telemetry !== true ? <EmptyResearch>Collection telemetry is unavailable in this API version. Zero stored decision errors cannot establish successful acquisition.</EmptyResearch> : <>
      <div className="panel research-toolbar"><div className="research-field"><label htmlFor="collection-session">Collection health session</label><select id="collection-session" value={sessionId} onChange={event => setSessionId(event.target.value)}><option value="">Choose a session</option>{options.map(session => <option key={session.session_id} value={session.session_id}>{session.session_id} · {session.network_id} · {session.mode}</option>)}</select></div><button disabled={!enabled || coverage.loading || attempts.loading} onClick={() => { coverage.refresh(); attempts.refresh(); }}>Refresh collection health</button></div>
      {!selected ? <EmptyResearch>Select a session to inspect recorded collection attempts.</EmptyResearch> : <><p className="tiny space-top">Session scope: {sessionId}. Counts and live pages are separate received snapshots. Changing the chain filter does not change this selected scope.</p><ResourceStatus resource={coverage} />
        {coverage.data && <section className="panel space-top" aria-label="Recorded collection attempt counts"><h3>Batch outcomes · all recorded attempts</h3><div className="research-metrics">{[
          ['Recorded batch attempts', coverage.data.attempts_started], ['Readiness batches', coverage.data.readiness_attempts], ['Research batches', coverage.data.research_attempts], ['No terminal outcome', coverage.data.in_progress],
          ['Readiness completed', coverage.data.readiness_completed], ['Batches with recorded decisions', coverage.data.decisions_recorded], ['Acquisition failed', coverage.data.acquisition_failed], ['Evaluation failed', coverage.data.evaluation_failed],
          ['Deadline exceeded', coverage.data.deadline_exceeded], ['Suppressed by control fence', coverage.data.suppressed], ['Worker cancelled', coverage.data.worker_cancelled], ['Decision rows recorded', coverage.data.decision_rows_recorded],
        ].map(([label, value]) => <div className="fact" key={label}><span className="metriclabel">{label}</span><strong><Exact value={value} /></strong></div>)}</div><p className="notice">Scheduled collection completeness: UNKNOWN. The denominator is recorded batch attempts. It excludes work that was never recorded and does not measure all scheduled work, market activity, opportunities or fills.</p><p className="tiny">Recorded attempt window: {coverage.data.window_start_at ?? 'Unknown'} → {coverage.data.window_end_at ?? 'Unknown'}. A batch without a terminal outcome may still be running or may have been interrupted; it is not classified as a failure.</p></section>}
        <p className="notice space-top">Attempt origin is not retained. These batches may include synthetic fixtures or real provider work; attempt counts alone cannot qualify market data provenance. Inspect linked decisions for their retained dataset origin.</p>
        <div className="sectionhead space-top"><div><h3>Live collection attempt page</h3><p>Failures, deliberate control suppression and unresolved batches retain separate outcomes.</p></div></div><ResourceStatus resource={attempts} />
        {attempts.data && !attempts.data.items.length && <EmptyResearch>No collection attempts were returned for this page. Acquisition reliability and scheduled completeness remain unknown.</EmptyResearch>}
        {attempts.data && attempts.data.items.length > 0 && <div className="research-scroll" tabIndex={0} aria-label="Collection attempt table scroll area"><table className="research-table"><caption>Recorded batch attempts and suggested checks</caption><thead><tr><th>Attempt / purpose</th><th>Outcome / scope</th><th>Recorded evidence</th><th>Suggested check</th></tr></thead><tbody>{attempts.data.items.map(attempt => <tr key={attempt.attempt_id}><td><strong className="mono">{attempt.attempt_id}</strong><span className="route-sub">{attempt.purpose} · {attempt.started_at}</span><span className="route-sub">Finished: {attempt.finished_at ?? 'No terminal timestamp'}</span></td><td><span className={'pill ' + (['IN_PROGRESS', 'ACQUISITION_FAILED', 'EVALUATION_FAILED', 'DEADLINE_EXCEEDED'].includes(attempt.outcome) ? 'amber' : '')}>{attempt.outcome === 'IN_PROGRESS' ? 'NO TERMINAL OUTCOME' : attempt.outcome.replaceAll('_', ' ')}</span><span className="route-sub">Generation {attempt.generation} · worker epoch {attempt.worker_epoch}</span><span className="route-sub">{attempt.reason ?? 'No reason code'}</span></td><td>{attempt.captured_pools} captured pools<span className="route-sub"><Exact value={attempt.decision_rows} /> decision rows</span><span className="route-sub">{attempt.elapsed_ms === null ? 'Elapsed time unavailable' : attempt.elapsed_ms + ' ms elapsed'}</span></td><td>{advice(attempt)}</td></tr>)}</tbody></table></div>}
        <Pagination label="collection attempts" page={pages.page} canPrevious={pages.canPrevious} canNext={pages.canNext(attempts.data?.next_cursor)} loading={attempts.loading} previous={pages.previous} next={() => { if (attempts.data?.next_cursor) pages.next(attempts.data.next_cursor); }} />
      </>}
    </>}
  </section>;
}
