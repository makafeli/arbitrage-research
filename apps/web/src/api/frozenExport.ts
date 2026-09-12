import { parseSession } from './client.ts';
import type { Session } from './client.ts';
import { parseCollectionAttempt } from './collection.ts';
import type { CollectionAttempt } from './collection.ts';
import { initialBalance, parseCoverage, parseDecision, parseJournal, parsePaperRun, parseReservation, record, requireValue, text, unsigned } from './research.ts';
import type { Coverage, InitialBalance, JournalRecord, PaperRunRecord, ReservationRecord, StoredDecision } from './research.ts';

export const MAX_FROZEN_EXPORT_BYTES = 8 * 1024 * 1024;
export const MAX_FROZEN_EXPORT_ROWS = 10_000;
export interface CaptureDependency {
  capture_id: string; manifest_digest: string; snapshot_ids: string[]; catalog_status: 'PRESENT' | 'MISSING';
  raw_artifact_status: 'NOT_VERIFIED'; expiration_status: 'UNKNOWN'; generation: string | null; admitted_at: string | null;
}
export type ExportCommand = { kind: 'INITIALIZE'; balances: InitialBalance[] }
  | { kind: 'RESERVE'; request: { attempt_id: string; principal_asset: string; principal: string; native_fee_budget: string } }
  | { kind: 'MARK_UNKNOWN'; attempt_id: string; reason: 'REDACTED_FREEFORM_TEXT' }
  | { kind: 'RESOLVE'; attempt_id: string; outcome: { kind: 'SUCCEEDED'; amount_out: string; actual_native_fee: string } | { kind: 'FAILED_INCLUDED'; actual_native_fee: string } | { kind: 'NOT_INCLUDED'; reason: 'REDACTED_FREEFORM_TEXT' } };
