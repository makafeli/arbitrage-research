import { useEffect, useRef, useState } from 'react';
import { ApiError, ControlApi, availableActions, errorMessage } from './api/client';
import type { Capabilities, CommandAction, Network, Opportunity, Page as ApiPage, Session } from './api/client';
import { RecordedDialog } from './components/RecordedOpportunities';
import { AccountAccess, PasswordSettings } from './components/AccountAccess';
import { OwnerOverview } from './components/OwnerOverview';
import { RealTradingPanel } from './components/RealTradingPanel';
import { OverviewPage } from './components/pages/OverviewPage';
import { OpportunitiesPage } from './components/pages/OpportunitiesPage';
import { RunsPage } from './components/pages/RunsPage';
import { SystemPage } from './components/pages/SystemPage';
import { SessionCreation } from './components/SessionCreation';
import { StrategiesPage } from './components/pages/StrategiesPage';
import type { PendingCommand } from './components/SessionCard';

const pages = ['Overview', 'Owner overview', 'Opportunities', 'Experiments', 'Runs', 'Strategies', 'System'] as const;
type Page = typeof pages[number];
interface Snapshot { sessions: ApiPage<Session>; opportunities: ApiPage<Opportunity>; capabilities: Capabilities; at: number }

export function ConnectedApp({ dark, onToggleTheme }: { dark: boolean; onToggleTheme: () => void }) {
  const api = useRef(new ControlApi()).current;
  const [page, setPage] = useState<Page>('Overview');
  const [filter, setFilter] = useState<Network | 'all'>('all');
  const [authenticated, setAuthenticated] = useState(false);
  const [costAssessmentPending, setCostAssessmentPending] = useState(false);
  const [paperCreationPending, setPaperCreationPending] = useState(false);
  const [sessionCreationPending, setSessionCreationPending] = useState(false);
  const [authLoading, setAuthLoading] = useState(true);
  const [tradingMode, setTradingMode] = useState<'paper' | 'real'>('paper');
  const [passwordSettings, setPasswordSettings] = useState(false);
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
  useEffect(() => {
    mounted.current = true;
    const controller = new AbortController();
    // Child requests share this authorization state. Do not unmount the
    // workspace on revocation: uncertain mutation payloads/keys must survive.
    const unsubscribe = api.subscribeAuth(() => {
      if (!mounted.current) return;
      const authorized = api.isAuthorized();
      setAuthenticated(authorized);
      if (authorized) {
        setAnnouncement('Authenticated. Loading service records.');
      } else {
        setOnline(false); setInspected(null);
        setAnnouncement('Authorization unavailable. Sign in again; unresolved requests are retained.');
      }
    });
    void api.auth(controller.signal).catch(e => {
      if (!mounted.current || controller.signal.aborted) return;
      if (!(e instanceof ApiError && (e.status === 401 || e.code === 'AUTH_CONTEXT_CHANGED'))) setError(errorMessage(e));
    }).finally(() => { if (mounted.current && !controller.signal.aborted) setAuthLoading(false); });
    return () => { mounted.current = false; unsubscribe(); controller.abort(); api.clearAuth(); };
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
      const authVersion = api.authorizationVersion();
      refreshingRef.current = true; setRefreshing(true);
      try {
        // Receipts are independent of market reads; a query failure cannot
        // manufacture a worker ACK or hide an ACK that was received.
        const waiting = Object.entries(commandRef.current).filter(([, p]) => p.receipt?.status === 'PENDING');
        const offset = waiting.length ? receiptOffset.current % waiting.length : 0;
        const pending = [...waiting.slice(offset), ...waiting.slice(0, offset)].slice(0, 5);
        receiptOffset.current = offset + pending.length;
        const receipts = await Promise.allSettled(pending.map(([, p]) => api.receipt(p.receipt!.command_id, controller.signal)));
        if (!active || authVersion !== api.authorizationVersion()) return;
        receipts.forEach((result, i) => {
          const [id, previous] = pending[i];
          if (result.status === 'fulfilled') {
            updateCommands(old => old[id]?.key === previous.key ? ({ ...old, [id]: { ...old[id], receipt: result.value, error: undefined } }) : old);
            if (previous.receipt?.status !== result.value.status) setAnnouncement(`${id}: ${result.value.action} ${result.value.status}. Worker receipt refreshed.`);
          } else updateCommands(old => old[id]?.key === previous.key ? ({ ...old, [id]: { ...old[id], error: `Receipt refresh unavailable. Last status retained: ${errorMessage(result.reason)}` } }) : old);
        });
        const results = await Promise.all([api.sessions(controller.signal), api.opportunities(controller.signal), api.capabilities(controller.signal)]);
        if (!active || authVersion !== api.authorizationVersion()) return;
        setSnapshot({ sessions: results[0], opportunities: results[1], capabilities: results[2], at: Date.now() });
        setOnline(true); setError(null); failureCount.current = 0; retryAt.current = 0;
      } catch (e) {
        if (!active || controller.signal.aborted || authVersion !== api.authorizationVersion()) return;
        setOnline(false); setError(errorMessage(e));
        failureCount.current += 1; retryAt.current = Date.now() + Math.min(30_000, 5_000 * 2 ** (failureCount.current - 1));
      } finally { refreshingRef.current = false; if (active) setRefreshing(false); }
    }
    refreshTrigger.current = refresh;
    void refresh();
    const timer = setInterval(() => { setTick(Date.now()); if (Date.now() >= retryAt.current) void refresh(); }, 5_000);
    return () => { active = false; controller.abort(); clearInterval(timer); refreshTrigger.current = async () => {}; };
  }, [api, authenticated]);
  async function logout() {
    try { await api.logout(); if (mounted.current && !api.isAuthorized()) setAnnouncement('Signed out. Existing commands continue independently.'); }
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
      void refreshTrigger.current();
    }
  }
  function commandSummary(command: PendingCommand) { return command.sending ? 'SENDING' : command.receipt?.status ?? (command.uncertain ? 'DELIVERY UNCERTAIN' : 'REJECTED'); }
  function navigate(next: Page) { setPage(next); setAnnouncement(`${next} selected. Connected service records.`); requestAnimationFrame(() => title.current?.focus()); }
  const allSessions = (snapshot?.sessions.items ?? []).filter(session => session.mode !== 'LIVE');
  const visibleSessions = allSessions.filter(s => filter === 'all' || s.network_id === filter);
  const rows = (snapshot?.opportunities.items ?? []).filter(o => o.mode !== 'LIVE' && (filter === 'all' || o.network_id === filter));
  const stale = !online || Boolean(snapshot && tick - snapshot.at > 15_000);
  const busy = Object.values(commands).some(c => c.sending);
  const unresolvedRequest = costAssessmentPending || paperCreationPending || sessionCreationPending || Object.values(commands).some(c => c.sending || c.uncertain);
  const stoppable = allSessions.filter(s => availableActions(s).includes('STOP') && !commands[s.session_id]?.uncertain && !commands[s.session_id]?.sending && !(commands[s.session_id]?.receipt?.status === 'PENDING' && commands[s.session_id]?.body.action === 'STOP'));
  return <>{!authenticated && <AccountAccess api={api} checking={authLoading} dark={dark} onToggleTheme={onToggleTheme} />}<div hidden={!authenticated}><a className="skip" href="#main">Skip to content</a><div className="shell">
    <header className="topbar"><div className="brand"><span className="brandmark" aria-hidden="true">↗</span><span>Arbitrage.</span></div><nav className="nav" aria-label="Primary navigation" hidden={tradingMode === 'real'}>{pages.map(name => <button key={name} aria-current={page === name ? 'page' : undefined} onClick={() => navigate(name)}>{name}</button>)}</nav><nav className="trading-modes" aria-label="Trading mode"><button aria-pressed={tradingMode === 'paper'} disabled={unresolvedRequest} onClick={() => setTradingMode('paper')}>Paper trading</button><button aria-pressed={tradingMode === 'real'} disabled={unresolvedRequest} onClick={() => { setInspected(null); setTradingMode('real'); }}>Real trading</button></nav><div className="topactions"><span className="pill blue">{tradingMode === 'paper' ? 'PAPER TRADING' : 'REAL TRADING'}</span><button disabled={unresolvedRequest} onClick={() => setPasswordSettings(value => !value)}>Change password</button><button onClick={onToggleTheme} aria-label={`Switch to ${dark ? 'light' : 'dark'} theme`}>{dark ? 'Light' : 'Dark'} theme</button>{authenticated && <button disabled={unresolvedRequest} onClick={() => { void logout(); }}>Sign out</button>}</div></header>
      <main id="main" className="workspace">{passwordSettings && authenticated && <PasswordSettings api={api} onClose={() => setPasswordSettings(false)} />}<RealTradingPanel hidden={tradingMode !== 'real'} /><div hidden={tradingMode !== 'paper'}><div className="demo connected-banner"><strong>PAPER TRADING</strong><span>Research data and hypothetical accounting only. Automatic paper fills are not enabled. Closing this page does not stop workers.</span></div>
        {unresolvedRequest && <p id="unresolved-mode-note" className="notice">An unresolved request is retained in this Paper trading workspace. Mode switching is disabled until its outcome is reconciled. Reloading or closing the page discards the local retry key; inspect authoritative records before creating equivalent work.</p>}<div className="pagehead"><div><h1 ref={title} tabIndex={-1}>{page === 'Overview' ? 'Paper trading overview' : page}</h1><p className="subtitle">Inspect evidence and control each immutable research session.</p></div><div className="filter"><label htmlFor="connected-chain">View chain</label><select id="connected-chain" value={filter} onChange={event => setFilter(event.target.value as Network | 'all')} aria-describedby="connected-filter-note"><option value="all">All chains</option><option value="solana-mainnet">Solana</option><option value="base-mainnet">Base</option></select><p className="scope-note" id="connected-filter-note">View filter only. Stop controls retain their explicitly named session scope.</p></div></div>
        {error && <div className="notice error-notice" role="alert"><strong>{snapshot ? 'Connection degraded. Last known snapshot retained.' : 'Service unavailable or request rejected.'}</strong><p>{error}</p><p>No synthetic records have been substituted.</p>{authenticated && <button className="space-top" disabled={refreshing} onClick={() => { void refreshTrigger.current(); }}>Retry connection</button>}</div>}

        {authenticated && !snapshot && <section className="panel" role="status"><h2>{refreshing ? 'Loading service records…' : 'No service snapshot available'}</h2><p className="muted">Waiting for sessions, capabilities and opportunity responses.</p></section>}
        {snapshot && <><section className="controlbar" aria-label="Connected session control summary"><div><strong><span className={`pill ${stale ? 'amber' : 'blue'}`}>{stale ? 'STALE / LAST KNOWN' : 'API CONNECTED'}</span></strong><p className="statustext">Snapshot received {new Date(snapshot.at).toISOString()}. {allSessions.length} sessions loaded{snapshot.sessions.next_cursor ? ' · additional sessions are outside this page' : ''}.</p><p className="tiny">These controls target registered sessions. Separately launched capture processes have their own lifecycle. API connectivity does not establish feed freshness or worker health.</p><div className="session-states" aria-label="All loaded session command results">{Object.entries(commands).map(([id, command]) => <span key={id} className="session-status">{id}: {command.body.action} · {commandSummary(command)}</span>)}</div></div><div className="controls"><button disabled={!authenticated || busy || !stoppable.length} onClick={() => { void Promise.allSettled(stoppable.map(s => send(s, 'STOP'))); }}>Stop {snapshot.sessions.next_cursor ? 'loaded' : 'all loaded'} sessions ({stoppable.length})</button><button disabled={!authenticated || refreshing} onClick={() => { void refreshTrigger.current(); }}>Refresh</button></div></section>
          {page === 'Overview' && <OverviewPage visibleSessions={visibleSessions} commands={commands} disabled={!authenticated} stale={stale} send={send} rows={rows} opportunityCapture={snapshot.capabilities.opportunity_capture} nextCursor={snapshot.opportunities.next_cursor} inspect={setInspected} />}
          <OpportunitiesPage hidden={page !== 'Opportunities'} rows={rows} opportunityCapture={snapshot.capabilities.opportunity_capture} nextCursor={snapshot.opportunities.next_cursor} inspect={setInspected} active={authenticated && tradingMode === 'paper' && page === 'Opportunities'} api={api} capabilities={snapshot.capabilities} sessions={allSessions} filter={filter} disabled={!authenticated || stale} canRetry={authenticated} onPendingChange={setCostAssessmentPending} />
          <RunsPage hidden={page !== 'Runs'} visibleSessions={visibleSessions} commands={commands} sessionsDisabled={!authenticated} stale={stale} send={send} active={authenticated && tradingMode === 'paper' && page === 'Runs'} api={api} capabilities={snapshot.capabilities} sessions={allSessions} filter={filter} workspaceDisabled={!authenticated || stale} canRetry={authenticated} onPendingChange={setPaperCreationPending} />
          <div hidden={page !== 'Owner overview'}><OwnerOverview active={authenticated && tradingMode === 'paper' && page === 'Owner overview'} api={api} sessions={allSessions} filter={filter} commands={commands} /></div>
          <SystemPage hidden={page !== 'System'} active={authenticated && tradingMode === 'paper' && page === 'System'} available={snapshot.capabilities.adapter_support === true} api={api} filter={filter} capabilities={snapshot.capabilities} sessions={allSessions} />
          <div hidden={page !== 'Experiments'}><SessionCreation capabilities={snapshot.capabilities} api={api} disabled={!authenticated || stale} canRetry={authenticated} onPendingChange={setSessionCreationPending} onCreated={session => { setAnnouncement(`Session ${session.session_id} created ${session.observed_state}. No start command was sent.`); void refreshTrigger.current(); }} /></div>
          {page === 'Strategies' && <StrategiesPage capabilities={snapshot.capabilities} />}
        </>}
        </div><footer className="footer"><span>Arbitrage · {tradingMode === 'paper' ? 'Paper trading' : 'Real trading'}</span><span>No wallet or live execution controls</span></footer><div className="sr" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
      </main></div><RecordedDialog opportunity={inspected} onClose={() => setInspected(null)} /></div></>;
}
