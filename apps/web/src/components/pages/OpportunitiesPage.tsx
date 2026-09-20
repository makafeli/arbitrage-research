import { useState } from 'react';
import type { Capabilities, ControlApi, Network, Opportunity, Session } from '../../api/client';
import type { Page } from '../../ConnectedApp';
import { DecisionExplorer } from '../DecisionExplorer';
import type { DecisionView } from '../DecisionExplorer';
import { Tabs, TabPanel } from '../ui/Tabs';
import type { Tab } from '../ui/Tabs';
import { RecordedSection } from './OverviewPage';

export type OppTab = 'opp-captured' | 'opp-decisions' | 'opp-costs' | 'opp-exports';
const tabs: readonly Tab<OppTab>[] = [
  { key: 'opp-captured', label: 'Captured' },
  { key: 'opp-decisions', label: 'Decisions' },
  { key: 'opp-costs', label: 'Costs' },
  { key: 'opp-exports', label: 'Exports' },
];
function toView(tab: OppTab): DecisionView | null { return tab === 'opp-decisions' ? 'decisions' : tab === 'opp-costs' ? 'costs' : tab === 'opp-exports' ? 'exports' : null; }
function toTab(view: DecisionView): OppTab { return view === 'costs' ? 'opp-costs' : view === 'exports' ? 'opp-exports' : 'opp-decisions'; }

export function OpportunitiesPage({ page, rows, opportunityCapture, nextCursor, inspect, active, api, capabilities, sessions, filter, disabled, canRetry, onPendingChange }: {
  page: Page;
  rows: Opportunity[]; opportunityCapture: boolean; nextCursor: string | null; inspect: (opportunity: Opportunity) => void;
  active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all'; disabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void;
}) {
  const [tab, setTab] = useState<OppTab>('opp-decisions');
  return <>
    {page === 'Opportunities' && <>
      <Tabs label="Opportunities sections" tabs={tabs} value={tab} onChange={setTab} />
      <TabPanel tab="opp-captured" value={tab}><RecordedSection rows={rows} opportunityCapture={opportunityCapture} nextCursor={nextCursor} inspect={inspect} /></TabPanel>
    </>}
    <div hidden={page !== 'Opportunities'}><DecisionExplorer active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} disabled={disabled} canRetry={canRetry} onPendingChange={onPendingChange} view={toView(tab)} onViewChange={view => setTab(toTab(view))} /></div>
  </>;
}
