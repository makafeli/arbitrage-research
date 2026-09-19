import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Session, CommandReceipt } from '../src/api/client.ts';
import type { CollectionAttempt } from '../src/api/collection.ts';
import type { Coverage, DecisionGroup, StoredDecision } from '../src/api/research.ts';
import type { StoredCostAssessment } from '../src/api/costs.ts';
import {
  statusSummary, healthSummary, findingsSummary, whatIfSummary, roadToLiveChecklist,
  parseStakeToMinor, collectionFreshness, NATIVE_DECIMALS,
} from '../src/domain/ownerOverview.ts';
import type { ProgressFile } from '../src/domain/ownerOverview.ts';

const NOW = Date.parse('2026-01-01T00:00:00.000Z');

function session(overrides: Partial<Session> = {}): Session {
  return { session_id: 's1', network_id: 'base-mainnet', mode: 'OBSERVE', observed_state: 'RUNNING', health: 'HEALTHY',
    desired_revision: '1', applied_revision: '1', outstanding_attempts: 0, execution_authorized: false,
    last_heartbeat_at: null, configuration_digest: 'sha256:' + 'a'.repeat(64), ...overrides };
}
function attempt(overrides: Partial<CollectionAttempt> = {}): CollectionAttempt {
  return { attempt_id: 'a1', session_id: 's1', network_id: 'base-mainnet', configuration_digest: 'cfg', experiment_id: 'e1',
    generation: '1', worker_epoch: '1', purpose: 'RESEARCH', outcome: 'DECISIONS_RECORDED', reason: null,
    captured_pools: 1, decision_rows: '1', decision_observation_ids: ['sha256:' + 'b'.repeat(64)], elapsed_ms: 10,
    started_at: new Date(NOW - 10_000).toISOString(), finished_at: new Date(NOW - 9_000).toISOString(), ...overrides };
}
function receipt(overrides: Partial<CommandReceipt> = {}): CommandReceipt {
  return { command_id: 'c1', session_id: 's1', revision: '1', status: 'APPLIED', action: 'START', accepted_at: new Date(NOW).toISOString(),
    applied_at: new Date(NOW).toISOString(), outstanding_attempts: 0, fence_effective: true, signer_revocation_status: 'NOT_APPLICABLE', ...overrides };
}

// ---- Block 1: Status ----
test('status: empty (no session) renders not-available-yet, not a fabricated state', () => {
  const result = statusSummary(null, null, [], NOW, 'en');
  assert.equal(result.hasSession, false);
  assert.match(result.modeLabel, /not available yet/);
  assert.match(result.lastCommandReceiptLabel, /does not expose a full command history/);
});
test('status: healthy session with a recent collection reports caught up', () => {
  const result = statusSummary(session(), receipt(), [attempt()], NOW, 'en');
  assert.equal(result.modeLabel, 'watching only (OBSERVE)');
  assert.equal(result.stateLabel, 'running');
  assert.equal(result.sourceLagLabel, 'is caught up (recently updated)');
  assert.match(result.lastCommandReceiptLabel, /applied/);
});
test('status: degraded session with a stale collection reports catching up in words', () => {
  const stale = attempt({ started_at: new Date(NOW - 6 * 60_000).toISOString(), finished_at: new Date(NOW - 6 * 60_000).toISOString() });
  const result = statusSummary(session({ observed_state: 'FAULTED' }), null, [stale], NOW, 'nl');
  assert.equal(result.stateLabel, 'in storing');
  assert.equal(result.sourceLagLabel, 'loopt in (haalt achterstand in)');
  assert.equal(result.lastCommandReceiptLabel, 'nog geen commando vanuit dit venster verstuurd; de dienst biedt geen volledige commandogeschiedenis');
});

