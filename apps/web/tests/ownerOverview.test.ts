import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import type { Session, CommandReceipt } from '../src/api/client.ts';
import type { CollectionAttempt } from '../src/api/collection.ts';
import type { Coverage, DecisionGroup, StoredDecision } from '../src/api/research.ts';
import type { StoredCostAssessment } from '../src/api/costs.ts';
import {
  statusSummary, healthSummary, findingsSummary, whatIfSummary, roadToLiveChecklist,
  parseStakeToMinor, collectionFreshness, STARTING_ASSET_DECIMALS,
  uuidV7FloorForTimestamp, fetchWindow, parseProgressFile,
} from '../src/domain/ownerOverview.ts';
import type { ProgressFile, WindowPage } from '../src/domain/ownerOverview.ts';

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
test('status: degraded session with a stale collection reports stale in words, matching Health', () => {
  const stale = attempt({ started_at: new Date(NOW - 6 * 60_000).toISOString(), finished_at: new Date(NOW - 6 * 60_000).toISOString() });
  const result = statusSummary(session({ observed_state: 'FAULTED' }), null, [stale], NOW, 'nl');
  assert.equal(result.stateLabel, 'in storing');
  assert.equal(result.sourceLagLabel, 'loopt vast — meer dan 5 minuten geen verzameling');
  assert.equal(result.lastCommandReceiptLabel, 'nog geen commando vanuit dit venster verstuurd; de dienst biedt geen volledige commandogeschiedenis');
});

