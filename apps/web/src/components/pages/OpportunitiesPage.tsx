import { useState } from 'react';
import type { Capabilities, ControlApi, Network, Opportunity, Session } from '../../api/client';
import { DecisionExplorer } from '../DecisionExplorer';
import type { OppTab } from '../DecisionExplorer';
import { Tabs, TabPanel, focusTab } from '../ui/Tabs';
import type { Tab } from '../ui/Tabs';
import { RecordedSection } from './OverviewPage';

const tabs: readonly Tab<OppTab>[] = [
  { key: 'opp-captured', label: 'Captured' },
  { key: 'opp-decisions', label: 'Decisions' },
  { key: 'opp-costs', label: 'Costs' },
  { key: 'opp-exports', label: 'Exports' },
];

export function OpportunitiesPage({ hidden, rows, opportunityCapture, nextCursor, inspect, active, api, capabilities, sessions, filter, disabled, canRetry, onPendingChange }: {
  hidden: boolean;
  rows: Opportunity[]; opportunityCapture: boolean; nextCursor: string | null; inspect: (opportunity: Opportunity) => void;
  active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all'; disabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void;
}) {
  const [tab, setTab] = useState<OppTab>('opp-decisions');
  return <div hidden={hidden}>
    <Tabs label="Opportunities sections" tabs={tabs} value={tab} onChange={setTab} />
    <TabPanel tab="opp-captured" value={tab}>{!hidden && <RecordedSection rows={rows} opportunityCapture={opportunityCapture} nextCursor={nextCursor} inspect={inspect} />}</TabPanel>
    <DecisionExplorer active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} disabled={disabled} canRetry={canRetry} onPendingChange={onPendingChange} view={tab} onViewChange={view => { setTab(view); focusTab(view); }} />
  </div>;
}
