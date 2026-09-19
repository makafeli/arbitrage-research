import { useEffect, useRef, useState } from 'react';
import { OpportunityTable } from './components/OpportunityTable';
import { OpportunityDialog } from './components/OpportunityDialog';
import { ChainPanels, ExperimentsView, RunsView, StrategiesView, SystemView } from './components/ResearchViews';
import { chains, opportunities, SAMPLE_CAPTURE } from './domain/fixtures';
import type { Opportunity } from './domain/fixtures';
import { initialSessions, pendingTransition, selectChains, transition } from './domain/lifecycle';
import type { Chain, ChainFilter, DemoAction, PendingOutcome } from './domain/lifecycle';

const pages = {
  overview: ['Overview', 'Research overview', 'Compare opportunities with their evidence, assumptions and data quality in view.'],
  opportunities: ['Opportunities', 'Opportunity explorer', 'Follow an observation from a quote to its supporting execution evidence.'],
  experiments: ['Experiments', 'Compare experiments', 'Test assumptions across shared periods before choosing a strategy.'],
  runs: ['Runs', 'Runs and control', 'Understand each session state and what happens after a stop request.'],
  strategies: ['Strategies', 'Strategy workspace', 'Keep chain scope, token eligibility and configuration versions explicit.'],
  system: ['System', 'System and data quality', 'Know when your observations are fresh enough to support a decision.'],
} as const;
type Page = keyof typeof pages;

