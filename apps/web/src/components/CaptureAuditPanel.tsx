import { useEffect, useRef, useState } from 'react';
import type { ControlApi } from '../api/client';
import type { FrozenExport } from '../api/frozenExport';
import type { CaptureAuditReport, CaptureAuditRequest } from '../domain/captureAudit';
import { createCaptureAuditRequest, readCaptureAuditReport } from '../domain/captureAudit';

export function CaptureAuditPanel({ api, bundle, active }: { api: ControlApi; bundle: FrozenExport; active: boolean }) {
  const [state, setState] = useState<{ request: CaptureAuditRequest | null; report: CaptureAuditReport | null; busy: boolean; error: string }>({ request: null, report: null, busy: false, error: '' });
  const serial = useRef(0), epoch = useRef(-1);
  useEffect(() => {
    const token = ++serial.current;
    epoch.current = api.authorizationVersion();
    setState({ request: null, report: null, busy: false, error: '' });
    const current = () => token === serial.current && api.isAuthorized() && api.authorizationVersion() === epoch.current;
    if (active && api.isAuthorized()) void createCaptureAuditRequest(bundle).then(request => {
      if (current()) setState({ request, report: null, busy: false, error: '' });
    }).catch(() => {
      if (current()) setState({ request: null, report: null, busy: false, error: 'No audit request was prepared. This export exceeds the 1,000-reference limit or contains unsupported capture identities.' });
    });
    const unsubscribe = api.subscribeAuth(() => { ++serial.current; setState({ request: null, report: null, busy: false, error: '' }); });
    return () => { ++serial.current; unsubscribe(); };
  }, [api, bundle, active]);
  function allowed() { return active && api.isAuthorized() && api.authorizationVersion() === epoch.current && state.request !== null; }
  function download(value: CaptureAuditRequest | CaptureAuditReport, kind: string) {
    if (!allowed()) return;
    const url = URL.createObjectURL(new Blob([JSON.stringify(value)], { type: 'application/json' }));
    const link = document.createElement('a'); link.href = url;
    link.download = 'arbitrage-' + kind + '-' + bundle.export_id.replace(/[^a-z0-9-]/gi, '-').slice(0, 80) + '.json';
    document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  async function importReport(file: File) {
    if (!allowed() || !state.request) return;
    const token = ++serial.current, auth = api.authorizationVersion();
    setState(previous => ({ ...previous, report: null, busy: true, error: '' }));
    try {
      const report = await readCaptureAuditReport(file, state.request);
      if (token === serial.current && api.isAuthorized() && api.authorizationVersion() === auth) setState(previous => ({ ...previous, report, busy: false }));
    } catch {
      if (token === serial.current && api.isAuthorized() && api.authorizationVersion() === auth) setState(previous => ({ ...previous, report: null, busy: false, error: 'The local report is invalid, too large, incomplete in its reference list, or belongs to a different frozen export. No result was attached.' }));
    }
  }
  const enabled = allowed(), report = enabled ? state.report : null;
  return <section className="notice space-top" aria-label="Local capture audit">
    <h4>Check the retained capture files</h4>
    <p className="tiny space-top">Download a request for every reference in this frozen export. Run the local auditor beside the quiesced capture volume, then import its JSON result here. No filesystem paths, credentials or report files are sent to the API.</p>
    <div className="research-actions space-top"><button disabled={!enabled} onClick={() => { if (state.request) download(state.request, 'audit-request'); }}>Download capture audit request</button>
      <label>Import local audit JSON <input type="file" accept=".json,application/json" className="audit-file" disabled={!enabled || state.busy} onChange={event => {
        const file = event.currentTarget.files?.[0]; event.currentTarget.value = ''; if (file) void importReport(file);
      }} /></label>
      {report && <button onClick={() => download(report, 'audit-report')}>Download bound audit report</button>}
    </div>
    {enabled && <p className="tiny space-top">Audit request covers {state.request!.capture_request.captures.length} capture references. Maximum: 1,000; larger requests are refused, never truncated.</p>}
    <details className="space-top"><summary>Local audit command and trust limits</summary>
      <p className="tiny">Run from the checked-out repository with private local paths and an explicit audit time in Unix milliseconds. See docs/25-EXPORT-CAPTURE-AUDIT.md. Save output outside the capture root.</p>
      <p className="mono tiny">python3 scripts/export_capture_audit.py --root /private/captures --request /private/request.json --now-ms AUDIT_TIME_MS &gt; /private/report.json</p>
      <p className="tiny">This browser validates the report structure and its link to this export, not who executed it or the truth of a supplied timestamp. A hash is not a signature. Empty reference sets do not establish complete coverage.</p>
    </details>
    {state.busy && <p role="status">Checking local report binding...</p>}
    {active && state.error && <p role={state.request ? 'alert' : 'status'}>{state.error}</p>}
    {report && <div className="space-top" role="status"><strong>Imported local audit: {report.audit.status}</strong>
      <p className="tiny">Operator-supplied, not independently authenticated. Checked at Unix milliseconds: <span className="mono">{report.audit.checked_at_ms}</span>.</p>
      <p className="tiny">{report.audit.references_reported} of {report.audit.references_requested} references reported. The original frozen JSON/CSV is unchanged; this is a separate point-in-time report.</p>
      <p className="tiny">{Object.entries(report.audit.raw_status_counts).map(([status, count]) => `${status}: ${count}`).join(' · ') || 'No references in this export.'}</p>
      {report.audit.dependencies.slice(0, 20).map(row => <p className="tiny space-top" key={row.capture_id + row.manifest_digest}><strong>{row.raw_artifact_status}</strong> · {row.capture_id}<br />Expiration: {row.expiration_status} · quote inputs declared complete: {String(row.quote_inputs_declared_complete)} · origin: {row.origin}</p>)}
      {report.audit.dependencies.length > 20 && <p className="tiny">Showing the first 20 results; the audit report download retains all results.</p>}
      <p className="tiny space-top">Replay: NOT ASSESSED. Market eligibility: false. Execution authorization: false. This result does not certify continuous retention, every worker volume or market coverage.</p>
    </div>}
  </section>;
}
