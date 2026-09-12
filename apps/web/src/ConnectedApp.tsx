import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { ApiError, ControlApi, availableActions, errorMessage } from './api/client';
import type { Capabilities, CommandAction, CommandReceipt, CommandRequest, CreateSession, Network, Opportunity, Page as ApiPage, Session } from './api/client';
import { RecordedDialog, RecordedTable } from './components/RecordedOpportunities';
import { DecisionExplorer } from './components/DecisionExplorer';
import { PaperWorkspace } from './components/PaperWorkspace';

const pages = ['Overview', 'Opportunities', 'Experiments', 'Runs', 'Strategies', 'System'] as const;
type Page = typeof pages[number];
const names = { 'base-mainnet': 'Base', 'solana-mainnet': 'Solana' };
interface Snapshot { sessions: ApiPage<Session>; opportunities: ApiPage<Opportunity>; capabilities: Capabilities; at: number }
interface PendingCommand { key: string; body: CommandRequest; receipt?: CommandReceipt; error?: string; sending: boolean; uncertain: boolean }
const actions: Record<CommandAction, string> = { START: 'Start', PAUSE: 'Pause', RESUME: 'Resume', STOP: 'Stop' };

export function ConnectedApp({ onDemo }: { onDemo: () => void }) {
  const api = useRef(new ControlApi()).current;
  const [page, setPage] = useState<Page>('Overview');
  const [light, setLight] = useState(document.body.classList.contains('light'));
  const [filter, setFilter] = useState<Network | 'all'>('all');
  const [authenticated, setAuthenticated] = useState(false);
  const [paperCreationPending, setPaperCreationPending] = useState(false);
  const [sessionCreationPending, setSessionCreationPending] = useState(false);
  const [authLoading, setAuthLoading] = useState(true);
  const [secret, setSecret] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [online, setOnline] = useState(false);
  const [announcement, setAnnouncement] = useState('Checking API session.');
  const [commands, setCommands] = useState<Record<string, PendingCommand>>({});
  const commandRef = useRef(commands);
  const mounted = useRef(true);
  const refreshingRef = useRef(false);
  const receiptOffset = useRef(0);
  const retryAt = useRef(0);
  const failureCount = useRef(0);
  const [inspected, setInspected] = useState<Opportunity | null>(null);
  const title = useRef<HTMLHeadingElement>(null);
  const refreshTrigger = useRef<() => Promise<void>>(async () => {});
  const [tick, setTick] = useState(Date.now());
  useEffect(() => { document.body.classList.toggle('light', light); }, [light]);
  useEffect(() => {
    mounted.current = true;
    const controller = new AbortController();
    void api.auth(controller.signal).then(() => { if (mounted.current) setAuthenticated(true); }).catch(e => {
      if (!mounted.current || controller.signal.aborted) return;
      if (!(e instanceof ApiError && e.status === 401)) setError(errorMessage(e));
    }).finally(() => { if (mounted.current) setAuthLoading(false); });
    return () => { mounted.current = false; controller.abort(); api.clearAuth(); };
  }, [api]);
  function updateCommands(update: (previous: Record<string, PendingCommand>) => Record<string, PendingCommand>) {
    const next = update(commandRef.current); commandRef.current = next;
    if (mounted.current) setCommands(next);
  }
  useEffect(() => {
    if (!authenticated) return;
    let active = true;
    const controller = new AbortController();
    async function refresh() {
      if (refreshingRef.current || !active) return;
      refreshingRef.current = true; setRefreshing(true);
      try {
        // Receipts are independent of market reads; a query failure cannot
        // manufacture a worker ACK or hide an ACK that was received.
        const waiting = Object.entries(commandRef.current).filter(([, p]) => p.receipt?.status === 'PENDING');
        const offset = waiting.length ? receiptOffset.current % waiting.length : 0;
        const pending = [...waiting.slice(offset), ...waiting.slice(0, offset)].slice(0, 5);
        receiptOffset.current = offset + pending.length;
        const receipts = await Promise.allSettled(pending.map(([, p]) => api.receipt(p.receipt!.command_id, controller.signal)));
        if (!active) return;
        receipts.forEach((result, i) => {
          const [id, previous] = pending[i];
          if (result.status === 'fulfilled') {
            updateCommands(old => old[id]?.key === previous.key ? ({ ...old, [id]: { ...old[id], receipt: result.value, error: undefined } }) : old);
            if (previous.receipt?.status !== result.value.status) setAnnouncement(`${id}: ${result.value.action} ${result.value.status}. Worker receipt refreshed.`);
          } else updateCommands(old => old[id]?.key === previous.key ? ({ ...old, [id]: { ...old[id], error: `Receipt refresh unavailable. Last status retained: ${errorMessage(result.reason)}` } }) : old);
        });
        const results = await Promise.all([api.sessions(controller.signal), api.opportunities(controller.signal), api.capabilities(controller.signal)]);
        if (!active) return;
        setSnapshot({ sessions: results[0], opportunities: results[1], capabilities: results[2], at: Date.now() });
        setOnline(true); setError(null); failureCount.current = 0; retryAt.current = 0;
      } catch (e) {
        if (!active || controller.signal.aborted) return;
        setOnline(false); setError(errorMessage(e));
        failureCount.current += 1; retryAt.current = Date.now() + Math.min(30_000, 5_000 * 2 ** (failureCount.current - 1));
        if (e instanceof ApiError && e.status === 401) setAuthenticated(false);
      } finally { refreshingRef.current = false; if (active) setRefreshing(false); }
    }
    refreshTrigger.current = refresh;
    void refresh();
    const timer = setInterval(() => { setTick(Date.now()); if (Date.now() >= retryAt.current) void refresh(); }, 5_000);
    return () => { active = false; controller.abort(); clearInterval(timer); refreshTrigger.current = async () => {}; };
  }, [api, authenticated]);
  async function login(event: FormEvent) {
    event.preventDefault(); setAuthLoading(true); setError(null);
    const entered = secret; setSecret('');
    try { await api.login(entered); if (mounted.current) { setAuthenticated(true); setAnnouncement('Authenticated. Loading service records.'); } }
    catch (e) { if (mounted.current) setError(errorMessage(e)); }
    finally { if (mounted.current) setAuthLoading(false); }
  }
  async function logout() {
    try { await api.logout(); setAuthenticated(false); setOnline(false); setAnnouncement('Signed out. Existing commands continue independently.'); }
    catch (e) { setError(errorMessage(e)); }
  }
  async function send(session: Session, action: CommandAction, retry = false) {
    const previous = commandRef.current[session.session_id];
    if (previous?.sending || (previous?.receipt?.status === 'PENDING' && (action !== 'STOP' || previous.body.action === 'STOP')) || (!retry && previous?.uncertain)) return;
    const pending: PendingCommand = retry && previous?.uncertain ? { ...previous, sending: true, error: undefined }
      : { key: crypto.randomUUID(), body: { action, expected_revision: previous?.receipt && BigInt(previous.receipt.revision) > BigInt(session.desired_revision) ? previous.receipt.revision : session.desired_revision }, sending: true, uncertain: false };
    updateCommands(old => ({ ...old, [session.session_id]: pending }));
    try {
      const receipt = await api.command(session.session_id, pending.body, pending.key);
      if (!mounted.current) return;
      updateCommands(old => ({ ...old, [session.session_id]: { ...pending, receipt, sending: false, uncertain: false } }));
      setAnnouncement(`${session.session_id}: ${receipt.action} ${receipt.status}. Acceptance is not worker application.`);
      void refreshTrigger.current();
    } catch (e) {
      if (!mounted.current) return;
      const uncertain = Boolean(retry && previous?.uncertain) || !(e instanceof ApiError && [400, 401, 403, 404, 409, 422, 429].includes(e.status));
      updateCommands(old => ({ ...old, [session.session_id]: { ...pending, sending: false, uncertain, error: errorMessage(e) } }));
      setAnnouncement(`${session.session_id}: ${uncertain ? 'delivery uncertain; retry uses the same request' : 'command rejected'}. ${errorMessage(e)}`);
      if (e instanceof ApiError && e.status === 401) setAuthenticated(false);
      void refreshTrigger.current();
    }
  }
  function commandSummary(command: PendingCommand) { return command.sending ? 'SENDING' : command.receipt?.status ?? (command.uncertain ? 'DELIVERY UNCERTAIN' : 'REJECTED'); }
  function navigate(next: Page) { setPage(next); setAnnouncement(`${next} selected. Connected service records.`); requestAnimationFrame(() => title.current?.focus()); }
  const allSessions = snapshot?.sessions.items ?? [];
  const visibleSessions = allSessions.filter(s => filter === 'all' || s.network_id === filter);
  const rows = (snapshot?.opportunities.items ?? []).filter(o => filter === 'all' || o.network_id === filter);
  const stale = !online || Boolean(snapshot && tick - snapshot.at > 15_000);
  const busy = Object.values(commands).some(c => c.sending);
  const unresolvedRequest = paperCreationPending || sessionCreationPending || Object.values(commands).some(c => c.sending || c.uncertain);
  const stoppable = allSessions.filter(s => availableActions(s).includes('STOP') && !commands[s.session_id]?.uncertain && !commands[s.session_id]?.sending && !(commands[s.session_id]?.receipt?.status === 'PENDING' && commands[s.session_id]?.body.action === 'STOP'));
  return <><a className="skip" href="#main">Skip to content</a><div className="shell">
    <aside className="sidebar"><div className="brand"><span className="brandmark" aria-hidden="true">↗</span>Arbitrage</div><div className="subbrand">RESEARCH WORKSPACE</div><p className="navlabel">Workspace</p><nav className="nav" aria-label="Primary navigation">{pages.map((name, i) => <button key={name} aria-current={page === name ? 'page' : undefined} onClick={() => navigate(name)}><span className="navindex" aria-hidden="true">0{i + 1}</span>{name}</button>)}</nav><div className="sidefoot"><strong>Research first</strong>Persisted service evidence.<br />Dataset origins stay explicit.</div></aside>
    <div className="main"><header className="topbar"><div><strong>Private workspace</strong><small>Same-origin research control API</small></div><div className="topactions"><span className="pill blue">CONNECTED MODE</span><button onClick={onDemo} disabled={unresolvedRequest} aria-describedby={unresolvedRequest ? 'unresolved-mode-note' : undefined}>Open demo</button><button onClick={() => setLight(v => !v)} aria-label={`Switch to ${light ? 'dark' : 'light'} theme`}>{light ? 'Dark' : 'Light'} theme</button>{authenticated && <button onClick={() => { void logout(); }}>Sign out</button>}</div></header>
      <main id="main" className="workspace"><div className="demo connected-banner"><strong>CONNECTED MODE</strong><span>Records and acknowledgements come from the API. Returning to the demo or closing this page does not stop workers.</span></div>
        {unresolvedRequest && <p id="unresolved-mode-note" className="notice">An unresolved request is retained in this Connected workspace. Mode switching is disabled until its outcome is reconciled. Reloading or closing the page discards the local retry key; inspect authoritative records before creating equivalent work.</p>}<div className="pagehead"><div><p className="eyebrow">Research / {page}</p><h1 ref={title} tabIndex={-1}>{page === 'Overview' ? 'Connected research overview' : page}</h1><p className="subtitle">Inspect evidence and control each immutable research session.</p></div><div className="filter"><label htmlFor="connected-chain">View chain</label><select id="connected-chain" value={filter} onChange={event => setFilter(event.target.value as Network | 'all')} aria-describedby="connected-filter-note"><option value="all">All chains</option><option value="solana-mainnet">Solana</option><option value="base-mainnet">Base</option></select><p className="scope-note" id="connected-filter-note">View filter only. Stop controls retain their explicitly named session scope.</p></div></div>
        {error && <div className="notice error-notice" role="alert"><strong>{snapshot ? 'Connection degraded. Last known snapshot retained.' : 'Service unavailable or request rejected.'}</strong><p>{error}</p><p>No synthetic records have been substituted.</p>{authenticated && <button className="space-top" disabled={refreshing} onClick={() => { void refreshTrigger.current(); }}>Retry connection</button>}</div>}
        {!authenticated && <section className="panel login-panel" aria-labelledby="login-title"><h2 id="login-title">Connect to your research service</h2><p className="muted space-top">Use the operator secret provisioned for this private service. Credentials and CSRF tokens are not saved in browser storage.</p><form className="connected-form" onSubmit={event => { void login(event); }}><label htmlFor="operator-secret">Operator secret</label><input id="operator-secret" type="password" autoComplete="current-password" value={secret} onChange={event => setSecret(event.target.value)} required disabled={authLoading} /><button className="primary" disabled={authLoading || !secret}>{authLoading ? 'Checking connection…' : 'Sign in'}</button></form><p className="tiny space-top">An unconfigured service can return an error here. The local demo remains a separate, explicit choice.</p></section>}
        {authenticated && !snapshot && <section className="panel" role="status"><h2>{refreshing ? 'Loading service records…' : 'No service snapshot available'}</h2><p className="muted">Waiting for sessions, capabilities and opportunity responses.</p></section>}
        {snapshot && <><section className="controlbar" aria-label="Connected session control summary"><div><strong><span className={`pill ${stale ? 'amber' : 'blue'}`}>{stale ? 'STALE / LAST KNOWN' : 'API CONNECTED'}</span></strong><p className="statustext">Snapshot received {new Date(snapshot.at).toISOString()}. {allSessions.length} sessions loaded{snapshot.sessions.next_cursor ? ' · additional sessions are outside this page' : ''}.</p><p className="tiny">These controls target registered sessions. Separately launched capture processes have their own lifecycle. API connectivity does not establish feed freshness or worker health.</p><div className="session-states" aria-label="All loaded session command results">{Object.entries(commands).map(([id, command]) => <span key={id} className="session-status">{id}: {command.body.action} · {commandSummary(command)}</span>)}</div></div><div className="controls"><button disabled={!authenticated || busy || !stoppable.length} onClick={() => { void Promise.allSettled(stoppable.map(s => send(s, 'STOP'))); }}>Stop {snapshot.sessions.next_cursor ? 'loaded' : 'all loaded'} sessions ({stoppable.length})</button><button disabled={!authenticated || refreshing} onClick={() => { void refreshTrigger.current(); }}>Refresh</button></div></section>
          {(page === 'Overview' || page === 'Runs') && <><div className="sectionhead"><div><h2>Research sessions</h2><p>Observed states remain unchanged until a fresh service response arrives.</p></div></div>{!visibleSessions.length ? <Empty title="No sessions in this view" text="Create a session from an operator-validated configuration in Experiments. The chain filter changes only this view." /> : <div className="cards">{visibleSessions.map(session => <SessionCard key={session.session_id} session={session} command={commands[session.session_id]} disabled={!authenticated} stale={stale} send={send} />)}</div>}</>}
          {(page === 'Overview' || page === 'Opportunities') && <section className="section-spacer"><div className="sectionhead"><div><h2>Recorded opportunities</h2><p>{rows.length} records on the latest API page. Counts are not full-history totals.</p></div><span className="pill">CAPTURED DATA ONLY</span></div>{!snapshot.capabilities.opportunity_capture && <div className="notice">The API process does not collect market data. Workers may independently persist captured evidence; returned records and provider status must be assessed separately.</div>}{!rows.length ? <Empty title="No captured quote records on this page" text="This filtered response does not establish collection coverage or adapter status. Inspect sessions and decision traces for retained observations, rejection evidence and input quality." /> : <RecordedTable rows={rows} inspect={setInspected} />}{snapshot.opportunities.next_cursor && <p className="notice">More records exist outside this page. Full-history browsing and comparison are not available in this view.</p>}</section>}
          <div hidden={page !== 'Opportunities'}><DecisionExplorer active={authenticated && page === 'Opportunities'} api={api} capabilities={snapshot.capabilities} sessions={allSessions} filter={filter} /></div>
          <div hidden={page !== 'Runs'}><PaperWorkspace active={authenticated && page === 'Runs'} api={api} capabilities={snapshot.capabilities} sessions={allSessions} filter={filter} disabled={!authenticated || stale} canRetry={authenticated} onPendingChange={setPaperCreationPending} /></div>
          {<div hidden={page !== 'Experiments'}><SessionCreation capabilities={snapshot.capabilities} api={api} disabled={!authenticated || stale} canRetry={authenticated} onPendingChange={setSessionCreationPending} onCreated={session => { setAnnouncement(`Session ${session.session_id} created ${session.observed_state}. No start command was sent.`); void refreshTrigger.current(); }} /></div>}
          {page === 'Strategies' && <section className="panel"><h2>Validated configuration registry</h2><p className="muted space-top">Configurations are immutable server references. A changed experiment requires a newly validated configuration.</p>{snapshot.capabilities.registered_configurations.length ? snapshot.capabilities.registered_configurations.map(config => <div className="listrow" key={config.configuration_digest}><div><h3>{config.mode} · {config.enabled_networks.map(n => names[n]).join(', ') || 'No enabled networks'}</h3><p className="mono">{config.configuration_digest}</p><p>Strategies: {config.strategy_ids.join(', ') || 'None registered'}</p></div><span className="pill">IMMUTABLE</span></div>) : <Empty title="No configurations registered" text="An operator must validate and register a research configuration before a session can be created." />}</section>}
          {page === 'System' && <section className="panel"><h2>Capabilities and data quality</h2><div className="facts"><div className="fact"><span className="metriclabel">API process market collection</span><strong>{snapshot.capabilities.market_data ? 'Available' : 'Unavailable'}</strong></div><div className="fact"><span className="metriclabel">API process opportunity capture</span><strong>{snapshot.capabilities.opportunity_capture ? 'Available' : 'Unavailable'}</strong></div><div className="fact"><span className="metriclabel">Coverage gaps</span><strong>Unknown</strong><p className="tiny">Stored observation bounds are available in the decision explorer. Continuous collection completeness remains unknown.</p></div><div className="fact"><span className="metriclabel">Research modes</span><strong>{snapshot.capabilities.modes.join(', ') || 'None'}</strong></div></div><div className="notice">Worker heartbeat and data age are reported per record or session. HTTP connectivity cannot establish their freshness. No live controls are offered in this research interface.</div></section>}
        </>}
        <footer className="footer"><span>Arbitrage Research · Connected API mode</span><span>No wallet or live execution controls</span></footer><div className="sr" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
      </main></div></div><RecordedDialog opportunity={inspected} onClose={() => setInspected(null)} /></>;
}
function Empty({ title, text }: { title: string; text: string }) { return <div className="panel space-top"><h3>{title}</h3><p className="muted space-top">{text}</p></div>; }
function SessionCard({ session, command, disabled, stale, send }: { session: Session; command?: PendingCommand; disabled: boolean; stale: boolean; send: (session: Session, action: CommandAction, retry?: boolean) => Promise<void> }) {
  const receipt = command?.receipt;
  const pendingRevision = session.desired_revision !== session.applied_revision;
  const blocked = disabled || stale || command?.sending || command?.uncertain || receipt?.status === 'PENDING';
  const stopBlocked = disabled || command?.sending || command?.uncertain || (receipt?.status === 'PENDING' && command?.body.action === 'STOP');
  return <section className="panel" aria-label={`Session ${session.session_id}`}><div className="sectionhead"><div><h3>{names[session.network_id]} · {session.mode}</h3><p className="mono">{session.session_id}</p></div><span className="pill">{session.observed_state}</span></div><div className="session-states"><span className="session-status">Health: {session.health}</span><span className="session-status">Desired revision: {session.desired_revision}</span><span className="session-status">Applied revision: {session.applied_revision}</span><span className="session-status">Unresolved attempts: {session.outstanding_attempts}</span></div><p className="tiny space-top">Last worker heartbeat: {session.last_heartbeat_at ?? 'Unknown; no heartbeat reported'}</p><p className="tiny mono">Configuration: {session.configuration_digest}</p>
    {pendingRevision && !receipt && <p className="notice">Desired and applied revisions differ. This can reflect unapplied, rejected or superseded history; a revision gap alone does not establish a pending worker acknowledgement. The service validates new requests.</p>}
    {receipt && <div className="notice" role="status"><strong>{receipt.action} · {receipt.status}</strong><p>Command {receipt.command_id} · revision {receipt.revision}</p><p>{receipt.status === 'PENDING' ? 'Accepted by the API. Awaiting worker acknowledgement; the admission fence is not yet confirmed.' : receipt.status === 'APPLIED' ? `Worker acknowledged at ${receipt.applied_at}. ${receipt.fence_effective ? 'Admission fence effective.' : 'See the separately refreshed observed session state.'}` : 'This command did not apply. Review the latest session before issuing another command.'}</p></div>}
    {command?.sending && <p className="notice" role="status">Sending {command.body.action}. Acceptance is not yet confirmed.</p>}
    {command?.error && <div className="notice error-notice" role="alert"><strong>{command.uncertain ? 'Delivery uncertain' : receipt ? 'Receipt status may be stale' : 'Command rejected'}</strong><p>{command.error}</p>{command.uncertain && <p>The server may have accepted this request. Retry reuses the same idempotency key and exact payload.</p>}</div>}
    {session.observed_state === 'DRAINING' && <p className="notice">The admission fence has applied, but {session.outstanding_attempts} previously emitted attempts remain unresolved. A stop cannot recall an emitted transaction.</p>}
    <div className="controls session-actions space-top">{(['START', 'PAUSE', 'RESUME', 'STOP'] as const).map(action => <button key={action} className={action === 'START' || action === 'RESUME' ? 'primary' : ''} disabled={Boolean(action === 'STOP' ? stopBlocked : blocked) || !availableActions(session).includes(action)} onClick={() => { void send(session, action); }}>{actions[action]}</button>)}{command?.uncertain && <button disabled={disabled || command.sending} onClick={() => { void send(session, command.body.action, true); }}>Retry same request</button>}</div>
    <p className="tiny space-top">{session.mode === 'LIVE' ? 'LIVE sessions have no controls in this interface.' : disabled ? 'Controls unavailable while signed out.' : stale ? 'Snapshot is stale. Stop remains available as a request using the last known revision; only a worker receipt can confirm it.' : blocked ? 'Wait for the pending acknowledgement or resolve delivery uncertainty before another command.' : 'Controls apply only to this session. Modes and configuration cannot be changed after creation.'}</p></section>;
}
function SessionCreation({ capabilities, api, disabled, canRetry, onCreated, onPendingChange }: { capabilities: Capabilities; api: ControlApi; disabled: boolean; canRetry: boolean; onCreated: (session: Session) => void; onPendingChange: (value: boolean) => void }) {
  const [digest, setDigest] = useState(''); const [network, setNetwork] = useState<Network | ''>('');
  const [experiment, setExperiment] = useState(''); const [reviewed, setReviewed] = useState(false);
  const [pending, setPending] = useState<{ body: CreateSession; key: string } | null>(null);
  const [sending, setSending] = useState(false); const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<Session | null>(null); const lock = useRef(false);
  const selected = capabilities.registered_configurations.find(c => c.configuration_digest === digest);
  const valid = Boolean(selected && selected.enabled_networks.includes(network as Network) && selected.strategy_ids.length && capabilities.modes.includes(selected.mode) && experiment.trim() && experiment.length <= 128 && reviewed);
  async function create(event: FormEvent) {
    event.preventDefault(); if (lock.current || (pending ? !canRetry : disabled || !valid)) return;
    lock.current = true; setSending(true); setError(null);
    const wasUncertain = pending !== null;
    const request = pending ?? { key: crypto.randomUUID(), body: { configuration_digest: selected!.configuration_digest, mode: selected!.mode, network_id: network as Network, experiment_id: experiment.trim(), strategy_ids: selected!.strategy_ids } };
    setPending(request); onPendingChange(true);
    try { const session = await api.create(request.body, request.key); setCreated(session); setPending(null); onPendingChange(false); onCreated(session); }
    catch (e) { setError(errorMessage(e)); if (!wasUncertain && e instanceof ApiError && [400, 401, 403, 409, 422, 429].includes(e.status)) { setPending(null); onPendingChange(false); } }
    finally { setSending(false); lock.current = false; }
  }
  return <section className="panel"><h2>Create a research session</h2><p className="muted space-top">Choose a validated configuration. Mode, network and digest are immutable. Creation does not start a worker; recovery must finish before a stopped session can start.</p>{!capabilities.registered_configurations.length && <div className="notice">Creation unavailable: no validated configurations are registered. Ask the service operator to register an eligible research configuration.</div>}<form className="connected-form" onSubmit={event => { void create(event); }}><fieldset disabled={disabled || sending || Boolean(pending)}><label htmlFor="configuration">Validated configuration</label><select id="configuration" value={digest} onChange={event => { setDigest(event.target.value); setNetwork(''); setReviewed(false); setCreated(null); }} required><option value="">Choose a configuration</option>{capabilities.registered_configurations.map(c => <option key={c.configuration_digest} value={c.configuration_digest}>{c.mode} · {c.configuration_digest}</option>)}</select><label htmlFor="session-network">Session network</label><select id="session-network" value={network} onChange={event => { setNetwork(event.target.value as Network); setReviewed(false); setCreated(null); }} required><option value="">Choose an enabled network</option>{selected?.enabled_networks.map(n => <option key={n} value={n}>{names[n]}</option>)}</select>{selected && !selected.enabled_networks.length && <p className="notice">Creation unavailable: this configuration has no enabled networks.</p>}<label htmlFor="experiment-id">Experiment reference</label><input id="experiment-id" value={experiment} onChange={event => { setExperiment(event.target.value); setCreated(null); }} required maxLength={128} autoComplete="off" />{selected && <div className="notice"><strong>Review immutable settings</strong><p>Mode: {selected.mode} · Network: {network ? names[network] : 'Select an enabled network'}</p><p className="mono">Digest: {selected.configuration_digest}</p><p>Strategies: {selected.strategy_ids.join(', ') || 'None; configuration cannot create a session'}</p><p>Virtual principal, native fee reserves, token eligibility and delay scenarios come from the validated configuration. This API does not expose their detailed values yet; review the source configuration before confirming.</p></div>}<label className="checkbox-label"><input type="checkbox" checked={reviewed} onChange={event => setReviewed(event.target.checked)} />I reviewed the source configuration, including virtual principal and separate native fee reserves.</label></fieldset>{error && <div className="notice error-notice" role="alert">{error}{pending && <p>Creation outcome is uncertain. Retry reuses the same exact request; settings remain locked.</p>}</div>}<button className="primary" disabled={sending || Boolean(created) || (pending ? !canRetry : disabled || !valid)}>{created ? 'Session created' : sending ? 'Creating session…' : pending ? 'Retry same creation request' : 'Create research session'}</button></form>{created && <div className="notice" role="status">Created {created.session_id} · {created.observed_state}. No start command was sent.</div>}<p className="tiny space-top">LIVE is unavailable. Pool selection, amounts and scenario editing require a separately validated configuration revision.</p></section>;
}
