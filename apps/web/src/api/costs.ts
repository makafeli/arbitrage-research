import type { AccountingAsset, StoredDecision } from './research.ts';
import { parseAsset, record, requireValue, text, unsigned } from './research.ts';

export const expenseKinds = ['NETWORK_EXECUTION', 'BASE_L1_DATA', 'PRIORITY_FEE', 'RELAY_TIP', 'FUNDING', 'ACCOUNT_SETUP', 'OTHER'] as const;
export type ExpenseKind = typeof expenseKinds[number];
export type Valuation = { kind: 'SAME_ASSET'; reference: string; valued_at_unix_ms: number }
  | { kind: 'RATIO'; numerator: string; denominator: string; reference: string; valued_at_unix_ms: number }
  | { kind: 'MISSING'; reason: string };
export type ExpenseAmount = { status: 'KNOWN'; asset: AccountingAsset; amount: string; valuation: Valuation } | { status: 'MISSING'; reason: string };
export interface Expense { kind: ExpenseKind; amount: ExpenseAmount }
export type Funding = { status: 'OWN_VIRTUAL_CAPITAL' } | { status: 'UNKNOWN'; reason: string };
export type Overhead = { status: 'NOT_ALLOCATED' } | { status: 'UNKNOWN'; reason: string }
  | { status: 'ALLOCATED'; amount_in_start_asset: string; method: string; version: string; reference: string };