// ---- Block 2: Health ----
test('health: empty attempts with no fault renders unknown, not invented green/red', () => {
  const result = healthSummary([], session(), NOW, 'en');
  assert.equal(result.level, 'unknown');
  assert.equal(result.collections, 0);
  assert.equal(result.admittedLabel, 'not available yet');
});
test('health: healthy window is green with the mandated reason', () => {
  const result = healthSummary([attempt()], session(), NOW, 'en');
  assert.equal(result.level, 'green');
  assert.equal(result.reasonLabel, 'no fault, no backlog and no provider failure seen');
  assert.equal(result.collections, 1);
  assert.equal(result.admittedLabel, '1/1');
});
test('health: a fault forces red even with fresh collections (exact mandated threshold)', () => {
  const result = healthSummary([attempt()], session({ observed_state: 'FAULTED' }), NOW, 'en');
  assert.equal(result.level, 'red');
  assert.equal(result.reasonLabel, 'the session is in a fault state');
});
test('health: no collection for over 5 minutes is red per the mandated threshold', () => {
  const stale = attempt({ started_at: new Date(NOW - 6 * 60_000).toISOString(), finished_at: new Date(NOW - 6 * 60_000).toISOString() });
  const result = healthSummary([stale], session(), NOW, 'en');
  assert.equal(result.level, 'red');
  assert.equal(result.reasonLabel, 'no collection received for over 5 minutes');
});
test('health: a provider failure in the window is amber, not red', () => {
  const failed = attempt({ outcome: 'ACQUISITION_FAILED', reason: 'PROVIDER_UNAVAILABLE' });
  const result = healthSummary([failed, attempt()], session(), NOW, 'en');
  assert.equal(result.level, 'amber');
  assert.equal(result.reasonLabel, 'a provider failed in this window');
  assert.equal(result.providerFailures, 1);
});
test('health: attempts older than 24h are excluded from the window counts', () => {
  const old = attempt({ started_at: new Date(NOW - 25 * 60 * 60_000).toISOString(), finished_at: new Date(NOW - 25 * 60 * 60_000).toISOString() });
  const result = healthSummary([old], session(), NOW, 'en');
  assert.equal(result.collections, 0);
});

// ---- collectionFreshness helper ----
test('collectionFreshness: null timestamp is unknown, not stale', () => {
  assert.equal(collectionFreshness(null, NOW).label, 'unknown');
});

// ---- Block 3: Findings ----
function decisionQuoted(overrides: Partial<StoredDecision['trace']> = {}, gross = '100'): StoredDecision {
  return { trace_id: 't1', recorded_at: new Date(NOW).toISOString(), trace: {
    schema_version: '1', observation_id: 'obs-' + gross, session_id: 's1', experiment_id: 'e1', generation: '1',
    configuration_digest: 'cfg', calculation_version: 'v1', strategy_id: 'strat', network_id: 'base-mainnet', mode: 'OBSERVE',
    source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'RECORDED_LIVE', observed_at_unix_ms: NOW, input_age_ms: 0,
    capture_refs: [], route: [{ pool_id: 'p1', asset_in: 'base-mainnet', asset_out: 'base-mainnet:0xUSDC', venue_family: 'v' }],
    amount_in_minor: '1000000000000000000', result: { status: 'QUOTED', quoted_output_minor: '2', gross_delta_minor: gross, included_pool_fees: [] },
    grouping: { version: '1', key: 'k', window_ms: 60000, window_start_ms: NOW }, diagnostics: [], ...overrides,
  } };
}
test('findings: no data anywhere renders an honest empty state', () => {
  const result = findingsSummary(null, [], [], 'en');
  assert.equal(result.hasData, false);
  assert.equal(result.totals, null);
  assert.deepEqual(result.topCandidates, []);
});
test('findings: healthy data ranks candidates by modeled edge and labels every one uniformly', () => {
  const coverage: Coverage = { session_id: 's1', raw_observations: '10', quoted_candidates: '2', rejected: '0', no_route: '0',
    data_unavailable: '0', unique_opportunity_groups: '2', eligible_attempts: null, reconciled_transactions: null,
    execution_accounting_available: false, collection_completeness: 'UNKNOWN', coverage_window_start_ms: NOW - 1000, coverage_window_end_ms: NOW };
  const groups: DecisionGroup[] = [{ grouping_version: '1', grouping_key: 'k', window_start_ms: NOW, dataset_origin: 'RECORDED_LIVE', source_kind: 'CAPTURED_MARKET_DATA', raw_observations: '10', quoted_candidates: '2', rejected: '0', no_route: '0', data_unavailable: '0' }];
  const decisions = [decisionQuoted({}, '50'), decisionQuoted({}, '150')];
  const result = findingsSummary(coverage, groups, decisions, 'en');
  assert.equal(result.hasData, true);
  assert.equal(result.totals?.quotedCandidates, '2');
  assert.equal(result.topCandidates[0].grossDeltaMinor, '150');
  assert.equal(result.topCandidates[0].evidenceLabel, 'candidate, not simulated');
});
test('findings: degraded (rejected/no-route only) still reports totals honestly with no top candidates', () => {
  const coverage: Coverage = { session_id: 's1', raw_observations: '5', quoted_candidates: '0', rejected: '5', no_route: '0',
    data_unavailable: '0', unique_opportunity_groups: '1', eligible_attempts: null, reconciled_transactions: null,
    execution_accounting_available: false, collection_completeness: 'UNKNOWN', coverage_window_start_ms: null, coverage_window_end_ms: null };
  const result = findingsSummary(coverage, [], [], 'en');
  assert.equal(result.totals?.rejected, '5');
  assert.deepEqual(result.topCandidates, []);
});

