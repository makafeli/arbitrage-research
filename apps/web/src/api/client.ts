import { parseAsset, parseCoverage, parseDecision, parseGroup, parseJournal, parsePaperRun, parseReservation } from './research.ts';
import type { AccountingAsset, InitialBalance } from './research.ts';
import { parseCollectionAttempt, parseCollectionCoverage } from './collection.ts';
import { MAX_FROZEN_EXPORT_BYTES, parseFrozenExport, verifyFrozenExport } from './frozenExport.ts';
export type Network = 'base-mainnet' | 'solana-mainnet';
export type ResearchMode = 'OBSERVE' | 'PAPER' | 'REPLAY';
export type SessionState = 'RECOVERING' | 'STOPPED' | 'RUNNING' | 'PAUSING' | 'PAUSED' | 'DRAINING' | 'FAULTED';
export type CommandAction = 'START' | 'PAUSE' | 'RESUME' | 'STOP';
export interface Session {
  session_id: string; network_id: Network; mode: ResearchMode | 'LIVE'; observed_state: SessionState;
  health: 'HEALTHY' | 'DEGRADED' | 'UNREACHABLE' | 'UNKNOWN'; desired_revision: string; applied_revision: string;
  outstanding_attempts: number; execution_authorized: boolean; last_heartbeat_at: string | null; configuration_digest: string;
}
export interface CommandRequest { action: CommandAction; expected_revision: string; reason?: string }
export interface CommandReceipt {
  command_id: string; session_id: string; revision: string; status: 'PENDING' | 'APPLIED' | 'REJECTED' | 'SUPERSEDED';
  action: CommandAction | 'DISARM'; accepted_at: string; applied_at: string | null; outstanding_attempts: number;
  fence_effective: boolean; signer_revocation_status: 'NOT_APPLICABLE' | 'PENDING' | 'ACKNOWLEDGED';
}
export interface Configuration {
  configuration_digest: string; mode: ResearchMode; enabled_networks: Network[]; strategy_ids: string[];
  paper_assets?: { network_id: Network; asset: AccountingAsset }[];
}
export interface Capabilities {
  modes: ResearchMode[]; live_execution: boolean; market_data: boolean; opportunity_capture: boolean;
  registered_configurations: Configuration[]; command_application?: string;
  decision_history?: boolean; paper_ledger?: boolean; paper_run_creation?: boolean; collection_telemetry?: boolean; session_export?: boolean;
}
export interface CreateSession {
  network_id: Network; mode: ResearchMode; configuration_digest: string; experiment_id: string; strategy_ids: string[];
}
export interface Opportunity {
  schema_version: string; source_kind: 'CAPTURED_MARKET_DATA'; dataset_origin?: 'RECORDED_LIVE'; opportunity_id: string; session_id: string;
  experiment_id: string; network_id: Network; mode: ResearchMode | 'LIVE'; evidence_label: 'CANDIDATE' | 'SIMULATED' | 'ESTIMATED_EXECUTABLE' | 'REALIZED';
  observed_at: string; snapshot: { snapshot_id: string; state_reference: string; source: string; consistent: boolean; complete: boolean; finality_label: string; age_ms: number };
  start_asset_id: string; amount_in_minor: string; quoted_output_minor: string; net_after_explicit_costs_minor: string | null;
  route: { pool_id: string; venue_family: string; asset_in: string; asset_out: string }[];
  costs: { kind: string; native_amount: { asset_id: string; amount_minor: string; decimals: number }; in_start_asset_minor: string; valuation_reference: string }[];
  simulation_status: 'NOT_RUN' | 'PASSED' | 'FAILED' | 'UNSUPPORTED'; inclusion_scenario_id: string | null;
  finality_status: string; transaction_id: string | null; reason_codes: string[]; execution_plan_digest: string | null;
  eligibility_checks: { state_fresh_and_coherent: boolean; atomic_route_supported: boolean; final_balance_guard_present: boolean; costs_complete: boolean; principal_and_fee_reservations_valid: boolean; simulation_matches_exact_plan: boolean };
}
export interface Page<T> { items: T[]; next_cursor: string | null }
export interface AuthSession { operator_id: string; csrf_token: string; expires_at: string }
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  constructor(status: number, code: string, message: string) { super(message); this.name = 'ApiError'; this.status = status; this.code = code; }
}

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new ApiError(0, 'INVALID_RESPONSE', 'The service returned an invalid response.');
  return value as Record<string, unknown>;
}
function assert(condition: unknown, message = 'The service response does not match the expected contract.'): asserts condition {
  if (!condition) throw new ApiError(0, 'INVALID_RESPONSE', message);
}
function oneOf(value: unknown, allowed: readonly string[]) { return typeof value === 'string' && allowed.includes(value); }
function string(value: unknown): value is string { return typeof value === 'string' && value.length > 0; }
function uint(value: unknown) { return typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value); }
function count(value: unknown) { return Number.isSafeInteger(value) && (value as number) >= 0; }
function date(value: unknown) { return typeof value === 'string' && Number.isFinite(Date.parse(value)); }
const networks = ['base-mainnet', 'solana-mainnet'];
const modes = ['OBSERVE', 'PAPER', 'REPLAY', 'LIVE'];
export function parseSession(value: unknown): Session {
  const v = object(value);
  assert(string(v.session_id) && oneOf(v.network_id, networks) && oneOf(v.mode, modes));
  assert(oneOf(v.observed_state, ['RECOVERING', 'STOPPED', 'RUNNING', 'PAUSING', 'PAUSED', 'DRAINING', 'FAULTED']));
  assert(oneOf(v.health, ['HEALTHY', 'DEGRADED', 'UNREACHABLE', 'UNKNOWN']) && uint(v.desired_revision) && uint(v.applied_revision));
  assert(count(v.outstanding_attempts) && typeof v.execution_authorized === 'boolean' && string(v.configuration_digest));
  assert(BigInt(v.applied_revision as string) <= BigInt(v.desired_revision as string));
  assert(v.mode === 'LIVE' || v.execution_authorized === false);
  assert(v.last_heartbeat_at === null || date(v.last_heartbeat_at));
  return v as unknown as Session;
}
export function parseReceipt(value: unknown): CommandReceipt {
  const v = object(value);
  assert(string(v.command_id) && string(v.session_id) && uint(v.revision));
  assert(oneOf(v.status, ['PENDING', 'APPLIED', 'REJECTED', 'SUPERSEDED']) && oneOf(v.action, ['START', 'PAUSE', 'RESUME', 'STOP', 'DISARM']));
  assert(date(v.accepted_at) && (v.applied_at === null || date(v.applied_at)) && count(v.outstanding_attempts));
  assert(typeof v.fence_effective === 'boolean' && oneOf(v.signer_revocation_status, ['NOT_APPLICABLE', 'PENDING', 'ACKNOWLEDGED']));
  assert(v.status !== 'APPLIED' || (date(v.applied_at) && (!['STOP', 'PAUSE', 'DISARM'].includes(v.action as string) || v.fence_effective)));
  return v as unknown as CommandReceipt;
}
export function parseOpportunity(value: unknown): Opportunity {
  const v = object(value);
  assert(v.source_kind === 'CAPTURED_MARKET_DATA', 'Synthetic records are rejected in Connected mode.');
  assert(oneOf(v.schema_version, ['1.0.0', '1.1.0']) && string(v.opportunity_id) && string(v.session_id) && string(v.experiment_id));
  assert(oneOf(v.network_id, networks) && oneOf(v.mode, modes) && date(v.observed_at));
  assert(oneOf(v.evidence_label, ['CANDIDATE', 'SIMULATED', 'ESTIMATED_EXECUTABLE', 'REALIZED']));
  assert(uint(v.amount_in_minor) && uint(v.quoted_output_minor));
  assert(v.net_after_explicit_costs_minor === null ? v.schema_version === '1.1.0' : typeof v.net_after_explicit_costs_minor === 'string' && /^-?(0|[1-9][0-9]*)$/.test(v.net_after_explicit_costs_minor));
  assert(v.schema_version !== '1.1.0' || v.dataset_origin === 'RECORDED_LIVE');
  assert(v.dataset_origin === undefined || v.dataset_origin === 'RECORDED_LIVE', 'Captured opportunity origin and source provenance disagree.');
  assert(string(v.start_asset_id) && Array.isArray(v.reason_codes) && v.reason_codes.every(string));
  assert(oneOf(v.simulation_status, ['NOT_RUN', 'PASSED', 'FAILED', 'UNSUPPORTED']));
  assert(oneOf(v.finality_status, ['NOT_APPLICABLE', 'PROVISIONAL', 'FINALIZED']));
  assert([v.transaction_id, v.execution_plan_digest, v.inclusion_scenario_id].every(x => x === null || string(x)));
  const snap = object(v.snapshot);
  assert(['snapshot_id', 'state_reference', 'source', 'finality_label'].every(k => string(snap[k])) && count(snap.age_ms));
  assert(typeof snap.consistent === 'boolean' && typeof snap.complete === 'boolean');
  const checks = object(v.eligibility_checks);
  assert(['state_fresh_and_coherent', 'atomic_route_supported', 'final_balance_guard_present', 'costs_complete', 'principal_and_fee_reservations_valid', 'simulation_matches_exact_plan'].every(k => typeof checks[k] === 'boolean'));
  assert(v.schema_version !== '1.1.0' || (checks.costs_complete ? v.net_after_explicit_costs_minor !== null : v.net_after_explicit_costs_minor === null), 'Incomplete external costs require unknown net; complete costs require numeric net.');
  assert(Array.isArray(v.route) && v.route.length >= 2 && v.route.length <= 3);
  for (const leg of v.route) { const l = object(leg); assert(['pool_id', 'venue_family', 'asset_in', 'asset_out'].every(k => string(l[k]))); }
  assert(Array.isArray(v.costs));
  for (const cost of v.costs) {
    const c = object(cost); const native = object(c.native_amount);
    assert(string(c.kind) && uint(c.in_start_asset_minor) && string(c.valuation_reference));
    assert(string(native.asset_id) && uint(native.amount_minor) && count(native.decimals) && (native.decimals as number) <= 255);
  }
  assert(v.evidence_label !== 'REALIZED' || (v.mode === 'LIVE' && string(v.transaction_id) && v.finality_status !== 'NOT_APPLICABLE'));
  assert(!['SIMULATED', 'ESTIMATED_EXECUTABLE'].includes(v.evidence_label as string) || (v.simulation_status === 'PASSED' && checks.simulation_matches_exact_plan && checks.atomic_route_supported && checks.final_balance_guard_present && string(v.execution_plan_digest)));
  assert(v.evidence_label !== 'ESTIMATED_EXECUTABLE' || (v.net_after_explicit_costs_minor !== null && Object.values(checks).every(x => x === true) && snap.consistent && snap.complete && string(v.inclusion_scenario_id)));
  return v as unknown as Opportunity;
}
function parseCapabilities(value: unknown): Capabilities {
  const v = object(value);
  assert(Array.isArray(v.modes) && v.modes.every(mode => oneOf(mode, modes.slice(0, 3))));
  assert(['live_execution', 'market_data', 'opportunity_capture'].every(k => typeof v[k] === 'boolean'));
  assert(['decision_history', 'paper_ledger', 'paper_run_creation', 'collection_telemetry', 'session_export'].every(k => v[k] === undefined || typeof v[k] === 'boolean'));
  assert(Array.isArray(v.registered_configurations));
  for (const config of v.registered_configurations) {
    const c = object(config);
    assert(string(c.configuration_digest) && oneOf(c.mode, modes.slice(0, 3)));
    assert(Array.isArray(c.enabled_networks) && c.enabled_networks.every(n => oneOf(n, networks)));
    assert(Array.isArray(c.strategy_ids) && c.strategy_ids.every(string));
    if (c.paper_assets !== undefined) {
      assert(Array.isArray(c.paper_assets) && c.paper_assets.length <= 128);
      for (const value of c.paper_assets) { const p = object(value); assert(oneOf(p.network_id, networks)); parseAsset(p.asset, p.network_id as string); }
    }
  }
  return v as unknown as Capabilities;
}
function parseAuth(value: unknown): AuthSession {
  const v = object(value); assert(string(v.operator_id) && string(v.csrf_token) && uint(v.expires_at));
  return v as unknown as AuthSession;
}
function parsePage<T>(value: unknown, parse: (v: unknown) => T): Page<T> {
  const v = object(value); assert(Array.isArray(v.items) && v.items.length <= 100 && (v.next_cursor === null || string(v.next_cursor)));
  return { items: v.items.map(parse), next_cursor: v.next_cursor as string | null };
}

