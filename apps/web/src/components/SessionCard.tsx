import { availableActions } from '../api/client';
import type { CommandAction, CommandReceipt, CommandRequest, Network, Session } from '../api/client';

export const names: Record<Network, string> = { 'base-mainnet': 'Base', 'solana-mainnet': 'Solana' };
const actions: Record<CommandAction, string> = { START: 'Start', PAUSE: 'Pause', RESUME: 'Resume', STOP: 'Stop' };

export interface PendingCommand { key: string; body: CommandRequest; receipt?: CommandReceipt; error?: string; sending: boolean; uncertain: boolean }

export function Empty({ title, text }: { title: string; text: string }) { return <div className="panel space-top"><h3>{title}</h3><p className="muted space-top">{text}</p></div>; }

export function SessionCard({ session, command, disabled, stale, send }: { session: Session; command?: PendingCommand; disabled: boolean; stale: boolean; send: (session: Session, action: CommandAction, retry?: boolean) => Promise<void> }) {
  const receipt = command?.receipt;
  const pendingRevision = session.desired_revision !== session.applied_revision;
  const blocked = disabled || stale || command?.sending || command?.uncertain || receipt?.status === 'PENDING';
  const stopBlocked = disabled || command?.sending || command?.uncertain || (receipt?.status === 'PENDING' && command?.body.action === 'STOP');
  return <section className="panel" aria-label={`Session ${session.session_id}`}><div className="sectionhead"><div><h3>{names[session.network_id]} · {session.mode}</h3><p className="mono">{session.session_id}</p></div><span className="pill">{session.observed_state}</span></div><div className="session-states"><span className="session-status">Health: {session.health}</span><span className="session-status">Desired revision: {session.desired_revision}</span><span className="session-status">Applied revision: {session.applied_revision}</span><span className="session-status">Unresolved attempts: {session.outstanding_attempts}</span></div><p className="tiny space-top">Last worker heartbeat: {session.last_heartbeat_at ?? 'Unknown; no heartbeat reported'}</p><p className="tiny mono">Configuration: {session.configuration_digest}</p>
    {pendingRevision && !receipt && <p className="notice">Desired and applied revisions differ. This can reflect unapplied, rejected or superseded history; a revision gap alone does not establish a pending worker acknowledgement. The service validates new requests.</p>}
    {receipt && <div className="notice" role="status"><strong>{receipt.action} · {receipt.status}</strong><p>Command {receipt.command_id} · revision {receipt.revision}</p><p>{receipt.status === 'PENDING' ? 'Accepted by the API. Awaiting worker acknowledgement; the admission fence is not yet confirmed.' : receipt.status === 'APPLIED' ? `Worker acknowledged at ${receipt.applied_at}. ${receipt.fence_effective ? 'Admission fence effective.' : 'See the separately refreshed observed session state.'}` : 'This command did not apply. Review the latest session before issuing another command.'}</p></div>}
    {command?.sending && <p className="notice" role="status">Sending {command.body.action}. Acceptance is not yet confirmed.</p>}
    {command?.error && <div className="notice error-notice" role="alert"><strong>{command.uncertain ? 'Delivery uncertain' : receipt ? 'Receipt status may be stale' : 'Command rejected'}</strong><p>{command.error}</p>{command.uncertain && <p>The server may have accepted this request. Retry reuses the same idempotency key and exact payload.</p>}</div>}
    {session.observed_state === 'DRAINING' && <p className="notice">The admission fence has applied, but {session.outstanding_attempts} previously emitted attempts remain unresolved. A stop cannot recall an emitted transaction.</p>}
    <div className="controls session-actions space-top">{(['START', 'PAUSE', 'RESUME', 'STOP'] as const).map(action => <button key={action} className={action === 'START' || action === 'RESUME' ? 'primary' : ''} disabled={Boolean(action === 'STOP' ? stopBlocked : blocked) || !availableActions(session).includes(action)} onClick={() => { void send(session, action); }}>{actions[action]}</button>)}{command?.uncertain && <button disabled={disabled || command.sending} onClick={() => { void send(session, command.body.action, true); }}>Retry same request</button>}</div>
    <p className="tiny space-top">{session.mode === 'LIVE' ? 'LIVE sessions have no controls in this interface.' : disabled ? 'Controls unavailable while signed out.' : stale ? 'Snapshot is stale. Stop remains available as a request using the last known revision; only a worker receipt can confirm it.' : blocked ? 'Wait for the pending acknowledgement or resolve delivery uncertainty before another command.' : 'Controls apply only to this session. Modes and configuration cannot be changed after creation.'}</p></section>;
}

export function SessionsSection({ visibleSessions, commands, disabled, stale, send }: { visibleSessions: Session[]; commands: Record<string, PendingCommand>; disabled: boolean; stale: boolean; send: (session: Session, action: CommandAction, retry?: boolean) => Promise<void> }) {
  return <>
    <div className="sectionhead"><div><h2>Research sessions</h2><p>Observed states remain unchanged until a fresh service response arrives.</p></div></div>
    {!visibleSessions.length ? <Empty title="No sessions in this view" text="Create a session from an operator-validated configuration in Experiments. The chain filter changes only this view." /> : <div className="cards">{visibleSessions.map(session => <SessionCard key={session.session_id} session={session} command={commands[session.session_id]} disabled={disabled} stale={stale} send={send} />)}</div>}
  </>;
}
