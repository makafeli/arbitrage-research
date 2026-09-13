import type { Network } from './client.ts';
import { parseChainFreshness } from './freshness.ts';
import type { ChainFreshness } from './freshness.ts';

export type Origin = 'SYNTHETIC' | 'MANUALLY_CONSTRUCTED' | 'RECORDED_LIVE';
export type SourceKind = 'SYNTHETIC_FIXTURE' | 'CAPTURED_MARKET_DATA';
export interface AccountingAsset { kind: 'TOKEN' | 'NATIVE'; identity: string }
export interface InitialBalance { asset: AccountingAsset; amount: string }
export interface PaperRunRecord {
  run_id: string; session_id: string; network_id: Network; mode: 'PAPER'; configuration_digest: string;
  created_at: string; revision: string; initial_balances: InitialBalance[];
  balances: { asset: AccountingAsset; free: string; reserved: string; total: string }[];
  outstanding_reservations: number; evidence_label: 'HYPOTHETICAL'; execution_authorized: false;
}
export interface JournalRecord {
  event_id: string; recorded_at: string;
  event: { run_id: string; network: Network; sequence: string; command_id: string;
    command: { kind: 'INITIALIZE' | 'RESERVE' | 'MARK_UNKNOWN' | 'RESOLVE' };
    postings: { asset: AccountingAsset; account: string; side: 'DEBIT' | 'CREDIT'; amount: string }[] };
}
export interface ReservationRecord { attempt_id: string; principal_asset: string; principal: string; native_fee_budget: string; state: 'RESERVED' | 'UNKNOWN' | 'RESOLVED' }
export interface DecisionTrace {
  schema_version: string; observation_id: string; session_id: string; experiment_id: string; generation: string;
  configuration_digest: string; calculation_version: string; strategy_id: string; network_id: Network;
  mode: 'OBSERVE' | 'PAPER' | 'REPLAY'; source_kind: SourceKind; dataset_origin: Origin;
  observed_at_unix_ms: number; input_age_ms: number | null; chain_freshness?: ChainFreshness;
  capture_refs: { capture_id: string; manifest_digest: string; snapshot_id: string }[];
  route: { pool_id: string; asset_in: string; asset_out: string; venue_family: string }[];
  amount_in_minor: string | null;
  result: { status: 'QUOTED'; quoted_output_minor: string; gross_delta_minor: string; included_pool_fees: string[] }
    | { status: 'REJECTED' | 'NO_ROUTE' | 'DATA_UNAVAILABLE'; reason_codes: string[] };
  grouping: { version: string; key: string; window_ms: number; window_start_ms: number };
  diagnostics: string[];
}
export interface StoredDecision { trace_id: string; recorded_at: string; trace: DecisionTrace }
export interface DecisionGroup {
  grouping_version: string; grouping_key: string; window_start_ms: number;
  dataset_origin: Origin; source_kind: SourceKind; raw_observations: string;
  quoted_candidates: string; rejected: string; no_route: string; data_unavailable: string;
}
export interface Coverage {
  session_id: string; raw_observations: string; quoted_candidates: string; rejected: string; no_route: string;
  data_unavailable: string; unique_opportunity_groups: string; eligible_attempts: string | null;
  reconciled_transactions: string | null; execution_accounting_available: boolean; collection_completeness: 'UNKNOWN';
  coverage_window_start_ms: number | null; coverage_window_end_ms: number | null;
}
export function requireValue(value: unknown, message = 'The research response does not match the expected service contract.'): asserts value {
  if (!value) throw new Error(message);
}
export function record(value: unknown): Record<string, unknown> {
  requireValue(Boolean(value) && typeof value === 'object' && !Array.isArray(value));
  return value as Record<string, unknown>;
}
export function text(value: unknown): value is string { return typeof value === 'string' && value.length > 0 && value.length <= 65536; }
export function unsigned(value: unknown): value is string {
  return typeof value === 'string' && /^(0|[1-9][0-9]{0,77})$/.test(value) && BigInt(value) < (1n << 256n);
}
function signed(value: unknown): value is string { return typeof value === 'string' && /^-?(0|[1-9][0-9]{0,77})$/.test(value) && BigInt(value) > -(1n << 256n) && BigInt(value) < (1n << 256n); }
function choice(value: unknown, values: readonly string[]): value is string { return typeof value === 'string' && values.includes(value); }
function instant(value: unknown): value is string { return text(value) && Number.isFinite(Date.parse(value)); }
function ms(value: unknown): value is number { return Number.isSafeInteger(value) && (value as number) >= 0 && (value as number) <= 8640000000000000; }
const networks = ['base-mainnet', 'solana-mainnet'];
function stringList(value: unknown): string[] { requireValue(Array.isArray(value) && value.length <= 128 && value.every(text)); return value as string[]; }
const reasonCodes: readonly string[] = ["NO_CAPTURE_INPUTS","CAPTURE_UNAVAILABLE","PROVIDER_UNAVAILABLE","CAPTURE_CONFIGURATION_MISMATCH","CAPTURE_ORIGIN_MISMATCH","CAPTURE_IDENTITY_MISMATCH","CAPTURE_CONTEXT_MISMATCH","CAPTURE_PAIR_INCOMPLETE","CAPTURE_DATA_INCOMPLETE","UNSUPPORTED_POOL_MODEL","UNSUPPORTED_TOKEN_BEHAVIOR","UNSUPPORTED_SIZE","UNSUPPORTED_REQUESTED_CAPABILITY","CURRENT_PROTOCOL_EQUIVALENCE_UNQUALIFIED","TOKEN_BEHAVIOR_UNQUALIFIED","RESEARCH_MATH_ONLY","FULL_TRANSACTION_SIMULATION_NOT_RUN","EXTERNAL_COSTS_UNAVAILABLE","VIRTUAL_FUNDING_NOT_RESERVED","NO_CONFIGURED_START_ASSET","NO_CONFIGURED_TRADE_SIZES","NO_ELIGIBLE_POOL_PAIRS","DISTINCT_POOL_REQUIRED","ASSET_CONTINUITY_MISMATCH","ARITHMETIC_OVERFLOW","ZERO_LIQUIDITY","INCOMPLETE_TICK_COVERAGE","MATH_INPUT_REJECTED","OUTPUT_ROUNDS_TO_ZERO","ROUTE_BUDGET_EXHAUSTED","EVALUATION_BUDGET_EXHAUSTED","MAX_POOL_BOUND_EXCEEDED","STALE_INPUT","WORK_GENERATION_CANCELLED","DEADLINE_EXPIRED","SNAPSHOT_NOT_ATOMIC","POOLS_OUTSIDE_CONFIG","SOURCE_QUALIFICATION_PENDING","RATE_LIMITED","CAPTURE_LIMIT_REACHED","QUOTE_CAPABILITY_UNQUALIFIED","NO_QUOTED_ROUTES","CHAIN_TIME_UNAVAILABLE","CHAIN_TIME_FUTURE","CHAIN_TIME_STALE","CHAIN_TIME_INVALID"];
function publicCodes(value: unknown, allowEmpty = false): string[] {
  requireValue(Array.isArray(value) && value.length <= 64 && (allowEmpty || value.length > 0) && value.every(code => typeof code === 'string' && reasonCodes.includes(code)), 'Only named public decision codes are accepted.');
  return value as string[];
}
function provenance(v: Record<string, unknown>) {
  requireValue(choice(v.dataset_origin, ['SYNTHETIC', 'MANUALLY_CONSTRUCTED', 'RECORDED_LIVE']));
  requireValue(v.source_kind === (v.dataset_origin === 'RECORDED_LIVE' ? 'CAPTURED_MARKET_DATA' : 'SYNTHETIC_FIXTURE'), 'Research origin and source provenance disagree.');
}
export function parseAsset(value: unknown, network?: string): AccountingAsset {
  const v = record(value); requireValue(choice(v.kind, ['TOKEN', 'NATIVE']) && text(v.identity));
  requireValue(network === undefined || (v.kind === 'NATIVE' ? v.identity === network : v.identity.startsWith(network + ':')));
  return { kind: v.kind as AccountingAsset['kind'], identity: v.identity };
}
export function initialBalance(value: unknown, network?: string): InitialBalance {
  const v = record(value); requireValue(unsigned(v.amount)); return { asset: parseAsset(v.asset, network), amount: v.amount };
}
export function parsePaperRun(value: unknown): PaperRunRecord {
  const v = record(value);
  requireValue(text(v.run_id) && text(v.session_id) && choice(v.network_id, networks) && v.mode === 'PAPER');
  requireValue(v.evidence_label === 'HYPOTHETICAL' && v.execution_authorized === false && text(v.configuration_digest) && unsigned(v.revision) && instant(v.created_at));
  requireValue(Number.isSafeInteger(v.outstanding_reservations) && (v.outstanding_reservations as number) >= 0);
  requireValue(Array.isArray(v.initial_balances) && v.initial_balances.length <= 100 && Array.isArray(v.balances) && v.balances.length <= 100);
  const initial = v.initial_balances.map(b => initialBalance(b, v.network_id as string));
  const balances = v.balances.map(value => {
    const b = record(value); requireValue(unsigned(b.free) && unsigned(b.reserved) && unsigned(b.total));
    requireValue(BigInt(b.free) + BigInt(b.reserved) === BigInt(b.total), 'Inconsistent paper balance: free plus reserved does not equal total.');
    return { asset: parseAsset(b.asset, v.network_id as string), free: b.free, reserved: b.reserved, total: b.total };
  });
  requireValue(new Set(balances.map(b => b.asset.kind + ':' + b.asset.identity)).size === balances.length);
  return { run_id: v.run_id, session_id: v.session_id, network_id: v.network_id as Network, mode: 'PAPER',
    configuration_digest: v.configuration_digest, created_at: v.created_at, revision: v.revision, initial_balances: initial,
    balances, outstanding_reservations: v.outstanding_reservations as number, evidence_label: 'HYPOTHETICAL', execution_authorized: false };
}
export function parseJournal(value: unknown): JournalRecord {
  const v = record(value), e = record(v.event), c = record(e.command);
  requireValue(text(v.event_id) && instant(v.recorded_at) && text(e.run_id) && choice(e.network, networks) && unsigned(e.sequence) && text(e.command_id));
  requireValue(choice(c.kind, ['INITIALIZE', 'RESERVE', 'MARK_UNKNOWN', 'RESOLVE']) && Array.isArray(e.postings) && e.postings.length <= 128);
  const postings = e.postings.map(value => {
    const p = record(value);
    requireValue(choice(p.account, ['AVAILABLE', 'RESERVED', 'INITIAL_CAPITAL', 'MARKET', 'FEES']) && choice(p.side, ['DEBIT', 'CREDIT']) && unsigned(p.amount));
    return { asset: parseAsset(p.asset, e.network as string), account: p.account, side: p.side as 'DEBIT' | 'CREDIT', amount: p.amount };
  });
  return { event_id: v.event_id, recorded_at: v.recorded_at, event: { run_id: e.run_id, network: e.network as Network,
    sequence: e.sequence, command_id: e.command_id, command: { kind: c.kind as JournalRecord['event']['command']['kind'] }, postings } };
}
export function parseReservation(value: unknown): ReservationRecord {
  const v = record(value);
  requireValue(text(v.attempt_id) && text(v.principal_asset) && unsigned(v.principal) && unsigned(v.native_fee_budget) && choice(v.state, ['RESERVED', 'UNKNOWN', 'RESOLVED']));
  return { attempt_id: v.attempt_id, principal_asset: v.principal_asset, principal: v.principal, native_fee_budget: v.native_fee_budget, state: v.state as ReservationRecord['state'] };
}
export function parseDecision(value: unknown): StoredDecision {
  const wrapper = record(value), v = record(wrapper.trace);
  requireValue(text(wrapper.trace_id) && instant(wrapper.recorded_at));
  provenance(v);
  requireValue(['schema_version', 'observation_id', 'session_id', 'experiment_id', 'configuration_digest', 'calculation_version', 'strategy_id'].every(key => text(v[key])));
  requireValue(v.schema_version === '1.0.0' || v.schema_version === '1.1.0');
  requireValue(v.schema_version === '1.0.0' ? !Object.hasOwn(v, 'chain_freshness') && v.calculation_version !== 'capture-pair-research-v2;bounds8x63;group1000;finalized-chain-time-v1' : Object.hasOwn(v, 'chain_freshness') && v.calculation_version === 'capture-pair-research-v2;bounds8x63;group1000;finalized-chain-time-v1', 'Decision schema and chain freshness calculation version disagree.');
  requireValue(unsigned(v.generation) && choice(v.network_id, networks) && choice(v.mode, ['OBSERVE', 'PAPER', 'REPLAY']) && ms(v.observed_at_unix_ms));
  requireValue(v.input_age_ms === null || ms(v.input_age_ms));
  requireValue(v.amount_in_minor === null || unsigned(v.amount_in_minor));
  requireValue(Array.isArray(v.capture_refs) && v.capture_refs.length <= 64 && Array.isArray(v.route) && v.route.length <= 2);
  const refs = v.capture_refs.map(value => { const r = record(value); requireValue(text(r.capture_id) && text(r.manifest_digest) && text(r.snapshot_id)); return { capture_id: r.capture_id, manifest_digest: r.manifest_digest, snapshot_id: r.snapshot_id }; });
  const route = v.route.map(value => { const r = record(value); requireValue(text(r.pool_id) && text(r.asset_in) && text(r.asset_out) && text(r.venue_family)); return { pool_id: r.pool_id, asset_in: r.asset_in, asset_out: r.asset_out, venue_family: r.venue_family }; });
  const r = record(v.result);
  let result: DecisionTrace['result'];
  if (r.status === 'QUOTED') {
    requireValue(unsigned(r.quoted_output_minor) && BigInt(r.quoted_output_minor) > 0n && signed(r.gross_delta_minor) && unsigned(v.amount_in_minor));
    requireValue(Array.isArray(r.included_pool_fees) && r.included_pool_fees.length === 2 && r.included_pool_fees.every(unsigned));
    requireValue(BigInt(r.quoted_output_minor) - BigInt(v.amount_in_minor) === BigInt(r.gross_delta_minor), 'Gross quote arithmetic is inconsistent.');
    result = { status: 'QUOTED', quoted_output_minor: r.quoted_output_minor, gross_delta_minor: r.gross_delta_minor, included_pool_fees: stringList(r.included_pool_fees) };
  } else {
    requireValue(choice(r.status, ['REJECTED', 'NO_ROUTE', 'DATA_UNAVAILABLE']));
    result = { status: r.status as 'REJECTED' | 'NO_ROUTE' | 'DATA_UNAVAILABLE', reason_codes: publicCodes(r.reason_codes) };
  }
  const g = record(v.grouping); requireValue(text(g.version) && text(g.key) && ms(g.window_ms) && ms(g.window_start_ms));
  const trace: DecisionTrace = { schema_version: v.schema_version as string, observation_id: v.observation_id as string,
    session_id: v.session_id as string, experiment_id: v.experiment_id as string, generation: v.generation,
    configuration_digest: v.configuration_digest as string, calculation_version: v.calculation_version as string, strategy_id: v.strategy_id as string,
    network_id: v.network_id as Network, mode: v.mode as DecisionTrace['mode'], source_kind: v.source_kind as SourceKind,
    dataset_origin: v.dataset_origin as Origin, observed_at_unix_ms: v.observed_at_unix_ms, input_age_ms: v.input_age_ms as number | null,
    capture_refs: refs, route, amount_in_minor: v.amount_in_minor as string | null, result,
    grouping: { version: g.version, key: g.key, window_ms: g.window_ms, window_start_ms: g.window_start_ms }, diagnostics: publicCodes(v.diagnostics, true) };
  // An absent legacy field stays absent: adding null changes sealed decision and export digests.
  if (Object.hasOwn(v, 'chain_freshness')) trace.chain_freshness = parseChainFreshness(v.chain_freshness, trace);
  return { trace_id: wrapper.trace_id, recorded_at: wrapper.recorded_at, trace };
}
export function parseGroup(value: unknown): DecisionGroup {
  const v = record(value); provenance(v);
  requireValue(text(v.grouping_version) && text(v.grouping_key) && ms(v.window_start_ms));
  for (const key of ['raw_observations', 'quoted_candidates', 'rejected', 'no_route', 'data_unavailable']) requireValue(unsigned(v[key]));
  return { grouping_version: v.grouping_version, grouping_key: v.grouping_key, window_start_ms: v.window_start_ms,
    source_kind: v.source_kind as SourceKind, dataset_origin: v.dataset_origin as Origin, raw_observations: v.raw_observations as string,
    quoted_candidates: v.quoted_candidates as string, rejected: v.rejected as string, no_route: v.no_route as string, data_unavailable: v.data_unavailable as string };
}
export function parseCoverage(value: unknown): Coverage {
  const v = record(value); requireValue(text(v.session_id) && v.collection_completeness === 'UNKNOWN' && typeof v.execution_accounting_available === 'boolean');
  for (const key of ['raw_observations', 'quoted_candidates', 'rejected', 'no_route', 'data_unavailable', 'unique_opportunity_groups']) requireValue(unsigned(v[key]));
  for (const key of ['eligible_attempts', 'reconciled_transactions']) requireValue(v[key] === null || unsigned(v[key]));
  requireValue(v.execution_accounting_available || (v.eligible_attempts === null && v.reconciled_transactions === null), 'Unavailable execution accounting must not be represented as zero.');
  requireValue((v.coverage_window_start_ms === null || ms(v.coverage_window_start_ms)) && (v.coverage_window_end_ms === null || ms(v.coverage_window_end_ms)));
  return { session_id: v.session_id, raw_observations: v.raw_observations as string, quoted_candidates: v.quoted_candidates as string,
    rejected: v.rejected as string, no_route: v.no_route as string, data_unavailable: v.data_unavailable as string,
    unique_opportunity_groups: v.unique_opportunity_groups as string, eligible_attempts: v.eligible_attempts as string | null,
    reconciled_transactions: v.reconciled_transactions as string | null, execution_accounting_available: v.execution_accounting_available,
    collection_completeness: 'UNKNOWN', coverage_window_start_ms: v.coverage_window_start_ms as number | null, coverage_window_end_ms: v.coverage_window_end_ms as number | null };
}