export interface CostScenario {
  schema_version: '1.0.0'; scenario_id: string; version: string; origin: 'MANUALLY_CONSTRUCTED'; provenance_reference: string;
  valuation_max_age_ms: number; fee_composition: 'BASE_EXECUTION_INCLUDES_PRIORITY' | 'SOLANA_BASE_EXCLUDES_PRIORITY';
  expenses: Expense[]; funding: Funding; overhead: Overhead;
}
export interface CostAssessmentRequest { observation_id: string; scenario: CostScenario }
export function machineLabel(value: unknown, bound = 64): value is string { return typeof value === 'string' && value.length > 0 && value.length <= bound && /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value); }
function canonicalSigned(value: unknown): value is string { return typeof value === 'string' && /^(0|-?[1-9][0-9]{0,77})$/.test(value) && BigInt(value) > -(1n << 256n) && BigInt(value) < (1n << 256n); }
function instant(value: unknown): value is number { return Number.isSafeInteger(value) && (value as number) >= 0 && (value as number) <= 8640000000000000; }
function only(value: Record<string, unknown>, keys: string[]) { requireValue(Object.keys(value).every(key => keys.includes(key)), 'Cost response contains unsupported fields.'); }
function reference(value: unknown): value is string { return machineLabel(value, 128) && (!value.startsWith('sha256:') || /^sha256:[a-f0-9]{64}$/.test(value)); }
function reason(value: unknown): string { requireValue(machineLabel(value, 128)); return value; }
function parseValuation(value: unknown): Valuation {
  const v = record(value);
  if (v.kind === 'MISSING') { only(v, ['kind', 'reason']); return { kind: 'MISSING', reason: reason(v.reason) }; }
  requireValue((v.kind === 'SAME_ASSET' || v.kind === 'RATIO') && reference(v.reference) && instant(v.valued_at_unix_ms));
  if (v.kind === 'SAME_ASSET') { only(v, ['kind', 'reference', 'valued_at_unix_ms']); return { kind: 'SAME_ASSET', reference: v.reference, valued_at_unix_ms: v.valued_at_unix_ms }; }
  only(v, ['kind', 'numerator', 'denominator', 'reference', 'valued_at_unix_ms']);
  requireValue(unsigned(v.numerator) && BigInt(v.numerator) > 0n && unsigned(v.denominator) && BigInt(v.denominator) > 0n);
  return { kind: 'RATIO', numerator: v.numerator, denominator: v.denominator, reference: v.reference, valued_at_unix_ms: v.valued_at_unix_ms };
}
function parseExpense(value: unknown, network: string): Expense {
  const v = record(value), a = record(v.amount); only(v, ['kind', 'amount']); requireValue(expenseKinds.includes(v.kind as ExpenseKind));
  if (a.status === 'MISSING') { only(a, ['status', 'reason']); return { kind: v.kind as ExpenseKind, amount: { status: 'MISSING', reason: reason(a.reason) } }; }
  only(a, ['status', 'asset', 'amount', 'valuation']); requireValue(a.status === 'KNOWN' && unsigned(a.amount));
  return { kind: v.kind as ExpenseKind, amount: { status: 'KNOWN', asset: parseAsset(a.asset, network), amount: a.amount, valuation: parseValuation(a.valuation) } };
}
function parseOverhead(value: unknown): Overhead {
  const v = record(value);
  if (v.status === 'NOT_ALLOCATED') { only(v, ['status']); return { status: 'NOT_ALLOCATED' }; }
  if (v.status === 'UNKNOWN') { only(v, ['status', 'reason']); return { status: 'UNKNOWN', reason: reason(v.reason) }; }
  only(v, ['status', 'amount_in_start_asset', 'method', 'version', 'reference']);
  requireValue(v.status === 'ALLOCATED' && unsigned(v.amount_in_start_asset) && machineLabel(v.method) && machineLabel(v.version) && reference(v.reference));
  return { status: 'ALLOCATED', amount_in_start_asset: v.amount_in_start_asset, method: v.method, version: v.version, reference: v.reference };
}
export function parseCostScenario(value: unknown, network: string): CostScenario {
  const v = record(value); only(v, ['schema_version', 'scenario_id', 'version', 'origin', 'provenance_reference', 'valuation_max_age_ms', 'fee_composition', 'expenses', 'funding', 'overhead']);
  requireValue(v.schema_version === '1.0.0' && v.origin === 'MANUALLY_CONSTRUCTED' && machineLabel(v.scenario_id) && machineLabel(v.version) && reference(v.provenance_reference));
  requireValue(Number.isSafeInteger(v.valuation_max_age_ms) && (v.valuation_max_age_ms as number) >= 1 && (v.valuation_max_age_ms as number) <= 86400000);
  requireValue(v.fee_composition === (network === 'base-mainnet' ? 'BASE_EXECUTION_INCLUDES_PRIORITY' : 'SOLANA_BASE_EXCLUDES_PRIORITY'));
  requireValue(Array.isArray(v.expenses) && v.expenses.length <= 7);
  const expenses = v.expenses.map(item => parseExpense(item, network)); requireValue(new Set(expenses.map(item => item.kind)).size === expenses.length);
  for (const expense of expenses) {
    if (expense.kind === (network === 'base-mainnet' ? 'PRIORITY_FEE' : 'BASE_L1_DATA')) requireValue(expense.amount.status === 'KNOWN' && expense.amount.amount === '0');
    if (['NETWORK_EXECUTION', 'BASE_L1_DATA', 'PRIORITY_FEE', 'RELAY_TIP'].includes(expense.kind) && expense.amount.status === 'KNOWN') requireValue(expense.amount.asset.kind === 'NATIVE');
  }
  const f = record(v.funding); requireValue(f.status === 'OWN_VIRTUAL_CAPITAL' || f.status === 'UNKNOWN'); only(f, f.status === 'UNKNOWN' ? ['status', 'reason'] : ['status']);
  const funding: Funding = f.status === 'OWN_VIRTUAL_CAPITAL' ? { status: 'OWN_VIRTUAL_CAPITAL' } : { status: 'UNKNOWN', reason: reason(f.reason) };
  return { schema_version: '1.0.0', scenario_id: v.scenario_id, version: v.version, origin: 'MANUALLY_CONSTRUCTED', provenance_reference: v.provenance_reference,
    valuation_max_age_ms: v.valuation_max_age_ms as number, fee_composition: v.fee_composition as CostScenario['fee_composition'], expenses, funding, overhead: parseOverhead(v.overhead) };
}
export interface CostBinding {
  observation_id: string; decision_digest: string; session_id: string; experiment_id: string; generation: string;
  configuration_digest: string; decision_calculation_version: string; dataset_origin: 'SYNTHETIC' | 'MANUALLY_CONSTRUCTED' | 'RECORDED_LIVE';
  network_id: 'base-mainnet' | 'solana-mainnet'; starting_asset: string; amount_in_minor: string; quoted_output_minor: string; observed_at_unix_ms: number;
}
export interface CostReport {
  starting_asset: string; gross_after_quote_included_costs: string; transaction_net: string | null; fully_allocated_net: string | null;
  expenses: { expense: Expense; in_start_asset: string | null }[]; overhead: Overhead; incomplete_reasons: string[];
}
export interface StoredCostAssessment {
  record_id: string; recorded_at: string; assessment: { schema_version: '1.0.0'; calculation_version: 'decision-bound-exact-costs-v1';
    assessment_id: string; scenario_digest: string; scenario: CostScenario; binding: CostBinding; report: CostReport; evidence: 'CANDIDATE' };
}
function digest(value: unknown): value is string { return typeof value === 'string' && /^sha256:[a-f0-9]{64}$/.test(value); }
export function canonicalCostJson(value: unknown): string {
  function sort(v: unknown): unknown { return Array.isArray(v) ? v.map(sort) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([key, item]) => [key, sort(item)])) : v; }
  return JSON.stringify(sort(value));
}
export function parseCostAssessment(value: unknown): StoredCostAssessment {
  const v = record(value), a = record(v.assessment), b = record(a.binding), r = record(a.report);
  only(v, ['record_id', 'recorded_at', 'assessment']); only(a, ['schema_version', 'calculation_version', 'assessment_id', 'scenario_digest', 'scenario', 'binding', 'report', 'evidence']);
  only(b, ['observation_id', 'decision_digest', 'session_id', 'experiment_id', 'generation', 'configuration_digest', 'decision_calculation_version', 'dataset_origin', 'network_id', 'starting_asset', 'amount_in_minor', 'quoted_output_minor', 'observed_at_unix_ms']);
  only(r, ['starting_asset', 'gross_after_quote_included_costs', 'transaction_net', 'fully_allocated_net', 'expenses', 'overhead', 'incomplete_reasons']);
  requireValue(text(v.record_id) && text(v.recorded_at) && Number.isFinite(Date.parse(v.recorded_at)));
  requireValue(a.schema_version === '1.0.0' && a.calculation_version === 'decision-bound-exact-costs-v1' && a.evidence === 'CANDIDATE' && digest(a.assessment_id) && digest(a.scenario_digest));
  requireValue(['observation_id', 'session_id', 'experiment_id', 'configuration_digest', 'decision_calculation_version', 'starting_asset'].every(k => text(b[k])) && digest(b.decision_digest));
  requireValue(unsigned(b.generation) && unsigned(b.amount_in_minor) && BigInt(b.amount_in_minor) > 0n && unsigned(b.quoted_output_minor) && instant(b.observed_at_unix_ms));
  requireValue(['SYNTHETIC', 'MANUALLY_CONSTRUCTED', 'RECORDED_LIVE'].includes(b.dataset_origin as string) && ['base-mainnet', 'solana-mainnet'].includes(b.network_id as string));
  requireValue((b.starting_asset as string).startsWith(b.network_id + ':'));
  const scenario = parseCostScenario(a.scenario, b.network_id as string);
  requireValue(r.starting_asset === b.starting_asset && canonicalSigned(r.gross_after_quote_included_costs) && r.gross_after_quote_included_costs === (BigInt(b.quoted_output_minor) - BigInt(b.amount_in_minor)).toString());
  requireValue((r.transaction_net === null || canonicalSigned(r.transaction_net)) && (r.fully_allocated_net === null || canonicalSigned(r.fully_allocated_net)));
  requireValue(Array.isArray(r.expenses) && r.expenses.length <= 7 && Array.isArray(r.incomplete_reasons) && r.incomplete_reasons.length <= 16 && r.incomplete_reasons.every(text));
  const expenses = r.expenses.map(value => { const e = record(value); only(e, ['expense', 'in_start_asset']); requireValue(e.in_start_asset === null || unsigned(e.in_start_asset)); return { expense: parseExpense(e.expense, b.network_id as string), in_start_asset: e.in_start_asset as string | null }; });
  requireValue(new Set(expenses.map(e => e.expense.kind)).size === expenses.length);
  for (const e of expenses) {
    const amount = e.expense.amount;
    if (amount.status === 'MISSING' || amount.valuation.kind === 'MISSING') requireValue(e.in_start_asset === null);
    else {
      const valuation = amount.valuation;
      requireValue(valuation.valued_at_unix_ms > 0 && valuation.valued_at_unix_ms <= (b.observed_at_unix_ms as number) && (b.observed_at_unix_ms as number) - valuation.valued_at_unix_ms <= scenario.valuation_max_age_ms);
      if (valuation.kind === 'SAME_ASSET') requireValue(amount.asset.kind === 'TOKEN' && amount.asset.identity === b.starting_asset && e.in_start_asset === amount.amount);
      else { requireValue(amount.asset.kind !== 'TOKEN' || amount.asset.identity !== b.starting_asset, 'Identical asset valuation requires SAME_ASSET.'); requireValue(e.in_start_asset === ((BigInt(amount.amount) * BigInt(valuation.numerator) + BigInt(valuation.denominator) - 1n) / BigInt(valuation.denominator)).toString()); }
    }
  }
  const overhead = parseOverhead(r.overhead);
  requireValue(canonicalCostJson(overhead) === canonicalCostJson(scenario.overhead));
  const incomplete = expenses.some(e => e.in_start_asset === null) || scenario.funding.status === 'UNKNOWN';
  requireValue(incomplete ? r.transaction_net === null && r.incomplete_reasons.length > 0 : r.transaction_net !== null && r.incomplete_reasons.length === 0);
  if (r.transaction_net !== null) requireValue(r.transaction_net === (BigInt(r.gross_after_quote_included_costs as string) - expenses.reduce((sum, e) => sum + BigInt(e.in_start_asset!), 0n)).toString());
  requireValue(r.transaction_net !== null && overhead.status === 'ALLOCATED' ? r.fully_allocated_net === (BigInt(r.transaction_net as string) - BigInt(overhead.amount_in_start_asset)).toString() : r.fully_allocated_net === null);
  const excluded = b.network_id === 'base-mainnet' ? 'PRIORITY_FEE' : 'BASE_L1_DATA';
  requireValue(expenseKinds.filter(k => k !== excluded).every(k => expenses.some(e => e.expense.kind === k)), 'Cost report omits an applicable component.');
  requireValue(scenario.expenses.every(expected => expenses.some(actual => canonicalCostJson(actual.expense) === canonicalCostJson(expected))), 'Cost report does not preserve the scenario.');
  return { record_id: v.record_id, recorded_at: v.recorded_at, assessment: { schema_version: '1.0.0', calculation_version: 'decision-bound-exact-costs-v1', assessment_id: a.assessment_id,
    scenario_digest: a.scenario_digest, scenario, binding: b as unknown as CostBinding, report: { starting_asset: r.starting_asset as string, gross_after_quote_included_costs: r.gross_after_quote_included_costs as string,
      transaction_net: r.transaction_net as string | null, fully_allocated_net: r.fully_allocated_net as string | null, expenses, overhead, incomplete_reasons: r.incomplete_reasons as string[] }, evidence: 'CANDIDATE' } };
}
export function requireCostDecision(assessment: StoredCostAssessment, source: StoredDecision) {
  const b = assessment.assessment.binding, t = source.trace;
  requireValue(t.result.status === 'QUOTED' && b.observation_id === t.observation_id && b.session_id === t.session_id && b.network_id === t.network_id
    && b.experiment_id === t.experiment_id && b.generation === t.generation && b.configuration_digest === t.configuration_digest && b.dataset_origin === t.dataset_origin
    && b.decision_calculation_version === t.calculation_version && b.starting_asset === t.route[0]?.asset_in && b.amount_in_minor === t.amount_in_minor
    && b.quoted_output_minor === t.result.quoted_output_minor && b.observed_at_unix_ms === t.observed_at_unix_ms, 'Cost assessment has the wrong decision scope.');
}
export async function verifyCostAssessment(item: StoredCostAssessment): Promise<StoredCostAssessment> {
  async function sha(value: unknown) { const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonicalCostJson(value))); return 'sha256:' + Array.from(new Uint8Array(bytes), byte => byte.toString(16).padStart(2, '0')).join(''); }
  requireValue(await sha(item.assessment.scenario) === item.assessment.scenario_digest, 'Cost scenario digest does not match its assumptions.');
  requireValue(await sha({ ...item.assessment, assessment_id: '' }) === item.assessment.assessment_id, 'Cost assessment digest does not match its retained result.'); return item;
}
