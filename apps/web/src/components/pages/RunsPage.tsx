import type { Capabilities, CommandAction, ControlApi, Network, Session } from '../../api/client';
import type { Page } from '../../ConnectedApp';
import { PaperWorkspace } from '../PaperWorkspace';
import { SessionsSection } from '../SessionCard';
import type { PendingCommand } from '../SessionCard';

export function RunsPage({ page, visibleSessions, commands, sessionsDisabled, stale, send, active, api, capabilities, sessions, filter, workspaceDisabled, canRetry, onPendingChange }: {
  page: Page;
  visibleSessions: Session[]; commands: Record<string, PendingCommand>; sessionsDisabled: boolean; stale: boolean;
  send: (session: Session, action: CommandAction, retry?: boolean) => Promise<void>;
  active: boolean; api: ControlApi; capabilities: Capabilities; sessions: Session[]; filter: Network | 'all'; workspaceDisabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void;
}) {
  return <>
    {page === 'Runs' && <SessionsSection visibleSessions={visibleSessions} commands={commands} disabled={sessionsDisabled} stale={stale} send={send} />}
    <div hidden={page !== 'Runs'}><PaperWorkspace active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} disabled={workspaceDisabled} canRetry={canRetry} onPendingChange={onPendingChange} /></div>
  </>;
}
