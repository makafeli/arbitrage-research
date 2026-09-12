import assert from 'node:assert/strict';
import test from 'node:test';
import { ControlApi } from '../src/api/client.ts';
import { parseCollectionAttempt, parseCollectionCoverage } from '../src/api/collection.ts';
import { MAX_FROZEN_EXPORT_BYTES, parseFrozenExport, verifyFrozenExport } from '../src/api/frozenExport.ts';
import { csvCell, frozenCsv, frozenJson } from '../src/domain/frozenExport.ts';
import { collectionAttempt, collectionCoverage, exportHash, frozenExportFixture, sealExport } from './exports.fixture.ts';
import { huge } from './research.fixture.ts';

function parseCsv(csv: string): string[][] {
  const rows: string[][] = []; let row: string[] = [], value = '', quoted = false;
  for (let i = 0; i < csv.length; i++) {
    const ch = csv[i];
    if (ch === '"') { if (quoted && csv[i + 1] === '"') { value += '"'; i++; } else quoted = !quoted; }
    else if (!quoted && ch === ',') { row.push(value); value = ''; }
    else if (!quoted && ch === '\r' && csv[i + 1] === '\n') { row.push(value); rows.push(row); row = []; value = ''; i++; }
    else value += ch;
  }
  assert.equal(quoted, false); assert.equal(value, ''); return rows;
}
test('collection telemetry separates unfinished batches, deliberate fences and acquisition failures with a recorded denominator', () => {
  const coverage = parseCollectionCoverage(collectionCoverage());
  assert.equal(coverage.collection_completeness, 'UNKNOWN'); assert.equal(coverage.attempts_started, '3');
  assert.equal(parseCollectionAttempt({ ...collectionAttempt(), outcome: 'SUPPRESSED', reason: 'GENERATION_FENCED' }).outcome, 'SUPPRESSED');
  assert.throws(() => parseCollectionAttempt({ ...collectionAttempt(), reason: 'https://provider.invalid?key=secret' }), /public collection reason/);
  assert.throws(() => parseCollectionAttempt({ ...collectionAttempt(), outcome: 'IN_PROGRESS', reason: null, finished_at: null }), /contract/);
  assert.throws(() => parseCollectionCoverage({ ...collectionCoverage(), attempts_started: '0' }), /purpose counts/);
  assert.throws(() => parseCollectionAttempt({ ...collectionAttempt(), outcome: 'DECISIONS_RECORDED', reason: null, decision_rows: '1', decision_observation_ids: [] }), /references/);
  assert.equal(parseCollectionAttempt({ ...collectionAttempt(), outcome: 'EVALUATION_FAILED', reason: 'RESOURCE_LIMIT' }).reason, 'RESOURCE_LIMIT');
  // Wall-clock adjustment does not negate positive monotonic elapsed time.
  assert.equal(parseCollectionAttempt({ ...collectionAttempt(), finished_at: '2026-09-12T11:59:59Z' }).elapsed_ms, 25);
});
test('frozen bundle verifies source counts, cross-resource scope and exact content digest without claiming raw retention', async () => {
  const raw = frozenExportFixture(), bundle = await verifyFrozenExport(parseFrozenExport(raw));
  assert.equal(bundle.data.paper_runs[0].run.balances[0].total, huge);
  assert.equal(bundle.data.capture_dependencies[1].catalog_status, 'MISSING'); assert.equal(bundle.data.capture_dependencies[0].raw_artifact_status, 'NOT_VERIFIED');
  assert.equal(JSON.parse(frozenJson(bundle)).content_sha256, raw.content_sha256);
  await assert.rejects(verifyFrozenExport({ ...bundle, content_sha256: exportHash }), /digest does not match/);
  assert.throws(() => parseFrozenExport({ ...raw, snapshot: { ...raw.snapshot, source_counts: { ...raw.snapshot.source_counts, decisions: '2' } } }));
  const wrong = structuredClone(raw); wrong.data.paper_runs[0].run.session_id = 'other'; assert.throws(() => parseFrozenExport(wrong));
  const missing = structuredClone(raw); missing.data.capture_dependencies.pop(); assert.throws(() => parseFrozenExport(missing), /omits a decision capture dependency/);
});
test('frozen exports fail closed on hidden properties and unredacted freeform journal values', () => {
  const bundle = frozenExportFixture();
  assert.throws(() => parseFrozenExport({ ...bundle, data: { ...bundle.data, operator_secret: 'do-not-export' } }), /unsupported fields/);
  const injected = structuredClone(bundle); Object.assign(injected.data.session, { provider_secret: 'do-not-export' });
  assert.throws(() => frozenJson(injected), /unsupported fields/);
  const command = structuredClone(bundle);
  command.data.paper_runs[0].journal[0].event.command = { kind: 'MARK_UNKNOWN', attempt_id: exportHash, reason: 'REDACTED_FREEFORM_TEXT' };
  assert.equal(parseFrozenExport(command).data.paper_runs[0].journal[0].event.command.kind, 'MARK_UNKNOWN');
  Object.assign(command.data.paper_runs[0].journal[0].event.command, { reason: 'https://provider.invalid?key=do-not-export' });
  assert.throws(() => frozenCsv(command));
});
test('CSV protects formula and Unicode prefixes while round-tripping commas, quotes, newlines and exact nested amounts', () => {
  for (const value of ['=SUM(1,2)', '  +1', '\t@SUM(1)', '\r\n-10', '\u200b=1', '\uFEFF＝1', '＋1', '\u00a0＠SUM(1)', '\x00=1']) {
    assert.equal(parseCsv(csvCell(value) + '\r\n')[0][0], "'" + value);
  }
  const bundle = frozenExportFixture();
  bundle.data.strategy_ids = ['commas, quotes "ok"\nnew line', '\t＝HYPERLINK("example")'];
  const rows = parseCsv(frozenCsv(sealExport(bundle)));
  assert.deepEqual(rows[0], ['record_type', 'session_id', 'record_id', 'parent_id', 'payload_json']);
  assert.equal(rows.slice(1).every(row => row.length === 5), true);
  const payload = (kind: string) => JSON.parse(rows.find(row => row[0] === kind)![4]);
  assert.equal(payload('PAPER_RUN').balances[0].total, huge);
  assert.deepEqual(payload('SESSION').strategy_ids, bundle.data.strategy_ids);
  assert.equal(payload('PAPER_JOURNAL_EVENT').event.command.balances[0].amount, huge);
  assert.equal(rows.filter(row => row[0] === 'COLLECTION_ATTEMPT').length, 3);
  assert.equal(rows.filter(row => row[0] === 'CAPTURE_DEPENDENCY').length, 2);
  assert.equal(payload('EXPORT_MANIFEST').snapshot.collection_completeness, 'UNKNOWN');
});
test('actual client downloads one scoped bounded bundle and rejects tampering, wrong scope and oversized responses', async () => {
  const urls: string[] = [], bundle = frozenExportFixture();
  const api = new ControlApi(async url => { urls.push(String(url)); return Response.json(bundle); });
  assert.equal((await api.frozenExport('session-paper')).export_id, bundle.export_id);
  assert.equal(urls[0], '/v1/sessions/session-paper/export');
  await assert.rejects(api.frozenExport('other'), /wrong session scope/);
  const tampered = new ControlApi(async () => Response.json({ ...bundle, content_sha256: exportHash }));
  await assert.rejects(tampered.frozenExport('session-paper'), /digest does not match/);
  const oversized = new ControlApi(async () => new Response(' '.repeat(MAX_FROZEN_EXPORT_BYTES + 1)));
  await assert.rejects(oversized.frozenExport('session-paper'), /8 MiB response bound/);
  const health = new ControlApi(async url => Response.json(String(url).includes('coverage') ? collectionCoverage() : { items: [collectionAttempt()], next_cursor: null }));
  await assert.rejects(health.collectionCoverage('other'), /wrong session scope/);
  await assert.rejects(health.collectionAttempts('other'), /wrong session scope/);
});
