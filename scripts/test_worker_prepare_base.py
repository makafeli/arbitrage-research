#!/usr/bin/env python3
"""Synthetic preparation tests; --container uses the shipped Rust parser offline."""
import contextlib
import copy
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import Mock, patch

import worker_prepare_base as p


def sample():
    inventory = json.loads((p.ROOT/'docs/registries/initial-identities.json').read_text())
    chain = next(c for c in inventory['chains'] if c['network_id'] == 'base-mainnet')
    report = {'network': 'base-mainnet', 'status': 'OBSERVATIONS_COLLECTED',
              'canonical_recheck_passed': True,
              'factory': {'address': chain['venue']['identity']['address'],
                          'runtime_sha256': chain['venue']['observed_runtime_sha256']},
              'assets': [{'address': a['identity']['address'], 'decimals': a['decimals'],
                          'runtime_sha256': a['observed_runtime_sha256']} for a in chain['assets']],
              'pools': [{'address': pool['identity']['address'], 'token0': p.profile.WETH,
                         'token1': p.profile.USDC, 'fee_millionths': pool['fee_millionths'],
                         'tick_spacing': pool['tick_spacing'],
                         'runtime_sha256': pool['observed_runtime_sha256'], 'tick': -200001,
                         'status': 'IDENTITY_AND_ACTIVE_LIQUIDITY_OBSERVED'} for pool in chain['pools']],
              'requests': [{'response_sha256': 'sha256:' + 'a'*64} for _ in range(3)]}
    return inventory, report


def fake_check(directory):
    return {'status': 'PROFILE_VALIDATED',
            'configuration_digest': p.sha((directory/'configuration.toml').read_bytes()),
            'registry_digest': p.sha((directory/'registry.json').read_bytes()),
            'mode': 'OBSERVE', 'network_id': 'base-mainnet', 'execution_authorized': False}


