import { chains } from '../domain/fixtures';
import type { Chain, DemoSession, PendingOutcome } from '../domain/lifecycle';

export function ChainPanels({ selected }: { selected: readonly Chain[] }) {
  return <div className="panels">{selected.map(chain => {
    const item = chains[chain];
    return <section className="panel" key={chain} aria-label={`${item.name} synthetic observations`}>
      <div className="panelhead"><div className="chainname"><div><h2>{item.name}</h2><span className="tiny">{item.venue}</span></div></div><span className="pill">SYNTHETIC</span></div>
      <div className="chainmetrics"><div><span className="metriclabel">Candidates</span><strong className="metricvalue">{item.candidates}</strong></div><div><span className="metriclabel">Simulated</span><strong className="metricvalue">{item.simulated}</strong></div><div><span className="metriclabel">Estimated executable</span><strong className="metricvalue">{item.eligible}</strong></div></div>
      <div className="quality"><span>Sample window coverage <strong>{item.coverage}%</strong></span><span>{item.freshness}</span></div>
      <div className="progress" role="img" aria-label={`Synthetic observation coverage ${item.coverage} percent`}><span style={{ width: `${item.coverage}%` }} /></div>
      <p className="tiny space-top">12 Sep 2026 · 11:00–12:00 UTC · {item.quality}</p>
    </section>;
  })}</div>;
}

export function ExperimentsView({ selected }: { selected: readonly Chain[] }) {
  return <><div className="panels">{selected.map(chain => {
    const item = chains[chain];
    return <section className="panel" key={chain}><div className="sectionhead"><h2>{item.name} baseline</h2><span className="pill">SYNTHETIC</span></div><p className="muted">{item.venue} · USDC start asset · example configuration v1</p><div className="facts"><div className="fact"><span className="metriclabel">Simulation evidence</span><strong>{item.simulated} examples</strong></div><div className="fact"><span className="metriclabel">Window coverage</span><strong>{item.coverage}%</strong></div></div><div className="notice">Illustrative values only. These are fixture labels, not backtest or simulator results.</div></section>;
  })}</div><section className="panel"><div className="sectionhead"><div><h2>Compare the assumptions</h2><p>Synthetic comparison design, not a backtest result.</p></div><span className="pill">DEMONSTRATION</span></div>
    <div className="listrow"><div><h3>Common observation window</h3><p>Use the same period and report gaps before comparing chains.</p></div><div className="right"><strong>60 minutes</strong><p>Illustrative cohort</p></div></div>
    <div className="listrow"><div><h3>Submission delay scenarios</h3><p>Re-evaluate against later state; do not invent a fixed chance of winning.</p></div><div className="right"><strong>Baseline / delayed</strong><p>Named scenarios</p></div></div>
    <div className="listrow"><div><h3>Net result scope</h3><p>Separate trade costs, failed attempts and monthly infrastructure costs.</p></div><div className="right"><strong>Explicit costs</strong><p>Unknown ≠ zero</p></div></div>
  </section></>;
}

interface RunsProps {
  sessions: readonly DemoSession[];
  history: readonly string[];
  pending: PendingOutcome;
  onPending: (action: 'SIMULATE' | 'RESOLVE') => void;
}

export function RunsView({ sessions, history, pending, onPending }: RunsProps) {
  return <div className="panels"><section className="panel"><div className="sectionhead"><h2>Current demo run group</h2><span className="pill paper">PAPER</span></div><p className="muted">Two independent sessions, each fixed to one network. Workspace controls send one local demo command per session. Their acknowledgments are independent.</p><div className="facts">{sessions.map(session => <div className="fact" key={session.chain}><span className="metriclabel">{chains[session.chain].name} · PAPER session</span><strong>{session.state}</strong><p className="tiny">Last command: {session.commandStatus}</p></div>)}</div><div className="timeline">{history.map((entry, index) => <div className="event" key={`${history.length - index}-${entry}`}><span>Local interface demonstration</span>{entry}</div>)}</div></section>
    <section className="panel"><div className="sectionhead"><h2>Pending outcome example</h2><span className="pill amber">SYNTHETIC</span></div><p className="muted">A separate illustration of a previously emitted live attempt. It is not part of the two paper sessions. Nothing is signed or submitted.</p>
      <div className="notice">{pending === 'INACTIVE' ? 'No scenario active. Nothing has been sent to a blockchain.' : pending === 'DRAINING' ? 'Stop APPLIED: the worker admission fence is acknowledged. State DRAINING: one prior outcome remains unresolved. A stop cannot recall a transaction already emitted.' : 'Outcome reconciled as no trade. Pending count zero; the illustrative session is now STOPPED. No real transaction occurred.'}</div>
      <div className="controls space-top"><button onClick={() => onPending('SIMULATE')} disabled={pending === 'DRAINING'}>Simulate pending outcome</button><button onClick={() => onPending('RESOLVE')} disabled={pending !== 'DRAINING'}>Resolve demo outcome</button></div>
      <p className="tiny space-top">No signing or network requests. Resolution is an illustrative manual state change.</p>
    </section>
  </div>;
}

export function StrategiesView({ selected }: { selected: readonly Chain[] }) {
  return <><div className="cards">{selected.map(chain => {
    const item = chains[chain];
    return <section className="panel" key={chain}><div className="sectionhead"><div><h2>{item.name} · USDC cycles</h2><p>Example configuration v1 · {item.venue}</p></div><span className="pill paper">PAPER DESIGN</span></div><div className="listrow"><div><h3>Route scope</h3><p>Same-chain distinct pools within the initial venue adapter.</p></div><strong>USDC → {item.asset} → USDC</strong></div><div className="listrow"><div><h3>Eligibility</h3><p>Allowlisted tokens, precise sizing, fresh state and complete simulation evidence.</p></div><span className="pill">Versioned rules</span></div><div className="listrow"><div><h3>Live execution</h3><p>No live strategy or connected signer in this demo.</p></div><span className="pill">Unavailable</span></div></section>;
  })}</div><div className="notice">These summaries are synthetic fixtures. Editing real configurations and selecting qualified pools are later implementation work. Live enablement requires separate delivery gates.</div></>;
}

export function SystemView({ selected }: { selected: readonly Chain[] }) {
  return <><div className="panels">{selected.map(chain => {
    const item = chains[chain];
    return <section className="panel" key={chain}><div className="sectionhead"><h2>{item.name} feed example</h2><span className="pill">SYNTHETIC</span></div><div className="facts"><div className="fact"><span className="metriclabel">Age at sample capture</span><strong>{item.freshness.split(' at')[0]}</strong></div><div className="fact"><span className="metriclabel">Coverage</span><strong>{item.coverage}%</strong></div></div><div className="notice">{item.quality}. These are fixed sample values; no feed is connected.</div></section>;
  })}</div><section className="panel"><div className="sectionhead"><h2>Trust and recovery</h2><span className="pill">DESIGN REQUIREMENTS</span></div><div className="listrow"><div><h3>Observation continuity</h3><p>Expose reconnect gaps, stale state, source lag and decoding failures.</p></div><span className="pill blue">Visible quality</span></div><div className="listrow"><div><h3>Control acknowledgment</h3><p>Accepted commands remain pending until each worker confirms application.</p></div><span className="pill blue">Per-session status</span></div><div className="listrow"><div><h3>Secrets and permissions</h3><p>This local demo has no wallet links, private-key fields or signing capability.</p></div><span className="pill">Not connected</span></div></section></>;
}
