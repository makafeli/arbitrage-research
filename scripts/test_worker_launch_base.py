#!/usr/bin/env python3
"""Offline launcher boundaries. Test doubles are not provider/runtime evidence."""
import copy
import io
import json
import os
import stat
from types import SimpleNamespace
from pathlib import Path
import tempfile
import signal
import time
import unittest
from contextlib import redirect_stdout, redirect_stderr
from unittest.mock import patch

import worker_launch_base as launch

SID = '12345678-1234-4234-9234-123456789abc'
ENV = {'ARB_OPERATOR_ID': 'operator', 'ARB_BASE_PROFILE_DIGEST': 'sha256:'+'a'*64,
       'ARB_BASE_RPC_URL': 'https://rpc.invalid/private-secret',
       'ARB_DATABASE_URL': 'postgres://user:db-secret@db.internal/arb',
       'ARB_RPC_MIN_INTERVAL_MS': '75'}
SESSION = {'status': 'BASE_SESSION_REGISTERED', 'registration_requested': False,
           'session': {'session_id': SID, 'mode': 'OBSERVE', 'network_id': 'base-mainnet',
                       'configuration_digest': ENV['ARB_BASE_PROFILE_DIGEST'], 'execution_authorized': False}}
SOURCE = {'status': 'STATUS', 'state': 'ACTIVE', 'dataset_origin': 'RECORDED_LIVE',
          'execution_authorized': False}


class LaunchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.pools = [{'pool': 'synthetic-one'}, {'pool': 'synthetic-two'}]
        (self.root/'registry.json').write_text(json.dumps({'pools': self.pools}))
        (self.root/'ingestion-registry.json').write_text(json.dumps(self.pools))
        self.calls = []

    def runner(self, argv, env):
        self.calls.append((argv, env))
        value = SESSION if 'worker-session' in argv[0] else SOURCE
        return 0, json.dumps(value).encode(), b''

    def test_restart_uses_original_session_and_never_initializes(self):
        env = launch.prepare(ENV, '--start', self.root, self.runner)
        self.assertEqual(env['ARB_SESSION_ID'], SID)
        self.assertEqual([x[0][1] for x in self.calls], ['--status', '--status'])
        self.assertEqual(env['ARB_BASE_MANAGED_INGESTION'], 'true')
        self.assertEqual(env['ARB_INGEST_STREAM_ID'], env['ARB_BASE_INGESTION_STREAM'])
        self.assertEqual(env['ARB_DATABASE_URL'], env['ARB_INGEST_DATABASE_URL'])

    def test_first_use_initializes_exactly_once_before_status(self):
        launch.prepare(ENV, '--initialize-and-start', self.root, self.runner)
        self.assertEqual([x[0][1] for x in self.calls], ['--status', '--initialize', '--status'])

    def test_uncertain_initialization_does_not_retry_or_launch(self):
        def runner(argv, env):
            if '--initialize' in argv:
                self.calls.append((argv, env))
                return 2, b'', b'private-provider-secret'
            return self.runner(argv, env)
        with self.assertRaisesRegex(launch.LaunchError, 'SOURCE_INITIALIZATION_REFUSED_OR_UNCERTAIN'):
            launch.prepare(ENV, '--initialize-and-start', self.root, runner)
        self.assertEqual(len(self.calls), 2)

    def test_session_failure_prevents_source_mutation(self):
        with self.assertRaisesRegex(launch.LaunchError, 'REGISTERED_SESSION_REQUIRED'):
            launch.prepare(ENV, '--initialize-and-start', self.root, lambda *_: (2,b'',b'secret'))

    def test_wrong_session_scope_and_anchor_are_rejected(self):
        for key, value in [('mode','LIVE'),('network_id','solana-mainnet'),
                           ('configuration_digest','sha256:'+'b'*64),('execution_authorized',True)]:
            item = copy.deepcopy(SESSION); item['session'][key] = value
            with self.subTest(key=key), self.assertRaises(launch.LaunchError):
                launch.prepare(ENV, '--start', self.root, lambda *_: (0,json.dumps(item).encode(),b''))

    def test_registry_mismatch_prevents_initialization(self):
        (self.root/'ingestion-registry.json').write_text('[{},{}]')
        with self.assertRaisesRegex(launch.LaunchError, 'INGESTION_REGISTRY_MISMATCH'):
            launch.prepare(ENV, '--initialize-and-start', self.root, self.runner)
        self.assertEqual(len(self.calls), 1)

    def test_registry_symlink_rejected(self):
        (self.root/'ingestion-registry.json').unlink()
        (self.root/'ingestion-registry.json').symlink_to(self.root/'registry.json')
        with self.assertRaisesRegex(launch.LaunchError, 'PROFILE_FILE_REJECTED'):
            launch.prepare(ENV, '--start', self.root, self.runner)

    def test_duplicate_registry_key_rejected(self):
        (self.root/'registry.json').write_text('{"pools":[],"pools":[]}')
        with self.assertRaisesRegex(launch.LaunchError, 'DUPLICATE_PROFILE_KEY'):
            launch.prepare(ENV, '--start', self.root, self.runner)

    def test_halted_missing_and_synthetic_sources_do_not_launch(self):
        for source in [dict(SOURCE, state='HALTED'), dict(SOURCE, dataset_origin='MANUALLY_CONSTRUCTED'),None]:
            def runner(argv,env):
                if 'base-ingest' in argv[0]:
                    return (2,b'',b'not found') if source is None else (0,json.dumps(source).encode(),b'')
                return self.runner(argv,env)
            with self.subTest(source=source), self.assertRaises(launch.LaunchError):
                launch.prepare(ENV, '--start', self.root, runner)

    def test_private_environment_has_no_account_signer_or_proxy_settings(self):
        env = launch.environment({**ENV, 'HTTPS_PROXY':'https://proxy.invalid',
                                   'ARB_OPERATOR_SECRET':'private', 'PGOPTIONS':'unsafe',
                                   'ARB_SESSION_ID':'foreign', 'PYTHONPATH':'foreign'})
        for key in ['HTTPS_PROXY','ARB_OPERATOR_SECRET','PGOPTIONS','ARB_SESSION_ID','PYTHONPATH']:
            self.assertNotIn(key, env)
        self.assertTrue(env['ARB_DATABASE_URL'].endswith('?sslmode=require'))

    def test_missing_endpoint_and_weak_database_modes_rejected(self):
        for key in ENV:
            with self.subTest(key=key), self.assertRaises((launch.LaunchError, ValueError)):
                launch.environment({k:v for k,v in ENV.items() if k!=key})
        for mode in ['prefer','allow','disable','','require&sslmode=require']:
            with self.subTest(mode=mode), self.assertRaises(launch.LaunchError):
                launch.environment({**ENV,'ARB_DATABASE_URL':ENV['ARB_DATABASE_URL']+'?sslmode='+mode})

    def test_no_args_is_inert_and_unknown_args_are_redacted(self):
        for args in [[],['--check']]:
            with redirect_stdout(io.StringIO()) as output, patch.object(launch,'prepare') as work:
                self.assertEqual(launch.main(args), 0)
                self.assertEqual(json.loads(output.getvalue())['provider_requests'], 0)
                work.assert_not_called()
        with redirect_stderr(io.StringIO()) as output:
            self.assertEqual(launch.main(['https://private-secret']), 2)
        self.assertNotIn('private-secret', output.getvalue())

    def test_child_stdout_and_stderr_are_not_forwarded(self):
        code, out, err = launch.call(['/usr/bin/python3','-c','import sys;print("bounded");sys.stderr.write("secret")'],
                                     {'PATH':'/usr/bin:/bin'})
        self.assertEqual((code,out,err),(0,b'bounded\n',b'secret'))

    def test_cancellation_terminates_real_preparation_child(self):
        previous = signal.signal(signal.SIGALRM, launch.cancelled)
        begin = time.monotonic()
        signal.setitimer(signal.ITIMER_REAL, 0.1)
        try:
            with self.assertRaisesRegex(launch.LaunchError, 'LAUNCH_CANCELLED'):
                launch.call(['/usr/bin/python3', '-c', 'import time;time.sleep(10)'], {'PATH':'/usr/bin:/bin'})
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            signal.signal(signal.SIGALRM, previous)
        self.assertLess(time.monotonic()-begin, 3)

    def test_regular_profile_json_is_read(self):
        self.assertEqual(launch.read_json(self.root/'registry.json'), {'pools': self.pools})

    def test_profile_read_remains_bounded_after_metadata_check(self):
        path = self.root/'large.json'
        path.write_text('{"large":"' + 'x' * 1048576 + '"}')
        stale = SimpleNamespace(st_mode=stat.S_IFREG | 0o600, st_size=0)
        # This reproduces growth between the old lstat and read_bytes calls.
        with patch.object(Path, 'lstat', return_value=stale), \
             patch.object(launch.os, 'fstat', return_value=stale):
            with self.assertRaisesRegex(launch.LaunchError, 'PROFILE_FILE_REJECTED'):
                launch.read_json(path)

    def test_fifo_and_directory_profiles_are_refused_without_blocking(self):
        fifo = self.root/'profile.fifo'
        os.mkfifo(fifo)
        for path in [fifo, self.root]:
            with self.subTest(path=path), self.assertRaisesRegex(launch.LaunchError, 'PROFILE_FILE_REJECTED'):
                launch.read_json(path)

    def test_missing_profile_has_fixed_error_code(self):
        with self.assertRaisesRegex(launch.LaunchError, 'PROFILE_FILE_REJECTED'):
            launch.read_json(self.root/'missing.json')

    def test_invalid_session_id_prevents_initialization(self):
        for sid in ['', 'foreign-session', SID.upper()]:
            item = copy.deepcopy(SESSION); item['session']['session_id'] = sid
            calls = []
            def runner(argv, env):
                calls.append(argv)
                return 0, json.dumps(item).encode(), b''
            with self.subTest(sid=sid), self.assertRaises((launch.LaunchError, ValueError)):
                launch.prepare(ENV, '--initialize-and-start', self.root, runner)
            self.assertEqual(len(calls), 1)

    def test_unexpected_registration_receipt_prevents_source_access(self):
        for change in [{'status': 'NOT_STARTED'}, {'registration_requested': True}]:
            item = {**SESSION, **change}
            with self.subTest(change=change), self.assertRaisesRegex(launch.LaunchError, 'SESSION_PROFILE_MISMATCH'):
                launch.prepare(ENV, '--start', self.root, lambda *_: (0, json.dumps(item).encode(), b''))

    def test_source_execution_authorization_is_rejected(self):
        def runner(argv, env):
            value = SESSION if 'worker-session' in argv[0] else dict(SOURCE, execution_authorized=True)
            return 0, json.dumps(value).encode(), b''
        with self.assertRaisesRegex(launch.LaunchError, 'ACTIVE_RECORDED_SOURCE_REQUIRED'):
            launch.prepare(ENV, '--start', self.root, runner)

    def test_privileged_launcher_is_refused_before_preparation(self):
        with patch.object(launch.os, 'getuid', return_value=0), \
             patch.object(launch, 'prepare') as work, redirect_stderr(io.StringIO()) as output:
            self.assertEqual(launch.main(['--start']), 2)
        self.assertEqual(json.loads(output.getvalue())['reason'], 'UNPRIVILEGED_WORKER_REQUIRED')
        work.assert_not_called()

    def test_real_child_output_is_bounded(self):
        code, out, err = launch.call(['/usr/bin/python3','-c','import sys;sys.stdout.write("x"*100000)'],
                                     {'PATH':'/usr/bin:/bin'})
        self.assertNotEqual(code, 0)
        self.assertLessEqual(len(out), launch.MAX_OUTPUT)
        self.assertLessEqual(len(err), launch.MAX_OUTPUT)


if __name__ == '__main__':
    unittest.main()
