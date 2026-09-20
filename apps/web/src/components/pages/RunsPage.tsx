import { useState } from 'react';
import type { Capabilities, CommandAction, ControlApi, Network, Session } from '../../api/client';
import type { Page } from '../../ConnectedApp';
import { PaperWorkspace } from '../PaperWorkspace';
import type { RunsTab } from '../PaperWorkspace';
import { SessionsSection } from '../SessionCard';
import type { PendingCommand } from '../SessionCard';
import { Tabs, focusTab } from '../ui/Tabs';
import type { Tab } from '../ui/Tabs';

const RUNS_TABS: readonly Tab<RunsTab>[] = [
  { key: 'runs-sessions', label: 'Sessions' },
  { key: 'runs-ledger', label: 'Ledger' },
  { key: 'runs-journal', label: 'Journal' },
  { key: 'runs-reservations', label: 'Reservations' },
];

export function RunsPage({ page, visibleSessions, commands, sessionsDisabled, stale, send, active, api, capabilities, sessions, filter, workspaceDisabled, canRetry, onPendingChange }: {
  page: Page;
  visibleSessions: Session[]; commands: Record<string, PendingCommand>; sessionsDisabled: boolean; stale: boolean;
  send: (session: Session, action: CommandAction, retry?: boolean) => Promise<void>;
  active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all'; workspaceDisabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void;
}) {
  const [tab, setTab] = useState<RunsTab>('runs-sessions');
  return <>
    {page === 'Runs' && <Tabs label="Runs sections" tabs={RUNS_TABS} value={tab} onChange={setTab} />}
    <div hidden={page !== 'Runs'}><PaperWorkspace active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} disabled={workspaceDisabled} canRetry={canRetry} onPendingChange={onPendingChange} view={tab} onViewChange={view => { setTab(view); focusTab(view); }} sessionsSection={page === 'Runs' ? <SessionsSection visibleSessions={visibleSessions} commands={commands} disabled={sessionsDisabled} stale={stale} send={send} /> : null} /></div>
  </>;
}
