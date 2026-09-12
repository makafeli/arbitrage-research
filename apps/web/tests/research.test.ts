import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { ControlApi, parseOpportunity } from '../src/api/client.ts';
import { parseCoverage, parseDecision, parseJournal, parsePaperRun, parseReservation } from '../src/api/research.ts';
import { MAX_EXPORT_BYTES, researchExport } from '../src/domain/researchExport.ts';
import { coverage, decision, huge, journal, nativeAsset, paperRun, principalAsset, reservation } from './research.fixture.ts';

test('paper accounting preserves exact large integers and rejects inconsistent or executable balances', () => {
  const parsed = parsePaperRun(paperRun());
  assert.equal(parsed.balances[0].total, huge);
  assert.equal(BigInt(parsed.balances[0].free) + BigInt(parsed.balances[0].reserved), BigInt(huge));
  assert.throws(() => parsePaperRun({ ...paperRun(), balances: [{ asset: principalAsset, free: huge, reserved: '9', total: huge }] }), /free plus reserved/);
  assert.throws(() => parsePaperRun({ ...paperRun(), execution_authorized: true }));
  assert.throws(() => parsePaperRun({ ...paperRun(), balances: [{ asset: { ...principalAsset, identity: 'solana-mainnet:wrong-chain' }, free: '0', reserved: '0', total: '0' }] }));
  assert.equal(parseReservation(reservation).state, 'UNKNOWN');
});
test('persisted synthetic and captured origins stay distinct and negative quotes remain gross candidates', () => {
  const synthetic = parseDecision(decision());
  assert.equal(synthetic.trace.dataset_origin, 'SYNTHETIC');
  assert.equal(synthetic.trace.input_age_ms, 15);
  assert.deepEqual(synthetic.trace.result, { status: 'QUOTED', quoted_output_minor: '990', gross_delta_minor: '-10', included_pool_fees: ['3', '2'] });
  assert.equal(parseDecision(decision('RECORDED_LIVE')).trace.source_kind, 'CAPTURED_MARKET_DATA');
  assert.throws(() => parseDecision({ ...decision(), trace: { ...decision().trace, source_kind: 'CAPTURED_MARKET_DATA' } }), /provenance disagree/);
  assert.throws(() => parseDecision({ ...decision(), trace: { ...decision().trace, result: { ...decision().trace.result, gross_delta_minor: '10' } } }), /arithmetic/);
  assert.throws(() => parseDecision(decision().trace), /contract/);
});
test('rejection evidence and unavailable execution coverage do not turn into zero or success', () => {
  const record = parseDecision({ ...decision(), trace: { ...decision().trace, amount_in_minor: null, input_age_ms: null, route: [], result: { status: 'DATA_UNAVAILABLE', reason_codes: ['NO_CAPTURE_INPUTS'] } } });
  assert.equal(record.trace.result.status, 'DATA_UNAVAILABLE');
  assert.equal(record.trace.input_age_ms, null);
  assert.throws(() => parseDecision({ ...decision(), trace: { ...decision().trace, diagnostics: ['https://provider.invalid?secret=fixture'] } }), /named public decision codes/);
  assert.equal(parseCoverage(coverage()).eligible_attempts, null);
  assert.throws(() => parseCoverage({ ...coverage(), eligible_attempts: '0' }), /must not be represented as zero/);
  assert.throws(() => parseCoverage({ ...coverage(), raw_observations: 3 }));
});
test('bounded exports project known fields, preserve exact units, and label limited journal evidence', () => {
  const raw = { ...journal(), private_unknown_field: 'do-not-export', event: { ...journal().event, command: { ...journal().event.command, operator_secret: 'do-not-export' } } };
  const parsed = parseJournal(raw);
  assert.equal(parsed.event.sequence, '9007199254740993');
  const json = researchExport('selected-journal-page', { items: [parsed], next_cursor: 'next-page' }, 1000, 2000);
  assert.equal(json.includes('do-not-export'), false);
  const exported = JSON.parse(json);
  assert.equal(exported.data.items[0].event.postings[0].amount, huge);
  assert.equal(exported.data.next_cursor, 'next-page');
  assert.equal(exported.snapshot_received_at, '1970-01-01T00:00:01.000Z');
  assert.match(exported.retention_notice, /not command inputs/);
  assert.match(exported.collection_notice, /does not establish complete collection/);
  assert.throws(() => researchExport('no-snapshot', [], null), /No received snapshot/);
  assert.throws(() => researchExport('too-large', 'x'.repeat(MAX_EXPORT_BYTES), 1000), /2 MiB/);
});
test('API requests have bounded explicit scopes; a wrong-session trace is rejected', async () => {
  const urls: string[] = [];
  const api = new ControlApi(async url => {
    urls.push(String(url));
    if (String(url).includes('/decision-coverage')) return Response.json(coverage());
    if (String(url).includes('/decisions?')) return Response.json({ items: [decision()], next_cursor: null });
    if (String(url).includes('/paper-runs/run-original/journal')) return Response.json({ items: [journal()], next_cursor: null });
    return Response.json({ items: [], next_cursor: null });
  });
  await api.decisions('session-paper', 'cursor/with?chars');
  await api.coverage('session-paper');
  await api.journal('run-original');
  await api.opportunities();
  assert.equal(urls[0], '/v1/decisions?limit=25&cursor=cursor%2Fwith%3Fchars&session_id=session-paper');
  assert.equal(urls[2], '/v1/paper-runs/run-original/journal?limit=25');
  assert.match(urls[3], /source_kind=CAPTURED_MARKET_DATA/);
  await assert.rejects(api.decisions('another-session'), /wrong session scope/);
});
test('paper creation retains idempotency, CSRF and immutable initial inventory', async () => {
  const calls: RequestInit[] = [];
  const body = { initial_balances: [{ asset: principalAsset, amount: huge }, { asset: nativeAsset, amount: '100' }] };
  const api = new ControlApi(async (url, init) => {
    calls.push(init!);
    return Response.json(String(url).endsWith('/auth/session') ? { operator_id: 'operator', csrf_token: 'test-csrf', expires_at: '2000000000' } : paperRun());
  });
  await api.auth(); await api.createPaperRun('session-paper', body, 'same-paper-key'); await api.createPaperRun('session-paper', body, 'same-paper-key');
  assert.equal((calls[1].headers as Record<string, string>)['X-CSRF-Token'], 'test-csrf');
  assert.equal((calls[1].headers as Record<string, string>)['Idempotency-Key'], 'same-paper-key');
  assert.equal(calls[1].body, calls[2].body);
  const changed = new ControlApi(async () => Response.json(paperRun()));
  await assert.rejects(changed.createPaperRun('session-paper', { initial_balances: [{ asset: principalAsset, amount: '1' }] }, 'key'), /initial balances/);
});
test('Opportunity 1.1 requires unknown net when external costs are incomplete', () => {
  const fixture = JSON.parse(readFileSync(new URL('../../../specs/opportunity.example.json', import.meta.url), 'utf8'));
  const captured = { ...fixture, schema_version: '1.1.0', source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'RECORDED_LIVE', net_after_explicit_costs_minor: null };
  assert.equal(parseOpportunity(captured).net_after_explicit_costs_minor, null);
  assert.throws(() => parseOpportunity({ ...captured, net_after_explicit_costs_minor: '100' }), /Incomplete external costs/);
  assert.throws(() => parseOpportunity({ ...captured, dataset_origin: 'SYNTHETIC' }));
  assert.throws(() => parseOpportunity({ ...captured, eligibility_checks: { ...captured.eligibility_checks, costs_complete: true } }), /complete costs require numeric net/);
  assert.throws(() => parseOpportunity({ ...fixture, source_kind: 'CAPTURED_MARKET_DATA', dataset_origin: 'MANUALLY_CONSTRUCTED' }), /provenance disagree/);
});
