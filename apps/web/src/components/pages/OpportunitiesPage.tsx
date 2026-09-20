import type { Capabilities, ControlApi, Network, Opportunity, Session } from '../../api/client';
import type { Page } from '../../ConnectedApp';
import { DecisionExplorer } from '../DecisionExplorer';
import { RecordedSection } from './OverviewPage';

export function OpportunitiesPage({ page, rows, opportunityCapture, nextCursor, inspect, active, api, capabilities, sessions, filter, disabled, canRetry, onPendingChange }: {
  page: Page;
  rows: Opportunity[]; opportunityCapture: boolean; nextCursor: string | null; inspect: (opportunity: Opportunity) => void;
  active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all'; disabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void;
}) {
  return <>
    {page === 'Opportunities' && <RecordedSection rows={rows} opportunityCapture={opportunityCapture} nextCursor={nextCursor} inspect={inspect} />}
    <div hidden={page !== 'Opportunities'}><DecisionExplorer active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} disabled={disabled} canRetry={canRetry} onPendingChange={onPendingChange} /></div>
  </>;
}
