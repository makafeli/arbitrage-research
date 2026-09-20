import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { ControlApi, errorMessage } from '../api/client';

export function AccountAccess({ api, checking, dark, onToggleTheme }: { api: ControlApi; checking: boolean; dark: boolean; onToggleTheme: () => void }) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [token, setToken] = useState(() => {
    const params = new URLSearchParams(window.location.hash.slice(1));
    const value = params.get('activate');
    return value && /^[a-f0-9]{64}$/.test(value) ? value : '';
  });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [help, setHelp] = useState(false);
  const [recoveryHint, setRecoveryHint] = useState(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    // Fragment credentials never enter an HTTP request URL or persistent storage.
    if (window.location.hash.includes('activate=')) window.history.replaceState(null, '', window.location.pathname + window.location.search);
    return () => { alive.current = false; };
  }, []);
  async function submit(event: FormEvent) {
    event.preventDefault(); if (busy || checking) return;
    setError(''); setMessage(''); setRecoveryHint(false);
    if (token && password !== confirmation) { setError('The passwords do not match.'); return; }
    setBusy(true); const entered = password; setPassword(''); setConfirmation('');
    try {
      if (token) {
        await api.activateAccount(email, entered, token);
        if (alive.current) { setToken(''); setMessage('Your password is saved. Sign in to continue.'); }
      } else await api.signIn(email, entered);
    } catch (failure) { if (alive.current) { setError(errorMessage(failure)); setRecoveryHint(Boolean(token)); } }
    finally { if (alive.current) setBusy(false); }
  }
  return <main className="account-screen">
    <div className="account-top"><a className="brand" href="/" aria-label="Arbitrage home"><span className="brandmark" aria-hidden="true">↗</span></a><button onClick={onToggleTheme} aria-label={`Switch to ${dark ? 'light' : 'dark'} theme`}>{dark ? 'Light' : 'Dark'} theme</button></div>
    <aside className="account-aside">
      <div className="brand"><span className="brandmark" aria-hidden="true">↗</span>Arbitrage.</div>
      <ul className="account-facts">
        <li>Private account access.</li>
        <li>No wallet or exchange credentials are requested here.</li>
        <li>There is no public registration.</li>
      </ul>
    </aside>
    <section className="panel account-card" aria-labelledby="login-title">
      <p className="eyebrow">Your trading workspace</p>
      <h1 id="login-title">{token ? 'Set your password' : 'Sign in'}</h1>
      <p className="muted">{token ? 'Activate or recover your private account with your one-time access link.' : 'Use your email address and password to access Paper trading and Real trading.'}</p>
      {error && <p className="notice error-notice" role="alert">{error}{recoveryHint && ' If a previous request was interrupted, try signing in with the password you chose.'}</p>}
      {message && <p className="notice" role="status">{message}</p>}
      <form className="connected-form" onSubmit={event => { void submit(event); }}>
        <label htmlFor="account-email">Email address</label><input id="account-email" type="email" autoComplete="username" value={email} maxLength={254} onChange={e => setEmail(e.target.value)} required disabled={busy || checking} />
        <label htmlFor="account-password">{token ? 'New password' : 'Password'}</label><input id="account-password" type="password" autoComplete={token ? 'new-password' : 'current-password'} value={password} onChange={e => setPassword(e.target.value)} minLength={token ? 15 : undefined} maxLength={128} required disabled={busy || checking} />
        {token && <><label htmlFor="account-confirm">Confirm password</label><input id="account-confirm" type="password" autoComplete="new-password" value={confirmation} onChange={e => setConfirmation(e.target.value)} minLength={15} maxLength={128} required disabled={busy || checking} /><p className="tiny">Use 15 to 128 characters. A long passphrase is welcome.</p></>}
        <button className="primary" type="submit" disabled={busy || checking || !email || !password}>{checking ? 'Checking session…' : busy ? 'Please wait…' : token ? 'Save password' : 'Sign in'}</button>
      </form>
      {token ? <button className="space-top" onClick={() => { setToken(''); setError(''); setPassword(''); setConfirmation(''); }}>Back to sign in</button> : <button className="space-top" onClick={() => setHelp(value => !value)}>First time here or forgot your password?</button>}
      {help && <p className="notice" role="status">Use the private account setup or recovery link supplied by the workspace owner. There is no public registration. Recovery links expire after 30 minutes and work once; no reset email has been sent.</p>}
      <p className="tiny space-top">Private account access. No wallet or exchange credentials are requested here.</p>
    </section>
  </main>;
}

export function PasswordSettings({ api, onClose }: { api: ControlApi; onClose: () => void }) {
  const [current, setCurrent] = useState(''); const [next, setNext] = useState(''); const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  async function save(event: FormEvent) {
    event.preventDefault(); if (busy) return;
    if (next !== confirm) { setError('The passwords do not match.'); return; }
    setBusy(true); setError(''); const previous = current; const replacement = next;
    setCurrent(''); setNext(''); setConfirm('');
    try { await api.changePassword(previous, replacement); onClose(); }
    catch (failure) { setError(errorMessage(failure)); }
    finally { setBusy(false); }
  }
  return <section className="panel account-settings" aria-label="Change password"><h2>Change password</h2><p>Changing your password signs out all sessions. Your trading data stays unchanged.</p>
    {error && <p role="alert" className="notice">{error}</p>}
    <form className="connected-form" onSubmit={event => { void save(event); }}>
      <label htmlFor="current-password">Current password</label><input id="current-password" type="password" autoComplete="current-password" value={current} onChange={e => setCurrent(e.target.value)} required maxLength={128} disabled={busy} />
      <label htmlFor="new-password">New password</label><input id="new-password" type="password" autoComplete="new-password" value={next} onChange={e => setNext(e.target.value)} required minLength={15} maxLength={128} disabled={busy} />
      <label htmlFor="confirm-password">Confirm new password</label><input id="confirm-password" type="password" autoComplete="new-password" value={confirm} onChange={e => setConfirm(e.target.value)} required minLength={15} maxLength={128} disabled={busy} />
      <button type="submit" className="primary" disabled={busy}>Save password</button><button type="button" onClick={onClose} disabled={busy}>Cancel</button>
    </form></section>;
}
