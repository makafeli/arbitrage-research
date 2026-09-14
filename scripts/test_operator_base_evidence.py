"""Offline operator-access tests; mocked responses do not qualify a provider."""
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

import collect_operator_base_evidence as c

SECRET = 'https://authorized.example/v2/private-credential-12345'


class OperatorEvidenceTests(unittest.TestCase):
    def invoke(self, args):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = c.main(args)
        return code, output.getvalue()

    def test_default_does_not_read_credentials_or_start_network(self):
        with patch.object(c, 'endpoint_from_environment') as read, patch.object(c, 'collect') as run:
            code, text = self.invoke([])
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(text)['network_calls'], 0)
        read.assert_not_called(); run.assert_not_called()

    def test_missing_secret_prevents_network_and_output_creation(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, {}, clear=True), patch.object(c, 'collect') as run:
            target = Path(root) / 'summary.json'
            code, text = self.invoke(['--collect', '--output', str(target)])
            self.assertEqual(code, 2)
            self.assertEqual(json.loads(text)['reason'], 'AUTHORIZED_BASE_ENDPOINT_MISSING')
            self.assertFalse(target.exists())
        run.assert_not_called()

    def test_invalid_endpoints_are_rejected_without_exposure(self):
        values = ['http://example.com/private', 'https://user:private@example.com',
                  'https://127.0.0.1/key', 'https://[::1]/key', 'https://localhost/key',
                  'https://server.internal/key', 'https://example.com:444/key',
                  'https://example.com/key#private', 'https://example.com/\nprivate',
                  'https://example.com./key', 'https://example.com\\@private/key',
                  'https://[invalid/key', 'file:///private/key']
        for value in values:
            with self.subTest(value=value), patch.dict(os.environ, {c.SECRET_NAME: value}), patch.object(c, 'collect') as run:
                code, text = self.invoke(['--collect'])
                self.assertEqual(code, 2)
                self.assertNotIn(value, text)
                self.assertNotIn('private', text)
                run.assert_not_called()

    def test_https_path_and_query_keys_remain_supported(self):
        for value in [SECRET, 'https://authorized.example:443/?key=secret-value']:
            with patch.dict(os.environ, {c.SECRET_NAME: value}):
                self.assertEqual(c.endpoint_from_environment(), value)

    def test_existing_output_refuses_before_network_and_is_unchanged(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, {c.SECRET_NAME: SECRET}), patch.object(c, 'collect') as run:
            target = Path(root) / 'retained.json'; target.write_text('retained')
            code, text = self.invoke(['--collect', '--output', str(target)])
            self.assertEqual(code, 2); self.assertEqual(target.read_text(), 'retained')
            self.assertNotIn(root, text); run.assert_not_called()

    def test_summary_allowlist_drops_url_and_raw_provider_content(self):
        with patch.object(c, 'OperatorBaseRpc') as make, patch.object(c.observer, 'observe') as observe:
            make.return_value.calls = [{'request': {'method': 'eth_chainId', 'params': []},
                                       'response_sha256': 'sha256:' + 'a'*64,
                                       'response_base64': 'secret-base64', 'endpoint': SECRET}]
            observe.return_value = {'network': 'base-mainnet', 'status': 'BLOCKED',
                                    'reason': 'HTTP_REFUSED', 'endpoint': SECRET, 'unknown_raw': SECRET}
            result = c.collect(SECRET)
        text = json.dumps(result)
        self.assertNotIn(SECRET, text); self.assertNotIn('secret-base64', text)
        self.assertFalse(result['raw_response_bytes_exported'])
        self.assertFalse(result['provider_identity_attested'])
        self.assertFalse(result['runtime_activation'])
        self.assertIn('response_sha256', result['requests'][0])

    def test_original_transport_does_not_retain_raw_success_or_rpc_error(self):
        for raw in [b'{"jsonrpc":"2.0","id":0,"result":"0x2105","extra":"private-credential"}',
                    b'{"jsonrpc":"2.0","id":0,"error":{"message":"private-credential"}}']:
            client = c.OperatorBaseRpc(SECRET)
            with patch.object(c.observer.urllib.request, 'build_opener') as opener, patch.object(c.observer.time, 'sleep'):
                response = opener.return_value.open.return_value.__enter__.return_value
                response.status = 200; response.read.return_value = raw
                try: client.call('eth_chainId', [])
                except c.observer.ObservationError: pass
                self.assertNotIn('response_base64', client.calls[0])
                self.assertNotIn('private-credential', json.dumps(client.calls))
                request = opener.return_value.open.call_args.args[0]
                self.assertEqual(request.full_url, SECRET)

    def test_refusal_stops_without_fallback_and_does_not_echo_key(self):
        with patch.object(c.observer.urllib.request, 'build_opener') as opener, patch.object(c.observer.time, 'sleep'):
            opener.return_value.open.side_effect = urllib.error.HTTPError(SECRET, 403, SECRET, {}, io.BytesIO(SECRET.encode()))
            result = c.collect(SECRET)
        self.assertEqual(result['status'], 'BLOCKED')
        self.assertEqual(result['reason'], 'HTTP_REFUSED')
        self.assertEqual(len(result['requests']), 1)
        self.assertEqual(opener.return_value.open.call_count, 1)
        self.assertNotIn('private-credential', json.dumps(result))

    def test_forbidden_methods_never_reach_transport(self):
        client = c.OperatorBaseRpc(SECRET)
        for method in ['eth_sendRawTransaction', 'getMultipleAccounts']:
            with patch.object(c.observer.PublicRpc, 'call') as upstream:
                with self.assertRaises(c.observer.ObservationError): client.call(method, [])
                upstream.assert_not_called()

    def test_existing_decoder_validates_complete_synthetic_base_path(self):
        from test_inspect_pool_candidates import FakeBase
        fixture = FakeBase()
        with patch.object(c.observer.PublicRpc, 'call', side_effect=fixture.call):
            result = c.collect(SECRET)
        self.assertEqual(result['status'], 'OBSERVATIONS_COLLECTED')
        self.assertEqual(len(result['pools']), 2)
        self.assertTrue(result['canonical_recheck_passed'])
        self.assertFalse(result['production_qualified'])
        self.assertNotIn(SECRET, json.dumps(result))
        self.assertGreater(len(fixture.calls), 10)

    def test_output_symlink_is_never_followed(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, {c.SECRET_NAME: SECRET}), patch.object(c, 'collect') as run:
            existing = Path(root) / 'retained'; existing.write_text('retained')
            target = Path(root) / 'link'; target.symlink_to(existing)
            code, _ = self.invoke(['--collect', '--output', str(target)])
            self.assertEqual(code, 2)
            self.assertEqual(existing.read_text(), 'retained')
            run.assert_not_called()

    def test_success_and_blocked_reports_stay_nonproduction_with_private_mode(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, {c.SECRET_NAME: SECRET}):
            for index, status in enumerate(['OBSERVATIONS_COLLECTED', 'BLOCKED']):
                target = Path(root) / f'{index}.json'
                result = {'status': status, 'requests': [], 'production_qualified': False}
                with patch.object(c, 'collect', return_value=result):
                    code, text = self.invoke(['--collect', '--output', str(target)])
                self.assertEqual(code, 0 if index == 0 else 2)
                self.assertEqual(json.loads(target.read_text()), result)
                self.assertEqual(target.stat().st_mode & 0o777, 0o600)
                self.assertNotIn(SECRET, text)


if __name__ == '__main__':
    unittest.main()
