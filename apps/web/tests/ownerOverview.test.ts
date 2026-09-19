import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import type { Session, CommandReceipt } from '../src/api/client.ts';
import type { CollectionCoverage } from '../src/api/collection.ts';
import type { Coverage, StoredDecision } from '../src/api/research.ts';
import type { StoredCostAssessment } from '../src/api/costs.ts';
import {
  statusSummary, healthSummary, findingsSummary, whatIfSummary,
  collectionFreshness, parseProgressFile, roadToLiveChecklist, ROAD_TO_LIVE_TICKET_IDS,
  STARTING_ASSET_ID, STARTING_ASSET_DECIMALS, parseStakeToMinor,
} from '../src/domain/ownerOverview.ts';

const NOW = Date.parse('2026-01-01T00:10:00.000Z');
const BASE_USDC = STARTING_ASSET_ID['base-mainnet'];
const BASE_WETH = 'base-mainnet:0x4200000000000000000000000000000000000006';

function session(overrides: Partial<Session> = {}): Session {
  return {
    session_id: 'session-1', network_id: 'base-mainnet', mode: 'OBSERVE', observed_state: 'RUNNING',
    health: 'UNKNOWN', desired_revision: '1', applied_revision: '1', outstanding_attempts: 0,
    execution_authorized: false, last_heartbeat_at: null, configuration_digest: 'cfg',
    ...overrides,
  };
}
function receipt(overrides: Partial<CommandReceipt> = {}): CommandReceipt {
  return {
    command_id: 'cmd-1', session_id: 'session-1', revision: '1', status: 'APPLIED', action: 'START',
    accepted_at: new Date(NOW).toISOString(), applied_at: new Date(NOW).toISOString(), outstanding_attempts: 0,
    fence_effective: false, signer_revocation_status: 'NOT_APPLICABLE',
    ...overrides,
  };
}
function coverage(overrides: Partial<CollectionCoverage> = {}): CollectionCoverage {
  return {
    session_id: 'session-1', denominator: 'RECORDED_COLLECTION_ATTEMPTS', collection_completeness: 'UNKNOWN',
    attempts_started: '0', readiness_attempts: '0', research_attempts: '0', in_progress: '0', readiness_completed: '0',
    decisions_recorded: '0', acquisition_failed: '0', evaluation_failed: '0', deadline_exceeded: '0',
    suppressed: '0', worker_cancelled: '0', decision_rows_recorded: '0',
    window_start_at: null, window_end_at: null,
    ...overrides,
  };
}
function decisionCoverage(overrides: Partial<Coverage> = {}): Coverage {
  return {
    session_id: 'session-1', raw_observations: '0', quoted_candidates: '0', rejected: '0', no_route: '0',
    data_unavailable: '0', unique_opportunity_groups: '0', eligible_attempts: null, reconciled_transactions: null,
    execution_accounting_available: false, collection_completeness: 'UNKNOWN',
    coverage_window_start_ms: null, coverage_window_end_ms: null,
    ...overrides,
  };
}
function quotedDecision(id: string, grossDeltaMinor: string, opts: { assetIn?: string; amountInMinor?: string } = {}): StoredDecision {
  const amountInMinor = opts.amountInMinor ?? '2000000';
  const quotedOutputMinor = (BigInt(amountInMinor) + BigInt(grossDeltaMinor)).toString();
  return {
    trace_id: 't-' + id, recorded_at: new Date(NOW).toISOString(),
    trace: {
      schema_version: '1.0.0', observation_id: id, session_id: 'session-1', experiment_id: 'e1', generation: '1',
      configuration_digest: 'cfg', calculation_version: 'v1', strategy_id: 'strat', network_id: 'base-mainnet', mode: 'OBSERVE',
      source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'RECORDED_LIVE', observed_at_unix_ms: NOW, input_age_ms: 0,
      capture_refs: [],
      route: [{ pool_id: 'p1', asset_in: opts.assetIn ?? BASE_USDC, asset_out: BASE_WETH, venue_family: 'v' }],
      amount_in_minor: amountInMinor,
      result: { status: 'QUOTED', quoted_output_minor: quotedOutputMinor, gross_delta_minor: grossDeltaMinor, included_pool_fees: ['0', '0'] },
      grouping: { version: '1', key: 'k', window_ms: 60000, window_start_ms: NOW },
      diagnostics: [],
    },
  };
}
function costAssessment(observationId: string, overrides: Partial<StoredCostAssessment['assessment']['report']> = {}): StoredCostAssessment {
  return {
    record_id: 'r-' + observationId, recorded_at: new Date(NOW).toISOString(),
    assessment: {
      schema_version: '1.0.0', calculation_version: 'decision-bound-exact-costs-v1', assessment_id: 'sha256:' + '0'.repeat(64),
      scenario_digest: 'sha256:' + '1'.repeat(64),
      scenario: { schema_version: '1.0.0', scenario_id: 's1', version: 'v1', origin: 'MANUALLY_CONSTRUCTED', provenance_reference: 'ref',
        valuation_max_age_ms: 60000, fee_composition: 'BASE_EXECUTION_INCLUDES_PRIORITY', expenses: [], funding: { status: 'OWN_VIRTUAL_CAPITAL' }, overhead: { status: 'NOT_ALLOCATED' } },
      binding: { observation_id: observationId, decision_digest: 'sha256:' + '2'.repeat(64), session_id: 'session-1', experiment_id: 'e1', generation: '1',
        configuration_digest: 'cfg', decision_calculation_version: 'v1', dataset_origin: 'RECORDED_LIVE', network_id: 'base-mainnet',
        starting_asset: BASE_USDC, amount_in_minor: '2000000', quoted_output_minor: '2500000', observed_at_unix_ms: NOW },
      report: { starting_asset: BASE_USDC, gross_after_quote_included_costs: '500000', transaction_net: '450000', fully_allocated_net: null,
        expenses: [], overhead: { status: 'NOT_ALLOCATED' }, incomplete_reasons: [], ...overrides },
      evidence: 'CANDIDATE',
    },
  };
}