export interface ExportJournal extends Omit<JournalRecord, 'event'> {
  source_payload_sha256: string; event: Omit<JournalRecord['event'], 'command'> & { command: ExportCommand };
}
export const exportSourceKeys = ['decisions', 'paper_runs', 'paper_journal_events', 'capture_catalog_entries', 'collection_attempts'] as const;
const methodValues = {
  amounts: 'BASE_UNIT_INTEGER_STRINGS', asset_decimals: 'NOT_RETAINED_IN_DATABASE', costs: 'UNKNOWN_COSTS_REMAIN_NULL',
  configuration_snapshot: 'DIGEST_ONLY', raw_artifacts: 'REFERENCED_NOT_INCLUDED_OR_VERIFIED',
  hash_format: 'SHA256_SORTED_KEY_COMPACT_JSON_SNAPSHOT_METHODOLOGY_DATA_V1', journal_projection: 'REDACTED_REPLAYABLE_ACCOUNTING_PROJECTION',
  execution_authorized: false, max_source_rows: '10000', max_bytes: '8388608',
} as const;
export interface FrozenExport {
  schema_version: '1.0.0'; export_id: string; exported_at: string; content_sha256: string;
  snapshot: { isolation: 'REPEATABLE_READ'; scope: 'COMPLETE_STORED_SESSION'; collection_completeness: 'UNKNOWN'; source_counts: Record<typeof exportSourceKeys[number], string> };
  methodology: typeof methodValues;
  data: { session: Session; experiment_id: string; strategy_ids: string[]; decision_coverage: Coverage;
    decisions: StoredDecision[]; paper_runs: { run: PaperRunRecord; journal: ExportJournal[]; reservations: ReservationRecord[] }[];
    capture_dependencies: CaptureDependency[]; collection_attempts: CollectionAttempt[] };
}
function digest(value: unknown): value is string { return typeof value === 'string' && /^sha256:[a-f0-9]{64}$/.test(value); }
function date(value: unknown): value is string { return text(value) && Number.isFinite(Date.parse(value)); }
function list(value: unknown, bound = MAX_FROZEN_EXPORT_ROWS): unknown[] { requireValue(Array.isArray(value) && value.length <= bound); return value; }
function texts(value: unknown, bound: number) { const values = list(value, bound); requireValue(values.every(text)); return values; }
function parseCommand(value: unknown, network: string): ExportCommand {
  const c = record(value);
  if (c.kind === 'INITIALIZE') return { kind: 'INITIALIZE', balances: list(c.balances, 100).map(v => initialBalance(v, network)) };
  requireValue(['RESERVE', 'MARK_UNKNOWN', 'RESOLVE'].includes(c.kind as string));
  if (c.kind === 'RESERVE') {
    const r = record(c.request); requireValue(digest(r.attempt_id) && text(r.principal_asset) && r.principal_asset.startsWith(network + ':') && unsigned(r.principal) && unsigned(r.native_fee_budget));
    return { kind: 'RESERVE', request: { attempt_id: r.attempt_id, principal_asset: r.principal_asset, principal: r.principal, native_fee_budget: r.native_fee_budget } };
  }
  requireValue(digest(c.attempt_id));
  if (c.kind === 'MARK_UNKNOWN') { requireValue(c.reason === 'REDACTED_FREEFORM_TEXT'); return { kind: 'MARK_UNKNOWN', attempt_id: c.attempt_id, reason: 'REDACTED_FREEFORM_TEXT' }; }
  const o = record(c.outcome);
  if (o.kind === 'NOT_INCLUDED') { requireValue(o.reason === 'REDACTED_FREEFORM_TEXT'); return { kind: 'RESOLVE', attempt_id: c.attempt_id, outcome: { kind: 'NOT_INCLUDED', reason: 'REDACTED_FREEFORM_TEXT' } }; }
  requireValue((o.kind === 'SUCCEEDED' || o.kind === 'FAILED_INCLUDED') && unsigned(o.actual_native_fee));
  if (o.kind === 'SUCCEEDED') { requireValue(unsigned(o.amount_out)); return { kind: 'RESOLVE', attempt_id: c.attempt_id, outcome: { kind: 'SUCCEEDED', amount_out: o.amount_out, actual_native_fee: o.actual_native_fee } }; }
  return { kind: 'RESOLVE', attempt_id: c.attempt_id, outcome: { kind: 'FAILED_INCLUDED', actual_native_fee: o.actual_native_fee } };
}
function parseExportJournal(value: unknown): ExportJournal {
  const raw = record(value), parsed = parseJournal(value);
  requireValue(digest(raw.source_payload_sha256) && digest(parsed.event.command_id));
  return { event_id: parsed.event_id, recorded_at: parsed.recorded_at, source_payload_sha256: raw.source_payload_sha256,
    event: { ...parsed.event, command: parseCommand(record(raw.event).command, parsed.event.network) } };
}
function parseCaptureDependency(value: unknown): CaptureDependency {
  const v = record(value); requireValue(text(v.capture_id) && text(v.manifest_digest));
  requireValue((v.catalog_status === 'PRESENT' || v.catalog_status === 'MISSING') && v.raw_artifact_status === 'NOT_VERIFIED' && v.expiration_status === 'UNKNOWN');
  requireValue(v.generation === null || unsigned(v.generation)); requireValue(v.admitted_at === null || date(v.admitted_at));
  requireValue(v.catalog_status === 'MISSING' ? v.generation === null && v.admitted_at === null : v.generation !== null && v.admitted_at !== null);
  return { capture_id: v.capture_id, manifest_digest: v.manifest_digest, snapshot_ids: texts(v.snapshot_ids, MAX_FROZEN_EXPORT_ROWS),
    catalog_status: v.catalog_status, raw_artifact_status: 'NOT_VERIFIED', expiration_status: 'UNKNOWN', generation: v.generation as string | null, admitted_at: v.admitted_at as string | null };
}
export function canonicalExportContent(value: Pick<FrozenExport, 'snapshot' | 'methodology' | 'data'>): string {
  function sorted(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(sorted);
    if (value !== null && typeof value === 'object') return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([key, entry]) => [key, sorted(entry)]));
    return value;
  }
  return JSON.stringify(sorted({ snapshot: value.snapshot, methodology: value.methodology, data: value.data }));
}
export function parseFrozenExport(value: unknown): FrozenExport {
  requireValue(new TextEncoder().encode(JSON.stringify(value)).length <= MAX_FROZEN_EXPORT_BYTES, 'Frozen export exceeds the 8 MiB response bound.');
  const v = record(value), s = record(v.snapshot), counts = record(s.source_counts), m = record(v.methodology), d = record(v.data);
  requireValue(v.schema_version === '1.0.0' && text(v.export_id) && date(v.exported_at) && digest(v.content_sha256));
  requireValue(s.isolation === 'REPEATABLE_READ' && s.scope === 'COMPLETE_STORED_SESSION' && s.collection_completeness === 'UNKNOWN');
  const source_counts = {} as FrozenExport['snapshot']['source_counts'];
  for (const key of exportSourceKeys) { requireValue(unsigned(counts[key])); source_counts[key] = counts[key]; }
  requireValue(Object.values(source_counts).reduce((sum, v) => sum + BigInt(v), 0n) <= BigInt(MAX_FROZEN_EXPORT_ROWS), 'Frozen export exceeds the complete snapshot source row bound.');
  for (const [key, expected] of Object.entries(methodValues)) requireValue(m[key] === expected, 'Unsupported frozen export methodology.');
  const session = parseSession(d.session);
  // Explicit projection prevents future/private response properties entering downloads.
  const projectedSession: Session = { session_id: session.session_id, network_id: session.network_id, mode: session.mode,
    observed_state: session.observed_state, health: session.health, desired_revision: session.desired_revision, applied_revision: session.applied_revision,
    outstanding_attempts: session.outstanding_attempts, execution_authorized: session.execution_authorized,
    last_heartbeat_at: session.last_heartbeat_at, configuration_digest: session.configuration_digest };
  requireValue(!projectedSession.execution_authorized && projectedSession.mode !== 'LIVE');
  requireValue(text(d.experiment_id));
  const decisions = list(d.decisions).map(parseDecision), coverage = parseCoverage(d.decision_coverage);
  const paper_runs = list(d.paper_runs).map(value => { const p = record(value); const run = parsePaperRun(p.run);
    const journal = list(p.journal, 5000).map(parseExportJournal), reservations = list(p.reservations, 5000).map(parseReservation);
    requireValue(journal.every(item => item.event.run_id === run.run_id && item.event.network === run.network_id));
    requireValue(reservations.every(item => digest(item.attempt_id) && item.principal_asset.startsWith(run.network_id + ':')));
    return { run, journal, reservations }; });
  const capture_dependencies = list(d.capture_dependencies, MAX_FROZEN_EXPORT_ROWS * 3).map(parseCaptureDependency);
  const collection_attempts = list(d.collection_attempts).map(parseCollectionAttempt);
  requireValue(decisions.every(item => item.trace.session_id === session.session_id && item.trace.network_id === session.network_id && item.trace.configuration_digest === session.configuration_digest && item.trace.experiment_id === d.experiment_id));
  requireValue(paper_runs.every(item => item.run.session_id === session.session_id && item.run.network_id === session.network_id && item.run.configuration_digest === session.configuration_digest));
  requireValue(collection_attempts.every(item => item.session_id === session.session_id && item.network_id === session.network_id && item.configuration_digest === session.configuration_digest && item.experiment_id === d.experiment_id));
  requireValue(coverage.session_id === session.session_id && coverage.raw_observations === source_counts.decisions);
  requireValue(source_counts.decisions === String(decisions.length) && source_counts.paper_runs === String(paper_runs.length)
    && source_counts.paper_journal_events === String(paper_runs.reduce((sum, run) => sum + run.journal.length, 0))
    && source_counts.capture_catalog_entries === String(capture_dependencies.filter(dep => dep.catalog_status === 'PRESENT').length)
    && source_counts.collection_attempts === String(collection_attempts.length), 'Frozen export source counts do not match its records.');
  const dependencies = new Map(capture_dependencies.map(dep => [JSON.stringify([dep.capture_id, dep.manifest_digest]), new Set(dep.snapshot_ids)]));
  requireValue(dependencies.size === capture_dependencies.length, 'Frozen export repeats a capture dependency.');
  requireValue(decisions.every(item => item.trace.capture_refs.every(ref => dependencies.get(JSON.stringify([ref.capture_id, ref.manifest_digest]))?.has(ref.snapshot_id))), 'Frozen export omits a decision capture dependency.');
  const observations = new Map(decisions.map(item => [item.trace.observation_id, item.trace]));
  requireValue(observations.size === decisions.length, 'Frozen export repeats an observation.');
  requireValue(collection_attempts.every(attempt => attempt.decision_observation_ids.every(id => observations.get(id)?.generation === attempt.generation)), 'Frozen export collection attempts reference absent or mismatched decisions.');
  const result: FrozenExport = { schema_version: '1.0.0', export_id: v.export_id, exported_at: v.exported_at, content_sha256: v.content_sha256,
    snapshot: { isolation: 'REPEATABLE_READ', scope: 'COMPLETE_STORED_SESSION', collection_completeness: 'UNKNOWN', source_counts },
    methodology: { ...methodValues }, data: { session: projectedSession, experiment_id: d.experiment_id, strategy_ids: texts(d.strategy_ids, 128),
      decision_coverage: coverage, decisions, paper_runs, capture_dependencies, collection_attempts } };
  // Unknown fields cannot be silently dropped under an unchanged source digest.
  requireValue(canonicalExportContent(result) === canonicalExportContent({ snapshot: s, methodology: m, data: d } as unknown as FrozenExport), 'Frozen export contains unsupported fields; no download was prepared.');
  return result;
}
export async function verifyFrozenExport(value: FrozenExport): Promise<FrozenExport> {
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonicalExportContent(value)));
  const actual = 'sha256:' + Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('');
  requireValue(actual === value.content_sha256, 'Frozen export content digest does not match the received snapshot.');
  return value;
}
