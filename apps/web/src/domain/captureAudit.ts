import type { FrozenExport } from '../api/frozenExport.ts';
import { parseFrozenExport, verifyFrozenExport } from '../api/frozenExport.ts';

export const MAX_CAPTURE_AUDIT_REQUEST_BYTES = 1024 * 1024;
export const MAX_CAPTURE_AUDIT_REPORT_BYTES = 2 * 1024 * 1024;
export const MAX_CAPTURE_AUDIT_REFERENCES = 1000;
const U64 = (1n << 64n) - 1n;
export interface CaptureAuditRequest {
  schema_version: 1; kind: 'FROZEN_EXPORT_CAPTURE_AUDIT_REQUEST';
  source_export: { schema_version: '1.1.0'; export_id: string; content_sha256: string; session_id: string };
  capture_request: { schema_version: 1; network: 'base-mainnet' | 'solana-mainnet'; config_digest: string;
    captures: { capture_id: string; manifest_digest: string }[] };
}
export const rawStatuses = ['AVAILABLE', 'MISSING', 'CORRUPT', 'INCOMPLETE', 'UNSUPPORTED', 'LIMIT_EXCEEDED', 'UNREADABLE'] as const;
type RawStatus = typeof rawStatuses[number];
export interface AuditDependency {
  capture_id: string; manifest_digest: string; raw_artifact_status: RawStatus; reason: string;
  expiration_status: 'UNKNOWN' | 'NOT_DECLARED' | 'EXPIRED' | 'NOT_EXPIRED';
  capture_time_status: 'UNKNOWN' | 'AFTER_AUDIT' | 'AT_OR_BEFORE_AUDIT';
  origin: 'UNKNOWN' | 'synthetic' | 'manually-constructed' | 'recorded-live';
  created_at_ms: string | null; raw_expires_at_ms: string | null; quote_inputs_declared_complete: boolean | null;
  verified_objects: number; missing_objects: number; corrupt_objects: number;
  replay_status: 'NOT_ASSESSED'; market_performance_eligible: false;
}
export interface CaptureAuditReport {
  schema_version: 1; kind: 'FROZEN_EXPORT_CAPTURE_AUDIT'; source_export: CaptureAuditRequest['source_export']; request_sha256: string;
  audit: { schema_version: 1; kind: 'CAPTURE_DEPENDENCY_AUDIT'; checked_at_ms: string;
    network: string; config_digest: string; request_sha256: string; status: 'COMPLETE' | 'COMPLETE_WITH_GAPS' | 'INCOMPLETE';
    references_requested: number; references_reported: number; raw_status_counts: Partial<Record<RawStatus, number>>;
    bytes_read_budgeted: string; max_bytes: string; source_authenticity: 'CALLER_RETAINED_DIGEST_NOT_INDEPENDENTLY_AUTHENTICATED';
    filesystem_snapshot: 'NON_ATOMIC_KEEP_ROOT_QUIESCED'; execution_authorized: false; dependencies: AuditDependency[] };
}
function check(value: unknown, message = 'Invalid or mismatched local audit report.'): asserts value { if (!value) throw new Error(message); }
function exact(value: unknown, keys: string): Record<string, unknown> {
  check(value !== null && typeof value === 'object' && !Array.isArray(value));
  const v = value as Record<string, unknown>;
  check(Object.keys(v).sort().join(',') === keys.split(' ').sort().join(',')); return v;
}
function label(value: unknown): value is string { return typeof value === 'string' && /^[A-Za-z0-9_.-]{1,100}$/.test(value) && value !== '.' && value !== '..'; }
function digest(value: unknown): value is string { return typeof value === 'string' && /^sha256:[a-f0-9]{64}$/.test(value); }
function uint(value: unknown, max = U64): value is string { return typeof value === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(value) && BigInt(value) <= max; }
function count(value: unknown, max: number): value is number { return Number.isInteger(value) && typeof value === 'number' && value >= 0 && value <= max; }
function oneOf(value: unknown, values: readonly string[]): boolean { return typeof value === 'string' && values.includes(value); }
function canonical(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (value !== null && typeof value === 'object') return '{' + Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([k, v]) => JSON.stringify(k) + ':' + canonical(v)).join(',') + '}';
  return JSON.stringify(value);
}
export async function auditRequestDigest(value: CaptureAuditRequest | CaptureAuditRequest['capture_request']): Promise<string> {
  const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonical(value)));
  return 'sha256:' + Array.from(new Uint8Array(bytes), n => n.toString(16).padStart(2, '0')).join('');
}
export async function createCaptureAuditRequest(value: FrozenExport): Promise<CaptureAuditRequest> {
  // Take a defensive typed snapshot before the first await. Never change the frozen bundle.
  const bundle = await verifyFrozenExport(parseFrozenExport(value)), session = bundle.data.session;
  check(label(bundle.export_id) && label(session.session_id) && digest(session.configuration_digest), 'This export does not contain auditable capture identities.');
  const refs = bundle.data.capture_dependencies;
  check(refs.length <= MAX_CAPTURE_AUDIT_REFERENCES, 'The local audit supports at most 1,000 references. No partial request was prepared.');
  check(refs.every(ref => label(ref.capture_id) && digest(ref.manifest_digest)), 'This export does not contain auditable capture identities.');
  const request: CaptureAuditRequest = { schema_version: 1, kind: 'FROZEN_EXPORT_CAPTURE_AUDIT_REQUEST',
    source_export: { schema_version: '1.1.0', export_id: bundle.export_id, content_sha256: bundle.content_sha256, session_id: session.session_id },
    capture_request: { schema_version: 1, network: session.network_id, config_digest: session.configuration_digest,
      captures: refs.map(({ capture_id, manifest_digest }) => ({ capture_id, manifest_digest })) } };
  check(new TextEncoder().encode(JSON.stringify(request)).length <= MAX_CAPTURE_AUDIT_REQUEST_BYTES);
  return request;
}
function dependency(value: unknown, ref: CaptureAuditRequest['capture_request']['captures'][number], now: bigint): AuditDependency {
  const v = exact(value, 'capture_id manifest_digest raw_artifact_status reason expiration_status capture_time_status origin created_at_ms raw_expires_at_ms quote_inputs_declared_complete verified_objects missing_objects corrupt_objects replay_status market_performance_eligible');
  check(v.capture_id === ref.capture_id && v.manifest_digest === ref.manifest_digest);
  check(oneOf(v.raw_artifact_status, rawStatuses) && typeof v.reason === 'string' && /^[A-Z_]{1,64}$/.test(v.reason));
  check(v.replay_status === 'NOT_ASSESSED' && v.market_performance_eligible === false);
  check(count(v.verified_objects, 4096) && count(v.missing_objects, 4096) && count(v.corrupt_objects, 4096));
  check(v.verified_objects + v.missing_objects + v.corrupt_objects <= 4096);
  if (v.created_at_ms === null) {
    check(v.origin === 'UNKNOWN' && v.raw_expires_at_ms === null && v.expiration_status === 'UNKNOWN'
      && v.capture_time_status === 'UNKNOWN' && v.quote_inputs_declared_complete === null);
    check(v.raw_artifact_status !== 'AVAILABLE');
  } else {
    check(uint(v.created_at_ms) && BigInt(v.created_at_ms) > 0n);
    check(oneOf(v.origin, ['synthetic', 'manually-constructed', 'recorded-live']) && typeof v.quote_inputs_declared_complete === 'boolean');
    check(v.capture_time_status === (BigInt(v.created_at_ms) > now ? 'AFTER_AUDIT' : 'AT_OR_BEFORE_AUDIT'));
    if (v.raw_expires_at_ms === null) check(v.expiration_status === 'NOT_DECLARED');
    else {
      check(uint(v.raw_expires_at_ms) && BigInt(v.raw_expires_at_ms) > BigInt(v.created_at_ms));
      check(v.expiration_status === (now >= BigInt(v.raw_expires_at_ms) ? 'EXPIRED' : 'NOT_EXPIRED'));
    }
  }
  if (v.raw_artifact_status === 'AVAILABLE') check(v.verified_objects > 0 && v.missing_objects === 0 && v.corrupt_objects === 0 && v.reason === 'DECLARED_BYTES_VERIFIED');
  return v as unknown as AuditDependency;
}
export async function verifyCaptureAuditReport(value: unknown, request: CaptureAuditRequest): Promise<CaptureAuditReport> {
  // This checks structure and export/request binding, NOT who ran an audit or when.
  request = structuredClone(request);
  const v = exact(value, 'schema_version kind source_export request_sha256 audit');
  check(v.schema_version === 1 && v.kind === 'FROZEN_EXPORT_CAPTURE_AUDIT');
  const source = exact(v.source_export, 'schema_version export_id content_sha256 session_id');
  check(Object.entries(request.source_export).every(([key, value]) => source[key] === value), 'Local audit belongs to another frozen export.');
  const a = exact(v.audit, 'schema_version kind checked_at_ms network config_digest request_sha256 status references_requested references_reported raw_status_counts bytes_read_budgeted max_bytes source_authenticity filesystem_snapshot execution_authorized dependencies');
  check(a.schema_version === 1 && a.kind === 'CAPTURE_DEPENDENCY_AUDIT' && a.network === request.capture_request.network && a.config_digest === request.capture_request.config_digest);
  check(uint(a.checked_at_ms) && BigInt(a.checked_at_ms) > 0n && uint(a.max_bytes, 1024n ** 4n) && BigInt(a.max_bytes) > 0n && uint(a.bytes_read_budgeted) && BigInt(a.bytes_read_budgeted) <= BigInt(a.max_bytes));
  check(a.source_authenticity === 'CALLER_RETAINED_DIGEST_NOT_INDEPENDENTLY_AUTHENTICATED' && a.filesystem_snapshot === 'NON_ATOMIC_KEEP_ROOT_QUIESCED' && a.execution_authorized === false);
  const refs = request.capture_request.captures;
  check(a.references_requested === refs.length && a.references_reported === refs.length && Array.isArray(a.dependencies) && a.dependencies.length === refs.length);
  const now = BigInt(a.checked_at_ms);
  const dependencies = a.dependencies.map((row, i) => dependency(row, refs[i], now));
  const counts: Partial<Record<RawStatus, number>> = {};
  for (const row of dependencies) counts[row.raw_artifact_status] = (counts[row.raw_artifact_status] ?? 0) + 1;
  const receivedCounts = exact(a.raw_status_counts, Object.keys(counts).join(' '));
  check(Object.entries(counts).every(([key, count]) => receivedCounts[key] === count));
  const gaps = !dependencies.length || dependencies.some(row => row.raw_artifact_status !== 'AVAILABLE' || row.expiration_status === 'EXPIRED' || row.capture_time_status === 'AFTER_AUDIT' || row.quote_inputs_declared_complete !== true);
  const limited = dependencies.some(row => ['LIMIT_EXCEEDED', 'UNREADABLE'].includes(row.raw_artifact_status));
  check(a.status === (limited ? 'INCOMPLETE' : gaps ? 'COMPLETE_WITH_GAPS' : 'COMPLETE'));
  // Build a detached report before awaiting WebCrypto. Uploaded objects cannot race the check.
  const result = JSON.parse(JSON.stringify({ ...v, source_export: source, audit: { ...a, raw_status_counts: counts, dependencies } })) as CaptureAuditReport;
  check(result.request_sha256 === await auditRequestDigest(request), 'Local audit request digest does not match this export.');
  check(result.audit.request_sha256 === await auditRequestDigest(request.capture_request));
  return result;
}
export async function readCaptureAuditReport(file: Pick<File, 'size' | 'text'>, request: CaptureAuditRequest): Promise<CaptureAuditReport> {
  check(file.size <= MAX_CAPTURE_AUDIT_REPORT_BYTES, 'Local audit report exceeds the 2 MiB limit.');
  const text = await file.text();
  check(new TextEncoder().encode(text).length <= MAX_CAPTURE_AUDIT_REPORT_BYTES);
  let value: unknown;
  try { value = JSON.parse(text); } catch { throw new Error('Invalid local audit JSON.'); }
  return verifyCaptureAuditReport(value, request);
}
