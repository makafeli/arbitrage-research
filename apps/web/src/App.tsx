import { useEffect, useState } from 'react';
import { ConnectedApp } from './ConnectedApp';

// Production has one authenticated application. No synthetic demo is imported.
export function App() {
  const [light, setLight] = useState(false);
  useEffect(() => { document.body.classList.toggle('light', light); }, [light]);
  useEffect(() => { document.title = 'Arbitrage | Paper & Real trading'; }, []);
  return <ConnectedApp light={light} onToggleTheme={() => setLight(value => !value)} />;
}
