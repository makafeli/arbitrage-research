import type { Network } from './client.ts';
import { record, requireValue, text, unsigned } from './research.ts';

export const collectionOutcomes = ['IN_PROGRESS', 'READINESS_COMPLETED', 'DECISIONS_RECORDED', 'ACQUISITION_FAILED', 'EVALUATION_FAILED', 'DEADLINE_EXCEEDED', 'SUPPRESSED', 'WORKER_CANCELLED'] as const;
export const collectionReasons = ['PROVIDER_UNAVAILABLE', 'INPUT_VALIDATION_FAILED', 'CAPTURE_STORAGE_UNAVAILABLE', 'RESOURCE_LIMIT', 'ACQUISITION_UNAVAILABLE', 'ACQUISITION_DEADLINE', 'EVALUATION_REJECTED', 'EVALUATION_DEADLINE', 'GENERATION_FENCED', 'WORKER_SHUTDOWN', 'TASK_FAILED'] as const;
export type CollectionOutcome = typeof collectionOutcomes[number];
export type CollectionReason = typeof collectionReasons[number];
export interface CollectionAttempt {
  attempt_id: string; session_id: string; network_id: Network; configuration_digest: string; experiment_id: string;
  generation: string; worker_epoch: string; purpose: 'READINESS' | 'RESEARCH'; outcome: CollectionOutcome;
  reason: CollectionReason | null; captured_pools: number; decision_rows: string; decision_observation_ids: string[]; elapsed_ms: number | null;
  started_at: string; finished_at: string | null;
}
export const collectionCountKeys = ['attempts_started', 'readiness_attempts', 'research_attempts', 'in_progress', 'readiness_completed', 'decisions_recorded', 'acquisition_failed', 'evaluation_failed', 'deadline_exceeded', 'suppressed', 'worker_cancelled', 'decision_rows_recorded'] as const;
export type CollectionCoverage = Record<typeof collectionCountKeys[number], string> & {
  session_id: string; denominator: 'RECORDED_COLLECTION_ATTEMPTS'; collection_completeness: 'UNKNOWN'; window_start_at: string | null; window_end_at: string | null;
};
function date(value: unknown): value is string { return text(value) && Number.isFinite(Date.parse(value)); }
function count(value: unknown): value is number { return Number.isSafeInteger(value) && (value as number) >= 0; }
export function parseCollectionAttempt(value: unknown): CollectionAttempt {
  const v = record(value);
  requireValue(['attempt_id', 'session_id', 'configuration_digest', 'experiment_id'].every(key => text(v[key])));
  requireValue(v.network_id === 'base-mainnet' || v.network_id === 'solana-mainnet');
  requireValue(unsigned(v.generation) && unsigned(v.worker_epoch) && unsigned(v.decision_rows));
  requireValue(v.purpose === 'READINESS' || v.purpose === 'RESEARCH');
  requireValue(collectionOutcomes.includes(v.outcome as CollectionOutcome));
  requireValue(v.reason === null || collectionReasons.includes(v.reason as CollectionReason), 'Only named public collection reason codes are accepted.');
  requireValue(count(v.captured_pools) && v.captured_pools <= 8 && (v.elapsed_ms === null || (count(v.elapsed_ms) && v.elapsed_ms <= 86_400_000)));
  requireValue(date(v.started_at) && (v.finished_at === null || date(v.finished_at)));
  requireValue(v.outcome === 'IN_PROGRESS' ? v.finished_at === null && v.reason === null : v.finished_at !== null, 'Collection attempt terminal state and timestamp disagree.');
  requireValue(Array.isArray(v.decision_observation_ids) && v.decision_observation_ids.length <= 64 && v.decision_observation_ids.every(id => typeof id === 'string' && /^sha256:[a-f0-9]{64}$/.test(id)) && new Set(v.decision_observation_ids).size === v.decision_observation_ids.length);
  requireValue(String(v.decision_observation_ids.length) === v.decision_rows, 'Collection decision references do not match the recorded row count.');
  requireValue(v.outcome === 'IN_PROGRESS' ? v.elapsed_ms === null && v.captured_pools === 0 && v.decision_rows === '0' : v.elapsed_ms !== null);
  requireValue(v.outcome === 'DECISIONS_RECORDED' ? v.purpose === 'RESEARCH' && BigInt(v.decision_rows) > 0n && BigInt(v.decision_rows) <= 64n : v.decision_rows === '0');
  requireValue(v.outcome !== 'READINESS_COMPLETED' || v.purpose === 'READINESS');
  requireValue(!['EVALUATION_FAILED'].includes(v.outcome as string) || v.purpose === 'RESEARCH');
  requireValue(v.reason !== 'EVALUATION_DEADLINE' || v.purpose === 'RESEARCH');
  const reasonsByOutcome: Record<CollectionOutcome, readonly CollectionReason[]> = {
    IN_PROGRESS: [], READINESS_COMPLETED: [], DECISIONS_RECORDED: [],
    ACQUISITION_FAILED: ['PROVIDER_UNAVAILABLE', 'INPUT_VALIDATION_FAILED', 'CAPTURE_STORAGE_UNAVAILABLE', 'RESOURCE_LIMIT', 'ACQUISITION_UNAVAILABLE', 'TASK_FAILED'],
    EVALUATION_FAILED: ['EVALUATION_REJECTED', 'RESOURCE_LIMIT', 'TASK_FAILED'], DEADLINE_EXCEEDED: ['ACQUISITION_DEADLINE', 'EVALUATION_DEADLINE'],
    SUPPRESSED: ['GENERATION_FENCED'], WORKER_CANCELLED: ['WORKER_SHUTDOWN'],
  };
  const allowedReasons = reasonsByOutcome[v.outcome as CollectionOutcome];
  requireValue(allowedReasons.length ? v.reason !== null && allowedReasons.includes(v.reason as CollectionReason) : v.reason === null, 'Collection outcome and reason code disagree.');
  return { attempt_id: v.attempt_id as string, session_id: v.session_id as string, network_id: v.network_id,
    configuration_digest: v.configuration_digest as string, experiment_id: v.experiment_id as string,
    generation: v.generation, worker_epoch: v.worker_epoch, purpose: v.purpose, outcome: v.outcome as CollectionOutcome,
    reason: v.reason as CollectionReason | null, captured_pools: v.captured_pools, decision_rows: v.decision_rows, decision_observation_ids: v.decision_observation_ids as string[],
    elapsed_ms: v.elapsed_ms as number | null, started_at: v.started_at, finished_at: v.finished_at as string | null };
}
export function parseCollectionCoverage(value: unknown): CollectionCoverage {
  const v = record(value);
  requireValue(text(v.session_id) && v.denominator === 'RECORDED_COLLECTION_ATTEMPTS' && v.collection_completeness === 'UNKNOWN');
  const counts = {} as Record<typeof collectionCountKeys[number], string>;
  for (const key of collectionCountKeys) { requireValue(unsigned(v[key])); counts[key] = v[key]; }
  requireValue((v.window_start_at === null || date(v.window_start_at)) && (v.window_end_at === null || date(v.window_end_at)));
  const total = BigInt(counts.attempts_started);
  requireValue(BigInt(counts.readiness_attempts) + BigInt(counts.research_attempts) === total, 'Collection purpose counts do not match recorded attempts.');
  requireValue(['in_progress', 'readiness_completed', 'decisions_recorded', 'acquisition_failed', 'evaluation_failed', 'deadline_exceeded', 'suppressed', 'worker_cancelled'].reduce((n, key) => n + BigInt(counts[key as keyof typeof counts]), 0n) === total, 'Collection outcome counts do not match recorded attempts.');
  return { ...counts, session_id: v.session_id, denominator: 'RECORDED_COLLECTION_ATTEMPTS', collection_completeness: 'UNKNOWN', window_start_at: v.window_start_at as string | null, window_end_at: v.window_end_at as string | null };
}
