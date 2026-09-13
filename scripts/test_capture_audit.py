#!/usr/bin/env python3
"""Adversarial local-file tests; synthetic fixtures are never market observations."""
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import capture_audit as a


def manifest(capture_id='capture-1', network='base-mainnet'):
    """Build a synthetic v1 storage manifest, without a protocol-replay claim."""
    context = {'kind': 'Evm', 'block_number': 1, 'block_hash': '0x' + 'a' * 64,
               'parent_hash': '0x' + 'b' * 64, 'block_timestamp_seconds': 1, 'finality': 'finalized'}
    if network == 'solana-mainnet':
        context = {'kind': 'Solana', 'slot': 42, 'genesis_hash': 'synthetic-genesis',
                   'commitment': 'finalized', 'account_context': 'single-getMultipleAccounts-response'}
    return {'schema_version': 1, 'capture_id': capture_id, 'origin': 'synthetic',
            'network': network, 'provider_alias': 'PRIVATE_ALIAS_NOT_FOR_REPORTS',
            'adapter_version': 'fixture-v1', 'adapter_source_commit': 'a' * 40,
            'build_digest': a.digest(b'fixture-build'), 'config_digest': a.digest(b'fixture-config'),
            'created_at_ms': 100, 'raw_expires_at_ms': 200, 'context': context,
            'first_sequence': 0, 'last_sequence': 0, 'required_inputs': ['state'],
            'missing_inputs': [], 'coherent': True, 'complete_for_quote': True,
            'objects': [{'name': 'rpc.json', 'sha256': a.digest(b'{"fixture":"PRIVATE_RAW_DATA"}'),
                         'bytes': len(b'{"fixture":"PRIVATE_RAW_DATA"}'), 'content_type': 'application/json'}]}


