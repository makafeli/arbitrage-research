import type { CommandAction, Opportunity, Session } from '../../api/client';
import { Empty, SessionsSection } from '../SessionCard';
import type { PendingCommand } from '../SessionCard';
import { RecordedTable } from '../RecordedOpportunities';

export function RecordedSection({ rows, opportunityCapture, nextCursor, inspect }: { rows: Opportunity[]; opportunityCapture: boolean; nextCursor: string | null; inspect: (opportunity: Opportunity) => void }) {
  return <section className="section-spacer"><div className="sectionhead"><div><h2>Recorded opportunities</h2><p>{rows.length} records on the latest API page. Counts are not full-history totals.</p></div><span className="pill">CAPTURED DATA ONLY</span></div>{!opportunityCapture && <div className="notice">The API process does not collect market data. Workers may independently persist captured evidence; returned records and provider status must be assessed separately.</div>}{!rows.length ? <Empty title="No captured quote records on this page" text="This filtered response does not establish collection coverage or adapter status. Inspect sessions and decision traces for retained observations, rejection evidence and input quality." /> : <RecordedTable rows={rows} inspect={inspect} />}{nextCursor && <p className="notice">More records exist outside this page. Full-history browsing and comparison are not available in this view.</p>}</section>;
}

export function OverviewPage({ visibleSessions, commands, disabled, stale, send, rows, opportunityCapture, nextCursor, inspect }: {
  visibleSessions: Session[]; commands: Record<string, PendingCommand>; disabled: boolean; stale: boolean;
  send: (session: Session, action: CommandAction, retry?: boolean) => Promise<void>;
  rows: Opportunity[]; opportunityCapture: boolean; nextCursor: string | null; inspect: (opportunity: Opportunity) => void;
}) {
  const running = visibleSessions.filter(session => session.observed_state === 'RUNNING').length;
  const unresolvedCommands = visibleSessions.filter(session => commands[session.session_id]?.sending || commands[session.session_id]?.uncertain).length;
  return <>
    <div className="stats">
      <div className="fact"><span className="metriclabel">Sessions in view</span><strong>{visibleSessions.length}</strong></div>
      <div className="fact"><span className="metriclabel">Running</span><strong>{running}</strong></div>
      <div className="fact"><span className="metriclabel">Unresolved commands</span><strong>{unresolvedCommands}</strong></div>
      <div className="fact"><span className="metriclabel">Records on this page</span><strong>{rows.length}</strong></div>
    </div>
    <p className="tiny">Counts describe this loaded page and view filter only.</p>
    <SessionsSection visibleSessions={visibleSessions} commands={commands} disabled={disabled} stale={stale} send={send} />
    <RecordedSection rows={rows} opportunityCapture={opportunityCapture} nextCursor={nextCursor} inspect={inspect} />
  </>;
}
