import { useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { ApiError, errorMessage } from '../api/client';
import type { ControlApi } from '../api/client';
import { expenseKinds, machineLabel, parseCostScenario, requireCostDecision } from '../api/costs';
import type { CostAssessmentRequest, CostScenario, Expense, ExpenseKind, StoredCostAssessment } from '../api/costs';
import type { StoredDecision } from '../api/research';
import { unsigned } from '../api/research';
import { useResearchPagination, useResearchResource } from '../hooks/useResearchResource';
import { EmptyResearch, Exact, ExportButton, OriginBadge, Pagination, ResourceStatus } from './ResearchShared';

const names: Record<ExpenseKind, string> = { NETWORK_EXECUTION: 'Network execution', BASE_L1_DATA: 'Base L1 data', PRIORITY_FEE: 'Solana priority fee', RELAY_TIP: 'Relay tip', FUNDING: 'Funding cost', ACCOUNT_SETUP: 'Account setup', OTHER: 'Other transaction costs' };
const nativeKinds: ExpenseKind[] = ['NETWORK_EXECUTION', 'BASE_L1_DATA', 'PRIORITY_FEE', 'RELAY_TIP', 'ACCOUNT_SETUP'];
interface Entry { known: boolean; amount: string }
interface Pending { sessionId: string; key: string; body: CostAssessmentRequest }
export function CostAssessmentWorkspace({ api, source, active, disabled, canRetry, onPendingChange }: {
  api: ControlApi; source: StoredDecision; active: boolean; disabled: boolean; canRetry: boolean; onPendingChange: (value: boolean) => void;
}) {
  const t = source.trace, scope = t.session_id + ':' + t.observation_id;
  const [scenarioId, setScenarioId] = useState(''), [version, setVersion] = useState('1'), [reference, setReference] = useState('');
  const [entries, setEntries] = useState<Partial<Record<ExpenseKind, Entry>>>({});
  const [numerator, setNumerator] = useState(''), [denominator, setDenominator] = useState('');
  const [valuationAt, setValuationAt] = useState(String(t.observed_at_unix_ms)), [maxAge, setMaxAge] = useState('60000');
  const [funding, setFunding] = useState<'UNKNOWN' | 'OWN_VIRTUAL_CAPITAL'>('UNKNOWN');
  const [overhead, setOverhead] = useState<'NOT_ALLOCATED' | 'UNKNOWN' | 'ALLOCATED'>('NOT_ALLOCATED');
  const [allocation, setAllocation] = useState(''), [allocationMethod, setAllocationMethod] = useState('');
  const [reviewed, setReviewed] = useState(false), [pending, setPending] = useState<Pending | null>(null);
  const [created, setCreated] = useState<StoredCostAssessment | null>(null), [error, setError] = useState(''), [sending, setSending] = useState(false);
  const sendingRef = useRef(false);
  const pages = useResearchPagination(scope);
  const history = useResearchResource(active, 'costs:' + scope + ':' + (pages.cursor ?? ''), async signal => {
    const page = await api.costAssessments(t.session_id, pages.cursor, signal);
    page.items.filter(item => item.assessment.binding.observation_id === t.observation_id).forEach(item => requireCostDecision(item, source)); return page;
  });
  const rows = history.data?.items.filter(item => item.assessment.binding.observation_id === t.observation_id) ?? [];
  const kinds = expenseKinds.filter(kind => kind !== (t.network_id === 'base-mainnet' ? 'PRIORITY_FEE' : 'BASE_L1_DATA'));
  const needsNative = kinds.some(kind => nativeKinds.includes(kind) && entries[kind]?.known);
  const ratioProvided = numerator !== '' || denominator !== '';
  const positive = (value: string) => unsigned(value) && BigInt(value) > 0n;
  const validRatio = positive(numerator) && positive(denominator);
  const time = Number(valuationAt), age = Number(maxAge);
  const validTime = /^[1-9][0-9]*$/.test(valuationAt) && Number.isSafeInteger(time) && time <= t.observed_at_unix_ms && t.observed_at_unix_ms - time <= age;
  const referenceValid = machineLabel(reference, 128) && (!reference.startsWith('sha256:') || /^sha256:[a-f0-9]{64}$/.test(reference));
  const valid = t.result.status === 'QUOTED' && machineLabel(scenarioId) && machineLabel(version) && referenceValid
    && /^[1-9][0-9]*$/.test(maxAge) && Number.isSafeInteger(age) && age <= 86400000 && validTime
    && kinds.every(kind => !entries[kind]?.known || unsigned(entries[kind]!.amount))
    && (!needsNative || !ratioProvided || validRatio) && (overhead !== 'ALLOCATED' || unsigned(allocation) && machineLabel(allocationMethod)) && reviewed;
  function edited(change: () => void) { change(); setReviewed(false); }
  function update(kind: ExpenseKind, update: Partial<Entry>) { edited(() => setEntries(previous => ({ ...previous, [kind]: { known: false, amount: '', ...previous[kind], ...update } }))); }
  function scenario(): CostScenario {
    const expenses: Expense[] = kinds.map(kind => {
      const entry = entries[kind];
      if (!entry?.known) return { kind, amount: { status: 'MISSING', reason: 'MANUAL_INPUT_NOT_PROVIDED' } };
      const native = nativeKinds.includes(kind);
      return { kind, amount: { status: 'KNOWN', asset: native ? { kind: 'NATIVE', identity: t.network_id } : { kind: 'TOKEN', identity: t.route[0].asset_in }, amount: entry.amount,
        valuation: native ? validRatio ? { kind: 'RATIO', numerator, denominator, reference, valued_at_unix_ms: time } : { kind: 'MISSING', reason: 'MANUAL_VALUATION_NOT_PROVIDED' }
          : { kind: 'SAME_ASSET', reference, valued_at_unix_ms: time } } };
    });
    return parseCostScenario({ schema_version: '1.0.0', scenario_id: scenarioId, version, origin: 'MANUALLY_CONSTRUCTED', provenance_reference: reference,
      valuation_max_age_ms: age, fee_composition: t.network_id === 'base-mainnet' ? 'BASE_EXECUTION_INCLUDES_PRIORITY' : 'SOLANA_BASE_EXCLUDES_PRIORITY', expenses,
      funding: funding === 'UNKNOWN' ? { status: 'UNKNOWN', reason: 'FUNDING_NOT_ESTABLISHED' } : { status: 'OWN_VIRTUAL_CAPITAL' },
      overhead: overhead === 'ALLOCATED' ? { status: 'ALLOCATED', amount_in_start_asset: allocation, method: allocationMethod, version, reference } : overhead === 'UNKNOWN' ? { status: 'UNKNOWN', reason: 'ALLOCATION_NOT_ESTABLISHED' } : { status: 'NOT_ALLOCATED' },
    }, t.network_id);
  }
  async function submit(event: FormEvent) {
    event.preventDefault(); if (sendingRef.current || (pending ? !canRetry : disabled || !active || !valid || created !== null)) return;
    const uncertain = pending !== null;
    const request = pending ?? { sessionId: t.session_id, key: crypto.randomUUID(), body: { observation_id: t.observation_id, scenario: scenario() } };
    sendingRef.current = true; setSending(true); setError(''); setPending(request); onPendingChange(true);
    try {
      const result = await api.createCostAssessment(request.sessionId, request.body, request.key); requireCostDecision(result, source);
      setCreated(result); setPending(null); onPendingChange(false); history.refresh();
    } catch (e) {
      const rejected = e instanceof ApiError && [400, 401, 403, 404, 409, 422, 429].includes(e.status);
      setError(!uncertain && rejected ? errorMessage(e) : 'Assessment delivery is uncertain. Retry the identical request to recover its durable result.');
      if (!uncertain && rejected) { setPending(null); onPendingChange(false); }
    } finally { setSending(false); sendingRef.current = false; }
  }
  return <section className="panel space-top cost-workspace" aria-labelledby="cost-workspace-title"><div className="sectionhead"><div><h3 id="cost-workspace-title">Hypothetical cost research</h3><p>Apply explicit manual assumptions to one retained gross quote.</p></div><span className="pill amber">MANUAL ASSUMPTIONS</span></div>
    <div className="notice"><p>Decision: <strong className="mono">{t.observation_id}</strong> · {t.network_id}</p><p>Start asset: <span className="mono">{t.route[0]?.asset_in}</span></p><p>Historical gross delta: <Exact value={t.result.status === 'QUOTED' ? t.result.gross_delta_minor : null} /> start-asset base units.</p><OriginBadge origin={t.dataset_origin} /><p>Source origin and manual scenario origin stay separate. Every result remains a hypothetical candidate. Saving an assessment creates no paper bookings and changes no quote.</p></div>
    <form className="connected-form cost-form" onSubmit={event => { void submit(event); }}><fieldset disabled={disabled || !active || sending || Boolean(pending) || Boolean(created)}>
      <div className="cost-field-grid"><div className="research-field"><label htmlFor="cost-scenario">Scenario identifier</label><input id="cost-scenario" value={scenarioId} maxLength={64} autoComplete="off" placeholder="e.g. conservative-fees" onChange={event => edited(() => setScenarioId(event.target.value))} required /></div><div className="research-field"><label htmlFor="cost-version">Scenario version</label><input id="cost-version" value={version} maxLength={64} onChange={event => edited(() => setVersion(event.target.value))} required /></div></div>
      <label htmlFor="cost-reference">Manual assumption reference</label><input id="cost-reference" value={reference} maxLength={128} autoComplete="off" placeholder="e.g. research-note-2026-09" onChange={event => edited(() => setReference(event.target.value))} required /><p className="tiny">Use an identifier or SHA-256 digest, starting with a letter or number. Letters, numbers, dots, colons, underscores and hyphens only. Never paste provider URLs or secrets.</p>
      <h4>Transaction cost components</h4><p className="tiny">All amounts are exact integer base units. Blank amounts are not zero. Choose “Known manual amount” and enter 0 to declare zero explicitly.</p><p className="notice">{t.network_id === 'base-mainnet' ? 'Base execution includes its priority part. L1 data is separate; priority must not be subtracted again.' : 'Solana base execution and priority fees are separate. Base L1 data does not apply.'} Pool fees and price impact are already in the gross quote.</p>
      <div className="cost-component-list">{kinds.map(kind => <div className="cost-component" key={kind}><div><strong>{names[kind]}</strong><p className="tiny">{nativeKinds.includes(kind) ? 'Native base units · ' + t.network_id : 'Start-asset base units'}</p></div><div className="research-field"><label htmlFor={'cost-status-' + kind}>{names[kind]} input</label><select id={'cost-status-' + kind} value={entries[kind]?.known ? 'KNOWN' : 'MISSING'} onChange={event => update(kind, { known: event.target.value === 'KNOWN' })}><option value="MISSING">Missing · unknown</option><option value="KNOWN">Known manual amount</option></select>{entries[kind]?.known && <><label htmlFor={'cost-amount-' + kind}>{names[kind]} base units</label><input id={'cost-amount-' + kind} inputMode="numeric" pattern="[0-9]+" maxLength={78} value={entries[kind]?.amount ?? ''} onChange={event => update(kind, { amount: event.target.value })} autoComplete="off" required /></>}</div></div>)}</div>
      <details className="cost-valuation" open={needsNative || undefined}><summary>Native fee valuation · exact manual ratio</summary><p className="tiny">Convert native base units into start-asset base units. Supply both sides of the ratio; decimals must already be accounted for. Costs round upward. Leaving the ratio empty keeps native fee valuation unknown, including when a native amount is zero.</p><div className="cost-field-grid"><div className="research-field"><label htmlFor="cost-numerator">Start-asset base units · numerator</label><input id="cost-numerator" inputMode="numeric" value={numerator} maxLength={78} onChange={event => edited(() => setNumerator(event.target.value))} /></div><div className="research-field"><label htmlFor="cost-denominator">Native base units · denominator</label><input id="cost-denominator" inputMode="numeric" value={denominator} maxLength={78} onChange={event => edited(() => setDenominator(event.target.value))} /></div></div></details>
      <div className="cost-field-grid"><div className="research-field"><label htmlFor="cost-valued-at">Valuation as of · Unix milliseconds</label><input id="cost-valued-at" inputMode="numeric" value={valuationAt} maxLength={16} onChange={event => edited(() => setValuationAt(event.target.value))} /></div><div className="research-field"><label htmlFor="cost-max-age">Maximum historical valuation age · ms</label><input id="cost-max-age" inputMode="numeric" value={maxAge} maxLength={8} onChange={event => edited(() => setMaxAge(event.target.value))} /></div></div><p className="tiny">Initial as-of value equals this historical decision’s timestamp ({new Date(t.observed_at_unix_ms).toISOString()}); it is a manual assumption, not a retrieved valuation. Future valuations are rejected. Maximum age is 1–86,400,000 ms.</p>
      <label htmlFor="cost-funding">Funding assumption</label><select id="cost-funding" value={funding} onChange={event => edited(() => setFunding(event.target.value as typeof funding))}><option value="UNKNOWN">Unknown · net remains unknown</option><option value="OWN_VIRTUAL_CAPITAL">Own virtual capital · hypothetical</option></select>
      <label htmlFor="cost-overhead">Shared operating overhead</label><select id="cost-overhead" value={overhead} onChange={event => edited(() => setOverhead(event.target.value as typeof overhead))}><option value="NOT_ALLOCATED">Not allocated · fully allocated net unknown</option><option value="UNKNOWN">Unknown allocation</option><option value="ALLOCATED">Explicit allocation in start-asset units</option></select>
      {overhead === 'ALLOCATED' && <><label htmlFor="cost-allocation">Overhead allocation · start-asset base units</label><input id="cost-allocation" inputMode="numeric" value={allocation} maxLength={78} onChange={event => edited(() => setAllocation(event.target.value))} required /><label htmlFor="cost-allocation-method">Allocation method identifier</label><input id="cost-allocation-method" value={allocationMethod} maxLength={64} onChange={event => edited(() => setAllocationMethod(event.target.value))} required /><p className="tiny">Allocation uses the scenario version and assumption reference above.</p></>}
      <label className="checkbox-label"><input type="checkbox" checked={reviewed} onChange={event => setReviewed(event.target.checked)} />I reviewed the exact units, historical valuation and hypothetical assumptions.</label>
    </fieldset>{error && <p className="notice error-notice" role="alert">{error}</p>}<button className="primary" disabled={sending || Boolean(created) || (pending ? !canRetry : disabled || !active || !valid)}>{created ? 'Cost assessment saved' : sending ? 'Saving cost assessment…' : pending ? 'Retry identical cost assessment' : 'Save hypothetical cost assessment'}</button></form>
    {pending && <p className="notice" role="status">Assessment scope is locked until delivery is resolved. Retries retain the original session, observation, assumptions and idempotency key.</p>}
    {created && <><div className="notice" role="status">Saved immutable assessment {created.record_id}. Source quote and paper balances are unchanged.</div><AssessmentResult item={created} /><button className="space-top" onClick={() => { setCreated(null); setReviewed(false); setVersion(''); }}>Prepare another cost scenario</button></>}
    <div className="sectionhead space-top"><div><h3>Retained cost assessments</h3><p>Showing this observation within the loaded session page. Other pages can contain further scenarios.</p></div><button disabled={!active || history.loading} onClick={history.refresh}>Refresh cost history</button></div><ResourceStatus resource={history} />
    {history.data && !rows.length && <EmptyResearch>No assessments for this observation on the loaded page. Missing history does not imply zero costs.</EmptyResearch>}
    {rows.map(item => <details key={item.record_id} className="cost-history space-top"><summary>{item.assessment.scenario.scenario_id} · v{item.assessment.scenario.version} · {item.recorded_at}</summary><AssessmentResult item={item} /><ExportButton label={'Export cost assessment ' + item.record_id} scope={'cost-assessment-' + item.record_id} data={item} receivedAt={history.at} /></details>)}
    <Pagination label="cost assessments" page={pages.page} canPrevious={pages.canPrevious} canNext={pages.canNext(history.data?.next_cursor)} loading={history.loading} previous={pages.previous} next={() => { if (history.data?.next_cursor) pages.next(history.data.next_cursor); }} />
  </section>;
}
function AssessmentResult({ item }: { item: StoredCostAssessment }) {
  const a = item.assessment, r = a.report;
  return <section className="space-top" aria-label={'Cost result ' + item.record_id}><span className="pill amber">HYPOTHETICAL · CANDIDATE</span><dl className="research-facts"><dt>Gross after quote-included costs</dt><dd><Exact value={r.gross_after_quote_included_costs} /></dd><dt>Net after transaction costs</dt><dd><Exact value={r.transaction_net} /></dd><dt>Net after allocated overhead</dt><dd><Exact value={r.fully_allocated_net} /></dd><dt>Amount unit</dt><dd>{r.starting_asset} · base units</dd><dt>Assumptions</dt><dd>{a.scenario.scenario_id} · {a.scenario.version} · MANUALLY_CONSTRUCTED</dd><dt>Funding / overhead</dt><dd>{a.scenario.funding.status} / {r.overhead.status}</dd><dt>Maximum historical valuation age</dt><dd>{a.scenario.valuation_max_age_ms} ms</dd><dt>Calculation version</dt><dd>{a.calculation_version}</dd></dl>
    {r.incomplete_reasons.length > 0 && <div className="notice"><strong>Net remains unknown</strong>{r.incomplete_reasons.map((reason, index) => <p key={index}>{reason}</p>)}</div>}
    <div className="research-scroll"><table className="research-table"><caption>Retained cost assumptions and exact valuation</caption><thead><tr><th>Component</th><th>Declared amount / asset</th><th>In start-asset base units</th></tr></thead><tbody>{r.expenses.map(entry => <tr key={entry.expense.kind}><td>{names[entry.expense.kind]}</td><td>{entry.expense.amount.status === 'KNOWN' ? <><Exact value={entry.expense.amount.amount} /><span className="route-sub">{entry.expense.amount.asset.kind} · {entry.expense.amount.asset.identity}</span><span className="route-sub">Valuation: {entry.expense.amount.valuation.kind}</span>{entry.expense.amount.valuation.kind !== 'MISSING' && <><span className="route-sub">{entry.expense.amount.valuation.kind === 'RATIO' ? entry.expense.amount.valuation.numerator + ' start units / ' + entry.expense.amount.valuation.denominator + ' fee units · rounded up' : 'Identical start asset · no conversion'}</span><span className="route-sub">As of {entry.expense.amount.valuation.valued_at_unix_ms} ms · {entry.expense.amount.valuation.reference}</span></>}</> : 'Missing · ' + entry.expense.amount.reason}</td><td><Exact value={entry.in_start_asset} /></td></tr>)}</tbody></table></div><p className="notice">A positive hypothetical net is not a simulated fill, execution eligibility or realized profit. Negative values are retained. Unallocated overhead remains unknown.</p>
    <details><summary>Immutable assessment references</summary><p className="mono">Assessment: {a.assessment_id}</p><p className="mono">Scenario: {a.scenario_digest}</p><p className="mono">Source decision: {a.binding.decision_digest}</p><p className="mono">Configuration: {a.binding.configuration_digest}</p><p>Assumption reference: {a.scenario.provenance_reference}</p></details></section>;
}