// ---- Status/Health from session.health + coverage --------------------------
test('status: worker alive shows the heartbeat age when health is DEGRADED', () => {
  const s = session({ health: 'DEGRADED', last_heartbeat_at: new Date(NOW - 9_000).toISOString() });
  const result = statusSummary(s, null, coverage(), NOW, NOW, 'en');
  assert.equal(result.workerAliveLabel, '9s');
});
test('status: worker not reachable when health is UNREACHABLE while running', () => {
  const s = session({ health: 'UNREACHABLE', observed_state: 'RUNNING' });
  const result = statusSummary(s, null, coverage(), NOW, NOW, 'en');
  assert.equal(result.workerAliveLabel, 'not reachable');
});
test('status: a deliberately stopped session reads "stopped, not collecting", not stale', () => {
  const s = session({ observed_state: 'STOPPED', health: 'UNREACHABLE' });
  const stale = coverage({ research_attempts: '5', decisions_recorded: '1', window_end_at: new Date(NOW - 3_600_000).toISOString() });
  const result = statusSummary(s, null, stale, NOW, NOW, 'en');
  assert.equal(result.sourceLagLabel, 'stopped, not collecting');
});
test('status: uses action_* copy for the last command receipt instead of the raw enum', () => {
  const result = statusSummary(session(), receipt({ action: 'PAUSE', status: 'APPLIED' }), coverage(), NOW, NOW, 'en');
  assert.ok(result.lastCommandReceiptLabel.includes('pause'));
  assert.ok(!result.lastCommandReceiptLabel.includes('PAUSE'));
});
test('status: empty (no session) renders not-available-yet, not a fabricated state', () => {
  const result = statusSummary(null, null, null, NOW, NOW, 'en');
  assert.equal(result.hasSession, false);
  assert.equal(result.modeLabel, 'not available yet');
  assert.equal(result.stateLabel, 'not available yet');
  assert.equal(result.workerAliveLabel, 'not available yet');
  assert.equal(result.sourceLagLabel, 'not available yet');
  assert.match(result.lastCommandReceiptLabel, /does not expose a full command history/);
});