class CaptureAuditTests(unittest.TestCase):
    """Exercise the filesystem, corruption reporting and CLI without RPC or databases."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.captures = self.root / 'captures'
        self.captures.mkdir()
        self.request = {'schema_version': 1, 'network': 'base-mainnet',
                        'config_digest': a.digest(b'fixture-config'), 'captures': []}
        self.value = manifest()
        self.publish(self.value)

    def publish(self, value):
        """Publish clearly synthetic exact bytes and retain the manifest digest separately."""
        path = self.captures / value['capture_id']
        path.mkdir(exist_ok=True)
        (path / 'rpc.json').write_bytes(b'{"fixture":"PRIVATE_RAW_DATA"}')
        raw = json.dumps(value, indent=2).encode()
        (path / 'manifest.json').write_bytes(raw)
        ref = {'capture_id': value['capture_id'], 'manifest_digest': a.digest(raw)}
        self.request['captures'].append(ref)
        return ref

    def changed_manifest(self, change):
        """Re-anchor a deliberately malformed manifest to exercise schema validation."""
        value = copy.deepcopy(self.value)
        change(value)
        raw = json.dumps(value).encode()
        (self.captures / 'capture-1/manifest.json').write_bytes(raw)
        self.request['captures'][0]['manifest_digest'] = a.digest(raw)

    def report(self, **kwargs):
        """Run at a deterministic instant without rewriting any historical result."""
        return a.audit(self.captures, self.request, 150, **kwargs)

    def first(self):
        """Select the only dependency in a single-reference report."""
        return self.report()['dependencies'][0]

    def test_available_bytes_expiry_and_secrets(self):
        """Byte availability never upgrades synthetic provenance to market eligibility."""
        report = self.report()
        row = report['dependencies'][0]
        self.assertEqual(report['status'], 'COMPLETE')
        self.assertEqual(row['raw_artifact_status'], 'AVAILABLE')
        self.assertEqual(row['expiration_status'], 'NOT_EXPIRED')
        self.assertEqual(row['origin'], 'synthetic')
        self.assertFalse(row['market_performance_eligible'])
        self.assertFalse(report['execution_authorized'])
        self.assertEqual(row['replay_status'], 'NOT_ASSESSED')
        public = json.dumps(report)
        for secret in ('PRIVATE_ALIAS', 'PRIVATE_RAW_DATA', str(self.root)):
            self.assertNotIn(secret, public)

    def test_solana_context_is_supported(self):
        """Both currently supported capture networks share the storage audit."""
        self.request['captures'] = []
        self.request['network'] = 'solana-mainnet'
        self.publish(manifest('solana-1', 'solana-mainnet'))
        self.assertEqual(self.first()['raw_artifact_status'], 'AVAILABLE')

    def test_expiry_at_boundary_does_not_hide_valid_bytes(self):
        """Expiration is evaluated at >= expiry, separately from actual file integrity."""
        for now, state in [(199, 'NOT_EXPIRED'), (200, 'EXPIRED'), (201, 'EXPIRED')]:
            with self.subTest(now=now):
                report = a.audit(self.captures, self.request, now)
                self.assertEqual(report['dependencies'][0]['expiration_status'], state)
                self.assertEqual(report['dependencies'][0]['raw_artifact_status'], 'AVAILABLE')
                self.assertEqual(report['status'], 'COMPLETE' if now < 200 else 'COMPLETE_WITH_GAPS')

    def test_future_capture_is_not_a_complete_as_of_audit(self):
        """An operator-supplied earlier audit time cannot certify future evidence."""
        report = a.audit(self.captures, self.request, 50)
        self.assertEqual(report['status'], 'COMPLETE_WITH_GAPS')
        self.assertEqual(report['dependencies'][0]['capture_time_status'], 'AFTER_AUDIT')

    def test_no_expiration_stays_explicit(self):
        """Missing retention policy is not silently rendered as a known expiry date."""
        self.changed_manifest(lambda m: m.update(raw_expires_at_ms=None))
        self.assertEqual(self.first()['expiration_status'], 'NOT_DECLARED')
        self.assertIsNone(self.first()['raw_expires_at_ms'])

    def test_exact_u64_timestamps_are_strings_in_report(self):
        """Values beyond JavaScript's integer precision survive losslessly."""
        self.changed_manifest(lambda m: m.update(created_at_ms=a.U64_MAX - 1, raw_expires_at_ms=a.U64_MAX))
        row = a.audit(self.captures, self.request, a.U64_MAX)['dependencies'][0]
        self.assertEqual(row['created_at_ms'], str(a.U64_MAX - 1))
        self.assertEqual(row['raw_expires_at_ms'], str(a.U64_MAX))
        self.assertEqual(row['expiration_status'], 'EXPIRED')

    def test_expired_missing_object_is_not_omitted(self):
        """A verified expired manifest can still disclose missing physical input."""
        (self.captures / 'capture-1/rpc.json').unlink()
        row = a.audit(self.captures, self.request, 200)['dependencies'][0]
        self.assertEqual(row['raw_artifact_status'], 'MISSING')
        self.assertEqual(row['expiration_status'], 'EXPIRED')
        self.assertEqual(row['missing_objects'], 1)

    def test_missing_directory_retains_requested_reference(self):
        """Every requested dependency appears even when the complete directory is absent."""
        self.request['captures'].append({'capture_id': 'absent', 'manifest_digest': a.digest(b'absent')})
        report = self.report()
        self.assertEqual(report['references_requested'], report['references_reported'])
        self.assertEqual(report['dependencies'][1]['capture_id'], 'absent')
        self.assertEqual(report['dependencies'][1]['raw_artifact_status'], 'MISSING')

    def test_missing_root_is_failure_not_a_missing_capture_inventory(self):
        """A disconnected or mistyped root must not look like successful collection."""
        with self.assertRaises(OSError):
            a.audit(self.root / 'does-not-exist', self.request, 150)

    def test_missing_manifest_and_incomplete_marker(self):
        """Uncommitted writes never become available capture evidence."""
        path = self.captures / 'capture-1'
        (path / 'manifest.json').unlink()
        self.assertEqual(self.first()['reason'], 'MANIFEST_MISSING')
        (path / 'INCOMPLETE').symlink_to(self.root / 'absent-marker-target')
        self.assertEqual(self.first()['reason'], 'INCOMPLETE_MARKER')

    def test_corrupt_object_and_manifest(self):
        """Exact manifest and object hashes, not just JSON shape, are mandatory."""
        (self.captures / 'capture-1/rpc.json').write_bytes(b'bad')
        self.assertEqual(self.first()['raw_artifact_status'], 'CORRUPT')
        self.assertEqual(self.first()['corrupt_objects'], 1)
        (self.captures / 'capture-1/manifest.json').write_bytes(b'{}')
        self.assertEqual(self.first()['reason'], 'MANIFEST_DIGEST_MISMATCH')
        self.assertEqual(self.first()['origin'], 'UNKNOWN')

    def test_all_objects_are_checked_and_counts_are_retained(self):
        """One missing object does not suppress the other integrity findings."""
        self.changed_manifest(lambda m: m['objects'].extend([
            {'name': 'missing.json', 'bytes': 2, 'sha256': a.digest(b'{}'), 'content_type': 'application/json'},
            {'name': 'bad.json', 'bytes': 2, 'sha256': a.digest(b'{}'), 'content_type': 'application/json'},
        ]))
        (self.captures / 'capture-1/bad.json').write_bytes(b'xx')
        row = self.first()
        self.assertEqual((row['verified_objects'], row['missing_objects'], row['corrupt_objects']), (1, 1, 1))
        self.assertEqual(row['raw_artifact_status'], 'CORRUPT')

    def test_wrong_identity_network_or_configuration(self):
        """A real directory with the wrong trusted context is never usable by this request."""
        changes = [('CAPTURE_ID_MISMATCH', lambda m: m.update(capture_id='other')),
                   ('NETWORK_MISMATCH', lambda m: m.update(network='solana-mainnet', context=manifest(network='solana-mainnet')['context'])),
                   ('CONFIGURATION_MISMATCH', lambda m: m.update(config_digest=a.digest(b'other')))]
        for reason, change in changes:
            with self.subTest(reason=reason):
                self.changed_manifest(change)
                self.assertEqual(self.first()['reason'], reason)
                self.assertEqual(self.first()['origin'], 'UNKNOWN')

    def test_unknown_version_is_explicitly_unsupported(self):
        """Future schema fields do not become accepted v1 evidence."""
        self.changed_manifest(lambda m: m.update(schema_version=2))
        self.assertEqual(self.first()['raw_artifact_status'], 'UNSUPPORTED')

    def test_malformed_current_manifest_fields(self):
        """The storage validator rejects invalid Rust types and declarations."""
        changes = [lambda m: m.update(schema_version=True), lambda m: m.update(extra='field'),
                   lambda m: m.update(origin='imagined-live'), lambda m: m.update(created_at_ms=0),
                   lambda m: m.update(created_at_ms=True), lambda m: m.update(created_at_ms=a.U64_MAX + 1),
                   lambda m: m.update(raw_expires_at_ms=100), lambda m: m.update(first_sequence=4, last_sequence=3),
                   lambda m: m.update(adapter_source_commit='main'), lambda m: m.update(build_digest='a' * 64),
                   lambda m: m.update(provider_alias='https://secret.invalid/key'),
                   lambda m: m.update(required_inputs=[]), lambda m: m.update(required_inputs=['a', 'a']),
                   lambda m: m.update(missing_inputs=['unknown']), lambda m: m.update(coherent=False),
                   lambda m: m.update(complete_for_quote=1), lambda m: m.update(objects=[]),
                   lambda m: m['objects'].append(copy.deepcopy(m['objects'][0])),
                   lambda m: m['objects'][0].update(bytes=True), lambda m: m['objects'][0].update(bytes=0),
                   lambda m: m['objects'][0].update(bytes=a.MAX_BUNDLE + 1),
                   lambda m: m['objects'][0].update(content_type='application/octet-stream'),
                   lambda m: m['objects'][0].update(name='../secret'), lambda m: m['objects'][0].update(name='manifest.json'),
                   lambda m: m['objects'][0].update(name='x.tmp'), lambda m: m['context'].update(block_number=1.0),
                   lambda m: m['context'].update(finality='latest'), lambda m: m['context'].update(extra='field')]
        for index, change in enumerate(changes):
            with self.subTest(index=index):
                self.changed_manifest(change)
                self.assertEqual(self.first()['raw_artifact_status'], 'CORRUPT')

    def test_incomplete_quote_state_keeps_storage_availability_separate(self):
        """Existing valid incomplete snapshots can be stored but cannot be called ready."""
        self.changed_manifest(lambda m: m.update(complete_for_quote=False, missing_inputs=['state']))
        report = self.report()
        self.assertEqual(report['dependencies'][0]['raw_artifact_status'], 'AVAILABLE')
        self.assertFalse(report['dependencies'][0]['quote_inputs_declared_complete'])
        self.assertEqual(report['status'], 'COMPLETE_WITH_GAPS')

    def test_recorded_live_still_never_proves_market_performance(self):
        """A provenance declaration alone does not qualify a provider or its output."""
        self.changed_manifest(lambda m: m.update(origin='recorded-live'))
        row = self.first()
        self.assertEqual(row['origin'], 'recorded-live')
        self.assertFalse(row['market_performance_eligible'])
        self.assertEqual(row['replay_status'], 'NOT_ASSESSED')

    def test_extra_files_or_directories_are_refused(self):
        """Unknown bundle members are not followed or silently ignored."""
        extra = self.captures / 'capture-1/extra'
        extra.mkdir()
        (extra / 'PRIVATE_SECRET').write_bytes(b'not traversed')
        self.assertEqual(self.first()['reason'], 'UNEXPECTED_BUNDLE_ENTRY')

    def test_symlinked_root_bundle_and_object_are_refused(self):
        """No-follow reads prevent traversing alias paths outside the operator's root."""
        alias = self.root / 'alias'
        alias.symlink_to(self.captures, target_is_directory=True)
        with self.assertRaises(a.AuditError):
            a.audit(alias, self.request, 150)
        path = self.captures / 'capture-1/rpc.json'
        path.unlink()
        path.symlink_to(self.root / 'outside')
        self.assertEqual(self.first()['raw_artifact_status'], 'UNREADABLE')
        self.request['captures'][0]['capture_id'] = 'alias'
        (self.captures / 'alias').symlink_to(self.captures / 'capture-1', target_is_directory=True)
        self.assertEqual(self.first()['raw_artifact_status'], 'UNREADABLE')

    def test_hardlinked_or_special_objects_are_refused(self):
        """Regular single-link artifacts only; a FIFO must not block the audit."""
        path = self.captures / 'capture-1/rpc.json'
        os.link(path, self.root / 'hardlink')
        self.assertEqual(self.first()['raw_artifact_status'], 'CORRUPT')
        (self.root / 'hardlink').unlink()
        path.unlink()
        os.mkfifo(path)
        self.assertEqual(self.first()['raw_artifact_status'], 'CORRUPT')

    def test_replacement_during_file_read_is_rejected(self):
        """Descriptor/path metadata is checked after consuming a file."""
        original_stat = a.os.stat
        changed = False
        def swapping_stat(path, *args, **kwargs):
            nonlocal changed
            if path == 'rpc.json' and not changed:
                changed = True
                replacement = self.root / 'replacement'
                replacement.write_bytes(b'{"fixture":"PRIVATE_RAW_DATA"}')
                os.replace(replacement, self.captures / 'capture-1/rpc.json')
            return original_stat(path, *args, **kwargs)
        with patch('capture_audit.os.stat', side_effect=swapping_stat):
            self.assertEqual(self.first()['raw_artifact_status'], 'CORRUPT')

    def test_request_rejects_duplicates_traversal_and_unknown_fields_before_io(self):
        """Malformed batches fail wholly rather than silently losing references."""
        changes = [lambda r: r.update(schema_version=True), lambda r: r.update(network='ethereum'),
                   lambda r: r.update(config_digest='invalid'), lambda r: r.update(extra=True),
                   lambda r: r['captures'].append(copy.deepcopy(r['captures'][0])),
                   lambda r: r['captures'][0].update(capture_id='../outside'),
                   lambda r: r['captures'][0].update(capture_id='a/b'),
                   lambda r: r['captures'][0].update(manifest_digest=True),
                   lambda r: r.update(captures=[{}] * 1001)]
        for index, change in enumerate(changes):
            request = copy.deepcopy(self.request)
            change(request)
            with self.subTest(index=index), patch('capture_audit.os.open') as opened:
                with self.assertRaises(a.AuditError):
                    a.audit(self.captures, request, 150)
                opened.assert_not_called()

    def test_same_id_different_digest_remains_two_distinct_dependencies(self):
        """Conflicting history must not be collapsed into one successful identity."""
        self.request['captures'].append({'capture_id': 'capture-1', 'manifest_digest': a.digest(b'other')})
        rows = self.report()['dependencies']
        self.assertEqual([r['raw_artifact_status'] for r in rows], ['AVAILABLE', 'CORRUPT'])

    def test_budget_exhaustion_reports_all_references(self):
        """Size limits produce explicit unassessed rows, not a shortened healthy report."""
        self.publish(manifest('capture-2'))
        report = self.report(max_bytes=1)
        self.assertEqual(report['status'], 'INCOMPLETE')
        self.assertEqual(report['references_reported'], 2)
        self.assertTrue(all(r['reason'] == 'AUDIT_BYTE_LIMIT' for r in report['dependencies']))

    def test_invalid_time_or_budget_is_rejected(self):
        """Runtime limits and as-of time require exact bounded integers."""
        for now in [0, -1, True, 1.0, a.U64_MAX + 1]:
            with self.subTest(now=now), self.assertRaises(a.AuditError):
                a.audit(self.captures, self.request, now)
        for limit in [0, -1, True, 1.5, 1024**4 + 1]:
            with self.subTest(limit=limit), self.assertRaises(a.AuditError):
                self.report(max_bytes=limit)

    def test_duplicate_malformed_or_nonfinite_json_is_refused(self):
        """Ambiguous parser input cannot pass by recomputing its digest."""
        for raw in [b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}', b'\xff', b'[' * 2000 + b']' * 2000]:
            with self.subTest(raw=raw[:30]), self.assertRaises(a.AuditError):
                a.decode(raw)

    def test_unpaired_escaped_unicode_is_rejected_in_values_and_keys(self):
        """Python's permissive JSON strings must not certify invalid Rust strings."""
        for raw in [br'"\ud800"', br'"\udfff"', br'{"\ud800":1}',
                    br'{"nested":["\ud800x"]}', br'"\udc00\ud800"',
                    br'"\ud800\ud800"']:
            with self.subTest(raw=raw), self.assertRaisesRegex(a.AuditError, 'INVALID_JSON'):
                a.decode(raw)

    def test_valid_unicode_and_literal_escape_text_are_not_normalized(self):
        """Valid scalar text, surrogate pairs and literal backslashes retain meaning."""
        for raw, expected in [(br'"\ud83d\ude80"', '\U0001f680'),
                              (br'"\\ud800"', r'\ud800'),
                              ('"caf\u00e9"'.encode(), 'caf\u00e9'),
                              (br'{"\ud83d\ude80":["\u0000"]}', {'\U0001f680': ['\x00']})]:
            with self.subTest(raw=raw):
                self.assertEqual(a.decode(raw), expected)

    def test_invalid_unicode_capture_does_not_hide_later_valid_reference(self):
        """A newly retained digest cannot repair a malformed string's schema."""
        self.changed_manifest(lambda m: m.update(required_inputs=['state', '\ud800']))
        self.publish(manifest('capture-2'))
        report = self.report()
        self.assertEqual(report['status'], 'COMPLETE_WITH_GAPS')
        self.assertEqual(report['references_reported'], 2)
        broken, valid = report['dependencies']
        self.assertEqual(broken['raw_artifact_status'], 'CORRUPT')
        self.assertEqual(broken['reason'], 'INVALID_JSON')
        self.assertEqual(broken['origin'], 'UNKNOWN')
        self.assertEqual(valid['raw_artifact_status'], 'AVAILABLE')

    def test_cli_invalid_unicode_manifest_is_a_redacted_gap(self):
        """Malformed manifest strings produce a retained gap, not a traceback."""
        self.changed_manifest(lambda m: m.update(required_inputs=['PRIVATE_SECRET\udfff']))
        path = self.root / 'request.json'
        path.write_text(json.dumps(self.request))
        result = subprocess.run(
            [sys.executable, str(Path(a.__file__).resolve()), '--root', str(self.captures),
             '--request', str(path), '--now-ms', '150'],
            capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(json.loads(result.stdout)['dependencies'][0]['reason'], 'INVALID_JSON')
        self.assertEqual(result.stderr, '')
        self.assertNotIn('PRIVATE_SECRET', result.stdout)
        self.assertNotIn(str(self.root), result.stdout)

    def test_empty_request_is_not_a_healthy_evidence_claim(self):
        """No requested inputs is an explicit evidence gap, not successful replay."""
        self.request['captures'] = []
        report = self.report()
        self.assertEqual(report['references_reported'], 0)
        self.assertEqual(report['status'], 'COMPLETE_WITH_GAPS')

    def test_sources_are_unchanged_and_reports_deterministic(self):
        """Auditing does not rewrite manifests, retention or historical raw input."""
        before = {str(p): p.read_bytes() for p in self.captures.rglob('*') if p.is_file()}
        self.assertEqual(self.report(), self.report())
        after = {str(p): p.read_bytes() for p in self.captures.rglob('*') if p.is_file()}
        self.assertEqual(before, after)

    def test_actual_cli_success_gaps_failure_and_no_secret_echo(self):
        """Run the entrypoint in real subprocesses and check its stable exit contract."""
        path = self.root / 'request.json'
        path.write_text(json.dumps(self.request))
        script = str(Path(a.__file__).resolve())
        command = [sys.executable, script, '--root', str(self.captures), '--request', str(path), '--now-ms', '150']
        result = subprocess.run(command, capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['status'], 'COMPLETE')
        (self.captures / 'capture-1/rpc.json').unlink()
        result = subprocess.run(command, capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(json.loads(result.stdout)['status'], 'COMPLETE_WITH_GAPS')
        path.write_text('{"PRIVATE_SECRET":"not-valid"}')
        result = subprocess.run(command, capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(json.loads(result.stdout)['status'], 'AUDIT_FAILED')
        self.assertNotIn('PRIVATE_SECRET', result.stdout + result.stderr)
        self.assertNotIn(str(self.root), result.stdout + result.stderr)

    def test_cli_refuses_symlinked_request(self):
        """Request metadata cannot be read through symlinks either."""
        path = self.root / 'request.json'
        path.symlink_to(self.captures / 'capture-1/manifest.json')
        args = ['capture_audit.py', '--root', str(self.captures), '--request', str(path), '--now-ms', '150']
        with patch.object(sys, 'argv', args), patch('sys.stdout', new_callable=io.StringIO) as out:
            self.assertEqual(a.main(), 2)
        self.assertEqual(json.loads(out.getvalue())['status'], 'AUDIT_FAILED')


if __name__ == '__main__':
    unittest.main()
