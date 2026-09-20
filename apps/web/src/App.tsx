import { useEffect, useState } from 'react';
import { ConnectedApp } from './ConnectedApp';

// Production has one authenticated application. No synthetic demo is imported.
export function App() {
  const [dark, setDark] = useState(false);
  useEffect(() => { document.body.classList.toggle('dark', dark); }, [dark]);
  useEffect(() => { document.title = 'Arbitrage | Paper & Real trading'; }, []);
  return <ConnectedApp dark={dark} onToggleTheme={() => setDark(value => !value)} />;
}
