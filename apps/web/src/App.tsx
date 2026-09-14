import { useEffect, useState } from 'react';
import { DemoApp } from './DemoApp';
import { ConnectedApp } from './ConnectedApp';

// The standalone demo never probes a service. Connected mode never falls back
// to synthetic records after a network or authentication failure.
export function App() {
  const [connected, setConnected] = useState(false);
  // A display preference belongs to the shell, not an authentication mode.
  // It never persists authorization, API records or session configuration.
  const [light, setLight] = useState(false);
  useEffect(() => { document.body.classList.toggle('light', light); }, [light]);
  const toggleTheme = () => setLight(value => !value);
  useEffect(() => { document.title = `Arbitrage Research — ${connected ? 'Connected' : 'Demo'}`; }, [connected]);
  return connected ? <ConnectedApp onDemo={() => setConnected(false)} light={light} onToggleTheme={toggleTheme} />
    : <DemoApp onConnect={() => setConnected(true)} light={light} onToggleTheme={toggleTheme} />;
}
