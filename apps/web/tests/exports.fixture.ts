import { createHash } from 'node:crypto';
import type { CollectionAttempt, CollectionCoverage } from '../src/api/collection.ts';
import type { FrozenExport } from '../src/api/frozenExport.ts';
import { canonicalExportContent } from '../src/api/frozenExport.ts';
import { parseDecision, parsePaperRun } from '../src/api/research.ts';
import { decision, huge, journal, paperRun } from './research.fixture.ts';

// Synthetic contract fixtures only; no provider or market observations.
export const exportHash = 'sha256:' + 'a'.repeat(64);
export function collectionAttempt(): CollectionAttempt {
  return { attempt_id: 'collection-attempt-fixture', session_id: 'session-paper', network_id: 'base-mainnet',
    configuration_digest: 'sha256:paper-fixture-config', experiment_id: 'experiment-fixture', generation: '9007199254740993', worker_epoch: '2',
    purpose: 'RESEARCH', outcome: 'ACQUISITION_FAILED', reason: 'PROVIDER_UNAVAILABLE', captured_pools: 0,
    decision_rows: '0', decision_observation_ids: [], elapsed_ms: 25, started_at: '2026-09-12T12:00:00Z', finished_at: '2026-09-12T12:00:00.025Z' };
}
export function collectionCoverage(): CollectionCoverage {
  return { session_id: 'session-paper', denominator: 'RECORDED_COLLECTION_ATTEMPTS', collection_completeness: 'UNKNOWN',
    attempts_started: '3', readiness_attempts: '0', research_attempts: '3', in_progress: '1', readiness_completed: '0', decisions_recorded: '0',
    acquisition_failed: '1', evaluation_failed: '0', deadline_exceeded: '0', suppressed: '1', worker_cancelled: '0', decision_rows_recorded: '0',
    window_start_at: '2026-09-12T12:00:00Z', window_end_at: '2026-09-12T12:00:00.025Z' };
}
export function sealExport(bundle: FrozenExport): FrozenExport {
  return { ...bundle, content_sha256: 'sha256:' + createHash('sha256').update(canonicalExportContent(bundle)).digest('hex') };
}
export function frozenExportFixture(): FrozenExport {
  const trace = parseDecision(decision());
  const run = parsePaperRun({ ...paperRun(), revision: '0', outstanding_reservations: 0,
    balances: paperRun().initial_balances.map(b => ({ asset: b.asset, free: b.amount, reserved: '0', total: b.amount })) });
  const event = journal();
  return sealExport({ schema_version: '1.1.0', export_id: 'export-fixture-one', exported_at: '2026-09-12T12:01:00Z', content_sha256: exportHash,
    snapshot: { isolation: 'REPEATABLE_READ', scope: 'COMPLETE_STORED_SESSION', collection_completeness: 'UNKNOWN',
      source_counts: { decisions: '1', paper_runs: '1', paper_journal_events: '1', capture_catalog_entries: '1', collection_attempts: '3', cost_assessments: '0' } },
    methodology: { amounts: 'BASE_UNIT_INTEGER_STRINGS', asset_decimals: 'NOT_RETAINED_IN_DATABASE', costs: 'QUOTED_COSTS_UNKNOWN_MANUAL_ASSESSMENTS_SEPARATE',
      configuration_snapshot: 'DIGEST_ONLY', raw_artifacts: 'REFERENCED_NOT_INCLUDED_OR_VERIFIED',
      hash_format: 'SHA256_SORTED_KEY_COMPACT_JSON_SNAPSHOT_METHODOLOGY_DATA_V1', journal_projection: 'REDACTED_REPLAYABLE_ACCOUNTING_PROJECTION',
      execution_authorized: false, max_source_rows: '10000', max_bytes: '8388608' },
    data: { session: { session_id: 'session-paper', network_id: 'base-mainnet', mode: 'PAPER', observed_state: 'STOPPED', health: 'UNKNOWN',
      desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'sha256:paper-fixture-config' },
      experiment_id: 'experiment-fixture', strategy_ids: ['usdc-cycle'], decision_coverage: { session_id: 'session-paper', raw_observations: '1', quoted_candidates: '1', rejected: '0', no_route: '0', data_unavailable: '0', unique_opportunity_groups: '1', eligible_attempts: null, reconciled_transactions: null, execution_accounting_available: false, collection_completeness: 'UNKNOWN', coverage_window_start_ms: 1789214400000, coverage_window_end_ms: 1789214400000 },
      cost_assessments: [], decisions: [trace], paper_runs: [{ run, journal: [{ event_id: event.event_id, recorded_at: event.recorded_at, source_payload_sha256: exportHash,
        event: { run_id: 'run-original', network: 'base-mainnet', sequence: '0', command_id: exportHash, command: { kind: 'INITIALIZE', balances: run.initial_balances },
          postings: [{ asset: run.initial_balances[0].asset, account: 'AVAILABLE', side: 'DEBIT', amount: huge }, { asset: run.initial_balances[0].asset, account: 'INITIAL_CAPITAL', side: 'CREDIT', amount: huge },
            { asset: run.initial_balances[1].asset, account: 'AVAILABLE', side: 'DEBIT', amount: '100' }, { asset: run.initial_balances[1].asset, account: 'INITIAL_CAPITAL', side: 'CREDIT', amount: '100' }] } }], reservations: [] }],
      capture_dependencies: trace.trace.capture_refs.map((ref, index) => ({ capture_id: ref.capture_id, manifest_digest: ref.manifest_digest, snapshot_ids: [ref.snapshot_id],
        catalog_status: index ? 'MISSING' : 'PRESENT', raw_artifact_status: 'NOT_VERIFIED', expiration_status: 'UNKNOWN', generation: index ? null : trace.trace.generation, admitted_at: index ? null : '2026-09-12T12:00:00Z' })),
      collection_attempts: [collectionAttempt(), { ...collectionAttempt(), attempt_id: 'attempt-fenced', outcome: 'SUPPRESSED', reason: 'GENERATION_FENCED' },
        { ...collectionAttempt(), attempt_id: 'attempt-unfinished', outcome: 'IN_PROGRESS', reason: null, elapsed_ms: null, finished_at: null }],
    } });
}
