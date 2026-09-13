import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import type { FrozenExport } from '../src/api/frozenExport.ts';
import { parseFrozenExport } from '../src/api/frozenExport.ts';
import type { CaptureAuditReport } from '../src/domain/captureAudit.ts';
import { createCaptureAuditRequest, verifyCaptureAuditReport, auditRequestDigest, readCaptureAuditReport, MAX_CAPTURE_AUDIT_REPORT_BYTES } from '../src/domain/captureAudit.ts';
import { auditExportFixture, frozenExportFixture, sealExport, exportHash } from './exports.fixture.ts';

const repository = fileURLToPath(new URL('../../../', import.meta.url));
const hash = (value: string) => 'sha256:' + createHash('sha256').update(value).digest('hex');

async function localRoundTrip(network: 'base-mainnet' | 'solana-mainnet' = 'base-mainnet', now = '150', budget?: string) {
  const root = mkdtempSync(join(tmpdir(), 'arb-export-audit-'));
  const bundle = JSON.parse(JSON.stringify(auditExportFixture()).replaceAll('base-mainnet', network)) as FrozenExport;
  const captures = join(root, 'captures'); mkdirSync(captures);
  try {
    const body = '{"fixture":"PRIVATE_RPC_RESPONSE"}';
    const first = bundle.data.capture_dependencies[0];
    const context = network === 'base-mainnet'
      ? { kind: 'Evm', block_number: 1, block_hash: '0x' + 'a'.repeat(64), parent_hash: '0x' + 'b'.repeat(64), block_timestamp_seconds: 1, finality: 'finalized' }
      : { kind: 'Solana', slot: 42, genesis_hash: 'synthetic-genesis', commitment: 'finalized', account_context: 'single-getMultipleAccounts-response' };
    const manifest = JSON.stringify({ schema_version: 1, capture_id: first.capture_id, origin: 'synthetic', network,
      provider_alias: 'PRIVATE_ALIAS', adapter_version: 'fixture-v1', adapter_source_commit: 'a'.repeat(40), build_digest: exportHash,
      config_digest: bundle.data.session.configuration_digest, created_at_ms: 100, raw_expires_at_ms: 200, context,
      first_sequence: 0, last_sequence: 0, required_inputs: ['state'], missing_inputs: [], coherent: true, complete_for_quote: true,
      objects: [{ name: 'rpc.json', sha256: hash(body), bytes: Buffer.byteLength(body), content_type: 'application/json' }] });
    first.manifest_digest = hash(manifest);
    for (const decision of bundle.data.decisions) for (const ref of decision.trace.capture_refs) {
      if (ref.capture_id === first.capture_id) ref.manifest_digest = first.manifest_digest;
    }
    const dir = join(captures, first.capture_id); mkdirSync(dir);
    writeFileSync(join(dir, 'manifest.json'), manifest); writeFileSync(join(dir, 'rpc.json'), body);
    const source = sealExport(bundle), before = JSON.stringify(source);
    const request = await createCaptureAuditRequest(source), requestFile = join(root, 'request.json');
    writeFileSync(requestFile, JSON.stringify(request));
    const args = [resolve(repository, 'scripts/export_capture_audit.py'), '--root', captures, '--request', requestFile, '--now-ms', now];
    if (budget) args.push('--max-bytes', budget);
    const run = spawnSync('python3', args, { encoding: 'utf8', timeout: 10_000, maxBuffer: MAX_CAPTURE_AUDIT_REPORT_BYTES });
    assert.ifError(run.error); assert.equal(run.stderr, '');
    assert.equal(run.status, budget ? 2 : 1, run.stdout); // The second reference is deliberately absent.
    const report = await verifyCaptureAuditReport(JSON.parse(run.stdout), request);
    assert.equal(JSON.stringify(source), before);
    assert.equal(readFileSync(join(dir, 'manifest.json'), 'utf8'), manifest);
    assert(!run.stdout.includes('PRIVATE')); assert(!run.stdout.includes(root));
    return { source, request, report };
  } finally { rmSync(root, { recursive: true, force: true }); }
}

