"""Offline checks for the bounded diagnostic; mocks are not provider evidence."""
import json
from pathlib import Path
import subprocess
import sys
import unittest

import probe_qualification as q


class QualificationTests(unittest.TestCase):
    def test_default_cli_does_not_probe(self):
        run = subprocess.run([sys.executable, q.__file__], capture_output=True, text=True, timeout=5)
        self.assertEqual(run.returncode, 0)
        self.assertEqual(json.loads(run.stdout)['status'], 'NOT_RUN')

    def test_success_and_error_latencies_are_separate(self):
        result = q.distribution([{'outcome': 'success', 'elapsed_ms': 10},
                                 {'outcome': 'http-rejected', 'elapsed_ms': 999}])
        self.assertEqual(result['successful_rpc']['p99_ms'], 10)
        self.assertEqual(result['failed_attempts']['p50_ms'], 999)
        self.assertIsNone(q.distribution([])['successful_rpc']['p50_ms'])

    def test_identity_failure_does_not_probe_state_or_echo_errors(self):
        seen = []
        def call(chain, method, params, sequence):
            seen.append((chain, method))
            return {'outcome': 'http-rejected', 'http_status': 403, 'response_body': 'private-upstream-text'}
        result = q.sample('base', call)
        self.assertEqual(len(seen), 1)
        self.assertNotIn('private-upstream-text', json.dumps(result))

    def test_base_is_hash_pinned_and_bounded(self):
        calls = []
        def call(chain, method, params, sequence):
            calls.append((method, params, sequence))
            value = '0x2105' if method == 'eth_chainId' else {'hash': '0x' + 'a'*64} if method == 'eth_getBlockByNumber' else '0x00'
            return {'outcome': 'success', 'result': value}
        self.assertEqual(len(q.sample('base', call)), 5)
        self.assertEqual([c[2] for c in calls], list(range(5)))
        self.assertTrue(all(c[1][-1] == {'blockHash':'0x'+'a'*64, 'requireCanonical':True} for c in calls[2:]))

    def test_base_stops_immediately_after_first_pool_refusal(self):
        calls = []
        def call(chain, method, params, sequence):
            calls.append(method)
            if method == 'eth_call':
                return {'outcome': 'http-rejected', 'http_status': 403}
            value = '0x2105' if method == 'eth_chainId' else {'hash': '0x' + 'a'*64} if method == 'eth_getBlockByNumber' else '0x00'
            return {'outcome': 'success', 'result': value}
        q.sample('base', call)
        self.assertEqual(calls.count('eth_call'), 1)

    def test_solana_is_bounded_contextual_and_read_only(self):
        calls = []
        def call(chain, method, params, sequence):
            calls.append((method, params))
            return {'outcome': 'success', 'result': q.SOLANA_GENESIS if method == 'getGenesisHash' else None}
        self.assertEqual(len(q.sample('solana', call)), 3)
        self.assertEqual(calls[-1][0], 'getProgramAccounts')
        self.assertTrue(calls[-1][1][1]['withContext'])
        self.assertEqual(calls[-1][1][1]['dataSlice']['length'], 245)

    def test_solana_missing_null_and_foreign_identity_stop_before_state(self):
        for result in ({'outcome': 'success'}, {'outcome': 'success', 'result': None},
                       {'outcome': 'success', 'result': 'other-cluster'}):
            calls = q.sample('solana', lambda *_, result=result: result.copy())
            self.assertEqual(len(calls), 1)
            self.assertEqual(calls[0]['outcome'], 'unexpected-chain-identity')

    def test_wrong_chain_identity_stops_state_calls(self):
        self.assertEqual(len(q.sample('base', lambda *_: {'outcome':'success','result':'0x1'})), 1)
        with self.assertRaises(ValueError):
            q.sample('other')

    def test_profile_storage_estimate_and_service_isolation(self):
        profile = json.loads((Path(__file__).parents[1] / 'benchmark-environment.json').read_text())
        s = profile['storage_assumptions']; c = s['captures_per_second'] * s['seconds_per_day']
        raw = c * s['raw_bytes_per_capture']; manifest = c * s['manifest_bytes_per_capture']; summary = c * s['summary_bytes_per_capture']
        self.assertEqual(raw, 11324620800)
        total = raw * s['raw_days'] + manifest * s['manifest_days'] + summary * s['summary_days'] * s['database_expansion_factor'] + (raw + manifest + summary) * s['daily_backup_sets']
        self.assertEqual(total, profile['storage_estimate_bytes']['without_reserve'])
        self.assertEqual((total*13+9)//10, profile['storage_estimate_bytes']['with_30_percent_reserve'])
        self.assertLess((total*13+9)//10, profile['host_hypothesis']['disk_bytes'])
        self.assertEqual(len({x['name'] for x in profile['services']}), 5)
        self.assertFalse(profile['deployment_applied'])
        self.assertFalse(profile['performance_qualified'])


if __name__ == '__main__':
    unittest.main()
