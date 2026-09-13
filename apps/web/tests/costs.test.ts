import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { ControlApi } from '../src/api/client.ts';
import { canonicalCostJson, parseCostAssessment, parseCostScenario, requireCostDecision, verifyCostAssessment } from '../src/api/costs.ts';
import { parseDecision } from '../src/api/research.ts';
import { frozenCsv, frozenJson } from '../src/domain/frozenExport.ts';
import { parseFrozenExport, verifyFrozenExport } from '../src/api/frozenExport.ts';
import { costAssessmentFixture, costScenarioFixture } from './costs.fixture.ts';
import { decision, huge } from './research.fixture.ts';
import { frozenExportFixture, sealExport } from './exports.fixture.ts';

test('manual costs preserve missing, explicit zero, negative net, huge units and unallocated overhead', () => {
  const missing = parseCostAssessment(costAssessmentFixture(costScenarioFixture(false)));
  assert.equal(missing.assessment.report.transaction_net, null);
  const zero = parseCostAssessment(costAssessmentFixture());
  assert.equal(zero.assessment.report.transaction_net, '-10'); assert.equal(zero.assessment.report.fully_allocated_net, null);
  const scenario = costScenarioFixture(); const other = scenario.expenses.find(e => e.kind === 'OTHER')!;
  assert.equal(other.amount.status, 'KNOWN'); if (other.amount.status === 'KNOWN') other.amount.amount = huge;
  scenario.overhead = { status: 'ALLOCATED', amount_in_start_asset: '2', method: 'per-observation', version: '1', reference: 'fixture-note' };
  const exact = parseCostAssessment(costAssessmentFixture(scenario));
  assert.equal(exact.assessment.report.transaction_net, (-10n - BigInt(huge)).toString());
  assert.equal(exact.assessment.report.fully_allocated_net, (-12n - BigInt(huge)).toString());
  assert.equal(exact.assessment.evidence, 'CANDIDATE'); assert.equal(exact.assessment.scenario.origin, 'MANUALLY_CONSTRUCTED');
});
test('cost contracts reject false completeness, quote replacement, wrong currencies and hidden fields', () => {
  const item = costAssessmentFixture(); requireCostDecision(item, parseDecision(decision()));
  assert.throws(() => requireCostDecision(item, parseDecision(decision('RECORDED_LIVE'))), /wrong decision scope/);
  const forged = structuredClone(item); forged.assessment.report.transaction_net = '10'; assert.throws(() => parseCostAssessment(forged));
  const missing = costAssessmentFixture(costScenarioFixture(false)); missing.assessment.report.transaction_net = '0'; assert.throws(() => parseCostAssessment(missing));
  const omitted = structuredClone(item); omitted.assessment.report.expenses.pop(); assert.throws(() => parseCostAssessment(omitted), /omits an applicable component/);
  assert.throws(() => parseCostAssessment({ ...item, provider_secret: 'do-not-retain' }), /unsupported fields/);
  assert.throws(() => parseCostScenario({ ...costScenarioFixture(), provenance_reference: 'https://provider.invalid?key=secret' }, 'base-mainnet'));
  assert.throws(() => parseCostScenario({ ...costScenarioFixture(), fee_composition: 'SOLANA_BASE_EXCLUDES_PRIORITY' }, 'base-mainnet'));
  const stale = structuredClone(item); const expense = stale.assessment.report.expenses[0].expense.amount;
  if (expense.status === 'KNOWN' && expense.valuation.kind !== 'MISSING') expense.valuation.valued_at_unix_ms++;
  assert.throws(() => parseCostAssessment(stale));
});
test('cost API preserves auth and retry identity, rejects wrong source and exposes bounded scoped history', async () => {
  const calls: { path: string; init: RequestInit }[] = [], item = costAssessmentFixture(), body = { observation_id: 'observation-fixture', scenario: item.assessment.scenario };
  const api = new ControlApi(async (path, init) => { calls.push({ path: String(path), init: init! });
    if (String(path).endsWith('/auth/session')) return Response.json({ operator_id: 'operator', csrf_token: 'cost-csrf', expires_at: '2000000000' });
    if (String(path).includes('?')) return Response.json({ items: [item], next_cursor: null }); return Response.json(item); });
  await api.auth(); await api.createCostAssessment('session-paper', body, 'same-cost-key'); await api.createCostAssessment('session-paper', body, 'same-cost-key');
  assert.equal(calls[1].init.body, calls[2].init.body); assert.equal((calls[1].init.headers as Record<string, string>)['Idempotency-Key'], 'same-cost-key');
  assert.equal((calls[1].init.headers as Record<string, string>)['X-CSRF-Token'], 'cost-csrf');
  await api.costAssessments('session-paper', 'cursor/with?chars'); assert.equal(calls[3].path, '/v1/sessions/session-paper/cost-assessments?limit=25&cursor=cursor%2Fwith%3Fchars');
  await api.costAssessment('session-paper', item.record_id);
  await assert.rejects(api.costAssessments('other'), /wrong session scope/);
  await assert.rejects(api.createCostAssessment('session-paper', { ...body, observation_id: 'other' }, 'key'), /wrong decision scope/);
  await assert.rejects(api.createCostAssessment('session-paper', { ...body, scenario: { ...body.scenario, version: '2' } }, 'key'), /submitted scenario/);
});
test('sixth frozen dataset retains cost scenario and negative exact net in lossless JSON and CSV', async () => {
  const bundle = frozenExportFixture(); bundle.data.cost_assessments = [costAssessmentFixture()]; bundle.snapshot.source_counts.cost_assessments = '1';
  const frozen = await verifyFrozenExport(parseFrozenExport(sealExport(bundle)));
  assert.equal(JSON.parse(frozenJson(frozen)).data.cost_assessments[0].assessment.report.transaction_net, '-10');
  assert.match(frozenCsv(frozen), /"COST_ASSESSMENT"/); assert.equal(frozen.snapshot.source_counts.cost_assessments, '1');
  const changed = structuredClone(bundle); changed.data.cost_assessments[0].assessment.binding.observation_id = 'missing';
  assert.throws(() => parseFrozenExport(sealExport(changed)), /omits a cost assessment source decision/);
  const falseCount = structuredClone(bundle); falseCount.snapshot.source_counts.cost_assessments = '0'; assert.throws(() => parseFrozenExport(falseCount), /source counts/);
  assert.equal(canonicalCostJson(bundle.data.cost_assessments[0].assessment.scenario), canonicalCostJson(frozen.data.cost_assessments[0].assessment.scenario));
});

test('independent Rust-replayed cost fixture verifies in the shipped TypeScript parser and SHA-256 checker', async () => {
  const fixture = JSON.parse(readFileSync(new URL('../../../specs/cost-assessment.example.json', import.meta.url), 'utf8'));
  const item = await verifyCostAssessment(parseCostAssessment(fixture.record));
  requireCostDecision(item, parseDecision(fixture.source_decision));
  assert.equal(item.assessment.report.transaction_net, '-2001'); assert.equal(item.assessment.report.fully_allocated_net, '-2501');
  await assert.rejects(verifyCostAssessment({ ...item, assessment: { ...item.assessment, scenario_digest: 'sha256:' + '0'.repeat(64) } }), /digest/);
});
