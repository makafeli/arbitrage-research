import { useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { ApiError, errorMessage } from '../api/client';
import type { Capabilities, ControlApi, CreateSession, Network, Session } from '../api/client';
import { names } from './SessionCard';

export function SessionCreation({ capabilities, api, disabled, canRetry, onCreated, onPendingChange }: { capabilities: Capabilities; api: ControlApi; disabled: boolean; canRetry: boolean; onCreated: (session: Session) => void; onPendingChange: (value: boolean) => void }) {
  const [digest, setDigest] = useState(''); const [network, setNetwork] = useState<Network | ''>('');
  const [experiment, setExperiment] = useState(''); const [reviewed, setReviewed] = useState(false);
  const [pending, setPending] = useState<{ body: CreateSession; key: string } | null>(null);
  const [sending, setSending] = useState(false); const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<Session | null>(null); const lock = useRef(false);
  const selected = capabilities.registered_configurations.find(c => c.configuration_digest === digest);
  const valid = Boolean(selected && selected.enabled_networks.includes(network as Network) && selected.strategy_ids.length && capabilities.modes.includes(selected.mode) && experiment.trim() && experiment.length <= 128 && reviewed);
  async function create(event: FormEvent) {
    event.preventDefault(); if (lock.current || (pending ? !canRetry : disabled || !valid)) return;
    lock.current = true; setSending(true); setError(null);
    const wasUncertain = pending !== null;
    const request = pending ?? { key: crypto.randomUUID(), body: { configuration_digest: selected!.configuration_digest, mode: selected!.mode, network_id: network as Network, experiment_id: experiment.trim(), strategy_ids: selected!.strategy_ids } };
    setPending(request); onPendingChange(true);
    try { const session = await api.create(request.body, request.key); setCreated(session); setPending(null); onPendingChange(false); onCreated(session); }
    catch (e) { setError(errorMessage(e)); if (!wasUncertain && e instanceof ApiError && [400, 401, 403, 409, 422, 429].includes(e.status)) { setPending(null); onPendingChange(false); } }
    finally { setSending(false); lock.current = false; }
  }
  const fieldDisabled = disabled || sending || Boolean(pending);
  return <section className="panel"><h2>Create a research session</h2><p className="muted space-top">Choose a validated configuration. Mode, network and digest are immutable. Creation does not start a worker; recovery must finish before a stopped session can start.</p>{!capabilities.registered_configurations.length && <div className="notice">Creation unavailable: no validated configurations are registered. Ask the service operator to register an eligible research configuration.</div>}<form className="connected-form" onSubmit={event => { void create(event); }}><ol className="steps">
    <li><span className="stepnum" aria-hidden="true">01</span><div><h3>Choose a validated configuration</h3><fieldset disabled={fieldDisabled}><label htmlFor="configuration">Validated configuration</label><select id="configuration" value={digest} onChange={event => { setDigest(event.target.value); setNetwork(''); setReviewed(false); setCreated(null); }} required><option value="">Choose a configuration</option>{capabilities.registered_configurations.map(c => <option key={c.configuration_digest} value={c.configuration_digest}>{c.mode} · {c.configuration_digest}</option>)}</select></fieldset>{selected && <div className="notice"><strong>Review immutable settings</strong><p>Mode: {selected.mode} · Network: {network ? names[network] : 'Select an enabled network'}</p><p className="mono">Digest: {selected.configuration_digest}</p><p>Strategies: {selected.strategy_ids.join(', ') || 'None; configuration cannot create a session'}</p><p>Virtual principal, native fee reserves, token eligibility and delay scenarios come from the validated configuration. This API does not expose their detailed values yet; review the source configuration before confirming.</p></div>}</div></li>
    <li><span className="stepnum" aria-hidden="true">02</span><div><h3>Name the session</h3><fieldset disabled={fieldDisabled}><label htmlFor="session-network">Session network</label><select id="session-network" value={network} onChange={event => { setNetwork(event.target.value as Network); setReviewed(false); setCreated(null); }} required><option value="">Choose an enabled network</option>{selected?.enabled_networks.map(n => <option key={n} value={n}>{names[n]}</option>)}</select>{selected && !selected.enabled_networks.length && <p className="notice">Creation unavailable: this configuration has no enabled networks.</p>}<label htmlFor="experiment-id">Experiment reference</label><input id="experiment-id" value={experiment} onChange={event => { setExperiment(event.target.value); setCreated(null); }} required maxLength={128} autoComplete="off" /></fieldset></div></li>
    <li><span className="stepnum" aria-hidden="true">03</span><div><h3>Review and create</h3><fieldset disabled={fieldDisabled}><label className="checkbox-label"><input type="checkbox" checked={reviewed} onChange={event => setReviewed(event.target.checked)} />I reviewed the source configuration, including virtual principal and separate native fee reserves.</label></fieldset>{error && <div className="notice error-notice" role="alert">{error}{pending && <p>Creation outcome is uncertain. Retry reuses the same exact request; settings remain locked.</p>}</div>}<button className="primary" disabled={sending || Boolean(created) || (pending ? !canRetry : disabled || !valid)}>{created ? 'Session created' : sending ? 'Creating session…' : pending ? 'Retry same creation request' : 'Create research session'}</button>{created && <div className="notice" role="status">Created {created.session_id} · {created.observed_state}. No start command was sent.</div>}</div></li>
  </ol></form><p className="tiny space-top">LIVE is unavailable. Pool selection, amounts and scenario editing require a separately validated configuration revision.</p></section>;
}
