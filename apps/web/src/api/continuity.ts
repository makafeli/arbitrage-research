import type { StoredDecision } from './research.ts';

export type ContinuityStatus = 'NO_KNOWN_INVALIDATION' | 'INVALIDATED' | 'UNTRACKED' | 'UNVERIFIABLE';
export type InvalidationReason = 'CONTINUITY_LOST' | 'PROVIDER_FAILURE' | 'RESOURCE_LIMIT' | 'INVALID_INPUT';
export interface DecisionContinuity {
  schema_version: '1.0.0'; assessment_kind: 'CONTINUITY_ONLY'; authorizes_execution: false;
  trace_id: string; session_id: string; observation_id: string; network_id: 'base-mainnet' | 'solana-mainnet';
  checked_at: string; policy_version: 'base-capture-continuity-v1'; continuity_status: ContinuityStatus;
  capture_count: string; bound_count: string; invalidation_reasons: InvalidationReason[];
}
const fields = ['schema_version', 'assessment_kind', 'authorizes_execution', 'trace_id', 'session_id', 'observation_id', 'network_id', 'checked_at', 'policy_version', 'continuity_status', 'capture_count', 'bound_count', 'invalidation_reasons'];
const statuses = ['NO_KNOWN_INVALIDATION', 'INVALIDATED', 'UNTRACKED', 'UNVERIFIABLE'];
const reasons = ['CONTINUITY_LOST', 'PROVIDER_FAILURE', 'RESOURCE_LIMIT', 'INVALID_INPUT'];
function check(value: unknown): asserts value {
  if (!value) throw new Error('Source continuity response does not match the expected contract.');
}
function identifier(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 128 && /^[A-Za-z0-9_.:-]+$/.test(value);
}
function count(value: unknown): value is string {
  return typeof value === 'string' && /^(0|[1-9][0-9]?)$/.test(value) && BigInt(value) <= 64n;
}
export function parseDecisionContinuity(value: unknown): DecisionContinuity {
  check(value && typeof value === 'object' && !Array.isArray(value));
  const v = value as Record<string, unknown>;
  check(Object.keys(v).length === fields.length && Object.keys(v).every(key => fields.includes(key)));
  check(v.schema_version === '1.0.0' && v.assessment_kind === 'CONTINUITY_ONLY' && v.authorizes_execution === false);
  check(identifier(v.trace_id) && identifier(v.session_id) && identifier(v.observation_id));
  check(v.network_id === 'base-mainnet' || v.network_id === 'solana-mainnet');
  check(typeof v.checked_at === 'string' && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?Z$/.test(v.checked_at) && Number.isFinite(Date.parse(v.checked_at)));
  check(v.policy_version === 'base-capture-continuity-v1' && statuses.includes(v.continuity_status as string));
  check(count(v.capture_count) && count(v.bound_count) && BigInt(v.bound_count) <= BigInt(v.capture_count));
  check(Array.isArray(v.invalidation_reasons) && v.invalidation_reasons.length <= 4);
  check(v.invalidation_reasons.every((reason, index, list) => typeof reason === 'string' && reasons.includes(reason) && (index === 0 || String(list[index - 1]) < reason)));
  const total = BigInt(v.capture_count), bound = BigInt(v.bound_count), invalid = v.invalidation_reasons.length;
  if (v.continuity_status === 'NO_KNOWN_INVALIDATION') check(v.network_id === 'base-mainnet' && total > 0n && bound === total && invalid === 0);
  if (v.continuity_status === 'INVALIDATED') check(v.network_id === 'base-mainnet' && bound > 0n && invalid > 0);
  if (v.continuity_status === 'UNTRACKED') check(v.network_id === 'base-mainnet' && (total === 0n || bound < total) && invalid === 0);
  return v as unknown as DecisionContinuity;
}
export function continuityForDecision(value: DecisionContinuity, record: StoredDecision): DecisionContinuity {
  check(value.trace_id === record.trace_id && value.session_id === record.trace.session_id && value.observation_id === record.trace.observation_id && value.network_id === record.trace.network_id);
  check(BigInt(value.capture_count) === BigInt(record.trace.capture_refs.length));
  return value;
}
export const continuityLabels: Record<ContinuityStatus, string> = {
  NO_KNOWN_INVALIDATION: 'No known source invalidation',
  INVALIDATED: 'Source invalidated',
  UNTRACKED: 'Source continuity not tracked',
  UNVERIFIABLE: 'Source continuity unverifiable',
};
export const invalidationLabels: Record<InvalidationReason, string> = {
  CONTINUITY_LOST: 'The recorded stream lost continuity.',
  PROVIDER_FAILURE: 'The source stream halted after a provider failure.',
  RESOURCE_LIMIT: 'The source stream halted at a resource limit.',
  INVALID_INPUT: 'The source stream halted after invalid input.',
};