class PreparationTests(unittest.TestCase):
    def test_default_is_inert(self):
        with patch.object(p.access, 'endpoint_from_environment') as endpoint, patch.object(p, 'prepare') as prepare, contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(p.main([]), 0)
        endpoint.assert_not_called(); prepare.assert_not_called()
        self.assertEqual(json.loads(output.getvalue())['provider_requests'], 0)

    def test_unknown_arguments_never_contact_provider(self):
        with patch.object(p.access, 'endpoint_from_environment') as endpoint, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(p.main(['--start']), 2)
        endpoint.assert_not_called()

    def test_missing_checker_blocks_before_secret_resolution(self):
        with patch.object(p, 'CHECKER', '/does-not-exist'), patch.object(p.access, 'endpoint_from_environment') as endpoint, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(p.main(['--prepare']), 2)
        endpoint.assert_not_called()

    def test_profile_matches_only_reviewed_base_identities(self):
        inventory, report = sample(); files = p.render(report, inventory)
        config = tomllib.loads(files['configuration.toml'].decode())
        self.assertEqual(config['deployment']['mode'], 'OBSERVE')
        self.assertEqual(config['storage']['database_secret_reference'], 'env:ARB_DATABASE_URL')
        self.assertEqual(config['storage']['capture_directory'], '/data/captures')
        self.assertEqual(config['storage']['capture_quota_bytes'], 134217728)
        self.assertEqual(config['research']['trade_sizes_minor'], ['1000000'])
        self.assertEqual(config['networks']['base']['registry_qualification_digest'], p.sha(files['registry.json']))
        self.assertFalse(config['networks']['solana']['enabled'])
        for key in ('enabled', 'signer_enabled', 'broadcast_enabled', 'flash_loans_enabled'):
            self.assertFalse(config['execution'][key])
        registry = json.loads(files['registry.json'])
        self.assertEqual(registry['pools'], json.loads(files['ingestion-registry.json']))
        for pool, observed in zip(registry['pools'], report['pools']):
            self.assertEqual(pool['bitmap_word_min'], observed['tick']//observed['tick_spacing']//256)
            self.assertEqual(pool['bitmap_word_min'], pool['bitmap_word_max'])

    def test_changed_identity_or_wrong_chain_is_rejected(self):
        inventory, original = sample()
        for mutate in (lambda r: r.update(network='ethereum-mainnet'),
                       lambda r: r['pools'][0].update(runtime_sha256='sha256:'+'0'*64),
                       lambda r: r['assets'][0].update(decimals=9),
                       lambda r: r.update(canonical_recheck_passed=False),
                       lambda r: r.update(pools=[r['pools'][0], r['pools'][0]])):
            report = copy.deepcopy(original); mutate(report)
            with self.assertRaises(p.ProfileError): p.render(report, inventory)

    def test_preparation_is_private_and_reuse_has_zero_provider_calls(self):
        _, report = sample(); collector = Mock(return_value=report)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            result = p.prepare(root, 'https://example.invalid/PRIVATE_VALUE', collector, fake_check)
            final = root/'base-v1'; before = {f.name: f.read_bytes() for f in final.iterdir()}
            reused = p.prepare(root, 'https://example.invalid/CHANGED_KEY', collector, fake_check)
            self.assertEqual(reused['status'], 'BASE_PROFILE_REUSED')
            self.assertFalse(reused['current_rpc_verified'])
            self.assertEqual(reused['provider_requests'], 0)
            self.assertEqual(before, {f.name: f.read_bytes() for f in final.iterdir()})
            collector.assert_called_once(); self.assertEqual(result['provider_requests'], 3)
            for f in final.iterdir():
                self.assertEqual(f.stat().st_mode & 0o777, 0o600)
                self.assertNotIn(b'PRIVATE_VALUE', f.read_bytes())
            self.assertNotIn('PRIVATE_VALUE', json.dumps(result))

    def test_changed_prepared_bytes_fail_without_rpc(self):
        _, report = sample(); collector = Mock(return_value=report)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); p.prepare(root, 'unused', collector, fake_check)
            (root/'base-v1'/'registry.json').write_text('{}')
            with self.assertRaisesRegex(p.ProfileError, 'EXISTING_PROFILE_CHANGED'):
                p.prepare(root, 'unused', collector, fake_check)
            self.assertEqual(collector.call_count, 1)

    def test_bad_identity_preserves_failure_and_publishes_no_profile(self):
        _, report = sample(); report['canonical_recheck_passed'] = False
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(p.ProfileError):
                p.prepare(root, 'unused', lambda _: report, fake_check)
            self.assertFalse((root/'base-v1').exists())
            failures = list(root.glob('.base-attempt-*/failure.json'))
            self.assertEqual(len(failures), 1)
            self.assertFalse(json.loads(failures[0].read_text())['automatic_retry'])

    def test_failed_rust_parser_does_not_publish(self):
        _, report = sample()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(p.ProfileError):
                p.prepare(root, 'unused', lambda _: report, Mock(side_effect=p.ProfileError('RUST_PROFILE_CHECK_FAILED')))
            self.assertFalse((root/'base-v1').exists())

    def test_nonprivate_or_symlink_root_is_rejected_before_rpc(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); collector = Mock(); root.chmod(0o755)
            with self.assertRaises(p.ProfileError): p.prepare(root, 'unused', collector, fake_check)
            root.chmod(0o700); link = root/'link'; link.symlink_to(root)
            with self.assertRaises(p.ProfileError): p.prepare(link, 'unused', collector, fake_check)
            collector.assert_not_called()

    def test_symlink_profile_is_rejected_before_rpc(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); (root/'base-v1').symlink_to(root); collector = Mock()
            with self.assertRaises(p.ProfileError): p.prepare(root, 'unused', collector, fake_check)
            collector.assert_not_called()

    def test_second_preparer_cannot_share_the_lock(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with (root/'.base-profile.lock').open('w') as held:
                p.fcntl.flock(held, p.fcntl.LOCK_EX | p.fcntl.LOCK_NB); collector = Mock()
                with self.assertRaisesRegex(p.ProfileError, 'PROFILE_PREPARATION_BUSY'):
                    p.prepare(root, 'unused', collector, fake_check)
                collector.assert_not_called()

    def test_child_does_not_receive_provider_or_account_environment(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); (root/'registry.json').write_text('{}')
            answer = {'status': 'PROFILE_VALIDATED', 'configuration_digest': 'sha256:'+'a'*64,
                      'registry_digest': p.sha(b'{}'), 'mode': 'OBSERVE',
                      'network_id': 'base-mainnet', 'execution_authorized': False}
            with patch.object(p.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, json.dumps(answer).encode())) as run:
                p.check_profile(root)
            self.assertEqual(set(run.call_args.kwargs['env']), {'PATH', 'HOME', 'LC_ALL'})
            self.assertNotIn('ARB_BASE_RPC_URL', str(run.call_args))


def container_check():
    _, report = sample(); root = Path(tempfile.mkdtemp(dir='/data/runtime'))
    result = p.prepare(root, 'https://fixture.invalid/NOT_A_KEY', lambda _: report)
    assert result['status'] == 'BASE_PROFILE_PREPARED'
    assert result['execution_authorized'] is False
    final = root/'base-v1'
    assert p.check_profile(final)['configuration_digest'] == result['configuration_digest']
    assert p.prepare(root, 'unused', Mock(side_effect=AssertionError('unexpected RPC')))['status'] == 'BASE_PROFILE_REUSED'
    (final/'registry.json').write_text('{}')
    try: p.check_profile(final)
    except p.ProfileError: pass
    else: raise AssertionError('changed registry was accepted')
    print('Synthetic profile passed actual shipped Rust validation; changed registry refused; no network or database.')


if __name__ == '__main__':
    if sys.argv[1:] == ['--container']: container_check()
    else: unittest.main()
