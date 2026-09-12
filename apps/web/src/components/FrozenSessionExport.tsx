import { useEffect, useRef, useState } from 'react';
import { ApiError } from '../api/client';
import type { ControlApi } from '../api/client';
import type { FrozenExport } from '../api/frozenExport';
import { exportSourceKeys } from '../api/frozenExport';
import { frozenCsv, frozenJson } from '../domain/frozenExport';
import { Exact } from './ResearchShared';

function failure(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.code === 'EXPORT_LIMIT_EXCEEDED' || error.status === 413) return 'This session exceeds the complete export bound of 10,000 source rows or 8 MiB. No partial export was prepared. Retain the database and use an operator-reviewed extraction for a larger session.';
    if (error.code === 'EXPORT_BUSY') return 'Another export is using the service export capacity. Retry preparation when it finishes; the previous frozen bundle remains available.';
    if (error.status === 429) return 'The service request limit was reached. Wait before retrying preparation; the previous frozen bundle remains available.';
    if (error.status === 401 || error.status === 403) return 'Your session cannot read this export. Sign in again with an operator that can read the selected research session.';
    if (error.status === 404) return 'The selected session export is unavailable to this operator.';
  }
  return 'The complete snapshot could not be prepared or its schema, counts or content digest could not be verified. No new download is available; check the service and retry.';
}
export function FrozenSessionExport({ api, sessionId, active, available }: { api: ControlApi; sessionId: string; active: boolean; available: boolean }) {
  const [state, setState] = useState<{ scope: string; bundle: FrozenExport | null; loading: boolean; error: string }>({ scope: '', bundle: null, loading: false, error: '' });
  const request = useRef<AbortController | null>(null);
  useEffect(() => () => { request.current?.abort(); request.current = null; }, [sessionId, active]);
  const bundle = state.scope === sessionId ? state.bundle : null;
  const loading = state.scope === sessionId && state.loading && request.current !== null;
  async function prepare() {
    if (!active || !available || request.current) return;
    const controller = new AbortController(); request.current = controller;
    setState(previous => ({ scope: sessionId, bundle: previous.scope === sessionId ? previous.bundle : null, loading: true, error: '' }));
    try {
      const next = await api.frozenExport(sessionId, controller.signal);
      if (!controller.signal.aborted) setState({ scope: sessionId, bundle: next, loading: false, error: '' });
    } catch (error) {
      if (!controller.signal.aborted) setState(previous => ({ ...previous, loading: false, error: failure(error) }));
    } finally { if (request.current === controller) request.current = null; }
  }
  function download(format: 'json' | 'csv') {
    if (!bundle) return;
    try {
      const value = format === 'json' ? frozenJson(bundle) : frozenCsv(bundle);
      const url = URL.createObjectURL(new Blob([value], { type: format === 'json' ? 'application/json' : 'text/csv;charset=utf-8' }));
      const link = document.createElement('a'); link.href = url;
      link.download = 'arbitrage-frozen-' + bundle.export_id.replace(/[^a-z0-9-]/gi, '-').slice(0, 80) + '.' + format;
      document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch { setState(previous => ({ ...previous, error: 'The bounded download could not be serialized. No partial file was prepared.' })); }
  }
  if (!available) return <p className="notice space-top">Frozen session exports are unavailable in this API version. Page exports remain limited received snapshots.</p>;
  return <section className="panel space-top frozen-export" aria-label="Frozen session export"><div className="sectionhead"><div><h3>Freeze a complete stored session</h3><p>A single database snapshot includes decisions, paper accounting, collection attempts and capture dependencies.</p></div><span className="pill blue">FROZEN EXPORT</span></div>
    <p className="tiny space-top">Selected session: {sessionId}. Live browsing pages can change independently. The five defined source datasets are decisions, paper runs, paper journal events, capture catalog entries and collection attempts. This is not a full database backup: configuration content, audit records and control history are outside the export. Up to 10,000 source rows and 8 MiB are read together; larger sessions fail without truncation.</p>
    <div className="research-actions space-top"><button disabled={!active || loading} onClick={() => { void prepare(); }}>{loading ? 'Preparing frozen snapshot…' : bundle ? 'Prepare a new frozen snapshot' : 'Prepare frozen session export'}</button><button disabled={!bundle} onClick={() => download('json')}>Download frozen JSON</button><button disabled={!bundle} onClick={() => download('csv')}>Download frozen CSV</button></div>
    {state.scope === sessionId && state.error && <p className="notice error-notice" role="alert">{state.error}</p>}
    {loading && <p className="tiny space-top" role="status">Reading one database snapshot and verifying its content digest…</p>}
    {bundle && <><div className="notice" role="status"><strong>Frozen database snapshot verified</strong><p>{bundle.exported_at} · {bundle.export_id}</p><p className="mono">Content SHA-256: {bundle.content_sha256}</p><p>Both downloads use this exact received bundle. Downloading does not refresh it.</p></div>
      <dl className="research-facts">{exportSourceKeys.map(key => <div className="export-count" key={key}><dt>{key.replaceAll('_', ' ')}</dt><dd><Exact value={bundle.snapshot.source_counts[key]} /></dd></div>)}</dl>
      <p className="notice">Defined research datasets: COMPLETE within this session and database transaction. Scheduled collection completeness: UNKNOWN. This export cannot establish unrecorded activity, market coverage, executable opportunities or realized profit.</p>
      <p className="tiny space-top">Amounts remain exact base-unit integer strings. Token decimals are not retained in this database, configuration content is represented by its digest, and unknown costs remain unknown. Paper reasons are redacted and paper identifiers are pseudonymized consistently; source event digests identify original journal payloads.</p>
      <details className="space-top"><summary>Capture dependency availability: {bundle.data.capture_dependencies.filter(item => item.catalog_status === 'MISSING').length} missing catalog entries · {bundle.data.capture_dependencies.length} raw artifacts unverified</summary>
        <p className="notice">A present catalog entry does not prove that its raw files remain available. Raw artifacts are excluded and unverified; expiration status is UNKNOWN. This bundle alone cannot reproduce capture-based calculations.</p>
        {bundle.data.capture_dependencies.slice(0, 20).map(item => <p className="tiny space-top" key={item.capture_id + item.manifest_digest}><strong>{item.catalog_status}</strong> · {item.capture_id}<br />{item.manifest_digest} · expiration UNKNOWN · raw artifact NOT VERIFIED</p>)}
        {bundle.data.capture_dependencies.length > 20 && <p className="tiny space-top">Showing the first 20 dependencies. Both frozen downloads contain every dependency in the snapshot.</p>}
      </details><p className="tiny space-top">CSV contains one typed row per record with its complete payload in a JSON cell. Parse payload_json to retain exact large integers and nested ledger commands. Formula guards protect scalar cells; amounts are not converted to spreadsheet numbers.</p>
    </>}
  </section>;
}
