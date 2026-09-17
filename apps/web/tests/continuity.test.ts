import test from 'node:test';
import assert from 'node:assert/strict';
import { ControlApi } from '../src/api/client.ts';
import { continuityForDecision, parseDecisionContinuity } from '../src/api/continuity.ts';
import { decision } from './research.fixture.ts';

function fixture() {
  const record = decision();
  return { schema_version: '1.0.0', assessment_kind: 'CONTINUITY_ONLY', authorizes_execution: false,
    trace_id: record.trace_id, session_id: record.trace.session_id, observation_id: record.trace.observation_id,
    network_id: record.trace.network_id, checked_at: '2026-09-17T08:00:00.000Z', policy_version: 'base-capture-continuity-v1',
    continuity_status: 'NO_KNOWN_INVALIDATION', capture_count: String(record.trace.capture_refs.length),
    bound_count: String(record.trace.capture_refs.length), invalidation_reasons: [] as string[] };
}
test('continuity is separate from original decision bytes and never authorizes execution', () => {
  const record = decision(), before = JSON.stringify(record);
  const current = continuityForDecision(parseDecisionContinuity(fixture()), record);
  assert.equal(current.authorizes_execution, false);
  const invalid = parseDecisionContinuity({ ...fixture(), continuity_status: 'INVALIDATED', invalidation_reasons: ['CONTINUITY_LOST'] });
  assert.equal(continuityForDecision(invalid, record).continuity_status, 'INVALIDATED');
  assert.equal(JSON.stringify(record), before);
});
test('empty, legacy and unsupported-chain status never becomes known-valid', () => {
  assert.equal(parseDecisionContinuity({ ...fixture(), continuity_status: 'UNTRACKED', capture_count: '0', bound_count: '0' }).capture_count, '0');
  assert.equal(parseDecisionContinuity({ ...fixture(), continuity_status: 'UNTRACKED', bound_count: '0' }).bound_count, '0');
  assert.equal(parseDecisionContinuity({ ...fixture(), network_id: 'solana-mainnet', continuity_status: 'UNVERIFIABLE' }).continuity_status, 'UNVERIFIABLE');
  assert.throws(() => parseDecisionContinuity({ ...fixture(), network_id: 'solana-mainnet' }));
});
test('unknown versions, extra fields, optimistic flags and contradictory counts are rejected', () => {
  for (const change of [
    { schema_version: '2.0.0' }, { policy_version: 'invented' }, { continuity_status: 'VERIFIED' },
    { authorizes_execution: true }, { assessment_kind: 'FRESH_AND_EXECUTABLE' }, { secret: 'unexpected' },
    { bound_count: '65' }, { bound_count: '-1' }, { capture_count: '00' }, { capture_count: 2 },
    { capture_count: '0', bound_count: '0' }, { bound_count: '0' }, { checked_at: 'not-a-date' },
    { continuity_status: 'INVALIDATED', invalidation_reasons: [] },
    { invalidation_reasons: ['PROVIDER_FAILURE'] },
    { continuity_status: 'UNTRACKED' },
  ]) assert.throws(() => parseDecisionContinuity({ ...fixture(), ...change }));
});
test('bounded public reasons cannot carry raw provider text or duplicates', () => {
  for (const values of [['raw-provider-url'], ['CONTINUITY_LOST', 'CONTINUITY_LOST'], ['RESOURCE_LIMIT', 'PROVIDER_FAILURE']]) {
    assert.throws(() => parseDecisionContinuity({ ...fixture(), continuity_status: 'INVALIDATED', invalidation_reasons: values }));
  }
  assert.equal(parseDecisionContinuity({ ...fixture(), continuity_status: 'INVALIDATED', invalidation_reasons: ['CONTINUITY_LOST', 'PROVIDER_FAILURE'] }).invalidation_reasons.length, 2);
});
test('inspection rejects another observation, trace, chain, session or capture denominator', () => {
  const current = parseDecisionContinuity(fixture()), record = decision();
  for (const change of [{ session_id: 'other' }, { observation_id: 'other' }, { trace_id: 'other' }, { network_id: 'solana-mainnet' as const }, { capture_count: '64' }]) {
    assert.throws(() => continuityForDecision({ ...current, ...change }, record));
  }
});
test('API continuity calls bind requested session and observation and preserve abort handling', async () => {
  const original = globalThis.fetch;
  try {
    globalThis.fetch = async (input, options) => {
      assert.match(String(input), /\/sessions\/session-paper\/decisions\/observation-fixture\/continuity$/);
      assert.equal(options?.credentials, 'same-origin');
      return new Response(JSON.stringify(fixture()), { status: 200 });
    };
    const api = new ControlApi();
    assert.equal((await api.decisionContinuity('session-paper', 'observation-fixture')).authorizes_execution, false);
    globalThis.fetch = async () => new Response(JSON.stringify({ ...fixture(), session_id: 'other' }), { status: 200 });
    await assert.rejects(api.decisionContinuity('session-paper', 'observation-fixture'));
    const controller = new AbortController(); controller.abort();
    await assert.rejects(api.decisionContinuity('session-paper', 'observation-fixture', controller.signal));
  } finally { globalThis.fetch = original; }
});