// ---- Health ladder order -----------------------------------------------------
test('health: empty (no coverage, no session) renders unknown, not invented green/red', () => {
  const result = healthSummary(null, null, NOW, 'en');
  assert.equal(result.level, 'unknown');
  assert.equal(result.collections, 'not available yet');
  assert.equal(result.admittedLabel, 'not available yet');
  assert.equal(result.failedAttempts, 'not available yet');
});
test('health: FAULTED session is red regardless of coverage', () => {
  const s = session({ observed_state: 'FAULTED', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '5', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.level, 'red');
});
test('health: unreachable worker while running is red even with recent coverage', () => {
  const s = session({ observed_state: 'RUNNING', health: 'UNREACHABLE' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '5', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.level, 'red');
});
test('health: a deliberately stopped session is unknown, not red, even with a stale window', () => {
  const s = session({ observed_state: 'PAUSED', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '1', window_end_at: new Date(NOW - 3_600_000).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.notEqual(result.level, 'red');
});
test('health: stale collection (RUNNING, no update for over 5 minutes) is red', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '3', window_end_at: new Date(NOW - 6 * 60_000).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.level, 'red');
});
test('health: catching up (coverage ~2 minutes old) is amber, not red', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '3', window_end_at: new Date(NOW - 2 * 60_000).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.level, 'amber');
});
test('health: failed attempts do not block green on a fresh coverage window with admits (F2)', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const freshWithFailures = coverage({ research_attempts: '5', decisions_recorded: '4', acquisition_failed: '1', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(freshWithFailures, s, NOW, 'en');
  assert.equal(result.level, 'green');
  assert.equal(result.failedAttempts, '1');
});
test('health: a stale window is still red even with admitted decisions and past failures', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const staleAndFailed = coverage({ research_attempts: '5', decisions_recorded: '3', acquisition_failed: '1', window_end_at: new Date(NOW - 6 * 60_000).toISOString() });
  const staleResult = healthSummary(staleAndFailed, s, NOW, 'en');
  assert.equal(staleResult.level, 'red');
});
// A STOPPED session's worker can legitimately go UNREACHABLE (nothing keeps its lease alive) — that
// must not be judged as a red outage, since the session was deliberately stopped, not lost.
test('health: STOPPED with health UNREACHABLE is unknown ("stopped, not collecting"), not red', () => {
  const s = session({ observed_state: 'STOPPED', health: 'UNREACHABLE' });
  const result = healthSummary(coverage(), s, NOW, 'en');
  assert.notEqual(result.level, 'red');
  assert.equal(result.reasonLabel, 'the session is not currently collecting (stopped or paused)');
});
test('status: STOPPED with health UNREACHABLE reads "stopped, not collecting"', () => {
  const s = session({ observed_state: 'STOPPED', health: 'UNREACHABLE' });
  const result = statusSummary(s, null, coverage(), NOW, NOW, 'en');
  assert.equal(result.sourceLagLabel, 'stopped, not collecting');
});
// RUNNING + UNKNOWN health means no heartbeat has ever been recorded yet — that is not the same
// as a lost worker (UNREACHABLE) and must read "no heartbeat received", not report red.
test('health: RUNNING with health UNKNOWN is not red (no heartbeat received yet)', () => {
  const s = session({ observed_state: 'RUNNING', health: 'UNKNOWN' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '5', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.notEqual(result.level, 'red');
});
test('status: RUNNING with health UNKNOWN reports "no heartbeat received", not stale', () => {
  const s = session({ observed_state: 'RUNNING', health: 'UNKNOWN' });
  const result = statusSummary(s, null, coverage(), NOW, NOW, 'en');
  assert.equal(result.workerAliveLabel, 'no heartbeat received from the worker yet');
});
test('health: green requires admitted > 0 even with no failures and fresh coverage', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const noAdmits = coverage({ research_attempts: '3', decisions_recorded: '0', in_progress: '3', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(noAdmits, s, NOW, 'en');
  assert.notEqual(result.level, 'green');

  const admitted = coverage({ research_attempts: '3', decisions_recorded: '3', window_end_at: new Date(NOW).toISOString() });
  const okResult = healthSummary(admitted, s, NOW, 'en');
  assert.equal(okResult.level, 'green');
});

// ---- Counters -----------------------------------------------------------------
test('health: suppressed (fenced) attempts are not counted as failed attempts', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '2', suppressed: '3', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.failedAttempts, '0');
});
test('health: admitted label reads "3/5" for 3 recorded of 5 started', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '5', decisions_recorded: '3', evaluation_failed: '2', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.admittedLabel, '3/5');
});
test('health: an acquisition_failed row (the RESOURCE_LIMIT/PROVIDER_UNAVAILABLE bucket) counts toward failed attempts', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const c = coverage({ research_attempts: '2', decisions_recorded: '1', acquisition_failed: '1', window_end_at: new Date(NOW).toISOString() });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.failedAttempts, '1');
});
test('health: acquisition_failed, evaluation_failed, deadline_exceeded and worker_cancelled each count once toward failed attempts', () => {
  const s = session({ observed_state: 'RUNNING', health: 'DEGRADED' });
  const c = coverage({
    research_attempts: '5', decisions_recorded: '1', readiness_completed: '1',
    acquisition_failed: '1', evaluation_failed: '1', deadline_exceeded: '1', worker_cancelled: '1',
    window_end_at: new Date(NOW).toISOString(),
  });
  const result = healthSummary(c, s, NOW, 'en');
  assert.equal(result.failedAttempts, '4');
});