// ---- Block 4: What if ----
test('whatIf: invalid stake input is rejected, not silently coerced', () => {
  const result = whatIfSummary('not-a-number', 'base-mainnet', [], [], 'en');
  assert.equal(result.stakeValid, false);
  assert.equal(result.stakeMinor, null);
});
test('whatIf: empty candidates for the default stake still returns the caveats', () => {
  const result = whatIfSummary('2', 'base-mainnet', [], [], 'en');
  assert.equal(result.stakeValid, true);
  assert.equal(result.stakeMinor, (2n * 10n ** 18n).toString());
  assert.deepEqual(result.candidates, []);
  assert.match(result.caveats[0], /not executed/);
  assert.match(result.noLeverageNote, /ARB-028/);
});
test('whatIf: scales a same-asset candidate proportionally with BigInt ratio math and finds its cost record', () => {
  const decision = decisionQuoted({}, '10');
  const cost: StoredCostAssessment = { record_id: 'r1', recorded_at: new Date(NOW).toISOString(), assessment: {
    schema_version: '1.0.0', calculation_version: 'decision-bound-exact-costs-v1', assessment_id: 'sha256:' + 'c'.repeat(64), scenario_digest: 'sha256:' + 'd'.repeat(64),
    scenario: {} as StoredCostAssessment['assessment']['scenario'],
    binding: { observation_id: decision.trace.observation_id, decision_digest: 'sha256:' + 'e'.repeat(64), session_id: 's1', experiment_id: 'e1', generation: '1',
      configuration_digest: 'cfg', decision_calculation_version: 'v1', dataset_origin: 'RECORDED_LIVE', network_id: 'base-mainnet', starting_asset: 'base-mainnet',
      amount_in_minor: '1000000000000000000', quoted_output_minor: '2', observed_at_unix_ms: NOW },
    report: { starting_asset: 'base-mainnet', gross_after_quote_included_costs: '10', transaction_net: '5', fully_allocated_net: '3', expenses: [], overhead: { status: 'NOT_ALLOCATED' }, incomplete_reasons: [] },
    evidence: 'CANDIDATE' } };
  const result = whatIfSummary('2', 'base-mainnet', [decision], [cost], 'en');
  assert.equal(result.candidates.length, 1);
  assert.equal(result.candidates[0].modeledGrossEdgeMinor, (10n * 2n * 10n ** 18n / (10n ** 18n)).toString());
  assert.equal(result.candidates[0].recordedCostLabel, '3');
});
test('whatIf: a different-network stake asset skips scaling for candidates in another start asset', () => {
  const decision = decisionQuoted({ network_id: 'solana-mainnet', route: [{ pool_id: 'p1', asset_in: 'solana-mainnet', asset_out: 'x', venue_family: 'v' }] }, '10');
  const result = whatIfSummary('2', 'base-mainnet', [decision], [], 'en');
  assert.equal(result.candidates.length, 0);
  assert.equal(result.skippedOtherAssetCount, 1);
});
test('parseStakeToMinor: rejects fractional precision beyond the asset decimals instead of rounding', () => {
  assert.equal(parseStakeToMinor('1.2345678901234567890', NATIVE_DECIMALS['base-mainnet']), null);
  assert.equal(parseStakeToMinor('1.5', NATIVE_DECIMALS['solana-mainnet']), 1_500_000_000n);
});

// ---- Block 5: Road to PAPER/live ----
const progressFixture: ProgressFile = { tickets: [
  { id: 'ARB-035', state: 'planned', remaining_acceptance: 'Evaluate all original ticket acceptance criteria before closure.' },
  { id: 'ARB-039', state: 'implemented_pending_acceptance', remaining_acceptance: 'Accept ARB-011/012/036/037...' },
  { id: 'ARB-041', state: 'in_progress', remaining_acceptance: 'Accept ARB-023/027/034/040...' },
] };
test('roadToLive: empty progress file renders not-available-yet per ticket, never a hand-typed guess', () => {
  const result = roadToLiveChecklist({ tickets: [] }, 'en');
  assert.equal(result.length, 6);
  assert.ok(result.every(item => item.stateLabel === 'not available yet'));
});
test('roadToLive: reads exact remaining_acceptance text and translates only the state word', () => {
  const result = roadToLiveChecklist(progressFixture, 'en');
  const arb035 = result.find(item => item.id === 'ARB-035');
  assert.equal(arb035?.stateLabel, 'planned');
  assert.equal(arb035?.remainingAcceptance, 'Evaluate all original ticket acceptance criteria before closure.');
  const arb039 = result.find(item => item.id === 'ARB-039');
  assert.equal(arb039?.stateLabel, 'built, waiting on acceptance');
});
test('roadToLive: a ticket missing from the register (degraded input) still fills every required id', () => {
  const result = roadToLiveChecklist(progressFixture, 'en');
  const arb044 = result.find(item => item.id === 'ARB-044');
  assert.equal(arb044?.stateLabel, 'not available yet');
});