for (const network of ['base-mainnet', 'solana-mainnet'] as const) {
  test(`${network}: real TypeScript export -> Python volume audit -> browser validator`, async () => {
    const { report, request } = await localRoundTrip(network);
    assert.equal(report.request_sha256, await auditRequestDigest(request));
    assert.equal(report.audit.dependencies[0].raw_artifact_status, 'AVAILABLE');
    assert.equal(report.audit.dependencies[1].raw_artifact_status, 'MISSING');
    assert.equal(report.audit.references_reported, 2);
    assert.equal(report.audit.execution_authorized, false);
  });
}
test('expiry and incomplete budget reports preserve every reference without optimistic totals', async () => {
  const { report } = await localRoundTrip('base-mainnet', '200');
  assert.equal(report.audit.dependencies[0].expiration_status, 'EXPIRED');
  const limited = await localRoundTrip('base-mainnet', '150', '1');
  assert.equal(limited.report.audit.status, 'INCOMPLETE'); assert.equal(limited.report.audit.references_reported, 2);
});
test('request generation rejects tampered exports, omitted dependencies and unqualified identities', async () => {
  const source = auditExportFixture(); source.content_sha256 = exportHash;
  await assert.rejects(createCaptureAuditRequest(source));
  const omitted = auditExportFixture(); omitted.data.capture_dependencies.pop();
  await assert.rejects(createCaptureAuditRequest(sealExport(omitted)));
  await assert.rejects(createCaptureAuditRequest(frozenExportFixture()), /auditable capture identities/);
});
test('request generation retains same capture ID with different digests and catalog-only dependencies', async () => {
  const source = auditExportFixture(), ref = source.data.capture_dependencies[0];
  source.data.capture_dependencies.push({ ...ref, manifest_digest: exportHash, snapshot_ids: [], catalog_status: 'MISSING', generation: null, admitted_at: null });
  const result = await createCaptureAuditRequest(sealExport(source));
  assert.equal(result.capture_request.captures.length, 3);
  assert.equal(result.capture_request.captures[2].capture_id, result.capture_request.captures[0].capture_id);
  assert.notEqual(result.capture_request.captures[2].manifest_digest, result.capture_request.captures[0].manifest_digest);
});
test('larger exports are refused, not silently reduced to the first thousand dependencies', async () => {
  const source = auditExportFixture();
  for (let i = 2; i < 1001; i++) source.data.capture_dependencies.push({ ...source.data.capture_dependencies[1], capture_id: `extra-${i}`, snapshot_ids: [] });
  await assert.rejects(createCaptureAuditRequest(sealExport(source)), /at most 1,000/);
});
test('a different export, session, network or request digest is rejected', async () => {
  const { request, report } = await localRoundTrip();
  const mutations: ((r: CaptureAuditReport) => void)[] = [r => { r.source_export.export_id = 'different'; },
    r => { r.source_export.session_id = 'different'; }, r => { r.source_export.content_sha256 = exportHash; },
    r => { r.request_sha256 = exportHash; }, r => { r.audit.request_sha256 = exportHash; },
    r => { r.audit.network = 'solana-mainnet'; }, r => { r.audit.config_digest = hash('wrong'); }];
  for (const mutate of mutations) { const value = structuredClone(report); mutate(value); await assert.rejects(verifyCaptureAuditReport(value, request)); }
});
test('incomplete, duplicated or substituted report rows are rejected even after counts change', async () => {
  const { request, report } = await localRoundTrip();
  for (const modify of [(r: CaptureAuditReport) => { r.audit.dependencies.pop(); r.audit.references_reported--; },
    (r: CaptureAuditReport) => { r.audit.dependencies[1] = structuredClone(r.audit.dependencies[0]); },
    (r: CaptureAuditReport) => { r.audit.dependencies[0].manifest_digest = exportHash; },
    (r: CaptureAuditReport) => { r.audit.dependencies.reverse(); }]) {
    const value = structuredClone(report); modify(value); await assert.rejects(verifyCaptureAuditReport(value, request));
  }
});
test('status, object counts, times, budget and promotion claims must be internally consistent', async () => {
  const { request, report } = await localRoundTrip();
  const mutations: ((r: CaptureAuditReport) => void)[] = [r => { r.audit.status = 'COMPLETE'; },
    r => { r.audit.raw_status_counts.AVAILABLE = 2; }, r => { r.audit.dependencies[0].verified_objects = 0; },
    r => { r.audit.checked_at_ms = '150.0'; }, r => { r.audit.bytes_read_budgeted = '1099511627777'; },
    r => { r.audit.dependencies[0].expiration_status = 'EXPIRED'; }, r => { r.audit.dependencies[0].capture_time_status = 'AFTER_AUDIT'; },
    r => { Object.assign(r.audit, { execution_authorized: true }); }, r => { Object.assign(r.audit.dependencies[0], { replay_status: 'VERIFIED' }); },
    r => { Object.assign(r.audit.dependencies[0], { market_performance_eligible: true }); }];
  for (const mutate of mutations) { const value = structuredClone(report); mutate(value); await assert.rejects(verifyCaptureAuditReport(value, request)); }
});
test('unknown fields are rejected at every envelope/report/dependency boundary', async () => {
  const { request, report } = await localRoundTrip();
  for (const field of ['root', 'source', 'audit', 'dependency', 'counts']) {
    const value = structuredClone(report);
    const target = field === 'root' ? value : field === 'source' ? value.source_export : field === 'audit' ? value.audit : field === 'counts' ? value.audit.raw_status_counts : value.audit.dependencies[0];
    Object.assign(target, { private_secret: 'NEVER_RENDER' });
    await assert.rejects(verifyCaptureAuditReport(value, request));
  }
});
test('uploaded file limits are checked before and after reading; bad JSON does not echo source', async () => {
  const request = await createCaptureAuditRequest(auditExportFixture()); let reads = 0;
  await assert.rejects(readCaptureAuditReport({ size: MAX_CAPTURE_AUDIT_REPORT_BYTES + 1, text: async () => { reads++; return '{}'; } }, request));
  assert.equal(reads, 0);
  await assert.rejects(readCaptureAuditReport({ size: 1, text: async () => ' '.repeat(MAX_CAPTURE_AUDIT_REPORT_BYTES + 1) }, request));
  await assert.rejects(readCaptureAuditReport({ size: 30, text: async () => '{"SECRET_RAW_INPUT"' }, request), error => error instanceof Error && !error.message.includes('SECRET'));
});
test('exact u64 audit time and detached result survive round-trip', async () => {
  const { request, report } = await localRoundTrip('base-mainnet', '18446744073709551615');
  assert.equal(report.audit.checked_at_ms, '18446744073709551615');
  const copy = await verifyCaptureAuditReport(report, request); report.audit.dependencies.length = 0;
  assert.equal(copy.audit.dependencies.length, 2);
});
test('a request snapshot remains unchanged when the caller mutates its source during hashing', async () => {
  const source = auditExportFixture(), promise = createCaptureAuditRequest(source);
  source.data.capture_dependencies.pop();
  const result = await promise;
  assert.equal(result.capture_request.captures.length, 2);
});
test('empty exports create empty requests, not evidence of a complete campaign', async () => {
  const example = JSON.parse(readFileSync(resolve(repository, 'specs/research-export.example.json'), 'utf8'));
  const request = await createCaptureAuditRequest(parseFrozenExport(example));
  assert.deepEqual(request.capture_request.captures, []);
});
