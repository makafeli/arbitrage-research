import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { parseDecision } from '../src/api/research.ts';
import type { StoredDecision } from '../src/api/research.ts';
import { canonicalCostJson, parseCostAssessment, requireCostDecision, verifyCostAssessment } from '../src/api/costs.ts';
import { parseFrozenExport, verifyFrozenExport } from '../src/api/frozenExport.ts';
import { frozenExportFixture, sealExport } from './exports.fixture.ts';
const fixtures = JSON.parse(readFileSync(new URL('../../../specs/chain-freshness.example.json', import.meta.url), 'utf8')) as { cases: { name: string; record: StoredDecision }[] };
const fixture = (name = 'within-policy') => structuredClone(fixtures.cases.find(c => c.name === name)!.record);

test('independent sealed chain-age fixtures retain exact age, historical elapsed and canonical observation hashes', () => {
  for (const { name, record } of fixtures.cases) {
    const parsed = parseDecision(record); assert.deepEqual(parsed, record);
    const { observation_id, ...preimage } = parsed.trace;
    assert.equal('sha256:' + createHash('sha256').update(canonicalCostJson(preimage)).digest('hex'), observation_id);
    assert.equal(parsed.trace.dataset_origin, 'MANUALLY_CONSTRUCTED');
    assert.equal(parsed.trace.chain_freshness!.evaluation_elapsed_ms, 750);
    assert.equal(parsed.trace.chain_freshness!.sources[0].age_ms, name === 'within-policy' ? 750 : name === 'stale' ? 5750 : null);
  }
});
test('chain freshness rejects tampered arithmetic, source scope, policy, schema and false quote promotion', () => {
  const mutations: ((v: StoredDecision) => void)[] = [
    v => v.trace.chain_freshness!.sources[0].age_ms!++, v => v.trace.chain_freshness!.sources[0].capture_id = 'different',
    v => v.trace.chain_freshness!.reference_observed_at_unix_ms++, v => v.trace.chain_freshness!.evaluation_elapsed_ms++,
    v => v.trace.chain_freshness!.policy.max_chain_age_ms = 0, v => v.trace.chain_freshness!.status = 'UNKNOWN',
    v => v.trace.schema_version = '1.0.0', v => v.trace.calculation_version = 'legacy',
    v => v.trace.chain_freshness!.sources[0].chain_time_seconds = 253402300800,
    v => v.trace.chain_freshness!.sources[0].source = { kind: 'SOLANA_ESTIMATED_BLOCK_TIME', slot: '1', genesis_hash: '11111111111111111111111111111111', account_context: 'ctx' },
  ];
  for (const mutate of mutations) { const changed = fixture(); mutate(changed); assert.throws(() => parseDecision(changed)); }
  const stale = fixture('stale'); stale.trace.result = fixture().trace.result; stale.trace.amount_in_minor = fixture().trace.amount_in_minor; stale.trace.route = fixture().trace.route; assert.throws(() => parseDecision(stale), /quoted result/);
  const missing = fixture('unknown'); if (missing.trace.result.status !== 'QUOTED') missing.trace.result.reason_codes = ['NO_CAPTURE_INPUTS']; assert.throws(() => parseDecision(missing), /failure is absent/);
  const absent = fixture(); delete absent.trace.chain_freshness; assert.throws(() => parseDecision(absent), /schema/);
});
test('Solana timestamps stay estimated with canonical slot, nonzero genesis and bounded context', () => {
  const value = fixture(); value.trace.network_id = 'solana-mainnet';
  for (const item of value.trace.chain_freshness!.sources) item.source = { kind: 'SOLANA_ESTIMATED_BLOCK_TIME', slot: '1234567', genesis_hash: 'So11111111111111111111111111111111111111112', account_context: 'fixture-finalized-accounts' };
  // Canonical synthetic 32-byte genesis identity; no provider qualification.
  assert.equal(parseDecision(value).trace.chain_freshness!.sources[0].source.kind, 'SOLANA_ESTIMATED_BLOCK_TIME');
  for (const context of ['x'.repeat(257), 'bad\ncontext', 'é'.repeat(129)]) { const changed = structuredClone(value); const source = changed.trace.chain_freshness!.sources[0].source; if (source.kind === 'SOLANA_ESTIMATED_BLOCK_TIME') source.account_context = context; assert.throws(() => parseDecision(changed)); }
  for (const genesis of ['1'.repeat(32), 'not-a-chain', '0'.repeat(32)]) { const changed = structuredClone(value); const source = changed.trace.chain_freshness!.sources[0].source; if (source.kind === 'SOLANA_ESTIMATED_BLOCK_TIME') source.genesis_hash = genesis; assert.throws(() => parseDecision(changed)); }
});
test('legacy decisions stay unmeasured and existing cost and frozen export hashes remain unchanged', async () => {
  const cost = JSON.parse(readFileSync(new URL('../../../specs/cost-assessment.example.json', import.meta.url), 'utf8'));
  const source = parseDecision(cost.source_decision); assert.equal(Object.hasOwn(source.trace, 'chain_freshness'), false);
  assert.deepEqual(source, cost.source_decision); requireCostDecision(await verifyCostAssessment(parseCostAssessment(cost.record)), source);
  assert.throws(() => parseDecision({ ...cost.source_decision, trace: { ...cost.source_decision.trace, chain_freshness: null } }));
  const legacy = JSON.parse(readFileSync(new URL('../../../specs/research-export.example.json', import.meta.url), 'utf8'));
  assert.equal((await verifyFrozenExport(parseFrozenExport(legacy))).content_sha256, legacy.content_sha256);
  const bundle = frozenExportFixture(), current = fixture();
  current.trace.session_id = bundle.data.session.session_id; current.trace.configuration_digest = bundle.data.session.configuration_digest; current.trace.experiment_id = bundle.data.experiment_id;
  const original = bundle.data.decisions[0].trace.capture_refs;
  current.trace.capture_refs = original; current.trace.chain_freshness!.sources.forEach((item, i) => item.capture_id = original[i].capture_id);
  bundle.data.decisions = [current]; bundle.snapshot.source_counts.decisions = '1'; bundle.data.decision_coverage.raw_observations = '1'; bundle.data.collection_attempts = []; bundle.snapshot.source_counts.collection_attempts = '0';
  assert.deepEqual((await verifyFrozenExport(parseFrozenExport(sealExport(bundle)))).data.decisions[0].trace.chain_freshness, current.trace.chain_freshness);
});
