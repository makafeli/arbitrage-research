import type { Capabilities, ControlApi, Network, Session } from '../../api/client';
import type { Page } from '../../ConnectedApp';
import { AdapterSupport } from '../AdapterSupport';
import { CollectionHealth } from '../CollectionHealth';
import { CapabilitiesPanel } from '../CapabilitiesPanel';

export function SystemPage({ page, active, available, api, filter, capabilities, sessions }: {
  page: Page; active: boolean; available: boolean; api: ControlApi; filter: Network | 'all'; capabilities: Capabilities; sessions: Session[];
}) {
  return <>
    <div hidden={page !== 'System'}><AdapterSupport active={active} available={available} api={api} filter={filter} /><CollectionHealth active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} /></div>
    {page === 'System' && <CapabilitiesPanel capabilities={capabilities} />}
  </>;
}
