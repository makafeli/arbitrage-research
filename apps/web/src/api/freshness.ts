import type { DecisionTrace } from './research.ts';

export type FreshnessStatus = 'UNKNOWN' | 'FUTURE' | 'STALE' | 'WITHIN_POLICY';
export type ChainTimeSource =
  | { kind: 'BASE_FINALIZED_BLOCK_TIMESTAMP'; block_number: string; block_hash: string; parent_hash: string }
  | { kind: 'SOLANA_ESTIMATED_BLOCK_TIME'; slot: string; genesis_hash: string; account_context: string };
export interface ChainFreshness {
  policy: { version: 'finalized-chain-time-v1'; max_chain_age_ms: number };
  reference_observed_at_unix_ms: number; evaluation_elapsed_ms: number;
  sources: { capture_id: string; source: ChainTimeSource; chain_time_seconds: number | null; age_ms: number | null; status: FreshnessStatus }[];
  status: FreshnessStatus;
}
const MAX_MS = 253402300799999;
function valid(value: unknown, message = 'Chain freshness does not match its retained source and policy.'): asserts value { if (!value) throw new Error(message); }
function object(value: unknown): Record<string, unknown> { valid(value && typeof value === 'object' && !Array.isArray(value)); return value as Record<string, unknown>; }
function keys(v: Record<string, unknown>, expected: string[]) { valid(Object.keys(v).length === expected.length && expected.every(key => Object.hasOwn(v, key)), 'Unsupported chain freshness fields.'); }
function integer(v: unknown, max = MAX_MS): v is number { return typeof v === 'number' && Number.isSafeInteger(v) && v >= 0 && v <= max; }
function u64(v: unknown): v is string { return typeof v === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(v) && BigInt(v) <= 18446744073709551615n; }
function hash(v: unknown): v is string { return typeof v === 'string' && /^0x[0-9a-f]{64}$/.test(v); }
export function base58Address(v: unknown): v is string {
  if (typeof v !== 'string' || v.length < 32 || v.length > 44) return false;
  const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let n = 0n;
  for (const c of v) { const digit = alphabet.indexOf(c); if (digit < 0) return false; n = n * 58n + BigInt(digit); }
  let bytes = 0; for (let left = n; left > 0n; left >>= 8n) bytes++;
  return bytes + (v.match(/^1*/)?.[0].length ?? 0) === 32;
}
function label(value: unknown): value is string { return typeof value === 'string' && value.length > 0 && new TextEncoder().encode(value).length <= 256 && !/[\u0000-\u001f\u007f-\u009f]/u.test(value); }
function source(value: unknown, network: DecisionTrace['network_id']): ChainTimeSource {
  const v = object(value);
  if (v.kind === 'BASE_FINALIZED_BLOCK_TIMESTAMP') {
    keys(v, ['kind', 'block_number', 'block_hash', 'parent_hash']);
    valid(network === 'base-mainnet' && u64(v.block_number) && hash(v.block_hash) && hash(v.parent_hash), 'Chain freshness source network or block context is invalid.');
    return { kind: v.kind, block_number: v.block_number, block_hash: v.block_hash, parent_hash: v.parent_hash };
  }
  keys(v, ['kind', 'slot', 'genesis_hash', 'account_context']);
  valid(v.kind === 'SOLANA_ESTIMATED_BLOCK_TIME' && network === 'solana-mainnet' && u64(v.slot) && base58Address(v.genesis_hash)
    && v.genesis_hash !== '1'.repeat(32) && label(v.account_context), 'Chain freshness source network or account context is invalid.');
  return { kind: v.kind, slot: v.slot, genesis_hash: v.genesis_hash, account_context: v.account_context };
}
export function parseChainFreshness(value: unknown, trace: DecisionTrace): ChainFreshness {
  const v = object(value), p = object(v.policy);
  keys(v, ['policy', 'reference_observed_at_unix_ms', 'evaluation_elapsed_ms', 'sources', 'status']); keys(p, ['version', 'max_chain_age_ms']);
  valid(p.version === 'finalized-chain-time-v1' && integer(p.max_chain_age_ms, 86400000) && p.max_chain_age_ms > 0, 'Unsupported chain freshness policy.');
  valid(integer(v.reference_observed_at_unix_ms) && v.reference_observed_at_unix_ms > 0 && v.reference_observed_at_unix_ms === trace.observed_at_unix_ms
    && integer(v.evaluation_elapsed_ms, MAX_MS - 1) && v.evaluation_elapsed_ms === trace.input_age_ms, 'Chain freshness reference time or evaluation elapsed does not match the decision.');
  const effective = v.reference_observed_at_unix_ms + v.evaluation_elapsed_ms;
  valid(Number.isSafeInteger(effective) && effective <= MAX_MS);
  valid(Array.isArray(v.sources) && v.sources.length <= 8 && v.sources.length === trace.capture_refs.length, 'Chain freshness capture scope does not match the decision.');
  const sources = v.sources.map((value, i) => {
    const s = object(value); keys(s, ['capture_id', 'source', 'chain_time_seconds', 'age_ms', 'status']);
    valid(label(s.capture_id) && s.capture_id === trace.capture_refs[i].capture_id, 'Chain freshness capture scope does not match the decision.');
    valid(s.chain_time_seconds === null || integer(s.chain_time_seconds, 253402300799), 'Chain timestamp is outside the supported range.');
    const timestamp = s.chain_time_seconds === null ? null : s.chain_time_seconds * 1000;
    const age = timestamp === null || timestamp > effective ? null : effective - timestamp;
    const status: FreshnessStatus = timestamp === null ? 'UNKNOWN' : timestamp > effective ? 'FUTURE' : age! > (p.max_chain_age_ms as number) ? 'STALE' : 'WITHIN_POLICY';
    valid(s.age_ms === age && s.status === status, 'Chain freshness age arithmetic or source status is inconsistent.');
    return { capture_id: s.capture_id, source: source(s.source, trace.network_id), chain_time_seconds: s.chain_time_seconds, age_ms: age, status };
  });
  valid(new Set(sources.map(s => s.capture_id)).size === sources.length, 'Chain freshness repeats a capture source.');
  const status: FreshnessStatus = sources.some(s => s.status === 'FUTURE') ? 'FUTURE' : !sources.length || sources.some(s => s.status === 'UNKNOWN') ? 'UNKNOWN' : sources.some(s => s.status === 'STALE') ? 'STALE' : 'WITHIN_POLICY';
  valid(v.status === status && (trace.result.status !== 'QUOTED' || status === 'WITHIN_POLICY'), 'Chain freshness aggregate or quoted result is inconsistent.');
  const reason = status === 'UNKNOWN' ? 'CHAIN_TIME_UNAVAILABLE' : status === 'FUTURE' ? 'CHAIN_TIME_FUTURE' : status === 'STALE' ? 'CHAIN_TIME_STALE' : null;
  valid(reason === null || (trace.result.status !== 'QUOTED' && trace.result.reason_codes.includes(reason)), 'Chain freshness failure is absent from the decision reasons.');
  return { policy: { version: 'finalized-chain-time-v1', max_chain_age_ms: p.max_chain_age_ms }, reference_observed_at_unix_ms: v.reference_observed_at_unix_ms,
    evaluation_elapsed_ms: v.evaluation_elapsed_ms, sources, status };
}
