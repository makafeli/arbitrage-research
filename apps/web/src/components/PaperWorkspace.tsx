import { useRef, useState } from 'react';
import type { FormEvent, ReactNode } from 'react';
import { ApiError, errorMessage } from '../api/client';
import type { Capabilities, Configuration, ControlApi, Network, Session } from '../api/client';
import type { InitialBalance, PaperRunRecord } from '../api/research';
import { unsigned } from '../api/research';
import { useResearchPagination, useResearchResource } from '../hooks/useResearchResource';
import { EmptyResearch, Exact, ExportButton, Pagination, ResourceStatus } from './ResearchShared';
import { FrozenSessionExport } from './FrozenSessionExport';
import { TabPanel } from './ui/Tabs';

export type RunsTab = 'runs-sessions' | 'runs-ledger' | 'runs-journal' | 'runs-reservations';
interface Props { active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all'; disabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void; sessionsSection: ReactNode; view: RunsTab; onViewChange: (tab: RunsTab) => void }
export function PaperWorkspace({ active, api, capabilities, sessions, filter, disabled, canRetry, onPendingChange, sessionsSection, view, onViewChange }: Props) {
  const [chosen, setChosen] = useState<Session | null>(null), [runId, setRunId] = useState(''), [creationLocked, setCreationLocked] = useState(false);
  const selected = sessions.find(session => session.session_id === chosen?.session_id) ?? chosen;
  const sessionId = selected?.session_id ?? '';
  const currentSession = sessions.some(session => session.session_id === sessionId);
  const available = capabilities.paper_ledger === true;
  // Session cards render on the Sessions tab regardless of paper-ledger availability; only the accounting body is gated.
  const workspace = available || creationLocked;
  const enabled = active && available && Boolean(sessionId);
  const pages = useResearchPagination(sessionId), journalPage = useResearchPagination(runId), reservationPage = useResearchPagination(runId);
  const runs = useResearchResource(enabled, 'runs:' + sessionId + ':' + (pages.cursor ?? ''), signal => api.paperRuns(sessionId, pages.cursor, signal));
  const run = useResearchResource(enabled && Boolean(runId), 'run:' + sessionId + ':' + runId, async signal => {
    const value = await api.paperRun(runId, signal);
    if (value.session_id !== sessionId || value.configuration_digest !== selected?.configuration_digest) throw new Error('The paper run is outside the selected frozen session.');
    return value;
  });
  const journal = useResearchResource(enabled && Boolean(runId), 'journal:' + runId + ':' + (journalPage.cursor ?? ''), signal => api.journal(runId, journalPage.cursor, signal));
  const reservations = useResearchResource(enabled && Boolean(runId), 'reservations:' + runId + ':' + (reservationPage.cursor ?? ''), signal => api.reservations(runId, reservationPage.cursor, signal));
  const options = sessions.filter(session => session.mode === 'PAPER' && (filter === 'all' || session.network_id === filter || session.session_id === sessionId));
  if (selected && !options.some(session => session.session_id === sessionId)) options.push(selected);
  const configuration = capabilities.registered_configurations.find(config => config.configuration_digest === selected?.configuration_digest);
  function refresh() { runs.refresh(); if (runId) { run.refresh(); journal.refresh(); reservations.refresh(); } }
  function onCreated(created: PaperRunRecord) { setRunId(created.run_id); runs.refresh(); run.refresh(); journal.refresh(); reservations.refresh(); }
  // Every tab panel stays mounted so each tab's aria-controls resolves; the gate text renders inside whichever panel is open.
  const gate = !workspace ? <EmptyResearch>Paper accounting is unavailable in this API version. No balances or performance estimates have been invented.</EmptyResearch>
    : !selected ? <EmptyResearch>Select a PAPER session to inspect retained runs and hypothetical accounting.</EmptyResearch> : null;
  const runGate = gate ?? (runId ? null : <EmptyResearch>Inspect a retained paper run on the Sessions tab to load its ledger, journal and reservations.</EmptyResearch>);
  return <>
        <TabPanel tab="runs-sessions" value={view}>
          {sessionsSection}
  <section className="section-spacer research-workspace" aria-labelledby="paper-workspace-title">
    <div className="sectionhead"><div><h2 id="paper-workspace-title">Paper accounting workspace</h2><p>Durable hypothetical inventory, reservations and journal evidence. Balances are not returns.</p></div><span className="pill paper">HYPOTHETICAL</span></div>
    {workspace && <div className="panel research-toolbar"><div className="research-field"><label htmlFor="paper-session">Paper session</label><select id="paper-session" value={sessionId} disabled={creationLocked} onChange={event => { setChosen(sessions.find(session => session.session_id === event.target.value) ?? null); setRunId(''); }}><option value="">Choose a PAPER session</option>{options.map(session => <option key={session.session_id} value={session.session_id}>{session.session_id} · {session.network_id} · {session.observed_state}</option>)}</select></div><button disabled={!enabled || runs.loading || run.loading || journal.loading || reservations.loading} onClick={refresh}>Refresh paper records</button></div>}
          {gate ?? (selected && <>
          <p className="tiny space-top">Session scope: {sessionId}. The chain selector filters choices only. Existing runs and their original balances remain retained when a new run is created.</p>
          <FrozenSessionExport key={sessionId} api={api} sessionId={sessionId} active={enabled} available={capabilities.session_export === true} />
          {creationLocked && <p className="notice" role="status">Paper creation scope is locked while delivery is unresolved. Retries use the original session, amounts and idempotency key.</p>}
          <PaperCreation key={sessionId} api={api} session={selected} configuration={configuration} allowed={capabilities.paper_run_creation === true} disabled={disabled || !active || !currentSession} canRetry={canRetry && active} onCreated={onCreated} onLock={value => { setCreationLocked(value); onPendingChange(value); }} />
          <div className="sectionhead space-top"><div><h3>Retained paper runs</h3><p>Up to 25 runs per page. A new run does not reset a previous ledger.</p></div></div><ResourceStatus resource={runs} />
          {runs.data && !runs.data.items.length && <EmptyResearch>No paper runs were returned for this page. No virtual funds have been initialized for this selection.</EmptyResearch>}
          {runs.data && runs.data.items.length > 0 && <div className="research-scroll"><table className="research-table"><caption>Persisted hypothetical paper runs</caption><thead><tr><th>Run / frozen configuration</th><th>Revision</th><th>Outstanding reservations</th><th>Inspect</th></tr></thead><tbody>{runs.data.items.map(item => <tr key={item.run_id}><td><strong>{item.run_id}</strong><span className="route-sub mono">{item.configuration_digest}</span><span className="route-sub">{item.created_at}</span><span className="pill paper">HYPOTHETICAL</span></td><td><Exact value={item.revision} /></td><td>{item.outstanding_reservations}</td><td><button aria-label={'Inspect paper run ' + item.run_id} aria-pressed={runId === item.run_id} onClick={() => { setRunId(item.run_id); onViewChange('runs-ledger'); }}>Inspect run</button></td></tr>)}</tbody></table></div>}
          <Pagination label="paper runs" page={pages.page} canPrevious={pages.canPrevious} canNext={pages.canNext(runs.data?.next_cursor)} loading={runs.loading} previous={pages.previous} next={() => { if (runs.data?.next_cursor) pages.next(runs.data.next_cursor); }} />
          </>)}
  </section>
        </TabPanel>
        <TabPanel tab="runs-ledger" value={view}>
          {runGate ?? <section className="space-top" aria-labelledby="paper-run-detail-title"><h3 id="paper-run-detail-title">Selected paper run: {runId}</h3><ResourceStatus resource={run} />
            {run.data && <><div className="panel space-top"><div className="sectionhead"><div><h3>Hypothetical balances · exact asset minor units</h3><p>Revision {run.data.revision}. Each asset is separate; no conversion, decimals or portfolio value are inferred.</p></div><ExportButton label="Export selected run JSON" scope={'paper-run-' + runId} data={run.data} receivedAt={run.at} /></div><p className="tiny mono">Frozen configuration: {run.data.configuration_digest}</p><div className="research-scroll"><table className="research-table"><caption>Current free, reserved and total hypothetical inventory</caption><thead><tr><th>Asset / kind</th><th>Free</th><th>Reserved</th><th>Total</th></tr></thead><tbody>{run.data.balances.map(balance => <tr key={balance.asset.kind + balance.asset.identity}><td>{balance.asset.identity}<span className="route-sub">{balance.asset.kind === 'NATIVE' ? 'NATIVE FEE ASSET' : 'TOKEN PRINCIPAL ASSET'}</span></td><td><Exact value={balance.free} /></td><td><Exact value={balance.reserved} /></td><td><Exact value={balance.total} /></td></tr>)}</tbody></table></div><details className="space-top"><summary>Immutable initial balances</summary>{run.data.initial_balances.map(balance => <p className="tiny space-top" key={balance.asset.kind + balance.asset.identity}>{balance.asset.kind} · {balance.asset.identity}: <Exact value={balance.amount} /></p>)}</details><p className="notice">HYPOTHETICAL accounting only. These balances do not establish full transaction simulation, actual fills or realized performance. Run comparisons are unavailable without comparable coverage, explicit valuation and simulation evidence.</p></div></>}
          </section>}
        </TabPanel>
        <TabPanel tab="runs-journal" value={view}>
          {runGate ?? <>
            <div className="sectionhead space-top"><div><h3>Paper journal</h3><p>Stored commands and double-entry postings, in their original asset units.</p></div><ExportButton label="Export journal page JSON" scope={'paper-journal-page-' + runId} data={journal.data} receivedAt={journal.at} /></div><ResourceStatus resource={journal} />
            {journal.data && !journal.data.items.length && <EmptyResearch>No journal entries were returned for this page.</EmptyResearch>}
            {journal.data?.items.map(item => <section className="panel space-top" key={item.event_id} aria-label={'Journal event ' + item.event.sequence}><div className="sectionhead"><div><h3>#{item.event.sequence} · {item.event.command.kind}</h3><p>{item.recorded_at} · {item.event_id}</p><p className="mono">Command: {item.event.command_id}</p></div><span className="pill paper">HYPOTHETICAL</span></div><div className="research-scroll"><table className="research-table"><caption>Postings for journal event {item.event.sequence}</caption><thead><tr><th>Asset</th><th>Account</th><th>Side</th><th>Minor units</th></tr></thead><tbody>{item.event.postings.map((posting, index) => <tr key={index}><td>{posting.asset.identity}<span className="route-sub">{posting.asset.kind}</span></td><td>{posting.account}</td><td>{posting.side}</td><td><Exact value={posting.amount} /></td></tr>)}</tbody></table></div></section>)}
            <Pagination label="journal entries" page={journalPage.page} canPrevious={journalPage.canPrevious} canNext={journalPage.canNext(journal.data?.next_cursor)} loading={journal.loading} previous={journalPage.previous} next={() => { if (journal.data?.next_cursor) journalPage.next(journal.data.next_cursor); }} />
          </>}
        </TabPanel>
        <TabPanel tab="runs-reservations" value={view}>
          {runGate ?? <>
            <h3 className="space-top">Reservation history</h3><p className="tiny">Amounts below are original reservation requests. The current balances above are authoritative for inventory still reserved.</p><ResourceStatus resource={reservations} />
            {reservations.data && !reservations.data.items.length && <EmptyResearch>No reservation records were returned for this page.</EmptyResearch>}
            {reservations.data && reservations.data.items.length > 0 && <div className="research-scroll"><table className="research-table"><caption>Original reservation requests and reconciliation states</caption><thead><tr><th>Attempt / asset</th><th>Principal requested</th><th>Original native fee budget</th><th>State</th></tr></thead><tbody>{reservations.data.items.map(item => <tr key={item.attempt_id}><td>{item.attempt_id}<span className="route-sub">{item.principal_asset}</span></td><td><Exact value={item.principal} /></td><td><Exact value={item.native_fee_budget} /></td><td><span className={'pill ' + (item.state === 'UNKNOWN' ? 'amber' : '')}>{item.state}</span></td></tr>)}</tbody></table></div>}
            <Pagination label="reservations" page={reservationPage.page} canPrevious={reservationPage.canPrevious} canNext={reservationPage.canNext(reservations.data?.next_cursor)} loading={reservations.loading} previous={reservationPage.previous} next={() => { if (reservations.data?.next_cursor) reservationPage.next(reservations.data.next_cursor); }} />
            <p className="tiny space-top">Balances, journal and reservations are separate received snapshots. Page exports contain only the selected bounded data, not a complete ledger audit. Journal export includes command kind and postings; original command inputs are omitted.</p>
          </>}
        </TabPanel>
  </>;
}
interface PendingCreation { sessionId: string; key: string; body: { initial_balances: InitialBalance[] } }
function PaperCreation({ api, session, configuration, allowed, disabled, canRetry, onCreated, onLock }: { api: ControlApi; session: Session; configuration?: Configuration; allowed: boolean; disabled: boolean; canRetry: boolean; onCreated: (run: PaperRunRecord) => void; onLock: (value: boolean) => void }) {
  const [token, setToken] = useState(''), [principal, setPrincipal] = useState(''), [native, setNative] = useState(''), [reviewed, setReviewed] = useState(false);
  const [pending, setPending] = useState<PendingCreation | null>(null), [created, setCreated] = useState<PaperRunRecord | null>(null);
  const [sending, setSending] = useState(false), [error, setError] = useState('');
  const lock = useRef(false);
  const assets = configuration?.paper_assets?.filter(asset => asset.network_id === session.network_id) ?? [];
  const tokens = assets.filter(item => item.asset.kind === 'TOKEN');
  const selectedToken = tokens.find(item => item.asset.identity === token)?.asset;
  const nativeAsset = assets.find(item => item.asset.kind === 'NATIVE' && item.asset.identity === session.network_id)?.asset;
  const sessionReady = session.mode === 'PAPER' && session.observed_state === 'STOPPED' && !session.execution_authorized;
  const registered = configuration?.mode === 'PAPER' && configuration.enabled_networks.includes(session.network_id) && configuration.configuration_digest === session.configuration_digest;
  const supported = allowed && registered && Boolean(tokens.length && nativeAsset);
  const valid = supported && sessionReady && Boolean(selectedToken) && unsigned(principal) && BigInt(principal) > 0n && unsigned(native) && reviewed;
  async function create(event: FormEvent) {
    event.preventDefault(); if (lock.current || (pending ? !canRetry : disabled || !valid || created)) return;
    const wasUncertain = pending !== null;
    const request = pending ?? { sessionId: session.session_id, key: crypto.randomUUID(), body: { initial_balances: [{ asset: selectedToken!, amount: principal }, { asset: nativeAsset!, amount: native }] } };
    lock.current = true; setSending(true); setError(''); setPending(request); onLock(true);
    try {
      const value = await api.createPaperRun(request.sessionId, request.body, request.key);
      if (value.configuration_digest !== session.configuration_digest) throw new Error('The creation response does not match the frozen configuration.');
      setCreated(value); setPending(null); onLock(false); onCreated(value);
    } catch (e) {
      const rejected = e instanceof ApiError && [400, 401, 403, 404, 409, 422, 429].includes(e.status);
      setError(rejected && !wasUncertain ? errorMessage(e) : 'Creation delivery is uncertain. Retry the same request to reconcile its durable result.');
      if (rejected && !wasUncertain) { setPending(null); onLock(false); }
    } finally { setSending(false); lock.current = false; }
  }
  return <section className="panel space-top" aria-labelledby="paper-create-title"><h3 id="paper-create-title">Initialize a new hypothetical run</h3><p className="muted space-top">A new ledger uses this session's frozen configuration. Existing runs and initial balances remain unchanged.</p>
    {!allowed ? <p className="notice">Creation unavailable: the API has not advertised paper run creation.</p> : !registered ? <p className="notice">Creation unavailable: the session's exact PAPER configuration is not enabled in the registry.</p> : !tokens.length || !nativeAsset ? <p className="notice">Creation unavailable: the validated configuration must expose token principal and a separate native fee asset.</p> : !sessionReady ? <p className="notice">Creation requires a STOPPED PAPER session. The service atomically checks pending commands before initializing a run.</p> : null}
    <form className="connected-form" onSubmit={event => { void create(event); }}><fieldset disabled={disabled || !supported || !sessionReady || sending || Boolean(pending) || Boolean(created)}>
      <label htmlFor="paper-principal-asset">Validated principal asset</label><select id="paper-principal-asset" value={token} onChange={event => { setToken(event.target.value); setReviewed(false); }} required><option value="">Choose a validated token</option>{tokens.map(item => <option key={item.asset.identity} value={item.asset.identity}>{item.asset.identity}</option>)}</select>
      <label htmlFor="paper-principal">Initial token principal · exact minor units</label><input id="paper-principal" inputMode="numeric" pattern="[0-9]+" maxLength={78} value={principal} onChange={event => { setPrincipal(event.target.value); setReviewed(false); }} required autoComplete="off" />
      <label htmlFor="paper-native">Initial native fee reserve · exact minor units</label><input id="paper-native" inputMode="numeric" pattern="[0-9]+" maxLength={78} value={native} onChange={event => { setNative(event.target.value); setReviewed(false); }} required autoComplete="off" /><p className="tiny">Native asset: {nativeAsset?.identity ?? 'Unavailable'}. Enter an explicit amount; zero means no native fee inventory, not an unknown or free network fee.</p>
      <div className="notice"><strong>Review immutable run settings</strong><p>Session: {session.session_id} · {session.network_id} · PAPER</p><p className="mono">Configuration: {session.configuration_digest}</p><p>Principal: {token || 'Choose an asset'} · {principal || 'Not entered'} minor units</p><p>Separate native inventory: {native || 'Not entered'} minor units</p></div><label className="checkbox-label"><input type="checkbox" checked={reviewed} onChange={event => setReviewed(event.target.checked)} />I reviewed these hypothetical amounts and the frozen configuration.</label>
    </fieldset>{error && <p className="notice error-notice" role="alert">{error}</p>}<button className="primary" disabled={sending || Boolean(created) || (pending ? !canRetry : disabled || !valid)}>{created ? 'Paper run created' : sending ? 'Creating paper run…' : pending ? 'Retry same paper creation' : 'Create hypothetical paper run'}</button></form>
    {created && <div className="notice" role="status">Created {created.run_id} with immutable initial balances. HYPOTHETICAL only; no market execution was authorized.<button className="space-top" onClick={() => { setCreated(null); setToken(''); setPrincipal(''); setNative(''); setReviewed(false); }}>Prepare another new run</button></div>}
    <p className="tiny space-top">Amounts use canonical decimal integer strings. No wallet keys, token approvals, settlement or ledger reset controls are provided.</p>
  </section>;
}
