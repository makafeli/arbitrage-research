import type { Capabilities, ControlApi, Session } from '../../api/client';
import { SessionCreation } from '../SessionCreation';

export function ExperimentsPage({ hidden, capabilities, api, disabled, canRetry, onPendingChange, onCreated }: {
  hidden: boolean; capabilities: Capabilities; api: ControlApi; disabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void; onCreated: (session: Session) => void;
}) {
  return <div hidden={hidden}><SessionCreation capabilities={capabilities} api={api} disabled={disabled} canRetry={canRetry} onPendingChange={onPendingChange} onCreated={onCreated} /></div>;
}