// ---- collectionFreshness helper (kept) -----------------------------------------
test('collectionFreshness: null timestamp is unknown', () => {
  assert.equal(collectionFreshness(null, NOW).label, 'unknown');
});

// ---- What-if: asset scoping -----------------------------------------------------
test('what-if: invalid stake input is rejected, not silently coerced', () => {
  const result = whatIfSummary('not-a-number', 'base-mainnet', [], [], 'en');
  assert.equal(result.stakeValid, false);
  assert.equal(result.stakeMinor, null);
});
test('what-if: empty candidates for the default stake still returns the caveats', () => {
  const result = whatIfSummary('2', 'base-mainnet', [], [], 'en');
  assert.equal(result.stakeValid, true);
  assert.equal(result.stakeMinor, (2n * 10n ** BigInt(STARTING_ASSET_DECIMALS)).toString());
  assert.deepEqual(result.candidates, []);
  assert.match(result.caveats[0], /not executed/);
  assert.match(result.noLeverageNote, /ARB-028/);
  assert.doesNotMatch(result.noLeverageNote, /ARB-041/);
  assert.match(result.noLeverageNote, /ARB-025/);
});
test('parseStakeToMinor: rejects fractional precision beyond the asset decimals instead of rounding', () => {
  assert.equal(parseStakeToMinor('1.2345678901234567890', STARTING_ASSET_DECIMALS), null);
  assert.equal(parseStakeToMinor('1.5', STARTING_ASSET_DECIMALS), 1_500_000n);
});
// Regression: stake must scale by its own ratio to amount_in_minor, not assume stake === amount.
// stake 4 USDC (4_000_000 minor) against a decision with amount_in_minor 2_000_000 and gross
// 500_000 is a 2x ratio, so the modeled edge is 1_000_000, not the raw recorded gross. Also
// asserts fully_allocated_net is preferred over transaction_net when both are present.
test('what-if: scales a same-asset candidate by its own stake:amount ratio and prefers fully_allocated_net', () => {
  const decision = quotedDecision('usdc-1', '500000', { amountInMinor: '2000000' });
  const cost = costAssessment('usdc-1', { transaction_net: '450000', fully_allocated_net: '400000' });
  const result = whatIfSummary('4', 'base-mainnet', [decision], [cost], 'en');
  assert.equal(result.candidates.length, 1);
  assert.equal(result.candidates[0].modeledGrossEdgeMinor, '1000000');
  assert.equal(result.candidates[0].recordedCostLabel, '400000');
});
test('what-if: a non-USDC asset_in row is skipped, not scaled', () => {
  const decisions = [quotedDecision('usdc-1', '500000'), quotedDecision('weth-1', '900000', { assetIn: BASE_WETH })];
  const result = whatIfSummary('2', 'base-mainnet', decisions, [], 'en');
  assert.equal(result.candidates.length, 1);
  assert.equal(result.candidates[0].observationId, 'usdc-1');
  assert.equal(result.skippedOtherAssetCount, 1);
});
test('what-if: cost lookup surfaces the recorded net for a matched observation', () => {
  const decisions = [quotedDecision('usdc-1', '500000')];
  const result = whatIfSummary('2', 'base-mainnet', decisions, [costAssessment('usdc-1')], 'en');
  assert.equal(result.candidates[0].recordedCostLabel, '450000');
});
test('what-if: carries dataset_origin onto each candidate', () => {
  const decisions = [quotedDecision('usdc-1', '500000')];
  const result = whatIfSummary('2', 'base-mainnet', decisions, [], 'en');
  assert.equal(result.candidates[0].datasetOrigin, 'RECORDED_LIVE');
});

