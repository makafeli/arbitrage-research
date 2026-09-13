#!/usr/bin/env python3
"""Synthetic envelope/CLI tests; never an attestation of production capture data."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import capture_audit as c
import export_capture_audit as e
from test_capture_audit import manifest


class ExportAuditTests(unittest.TestCase):
    """Check binding, retained gaps, CLI redaction and unchanged source bytes."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.captures = self.root / 'captures'
        directory = self.captures / 'capture-1'
        directory.mkdir(parents=True)
        self.raw = json.dumps(manifest()).encode()
        (directory / 'manifest.json').write_bytes(self.raw)
        (directory / 'rpc.json').write_bytes(b'{"fixture":"PRIVATE_RAW_DATA"}')
        self.request = {'schema_version': 1, 'kind': e.REQUEST_KIND,
                        'source_export': {'schema_version': '1.1.0', 'export_id': 'export-1',
                                          'content_sha256': c.digest(b'synthetic-export'), 'session_id': 'session-1'},
                        'capture_request': {'schema_version': 1, 'network': 'base-mainnet',
                                            'config_digest': manifest()['config_digest'], 'captures': [
                                                {'capture_id': 'capture-1', 'manifest_digest': c.digest(self.raw)}]}}

    def test_bound_report_preserves_original_audit_and_source_bytes(self):
        """Envelope adds binding without altering v1 auditing or retained files."""
        before = copy.deepcopy(self.request)
        result = e.audit_export(self.captures, self.request, 150)
        self.assertEqual(result['audit'], c.audit(self.captures, self.request['capture_request'], 150))
        self.assertEqual(result['source_export'], self.request['source_export'])
        self.assertEqual(result['request_sha256'], e.canonical_digest(self.request))
        self.assertEqual(before, self.request)
        self.assertEqual((self.captures / 'capture-1/manifest.json').read_bytes(), self.raw)
        self.assertNotIn('PRIVATE_RAW_DATA', json.dumps(result))
        self.assertNotIn(str(self.root), json.dumps(result))

    def test_every_binding_field_changes_envelope_digest(self):
        """A changed export/session/content identity cannot reuse the original request hash."""
        before = e.canonical_digest(self.request)
        for key, value in [('export_id', 'export-2'), ('session_id', 'session-2'),
                           ('content_sha256', c.digest(b'changed-export'))]:
            request = copy.deepcopy(self.request)
            request['source_export'][key] = value
            self.assertNotEqual(before, e.canonical_digest(request))

    def test_bad_envelopes_fail_before_root_access(self):
        """Unknown fields and unsafe identities are rejected, not reflected into reports."""
        for mutate in [lambda r: r.update(schema_version=True), lambda r: r.update(kind='OTHER'),
                       lambda r: r.update(secret='PRIVATE'), lambda r: r.update(source_export=[]),
                       lambda r: r['source_export'].update(schema_version='2'),
                       lambda r: r['source_export'].update(session_id='../bad'),
                       lambda r: r['source_export'].update(export_id=[]),
                       lambda r: r['source_export'].update(content_sha256='not-hashed'),
                       lambda r: r['capture_request']['captures'].append(r['capture_request']['captures'][0])]:
            request = copy.deepcopy(self.request)
            mutate(request)
            with self.subTest(request=request), patch.object(c, 'audit') as audit:
                with self.assertRaises(c.AuditError):
                    e.audit_export(self.captures, request, 150)
                audit.assert_not_called()

    def test_missing_expired_and_budget_results_remain_complete_lists(self):
        """An incomplete audit still reports all declared references rather than truncating."""
        self.request['capture_request']['captures'].append({'capture_id': 'absent', 'manifest_digest': c.digest(b'absent')})
        result = e.audit_export(self.captures, self.request, 200)['audit']
        self.assertEqual(result['status'], 'COMPLETE_WITH_GAPS')
        self.assertEqual(result['references_reported'], 2)
        self.assertEqual(result['dependencies'][0]['expiration_status'], 'EXPIRED')
        self.assertEqual(result['dependencies'][1]['raw_artifact_status'], 'MISSING')
        limited = e.audit_export(self.captures, self.request, 150, max_bytes=1)['audit']
        self.assertEqual(limited['status'], 'INCOMPLETE')
        self.assertEqual(limited['references_reported'], 2)

    def test_changed_network_or_config_is_not_accepted_as_available(self):
        """The envelope cannot override the existing manifest context checks."""
        for key, value in [('network', 'solana-mainnet'), ('config_digest', c.digest(b'wrong'))]:
            request = copy.deepcopy(self.request)
            request['capture_request'][key] = value
            self.assertEqual(e.audit_export(self.captures, request, 150)['audit']['dependencies'][0]['raw_artifact_status'], 'CORRUPT')

    def test_empty_and_oversized_reference_lists_are_not_successful_coverage(self):
        """Empty references are a gap; too many fail before auditing."""
        self.request['capture_request']['captures'] = []
        self.assertEqual(e.audit_export(self.captures, self.request, 150)['audit']['status'], 'COMPLETE_WITH_GAPS')
        self.request['capture_request']['captures'] = [{'capture_id': str(i), 'manifest_digest': c.digest(b'x')} for i in range(1001)]
        with self.assertRaises(c.AuditError):
            e.audit_export(self.captures, self.request, 150)

    def test_actual_cli_exits_and_redaction(self):
        """Run the real process for success, gaps and request failure."""
        path = self.root / 'request.json'
        path.write_text(json.dumps(self.request))
        command = [sys.executable, e.__file__, '--root', str(self.captures), '--request', str(path), '--now-ms']
        for now, code in [('150', 0), ('200', 1)]:
            result = subprocess.run(command + [now], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, code, result.stderr)
            self.assertEqual(json.loads(result.stdout)['kind'], e.REPORT_KIND)
        for payload in ['{"secret":"PRIVATE_UNTRUSTED"}', '{"schema_version":1,"schema_version":1}', 'NaN', '"\\ud800"']:
            path.write_text(payload)
            result = subprocess.run(command + ['150'], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stderr, '')
            self.assertNotIn('PRIVATE', result.stdout)
            self.assertNotIn(str(self.root), result.stdout)
            self.assertEqual(json.loads(result.stdout)['status'], 'EXPORT_AUDIT_FAILED')

    def test_exact_large_audit_timestamp_is_retained(self):
        """Times beyond JavaScript integer precision stay decimal strings."""
        result = e.audit_export(self.captures, self.request, c.U64_MAX)
        self.assertEqual(result['audit']['checked_at_ms'], str(c.U64_MAX))
        self.assertFalse(result['audit']['execution_authorized'])


if __name__ == '__main__':
    unittest.main()
