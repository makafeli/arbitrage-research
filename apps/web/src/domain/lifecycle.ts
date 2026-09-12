// Interface demonstration only. A production session is bound to ONE network.
// A workspace command is a fan-out of separate per-session commands.
export type Chain = 'solana' | 'base';
export type ChainFilter = Chain | 'all';
export type DemoState = 'STOPPED' | 'RUNNING' | 'PAUSING' | 'PAUSED';
export type CommandStatus = 'NONE' | 'PENDING' | 'APPLIED';
export type DemoAction = 'START' | 'PAUSE' | 'STOP' | 'ACK_STOP';
export interface DemoSession {
  chain: Chain;
  state: DemoState;
  commandStatus: CommandStatus;
}

export const DEMO_SCOPE: readonly Chain[] = ['solana', 'base'];
export function initialSessions(): DemoSession[] {
  return DEMO_SCOPE.map(chain => ({ chain, state: 'STOPPED', commandStatus: 'NONE' }));
}

// Invalid transitions are ignored. ACK_STOP only means the local fence has
// applied; this no-attempt demo has nothing to reconcile, so it can stop.
export function transition(session: DemoSession, action: DemoAction): DemoSession {
  if (action === 'START' && (session.state === 'STOPPED' || session.state === 'PAUSED')) {
    return { ...session, state: 'RUNNING', commandStatus: 'APPLIED' };
  }
  if (action === 'PAUSE' && session.state === 'RUNNING') {
    return { ...session, state: 'PAUSED', commandStatus: 'APPLIED' };
  }
  if (action === 'STOP' && (session.state === 'RUNNING' || session.state === 'PAUSED')) {
    return { ...session, state: 'PAUSING', commandStatus: 'PENDING' };
  }
  if (action === 'ACK_STOP' && session.state === 'PAUSING') {
    return { ...session, state: 'STOPPED', commandStatus: 'APPLIED' };
  }
  return session;
}

export function selectChains(filter: ChainFilter): readonly Chain[] {
  return filter === 'all' ? DEMO_SCOPE : [filter];
}

export type PendingOutcome = 'INACTIVE' | 'DRAINING' | 'STOPPED';
export function pendingTransition(current: PendingOutcome, action: 'SIMULATE' | 'RESOLVE'): PendingOutcome {
  if (action === 'SIMULATE' && current !== 'DRAINING') return 'DRAINING';
  if (action === 'RESOLVE' && current === 'DRAINING') return 'STOPPED';
  return current;
}