export function DemoApp({ onConnect, light, onToggleTheme }: { onConnect: () => void; light: boolean; onToggleTheme: () => void }) {
  const [page, setPage] = useState<Page>('overview');
  const [filter, setFilter] = useState<ChainFilter>('all');
  const [sessions, setSessions] = useState(initialSessions);
  const [history, setHistory] = useState<string[]>(['Two stopped paper demo sessions. No workers or submitted transactions.']);
  const [pending, setPending] = useState<PendingOutcome>('INACTIVE');
  const [inspected, setInspected] = useState<Opportunity | null>(null);
  const [announcement, setAnnouncement] = useState('');
  const title = useRef<HTMLHeadingElement>(null);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const selected = selectChains(filter);
  const rows = opportunities.filter(item => selected.includes(item.chain));
  const stopping = sessions.some(session => session.state === 'PAUSING');
  const running = sessions.some(session => session.state === 'RUNNING');
  const allStopped = sessions.every(session => session.state === 'STOPPED');
  const paused = sessions.every(session => session.state === 'PAUSED');
  const stateLabel = stopping ? 'Stop requested' : running ? 'Running' : paused ? 'Paused' : 'Stopped';

  useEffect(() => () => timers.current.forEach(clearTimeout), []);

  function navigate(next: Page) {
    setPage(next);
    setAnnouncement(`${pages[next][0]} selected. Synthetic local demo content.`);
    requestAnimationFrame(() => title.current?.focus());
  }
  function log(message: string) {
    setHistory(entries => [message, ...entries].slice(0, 12));
    setAnnouncement(message);
  }
  function dispatch(action: DemoAction, chain?: Chain) {
    setSessions(current => current.map(session => !chain || session.chain === chain ? transition(session, action) : session));
  }
  function start() {
    dispatch('START');
    log('Local demo START / RESUME APPLIED to both independent paper sessions. Run scope remains Solana + Base.');
  }
  function pause() {
    dispatch('PAUSE');
    log('Local demo PAUSE APPLIED per session. Admission gates closed; observation and reconciliation would continue.');
  }
  function stop() {
    dispatch('STOP');
    log('Two local demo STOP commands accepted / PENDING. Awaiting each admission-fence acknowledgment.');
    timers.current.forEach(clearTimeout);
    timers.current = (['solana', 'base'] as const).map((chain, index) => setTimeout(() => {
      dispatch('ACK_STOP', chain);
      log(`${chains[chain].name} demo STOP APPLIED. No unresolved attempts; this session is STOPPED.`);
    }, 650 + index * 400));
  }

  return <>
    <a className="skip" href="#main">Skip to content</a>
    <div className="shell">
      <aside className="sidebar"><div className="brand"><span className="brandmark" aria-hidden="true">↗</span>Arbitrage</div><div className="subbrand">RESEARCH WORKSPACE</div><p className="navlabel">Workspace</p><nav className="nav" aria-label="Primary navigation">{(Object.keys(pages) as Page[]).map(key => <button key={key} aria-current={page === key ? 'page' : undefined} onClick={() => navigate(key)}>{pages[key][0]}</button>)}</nav><div className="sidefoot"><strong>Research first</strong>Evidence, assumptions and operational control.<br />Synthetic data only.</div></aside>
      <div className="main"><header className="topbar"><div><strong>Private workspace</strong><small>Solana + Base · Dashboard scaffold</small></div><div className="topactions"><button onClick={onConnect}>Connect API</button><span className="pill paper">PAPER MODE DEMO</span><button onClick={onToggleTheme} aria-label={`Switch to ${light ? 'dark' : 'light'} theme`}>{light ? 'Dark' : 'Light'} theme</button></div></header>
        <main className="workspace" id="main">
          <div className="demo"><strong>LOCAL SYNTHETIC DEMO</strong><span>All values, routes and outcomes are fictional examples. Controls change local interface state only. No market connection, wallet or trading.</span></div>
          <div className="pagehead"><div><h1 ref={title} tabIndex={-1}>{pages[page][1]}</h1><p className="subtitle">{pages[page][2]}</p></div><div className="filter"><label htmlFor="chain">View chain</label><select id="chain" value={filter} onChange={event => { setFilter(event.target.value as ChainFilter); setAnnouncement(`View filtered to ${event.target.selectedOptions[0].text}. Both session scopes remain unchanged.`); }} aria-describedby="filter-scope"><option value="all">All chains</option><option value="solana">Solana</option><option value="base">Base</option></select><p className="scope-note" id="filter-scope">View filter only. Run group always contains two separate chain sessions.</p></div></div>
          <section className="controlbar" aria-label="Local demo controls for both paper sessions"><div><strong><span className="dot" aria-hidden="true" />{stateLabel} <span className="pill">SYNTHETIC RUN GROUP</span></strong><p className="statustext">{stopping ? 'STOP accepted / PENDING until each local demo fence applies.' : running ? 'Two independent paper demo sessions. No worker is running.' : paused ? 'Admission closed. Observation and reconciliation would continue.' : 'No pending outcomes. Start demonstrates both chain sessions.'}</p><div className="session-states">{sessions.map(session => <span className="session-status" key={session.chain}>{chains[session.chain].name}: {session.state} · {session.commandStatus}</span>)}</div></div><div className="controls"><button className="primary" onClick={start} disabled={running || stopping}>{paused ? 'Resume demo' : 'Start demo'}</button><button onClick={pause} disabled={!running || stopping}>Pause demo</button><button onClick={stop} disabled={allStopped || stopping}>Stop demo</button></div></section>
          <section aria-label={pages[page][0]}>
            {page === 'overview' && <><ChainPanels selected={selected} /><div className="sectionhead"><div><h2>Latest observations</h2><p>Synthetic cohort · {SAMPLE_CAPTURE} · Six illustrative records</p></div><button className="textbutton" onClick={() => navigate('opportunities')}>View all →</button></div><OpportunityTable data={rows.slice(0, 4)} onInspect={setInspected} /><div className="bottomnote"><span className="noteindex" aria-hidden="true">i</span><p><strong>Evidence before totals.</strong> Candidates are quotes. “Simulated” requires complete atomic transaction simulation. “Estimated executable” adds freshness, funding, guards and a named inclusion scenario; it is still an estimate. These labels describe fictional examples.</p></div></>}
            {page === 'opportunities' && <><div className="sectionhead"><div><h2>Opportunity explorer</h2><p>Inspect each synthetic route and its assumptions.</p></div><span className="pill">{rows.length} SAMPLE RECORDS</span></div><OpportunityTable data={rows} onInspect={setInspected} /><div className="notice">No realized trades exist in this demo. Net ranges are fictional scenarios in USDC, not observed returns or live opportunities.</div></>}
            {page === 'experiments' && <ExperimentsView selected={selected} />}
            {page === 'runs' && <RunsView sessions={sessions} history={history} pending={pending} onPending={action => { setPending(value => pendingTransition(value, action)); setAnnouncement(action === 'SIMULATE' ? 'Separate synthetic scenario: stop applied, outcome unresolved, state draining.' : 'Synthetic outcome reconciled as no trade. No transaction occurred.'); }} />}
            {page === 'strategies' && <StrategiesView selected={selected} />}
            {page === 'system' && <SystemView selected={selected} />}
          </section>
          <footer className="footer"><span>Arbitrage Research · Dashboard scaffold · All data is synthetic</span><span>View filter only · No external connections</span></footer>
          <div className="sr" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
        </main>
      </div>
    </div>
    <OpportunityDialog opportunity={inspected} onClose={() => setInspected(null)} />
  </>;
}
