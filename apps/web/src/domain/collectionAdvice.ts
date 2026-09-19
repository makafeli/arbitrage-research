// Pure, framework-free advice derivation for the Collection attempt health table (issue #179).
// No React here so `node --test` can exercise it directly.
//
// captured_pools discriminates two ACQUISITION_FAILED/ACQUISITION_UNAVAILABLE cases that the
// worker (apps/research-worker/src/main.rs) otherwise reports identically:
//   - captured_pools > 0: the capture succeeded but the source had not yet reached this
//     capture's anchor block (main.rs:970 `admit = source_ready && batch.source_caught_up`,
//     main.rs:1049-1052 the `!batch.source_caught_up` branch finishing as ACQUISITION_FAILED /
//     ACQUISITION_UNAVAILABLE). This is catch-up, not a provider problem; nothing should be
//     retried, the owner only waits.
//   - captured_pools === 0: acquisition failed before any capture (main.rs:434, 444), a genuine
//     provider/RPC problem.
import type { CollectionAttempt, CollectionReason } from '../api/collection.ts';

const guidance: Record<CollectionReason, string> = {
  PROVIDER_UNAVAILABLE: 'Check the configured provider availability and credentials in the service environment, then inspect the next recorded attempt.',
  INPUT_VALIDATION_FAILED: 'Review the pool registry and the frozen configuration for unsupported or inconsistent inputs.',
  CAPTURE_STORAGE_UNAVAILABLE: 'Check the capture store and database availability before starting another research attempt.',
  RESOURCE_LIMIT: 'Review the configured pool count and capture bounds. Reduce the workload before retrying.',
  ACQUISITION_UNAVAILABLE: 'Inspect provider availability and the configured pool inputs before retrying collection.',
  ACQUISITION_DEADLINE: 'Check provider latency and request limits. This attempt exceeded its acquisition budget.',
  EVALUATION_REJECTED: 'Review supported route inputs and the frozen strategy configuration.',
  EVALUATION_DEADLINE: 'Review evaluation workload and resource pressure. This attempt exceeded its evaluation budget.',
  GENERATION_FENCED: 'Compare this generation with the current session command. A stopped or superseded generation cannot admit decisions.',
  WORKER_SHUTDOWN: 'Check the intended worker lifecycle and restart status. Shutdown does not establish a completed research batch.',
  TASK_FAILED: 'Inspect the worker health and restricted service diagnostics, then verify a later attempt completes.',
};

const CATCH_UP_ADVICE = 'Catching up: the capture succeeded, but the source is still behind its anchor block, so this attempt could not admit decisions. The provider is not the problem; do not retry. Wait for the source to catch up — the owner overview shows the catch-up state; the exact block lag is in the worker log (`capture-written`, `source_lag_blocks`).';
const CAPTURE_VOLUME_FULL_ADVICE = 'Capture volume full: the capture store reached its byte quota (or one capture exceeded its byte/request quota). The retention prune frees space automatically on the next capture; if this repeats, review the capture quota and pool count with the operator. Do not retry manually.';

function isCatchUp(attempt: CollectionAttempt): boolean {
  return attempt.outcome === 'ACQUISITION_FAILED' && attempt.reason === 'ACQUISITION_UNAVAILABLE' && attempt.captured_pools > 0;
}

export function advice(attempt: CollectionAttempt): string {
  if (attempt.outcome === 'IN_PROGRESS') return 'No terminal outcome recorded. The batch may still be running or may have been interrupted. Compare its start time with the worker heartbeat; do not count it as success or failure.';
  if (isCatchUp(attempt)) return CATCH_UP_ADVICE;
  if (attempt.outcome === 'ACQUISITION_FAILED' && attempt.reason === 'RESOURCE_LIMIT') return CAPTURE_VOLUME_FULL_ADVICE;
  if (attempt.reason) return guidance[attempt.reason];
  if (attempt.outcome === 'READINESS_COMPLETED') return 'Readiness capture completed. This batch does not establish research decisions or continuous market coverage.';
  return 'Decisions were durably admitted for this batch. Their quote, rejection and data quality evidence remains separate from executable results.';
}

export function attemptTone(attempt: CollectionAttempt): 'amber' | 'catching-up' | '' {
  if (isCatchUp(attempt)) return 'catching-up';
  if (['IN_PROGRESS', 'ACQUISITION_FAILED', 'EVALUATION_FAILED', 'DEADLINE_EXCEEDED'].includes(attempt.outcome)) return 'amber';
  return '';
}