// ---- Block 2: Health ----
test('health: empty attempts with no fault renders unknown, not invented green/red', () => {
  const result = healthSummary([], session(), NOW, 'en');
  assert.equal(result.level, 'unknown');
  assert.equal(result.collections, 0);
  assert.equal(result.admittedLabel, 'not available yet');
});
test('health: healthy window is green with the ok reason', () => {
  const result = healthSummary([attempt()], session(), NOW, 'en');
  assert.equal(result.level, 'green');
  assert.equal(result.reasonLabel, 'no fault, no backlog and no provider failure seen');
  assert.equal(result.collections, 1);
  assert.equal(result.admittedLabel, '1/1');
});
test('health: a fault forces red even with fresh collections', () => {
  const result = healthSummary([attempt()], session({ observed_state: 'FAULTED' }), NOW, 'en');
  assert.equal(result.level, 'red');
  assert.equal(result.reasonLabel, 'the session is in a fault state');
});
test('health: no collection for over 5 minutes is red', () => {
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
// Real Base mainnet addresses (USDC, WETH9) so asset_in/asset_out are valid `network:address`
// AssetId strings, as crates/arb-domain/src/identity.rs requires — a bare 'base-mainnet' is
// never a real value here. amount_in_minor is 2 USDC at 6 decimals, and quoted_output_minor is
// derived so quoted_output_minor - amount_in_minor === gross_delta_minor (see api/research.ts's
// parseDecision invariant).
const BASE_USDC = 'base-mainnet:0x833589fcd6edb6e08f4c7c32d4f71b54bda02913';
const BASE_WETH = 'base-mainnet:0x4200000000000000000000000000000000000006';
const DEFAULT_AMOUNT_IN_MINOR = 2_000_000n;
function decisionQuoted(overrides: Partial<StoredDecision['trace']> = {}, gross = '100'): StoredDecision {
  const amountInMinor = overrides.amount_in_minor ? BigInt(overrides.amount_in_minor) : DEFAULT_AMOUNT_IN_MINOR;
  return { trace_id: 't1', recorded_at: new Date(NOW).toISOString(), trace: {
    schema_version: '1', observation_id: 'obs-' + gross, session_id: 's1', experiment_id: 'e1', generation: '1',
    configuration_digest: 'cfg', calculation_version: 'v1', strategy_id: 'strat', network_id: 'base-mainnet', mode: 'OBSERVE',
    source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'RECORDED_LIVE', observed_at_unix_ms: NOW, input_age_ms: 0,
    capture_refs: [], route: [{ pool_id: 'p1', asset_in: BASE_USDC, asset_out: BASE_WETH, venue_family: 'v' }],
    amount_in_minor: amountInMinor.toString(), result: { status: 'QUOTED', quoted_output_minor: (amountInMinor + BigInt(gross)).toString(), gross_delta_minor: gross, included_pool_fees: [] },
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
  assert.equal(result.stakeMinor, (2n * 10n ** BigInt(STARTING_ASSET_DECIMALS)).toString());
  assert.deepEqual(result.candidates, []);
  assert.match(result.caveats[0], /not executed/);
  assert.match(result.noLeverageNote, /ARB-028/);
  assert.doesNotMatch(result.noLeverageNote, /ARB-041/);
  assert.match(result.noLeverageNote, /ARB-025/);
});
test('whatIf: scales a same-asset candidate proportionally with BigInt ratio math and finds its cost record', () => {
  const decision = decisionQuoted({}, '10');
  const cost: StoredCostAssessment = { record_id: 'r1', recorded_at: new Date(NOW).toISOString(), assessment: {
    schema_version: '1.0.0', calculation_version: 'decision-bound-exact-costs-v1', assessment_id: 'sha256:' + 'c'.repeat(64), scenario_digest: 'sha256:' + 'd'.repeat(64),
    scenario: {} as StoredCostAssessment['assessment']['scenario'],
    binding: { observation_id: decision.trace.observation_id, decision_digest: 'sha256:' + 'e'.repeat(64), session_id: 's1', experiment_id: 'e1', generation: '1',
      configuration_digest: 'cfg', decision_calculation_version: 'v1', dataset_origin: 'RECORDED_LIVE', network_id: 'base-mainnet', starting_asset: BASE_USDC,
      amount_in_minor: decision.trace.amount_in_minor as string, quoted_output_minor: decision.trace.result.status === 'QUOTED' ? decision.trace.result.quoted_output_minor : '0', observed_at_unix_ms: NOW },
    report: { starting_asset: BASE_USDC, gross_after_quote_included_costs: '10', transaction_net: '5', fully_allocated_net: '3', expenses: [], overhead: { status: 'NOT_ALLOCATED' }, incomplete_reasons: [] },
    evidence: 'CANDIDATE' } };
  const result = whatIfSummary('2', 'base-mainnet', [decision], [cost], 'en');
  assert.equal(result.candidates.length, 1);
  // stake 2 USDC == the fixture's own amount_in_minor, so the ratio is 1: modeled edge equals the recorded gross.
  assert.equal(result.candidates[0].modeledGrossEdgeMinor, '10');
  assert.equal(result.candidates[0].recordedCostLabel, '3');
});
test('whatIf: a different-network stake asset skips scaling for candidates in another start asset', () => {
  const SOLANA_USDC = 'solana-mainnet:EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
  const WRAPPED_SOL = 'solana-mainnet:So11111111111111111111111111111111111111112';
  const decision = decisionQuoted({ network_id: 'solana-mainnet', route: [{ pool_id: 'p1', asset_in: SOLANA_USDC, asset_out: WRAPPED_SOL, venue_family: 'v' }] }, '10');
  const result = whatIfSummary('2', 'base-mainnet', [decision], [], 'en');
  assert.equal(result.candidates.length, 0);
  assert.equal(result.skippedOtherAssetCount, 1);
});
test('parseStakeToMinor: rejects fractional precision beyond the asset decimals instead of rounding', () => {
  assert.equal(parseStakeToMinor('1.2345678901234567890', STARTING_ASSET_DECIMALS), null);
  assert.equal(parseStakeToMinor('1.5', STARTING_ASSET_DECIMALS), 1_500_000n);
});

// ---- Attempts windowing helpers ----
test('uuidV7FloorForTimestamp: produces a canonical UUIDv7 whose timestamp bits round-trip', () => {
  const ms = Date.parse('2026-01-01T00:00:00.000Z');
  const uuid = uuidV7FloorForTimestamp(ms);
  assert.match(uuid, /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-8[0-9a-f]{3}-[0-9a-f]{12}$/);
  const decodedMs = parseInt(uuid.replace(/-/g, '').slice(0, 12), 16);
  assert.equal(decodedMs, ms);
});
test('uuidV7FloorForTimestamp: clamps negative timestamps to zero instead of throwing', () => {
  assert.doesNotThrow(() => uuidV7FloorForTimestamp(-1));
});
test('fetchWindow: pages forward until next_cursor is null, concatenating items in order', async () => {
  const pages: WindowPage<number>[] = [
    { items: [1, 2], next_cursor: 'c1' },
    { items: [3], next_cursor: null },
  ];
  let calls = 0;
  const result = await fetchWindow(async cursor => { assert.equal(cursor, calls === 0 ? 'floor' : 'c1'); calls++; return pages[calls - 1]; }, 'floor', 4);
  assert.deepEqual(result.items, [1, 2, 3]);
  assert.equal(result.truncated, false);
  assert.equal(calls, 2);
});
test('fetchWindow: stops at the page cap and reports truncation instead of paging forever', async () => {
  let calls = 0;
  const result = await fetchWindow(async () => { calls++; return { items: [calls], next_cursor: 'more' }; }, 'floor', 3);
  assert.equal(calls, 3);
  assert.equal(result.items.length, 3);
  assert.equal(result.truncated, true);
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

// ---- parseProgressFile: validates the real, hand-maintained register (item 6: catch schema drift here) ----
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
