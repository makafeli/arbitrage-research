import { createHash } from 'node:crypto';
import type { CostScenario, StoredCostAssessment } from '../src/api/costs.ts';
import { canonicalCostJson, expenseKinds } from '../src/api/costs.ts';
import { decision, principalAsset } from './research.fixture.ts';

// Synthetic transport fixtures only, never market valuations or fee estimates.
export function costScenarioFixture(complete = true): CostScenario {
  return { schema_version: '1.0.0', scenario_id: 'manual-fixture', version: '1', origin: 'MANUALLY_CONSTRUCTED', provenance_reference: 'fixture-note', valuation_max_age_ms: 60000,
    fee_composition: 'BASE_EXECUTION_INCLUDES_PRIORITY', expenses: expenseKinds.filter(kind => kind !== 'PRIORITY_FEE').map(kind => ({ kind, amount: complete
      ? { status: 'KNOWN', asset: ['FUNDING', 'OTHER'].includes(kind) ? principalAsset : { kind: 'NATIVE', identity: 'base-mainnet' }, amount: '0',
        valuation: ['FUNDING', 'OTHER'].includes(kind) ? { kind: 'SAME_ASSET', reference: 'fixture-note', valued_at_unix_ms: decision().trace.observed_at_unix_ms }
          : { kind: 'RATIO', numerator: '2', denominator: '1', reference: 'fixture-note', valued_at_unix_ms: decision().trace.observed_at_unix_ms } }
      : { status: 'MISSING', reason: 'MANUAL_INPUT_NOT_PROVIDED' } })),
    funding: complete ? { status: 'OWN_VIRTUAL_CAPITAL' } : { status: 'UNKNOWN', reason: 'FUNDING_NOT_ESTABLISHED' }, overhead: { status: 'NOT_ALLOCATED' } };
}
function digest(value: unknown) { return 'sha256:' + createHash('sha256').update(canonicalCostJson(value)).digest('hex'); }
export function sealCost(item: StoredCostAssessment): StoredCostAssessment {
  item.assessment.scenario_digest = digest(item.assessment.scenario);
  item.assessment.assessment_id = digest({ ...item.assessment, assessment_id: '' }); return item;
}
export function costAssessmentFixture(scenario = costScenarioFixture(), observation = 'observation-fixture', recordId = '01998d01-2222-7000-8000-000000000001'): StoredCostAssessment {
  const trace = decision('SYNTHETIC', observation).trace;
  const expenses = scenario.expenses.map(expense => { const a = expense.amount; const valued = a.status === 'MISSING' || a.valuation.kind === 'MISSING' ? null
    : a.valuation.kind === 'SAME_ASSET' ? a.amount : ((BigInt(a.amount) * BigInt(a.valuation.numerator) + BigInt(a.valuation.denominator) - 1n) / BigInt(a.valuation.denominator)).toString();
    return { expense, in_start_asset: valued }; });
  const incomplete_reasons = expenses.filter(e => e.in_start_asset === null).map(e => e.expense.kind + ': manual input unavailable');
  if (scenario.funding.status === 'UNKNOWN') incomplete_reasons.push('funding assumption: FUNDING_NOT_ESTABLISHED');
  const net = incomplete_reasons.length ? null : (-10n - expenses.reduce((sum, e) => sum + BigInt(e.in_start_asset!), 0n)).toString();
  return sealCost({ record_id: recordId, recorded_at: '2026-09-12T12:03:00Z', assessment: { schema_version: '1.0.0', calculation_version: 'decision-bound-exact-costs-v1',
    assessment_id: '', scenario_digest: '', scenario, binding: { observation_id: observation, decision_digest: digest(trace), session_id: trace.session_id,
      experiment_id: trace.experiment_id, generation: trace.generation, configuration_digest: trace.configuration_digest, decision_calculation_version: trace.calculation_version,
      dataset_origin: 'SYNTHETIC', network_id: 'base-mainnet', starting_asset: principalAsset.identity, amount_in_minor: '1000', quoted_output_minor: '990', observed_at_unix_ms: trace.observed_at_unix_ms },
    report: { starting_asset: principalAsset.identity, gross_after_quote_included_costs: '-10', transaction_net: net,
      fully_allocated_net: net !== null && scenario.overhead.status === 'ALLOCATED' ? (BigInt(net) - BigInt(scenario.overhead.amount_in_start_asset)).toString() : null,
      expenses, overhead: scenario.overhead, incomplete_reasons }, evidence: 'CANDIDATE' } });
}