export class ControlApi {
  private csrf: string | null = null;
  private readonly transport: typeof fetch;
  // Native browser fetch requires the Window receiver. Keeping it directly as
  // a class property and calling this.transport(...) triggers Illegal invocation.
  constructor(transport: typeof fetch = (input, init) => globalThis.fetch(input, init)) { this.transport = transport; }
  clearAuth() { this.csrf = null; }
  private async request(path: string, options: { body?: unknown; key?: string; signal?: AbortSignal; method?: string; maxResponseBytes?: number } = {}): Promise<unknown> {
    const headers: Record<string, string> = { Accept: 'application/json' };
    if (options.body !== undefined) headers['Content-Type'] = 'application/json';
    if (options.method === 'POST' && this.csrf) headers['X-CSRF-Token'] = this.csrf;
    if (options.key) headers['Idempotency-Key'] = options.key;
    const response = await this.transport(`/v1${path}`, { method: options.method ?? 'GET', credentials: 'same-origin', cache: 'no-store', headers, body: options.body === undefined ? undefined : JSON.stringify(options.body), signal: options.signal ? AbortSignal.any([options.signal, AbortSignal.timeout(10_000)]) : AbortSignal.timeout(10_000) });
    if (response.status === 204) return null;
    let body: unknown;
    try {
      if (options.maxResponseBytes && response.body) {
        const reader = response.body.getReader(), chunks: Uint8Array[] = []; let size = 0;
        while (true) {
          const next = await reader.read(); if (next.done) break;
          size += next.value.byteLength;
          if (size > options.maxResponseBytes) { await reader.cancel(); throw new ApiError(response.status, 'EXPORT_LIMIT_EXCEEDED', 'Frozen export exceeds the 8 MiB response bound. No partial download was prepared.'); }
          chunks.push(next.value);
        }
        const bytes = new Uint8Array(size); let offset = 0;
        for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
        body = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
      } else body = await response.json();
    } catch (e) { if (e instanceof ApiError) throw e; throw new ApiError(response.status, 'INVALID_RESPONSE', 'The API did not return JSON. Check the same-origin service configuration.'); }
    if (!response.ok) {
      const error = object(body);
      if (response.status === 401) this.clearAuth();
      throw new ApiError(response.status, typeof error.code === 'string' ? error.code : 'REQUEST_FAILED', typeof error.message === 'string' ? error.message : 'The service rejected the request.');
    }
    return body;
  }
  async auth(signal?: AbortSignal) { const auth = parseAuth(await this.request('/auth/session', { signal })); this.csrf = auth.csrf_token; return auth; }
  async login(operator_secret: string) { const auth = parseAuth(await this.request('/auth/login', { method: 'POST', body: { operator_secret } })); this.csrf = auth.csrf_token; return auth; }
  async logout() { await this.request('/auth/logout', { method: 'POST' }); this.clearAuth(); }
  async capabilities(signal?: AbortSignal) { return parseCapabilities(await this.request('/capabilities', { signal })); }
  async sessions(signal?: AbortSignal, cursor?: string) { return parsePage(await this.request(`/sessions?limit=100${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ''}`, { signal }), parseSession); }
  async opportunities(signal?: AbortSignal) { return parsePage(await this.request('/opportunities?limit=100&source_kind=CAPTURED_MARKET_DATA', { signal }), parseOpportunity); }
  async decisions(sessionId: string, cursor?: string, signal?: AbortSignal) {
    const page = parsePage(await this.request('/decisions?' + this.query(cursor, sessionId), { signal }), parseDecision);
    assert(page.items.every(item => item.trace.session_id === sessionId), 'Decision response has the wrong session scope.'); return page;
  }
  async decision(observationId: string, signal?: AbortSignal) {
    const item = parseDecision(await this.request('/decisions/' + encodeURIComponent(observationId), { signal }));
    assert(item.trace.observation_id === observationId, 'Decision response has the wrong observation.'); return item;
  }
  async decisionGroups(sessionId: string, cursor?: string, signal?: AbortSignal) {
    return parsePage(await this.request('/decision-groups?' + this.query(cursor, sessionId), { signal }), parseGroup);
  }
  async coverage(sessionId: string, signal?: AbortSignal) {
    const coverage = parseCoverage(await this.request('/decision-coverage?session_id=' + encodeURIComponent(sessionId), { signal }));
    assert(coverage.session_id === sessionId, 'Coverage response has the wrong session scope.'); return coverage;
  }
  async collectionCoverage(sessionId: string, signal?: AbortSignal) {
    const coverage = parseCollectionCoverage(await this.request('/sessions/' + encodeURIComponent(sessionId) + '/collection-coverage', { signal }));
    assert(coverage.session_id === sessionId, 'Collection coverage response has the wrong session scope.'); return coverage;
  }
  async collectionAttempts(sessionId: string, cursor?: string, signal?: AbortSignal) {
    const page = parsePage(await this.request('/sessions/' + encodeURIComponent(sessionId) + '/collection-attempts?' + this.query(cursor), { signal }), parseCollectionAttempt);
    assert(page.items.every(item => item.session_id === sessionId), 'Collection attempts response has the wrong session scope.'); return page;
  }
  async frozenExport(sessionId: string, signal?: AbortSignal) {
    const bundle = parseFrozenExport(await this.request('/sessions/' + encodeURIComponent(sessionId) + '/export', { signal, maxResponseBytes: MAX_FROZEN_EXPORT_BYTES }));
    assert(bundle.data.session.session_id === sessionId, 'Frozen export response has the wrong session scope.');
    return verifyFrozenExport(bundle);
  }
  async paperRuns(sessionId: string, cursor?: string, signal?: AbortSignal) {
    const page = parsePage(await this.request('/sessions/' + encodeURIComponent(sessionId) + '/paper-runs?' + this.query(cursor), { signal }), parsePaperRun);
    assert(page.items.every(item => item.session_id === sessionId), 'Paper runs response has the wrong session scope.'); return page;
  }
  async paperRun(runId: string, signal?: AbortSignal) {
    const run = parsePaperRun(await this.request('/paper-runs/' + encodeURIComponent(runId), { signal }));
    assert(run.run_id === runId, 'Paper run response has the wrong run scope.'); return run;
  }
  async createPaperRun(sessionId: string, body: { initial_balances: InitialBalance[] }, key: string) {
    const run = parsePaperRun(await this.request('/sessions/' + encodeURIComponent(sessionId) + '/paper-runs', { method: 'POST', body, key }));
    assert(run.session_id === sessionId, 'Paper creation response has the wrong session scope.');
    assert(run.initial_balances.length === body.initial_balances.length && body.initial_balances.every(expected => run.initial_balances.some(actual => actual.asset.kind === expected.asset.kind && actual.asset.identity === expected.asset.identity && actual.amount === expected.amount)), 'Paper creation response does not preserve the requested initial balances.'); return run;
  }
  async journal(runId: string, cursor?: string, signal?: AbortSignal) {
    const page = parsePage(await this.request('/paper-runs/' + encodeURIComponent(runId) + '/journal?' + this.query(cursor), { signal }), parseJournal);
    assert(page.items.every(item => item.event.run_id === runId), 'Journal response has the wrong run scope.'); return page;
  }
  async reservations(runId: string, cursor?: string, signal?: AbortSignal) {
    return parsePage(await this.request('/paper-runs/' + encodeURIComponent(runId) + '/reservations?' + this.query(cursor), { signal }), parseReservation);
  }
  private query(cursor?: string, sessionId?: string) {
    const params = new URLSearchParams({ limit: '25' });
    if (cursor) params.set('cursor', cursor);
    if (sessionId) params.set('session_id', sessionId);
    return params.toString();
  }