// ---- Top candidates: filter, sort, cap ---------------------------------------
test('findings: no data anywhere renders an honest empty state', () => {
  const result = findingsSummary(null, [], [], 'en');
  assert.equal(result.hasData, false);
  assert.equal(result.totals, null);
  assert.deepEqual(result.topCandidates, []);
});
test('findings: a zero-edge QUOTED row is excluded from Top candidates, same as a negative one', () => {
  const decisions = [quotedDecision('a', '100'), quotedDecision('zero', '0')];
  const result = findingsSummary(decisionCoverage(), [], decisions, 'en');
  assert.ok(!result.topCandidates.some(c => c.observationId === 'zero'), 'zero-edge decision must be filtered out');
});
test('findings: six QUOTED decisions produce five sorted rows with negative-edge filtered out', () => {
  const decisions = [
    quotedDecision('a', '100'), quotedDecision('b', '500'), quotedDecision('c', '-50'),
    quotedDecision('d', '300'), quotedDecision('e', '200'), quotedDecision('f', '400'),
  ];
  const result = findingsSummary(decisionCoverage(), [], decisions, 'en');
  assert.equal(result.topCandidates.length, 5);
  assert.deepEqual(result.topCandidates.map(c => c.observationId), ['b', 'f', 'd', 'e', 'a']);
  assert.ok(!result.topCandidates.some(c => c.observationId === 'c'), 'negative-edge decision must be filtered out');
});
test('findings: ties keep a stable order', () => {
  const decisions = [quotedDecision('x', '100'), quotedDecision('y', '100')];
  const result = findingsSummary(decisionCoverage(), [], decisions, 'en');
  assert.deepEqual(result.topCandidates.map(c => c.observationId), ['x', 'y']);
});
test('findings: carries dataset_origin onto each top candidate', () => {
  const decisions = [quotedDecision('a', '100')];
  const result = findingsSummary(decisionCoverage(), [], decisions, 'en');
  assert.equal(result.topCandidates[0].datasetOrigin, 'RECORDED_LIVE');
});
test('findings: exposes first/last batch timestamps from decision coverage', () => {
  const result = findingsSummary(decisionCoverage({ coverage_window_start_ms: 1000, coverage_window_end_ms: 2000 }), [], [], 'en');
  assert.equal(result.firstBatchAtMs, 1000);
  assert.equal(result.lastBatchAtMs, 2000);
});

// ---- Progress file / road-to-live checklist -----------------------------------
test('parseProgressFile: accepts a minimal valid ticket list', () => {
  const parsed = parseProgressFile({ tickets: [{ id: 'ARB-035', state: 'completed' }] });
  assert.equal(parsed.tickets.length, 1);
});
test('parseProgressFile: rejects a non-array tickets field', () => {
  assert.throws(() => parseProgressFile({ tickets: 'nope' }));
});
test('parseProgressFile: accepts the real planning/implementation-progress.json with a non-empty ticket list', () => {
  const raw = JSON.parse(readFileSync(new URL('../../../planning/implementation-progress.json', import.meta.url), 'utf8'));
  const parsed = parseProgressFile(raw);
  assert.ok(parsed.tickets.length > 0);
  assert.ok(parsed.tickets.every(ticket => typeof ticket.id === 'string' && typeof ticket.state === 'string'));
});
test('parseProgressFile: rejects a malformed register instead of silently casting it', () => {
  assert.throws(() => parseProgressFile({ tickets: [{ state: 'planned' }] }));
  assert.throws(() => parseProgressFile({}));
});
test('roadToLiveChecklist: reports "not available yet" for a missing ticket id', () => {
  const checklist = roadToLiveChecklist({ tickets: [] }, 'en');
  assert.equal(checklist.length, ROAD_TO_LIVE_TICKET_IDS.length);
  assert.equal(checklist[0].stateLabel, 'not available yet');
});
test('roadToLiveChecklist: maps a known ticket state to its plain-language label', () => {
  const checklist = roadToLiveChecklist({ tickets: [{ id: 'ARB-035', state: 'completed', remaining_acceptance: '' }] }, 'en');
  const item = checklist.find(i => i.id === 'ARB-035')!;
  assert.equal(item.stateLabel, 'accepted');
});
