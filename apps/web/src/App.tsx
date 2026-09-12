import { useEffect, useState } from 'react';
import { DemoApp } from './DemoApp';
import { ConnectedApp } from './ConnectedApp';

// The standalone demo never probes a service. Connected mode never falls back
// to synthetic records after a network or authentication failure.
export function App() {
  const [connected, setConnected] = useState(false);
  useEffect(() => { document.title = `Arbitrage Research — ${connected ? 'Connected' : 'Demo'}`; }, [connected]);
  return connected ? <ConnectedApp onDemo={() => setConnected(false)} />
    : <DemoApp onConnect={() => setConnected(true)} />;
}
