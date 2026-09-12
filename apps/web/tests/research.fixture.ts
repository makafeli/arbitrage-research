import type { PaperRunRecord } from '../src/api/research.ts';

// Contract fixtures only. These records are not captured market evidence.
export const principalAsset = { kind: 'TOKEN' as const, identity: 'base-mainnet:0x0000000000000000000000000000000000000001' };
export const nativeAsset = { kind: 'NATIVE' as const, identity: 'base-mainnet' };
export const huge = '900719925474099312345678901234567890';
export function paperRun(): PaperRunRecord {
  return { run_id: 'run-original', session_id: 'session-paper', network_id: 'base-mainnet', mode: 'PAPER',
    configuration_digest: 'sha256:paper-fixture-config', created_at: '2026-09-12T12:00:00Z', revision: '9007199254740993',
    initial_balances: [{ asset: principalAsset, amount: huge }, { asset: nativeAsset, amount: '100' }],
    balances: [{ asset: principalAsset, free: (BigInt(huge) - 9n).toString(), reserved: '9', total: huge }, { asset: nativeAsset, free: '80', reserved: '20', total: '100' }],
    outstanding_reservations: 1, evidence_label: 'HYPOTHETICAL', execution_authorized: false };
}
export function decision(origin: 'SYNTHETIC' | 'MANUALLY_CONSTRUCTED' | 'RECORDED_LIVE' = 'SYNTHETIC', observation = 'observation-fixture') {
  return { trace_id: 'trace-' + observation, recorded_at: '2026-09-12T12:00:01Z', trace: {
    schema_version: '1.0.0', observation_id: observation, session_id: 'session-paper', experiment_id: 'experiment-fixture',
    generation: '9007199254740993', configuration_digest: 'sha256:paper-fixture-config', calculation_version: 'constant-product-v1',
    strategy_id: 'usdc-cycle', network_id: 'base-mainnet', mode: 'PAPER',
    source_kind: origin === 'RECORDED_LIVE' ? 'CAPTURED_MARKET_DATA' : 'SYNTHETIC_FIXTURE', dataset_origin: origin,
    observed_at_unix_ms: 1789214400000, input_age_ms: 15 as number | null,
    capture_refs: [{ capture_id: 'capture-fixture', manifest_digest: 'sha256:manifest-fixture', snapshot_id: 'sha256:manifest-fixture' }, { capture_id: 'capture-fixture-second', manifest_digest: 'sha256:manifest-fixture-second', snapshot_id: 'sha256:manifest-fixture-second' }],
    route: [{ pool_id: 'base-mainnet:pool-fixture-a', asset_in: principalAsset.identity, asset_out: 'base-mainnet:token-b', venue_family: 'uniswap-v3' }, { pool_id: 'base-mainnet:pool-fixture-b', asset_in: 'base-mainnet:token-b', asset_out: principalAsset.identity, venue_family: 'uniswap-v3' }],
    amount_in_minor: '1000', result: { status: 'QUOTED', quoted_output_minor: '990', gross_delta_minor: '-10', included_pool_fees: ['3', '2'] },
    grouping: { version: 'group-v1', key: 'group-' + origin, window_ms: 1000, window_start_ms: 1789214400000 },
    diagnostics: ['EXTERNAL_COSTS_UNAVAILABLE'],
  } };
}
export function coverage() {
  return { session_id: 'session-paper', raw_observations: '3', quoted_candidates: '1', rejected: '1', no_route: '0', data_unavailable: '1',
    unique_opportunity_groups: '1', eligible_attempts: null, reconciled_transactions: null, execution_accounting_available: false,
    collection_completeness: 'UNKNOWN', coverage_window_start_ms: 1789214400000, coverage_window_end_ms: 1789214400000 };
}
export function journal() {
  return { event_id: 'journal-fixture', recorded_at: '2026-09-12T12:00:00Z', event: { run_id: 'run-original', network: 'base-mainnet', sequence: '9007199254740993',
    command_id: 'paper-command-fixture', command: { kind: 'INITIALIZE', balances: paperRun().initial_balances },
    postings: [{ asset: principalAsset, account: 'AVAILABLE', side: 'DEBIT', amount: huge }, { asset: principalAsset, account: 'INITIAL_CAPITAL', side: 'CREDIT', amount: huge }] } };
}
export const reservation = { attempt_id: 'attempt-fixture', principal_asset: principalAsset.identity, principal: '9', native_fee_budget: '20', state: 'UNKNOWN' };
