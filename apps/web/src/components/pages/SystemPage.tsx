import { useState } from 'react';
import type { Capabilities, ControlApi, Network, Session } from '../../api/client';
import { AdapterSupport } from '../AdapterSupport';
import { CollectionHealth } from '../CollectionHealth';
import { CapabilitiesPanel } from '../CapabilitiesPanel';
import { Tabs, TabPanel } from '../ui/Tabs';

type SystemTab = 'sys-adapters' | 'sys-collection' | 'sys-capabilities';
const systemTabs = [
  { key: 'sys-adapters', label: 'Adapters' },
  { key: 'sys-collection', label: 'Collection' },
  { key: 'sys-capabilities', label: 'Capabilities' },
] as const;

export function SystemPage({ hidden, active, available, api, filter, capabilities, sessions }: {
  hidden: boolean; active: boolean; available: boolean; api: ControlApi; filter: Network | 'all'; capabilities: Capabilities; sessions: Session[];
}) {
  const [tab, setTab] = useState<SystemTab>('sys-adapters');
  return <div hidden={hidden}>
    <Tabs label="System sections" tabs={systemTabs} value={tab} onChange={setTab} />
    <TabPanel tab="sys-adapters" value={tab}><AdapterSupport active={active} available={available} api={api} filter={filter} /></TabPanel>
    <TabPanel tab="sys-collection" value={tab}><CollectionHealth active={active} api={api} capabilities={capabilities} sessions={sessions} filter={filter} /></TabPanel>
    <TabPanel tab="sys-capabilities" value={tab}><CapabilitiesPanel capabilities={capabilities} /></TabPanel>
  </div>;
}