  async create(body: CreateSession, key: string) { return parseSession(await this.request('/sessions', { method: 'POST', body, key })); }
  async command(sessionId: string, body: CommandRequest, key: string) {
    const receipt = parseReceipt(await this.request(`/sessions/${encodeURIComponent(sessionId)}/commands`, { method: 'POST', body, key }));
    assert(receipt.session_id === sessionId && receipt.action === body.action, 'The command receipt does not match the requested session and action.');
    return receipt;
  }
  async receipt(commandId: string, signal?: AbortSignal) {
    const receipt = parseReceipt(await this.request(`/commands/${encodeURIComponent(commandId)}`, { signal }));
    assert(receipt.command_id === commandId, 'The command receipt does not match the requested command.');
    return receipt;
  }
}

export function availableActions(session: Session): CommandAction[] {
  if (session.mode === 'LIVE') return [];
  if (session.observed_state === 'STOPPED' && session.desired_revision !== session.applied_revision) return ['START', 'STOP'];
  switch (session.observed_state) {
    case 'STOPPED': return ['START'];
    case 'RUNNING': return ['PAUSE', 'STOP'];
    case 'PAUSED': return ['RESUME', 'STOP'];
    case 'PAUSING': case 'DRAINING': case 'FAULTED': case 'RECOVERING': return ['STOP'];
  }
}
export function errorMessage(error: unknown) { return error instanceof Error ? error.message : 'The service could not be reached.'; }
