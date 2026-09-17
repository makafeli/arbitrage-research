#!/usr/bin/env python3
"""Offline readiness boundary tests; actual PostgreSQL is tested in containers CI."""
import copy
import io
import json
import os
import subprocess
import unittest
from contextlib import redirect_stderr
from types import SimpleNamespace
from unittest.mock import patch

import worker_readiness as ready


def fixture():
    return {'schema_version': 1, 'read_only': True, 'configuration_count': '0',
            'enabled_base_profile_count': '0', 'session_count': '0', 'stream_count': '0',
            'sessions': [], 'streams': []}


class ReadinessTests(unittest.TestCase):
    def test_empty_scoped_database_is_inspected_not_qualified(self):
        def runner(argv, **kwargs):
            self.assertNotIn('postgres://', ' '.join(argv))
            self.assertIn('READ ONLY', kwargs['input'].decode())
            self.assertIn('ROLLBACK;', kwargs['input'].decode())
            self.assertEqual(kwargs['timeout'], 15)
            self.assertIs(kwargs['preexec_fn'], ready.child_limits)
            self.assertIs(kwargs['stderr'], subprocess.DEVNULL)
            self.assertEqual(set(kwargs['env']), {'PATH', 'HOME', 'LC_ALL', 'PGDATABASE',
                                                'PGCONNECT_TIMEOUT', 'PGPASSFILE', 'PGOPTIONS', 'PGHOST', 'PGPORT',
                                                'PGUSER', 'PGPASSWORD', 'PGSSLMODE'})
            kwargs['stdout'].write(json.dumps(fixture()).encode())
            return SimpleNamespace(returncode=0)
        result = ready.inspect({'ARB_OPERATOR_ID': 'operator', 'ARB_DATABASE_URL': 'postgres://test:private@db/test'}, runner)
        self.assertEqual(result['status'], 'WORKER_READINESS_INSPECTED')
        self.assertFalse(result['runtime_qualified'])
        self.assertEqual(result['missing_settings'], list(ready.REQUIRED_SETTINGS))
        self.assertNotIn('private', json.dumps(result))

    def test_no_arbitrary_sql_or_account_tables(self):
        for word in ['operator_account', 'password_hash', 'INSERT ', 'UPDATE ', 'DELETE ', 'CREATE ', 'DROP ']:
            self.assertNotIn(word, ready.SQL)
        self.assertEqual(ready.SQL.count("operator_id=:'operator'"), 6)
        self.assertIn('statement_timeout', ready.SQL)
        self.assertIn('lock_timeout', ready.SQL)
        self.assertEqual(ready.SQL.count('LIMIT 20'), 2)

    def test_missing_operator_or_database_stops_before_subprocess(self):
        for env in [{}, {'ARB_OPERATOR_ID': 'operator'}, {'ARB_OPERATOR_ID': 'bad id', 'ARB_DATABASE_URL': 'postgres://db/db'},
                    {'ARB_OPERATOR_ID': 'operator', 'ARB_DATABASE_URL': 'https://private.invalid'}]:
            with self.subTest(env=env), self.assertRaises(ready.ReadinessError):
                ready.inspect(env, lambda *a, **k: self.fail('must not execute'))

    def test_unavailable_or_oversized_database_response_is_fixed_error(self):
        for mode in ['failed', 'timeout', 'oversized', 'malformed']:
            def runner(argv, **kwargs):
                if mode == 'timeout':
                    raise subprocess.TimeoutExpired('private-connection', 15)
                kwargs['stdout'].write(b'x' * (ready.MAX_OUTPUT + 1) if mode == 'oversized' else b'bad data')
                return SimpleNamespace(returncode=1 if mode == 'failed' else 0)
            with self.subTest(mode=mode), self.assertRaises(ready.ReadinessError) as caught:
                ready.inspect({'ARB_OPERATOR_ID': 'operator', 'ARB_DATABASE_URL': 'postgres://test:private@db/test'}, runner)
            self.assertNotIn('private-connection', str(caught.exception))

    def test_malformed_counts_extra_fields_or_writes_rejected(self):
        for change in [{'session_count': 0}, {'configuration_count': '-1'}, {'enabled_base_profile_count': '1'},
                       {'read_only': False}, {'password': 'never-show'}, {'schema_version': True},
                       {'session_count': '1'}, {'sessions': None}]:
            with self.subTest(change=change), self.assertRaises(ready.ReadinessError):
                ready.validate({**fixture(), **change})

    def test_session_and_source_reports_are_strict_and_bounded(self):
        value = fixture()
        value.update(configuration_count='1', session_count='1', stream_count='1')
        value['sessions'] = [dict(session_id='session-1', mode='OBSERVE', observed_state='STOPPED',
                                 configuration_digest='sha256:'+'a'*64, desired_revision='0',
                                 applied_revision='0', lease_active=False)]
        value['streams'] = [dict(stream_id='stream-1', state='HALTED', revision='2',
                                checkpoint_number='123', registry_digest='sha256:'+'b'*64)]
        self.assertEqual(ready.validate(value), value)
        for field, change in [('sessions', {'session_id': 'https://private.invalid'}),
                              ('sessions', {'configuration_digest': 'unknown'}),
                              ('sessions', {'mode': 'LIVE'}), ('sessions', {'applied_revision': '1'}),
                              ('streams', {'state': 'READY'}), ('streams', {'checkpoint_number': None})]:
            bad = copy.deepcopy(value)
            bad[field][0].update(change)
            with self.subTest(change=change), self.assertRaises(ready.ReadinessError):
                ready.validate(bad)

    def test_missing_settings_only_reports_names(self):
        def runner(argv, **kwargs):
            kwargs['stdout'].write(json.dumps(fixture()).encode())
            return SimpleNamespace(returncode=0)
        env = {'ARB_OPERATOR_ID': 'operator', 'ARB_DATABASE_URL': 'postgres://test:private@db/test',
               **{name: 'private-value' for name in ready.REQUIRED_SETTINGS}}
        result = ready.inspect(env, runner)
        self.assertEqual(result['missing_settings'], [])
        self.assertFalse(result['runtime_qualified'])
        self.assertNotIn('private-value', json.dumps(result))

    def test_connection_options_cannot_inject_child_configuration(self):
        for suffix in ['?options=-c+default_transaction_read_only=off',
                       '?sslmode=disable', '?sslmode=require&sslmode=prefer', '#private']:
            with self.subTest(suffix=suffix), self.assertRaises(ready.ReadinessError):
                ready.inspect({'ARB_OPERATOR_ID': 'operator',
                               'ARB_DATABASE_URL': 'postgres://test:private@db/test' + suffix},
                              lambda *a, **k: self.fail('must not run'))

    def test_main_refuses_arguments_and_redacts_environment(self):
        output = io.StringIO()
        with patch.dict(os.environ, {}, clear=True), redirect_stderr(output):
            self.assertEqual(ready.main([]), 2)
            self.assertEqual(ready.main(['--initialize']), 2)
        self.assertNotIn('Traceback', output.getvalue())


if __name__ == '__main__':
    unittest.main()
