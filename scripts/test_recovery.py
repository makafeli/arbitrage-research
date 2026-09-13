#!/usr/bin/env python3
"""Offline adversarial tests; database commands are mocked, not claimed as integration."""
import copy
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch
import recovery as r
HEAD = 'a' * 40
EVIDENCE = {'relations': [{'schema': 'public', 'name': 'fixture', 'kind': 'r', 'rows': 1, 'bytes': 3, 'sha256': r.digest(b'42\n')}]}

class RecoveryTests(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.captures = self.root / 'source'
        self.captures.mkdir()
        (self.captures / 'capture-1').mkdir()
        (self.captures / 'capture-1' / 'manifest.json').write_bytes(b'{"origin":"SYNTHETIC","expires_at":"2000-01-01"}\n')
        (self.captures / 'capture-1' / 'data.bin').write_bytes(bytes(range(256)))
        self.bundle = self.root / 'bundle'

    def make_backup(self, **kwargs):

        def execute(args, *_a, **_kw):
            if args[0] == 'pg_dump':
                Path(args[args.index('--file') + 1]).write_bytes(b'PGDMP-not-a-real-dump-fixture')
        with patch('recovery.sql', return_value=b'170011\n'), patch('recovery.db_evidence', return_value=EVIDENCE), patch('recovery.command', side_effect=execute):
            result = r.backup('research', self.captures, self.bundle, HEAD, True, **kwargs)
        return result['manifest_sha256']

    def rewrite_manifest(self, change):
        manifest = json.loads((self.bundle / 'manifest.json').read_bytes())
        change(manifest)
        raw = r.canonical(manifest)
        (self.bundle / 'manifest.json').write_bytes(raw)
        return r.digest(raw)

    def test_backup_verifies_exact_bytes_permissions_and_retention(self):
        sha = self.make_backup()
        manifest, evidence = r.verify(self.bundle, sha)
        self.assertEqual(evidence, EVIDENCE)
        self.assertEqual(manifest['source_commit'], HEAD)
        self.assertEqual(manifest['capture_validation'], 'FILE_BYTES_ONLY')
        for path in self.captures.rglob('*'):
            if path.is_file():
                self.assertEqual(path.read_bytes(), (self.bundle / 'captures' / path.relative_to(self.captures)).read_bytes())
        self.assertEqual(stat.S_IMODE(self.bundle.stat().st_mode), 448)
        self.assertEqual(stat.S_IMODE((self.bundle / 'manifest.json').stat().st_mode), 384)
        self.assertIn(b'2000-01-01', (self.bundle / 'captures/capture-1/manifest.json').read_bytes())

    def test_new_output_never_overwrites_existing_directory(self):
        self.bundle.mkdir()
        sentinel = self.bundle / 'keep'
        sentinel.write_bytes(b'original')
        with self.assertRaises(FileExistsError):
            self.make_backup()
        self.assertEqual(sentinel.read_bytes(), b'original')

    def test_backup_requires_quiescence_and_exact_source_before_commands(self):
        with patch('recovery.command') as command:
            for sha, stopped in [(HEAD, False), ('main', True), (None, True)]:
                with self.subTest(sha=sha), self.assertRaises(r.RecoveryError):
                    r.backup('research', self.captures, self.bundle, sha, stopped)
            command.assert_not_called()

    def test_output_inside_captures_is_rejected(self):
        with self.assertRaises(r.RecoveryError):
            r.backup('research', self.captures, self.captures / 'out', HEAD, True)

    def test_failed_dump_has_no_completed_manifest(self):
        with patch('recovery.sql', return_value=b'170011'), patch('recovery.db_evidence', return_value=EVIDENCE), patch('recovery.command', side_effect=r.RecoveryError('fixture failure')):
            with self.assertRaises(r.RecoveryError):
                r.backup('research', self.captures, self.bundle, HEAD, True)
        self.assertFalse((self.bundle / 'manifest.json').exists())

    def test_database_change_has_no_completed_manifest(self):

        def dump(args, *_a, **_kw):
            Path(args[-1]).write_bytes(b'PGDMP-fixture')
        with patch('recovery.sql', return_value=b'170011'), patch('recovery.db_evidence', side_effect=[EVIDENCE, {'relations': []}]), patch('recovery.command', side_effect=dump):
            with self.assertRaisesRegex(r.RecoveryError, 'Database changed'):
                r.backup('research', self.captures, self.bundle, HEAD, True)
        self.assertFalse((self.bundle / 'manifest.json').exists())

    def test_capture_change_has_no_completed_manifest(self):

        def dump(args, *_a, **_kw):
            Path(args[-1]).write_bytes(b'PGDMP-fixture')
            (self.captures / 'capture-1/data.bin').write_bytes(b'changed')
        with patch('recovery.sql', return_value=b'170011'), patch('recovery.db_evidence', return_value=EVIDENCE), patch('recovery.command', side_effect=dump):
            with self.assertRaises(r.RecoveryError):
                r.backup('research', self.captures, self.bundle, HEAD, True)
        self.assertFalse((self.bundle / 'manifest.json').exists())

    def test_wrong_manifest_digest_fails(self):
        self.make_backup()
        for sha in ['b' * 64, '', 'sha256:' + 'a' * 64, 'xyz']:
            with self.subTest(sha=sha), self.assertRaises(r.RecoveryError):
                r.verify(self.bundle, sha)

    def test_missing_extra_and_modified_files_fail(self):
        sha = self.make_backup()
        path = self.bundle / 'captures/capture-1/data.bin'
        original = path.read_bytes()
        for mode in ['missing', 'modified', 'extra']:
            with self.subTest(mode=mode):
                if mode == 'missing':
                    path.unlink()
                elif mode == 'modified':
                    path.write_bytes(b'bad')
                else:
                    (self.bundle / 'extra').write_bytes(b'bad')
                with self.assertRaises((r.RecoveryError, OSError)):
                    r.verify(self.bundle, sha)
                path.write_bytes(original)
                (self.bundle / 'extra').unlink(missing_ok=True)

    def test_invalid_manifests_fail_even_with_recomputed_digest(self):
        sha = self.make_backup()
        original = (self.bundle / 'manifest.json').read_bytes()
        changes = [lambda m: m.update(extra='unexpected'), lambda m: m.update(created_at='not-a-date'), lambda m: m.update(schema_version=True), lambda m: m.update(kind='OTHER'), lambda m: m['files'].append(copy.deepcopy(m['files'][0])), lambda m: m['files'].pop(0), lambda m: m['files'][0].update(bytes=-1), lambda m: m['files'][0].update(bytes=True), lambda m: m['files'][0].update(sha256='wrong'), lambda m: m.update(quiescence='ATOMIC_GUARANTEED')]
        for change in changes:
            with self.subTest(change=change):
                (self.bundle / 'manifest.json').write_bytes(original)
                changed = self.rewrite_manifest(change)
                with self.assertRaises((r.RecoveryError, ValueError, TypeError)):
                    r.verify(self.bundle, changed)
        (self.bundle / 'manifest.json').write_bytes(original)
        r.verify(self.bundle, sha)

    def test_unsafe_paths_fail(self):
        for path in ['', '/tmp/a', '../x', 'a/../b', 'a//b', 'a\\b', 'a/./b', '.', 'a/\x00b', '/'.join(['a'] * 65)]:
            with self.subTest(path=path), self.assertRaises(r.RecoveryError):
                r.relative(path)

    def test_duplicate_json_keys_fail(self):
        self.make_backup()
        raw = b'{"schema_version":1,"schema_version":2}'
        (self.bundle / 'manifest.json').write_bytes(raw)
        with self.assertRaises(r.RecoveryError):
            r.verify(self.bundle, r.digest(raw))

    def test_file_and_directory_symlinks_and_hardlinks_fail(self):
        for kind in ['file-link', 'directory-link', 'hardlink', 'fifo']:
            path = self.captures / 'unsafe'
            if kind == 'file-link':
                path.symlink_to(self.captures / 'capture-1/data.bin')
            elif kind == 'directory-link':
                path.symlink_to(self.captures / 'capture-1', target_is_directory=True)
            elif kind == 'hardlink':
                os.link(self.captures / 'capture-1/data.bin', path)
            else:
                os.mkfifo(path)
            with self.subTest(kind=kind), self.assertRaises((r.RecoveryError, OSError)):
                r.inventory(self.captures, r.Limits())
            path.unlink()

    def test_root_symlink_is_rejected(self):
        path = self.root / 'alias'
        path.symlink_to(self.captures, target_is_directory=True)
        with self.assertRaises(r.RecoveryError):
            r.root_path(path)

    def test_bounded_files_bytes_and_invalid_limits(self):
        for limits in [r.Limits(files=1), r.Limits(bytes=1)]:
            with self.assertRaises(r.RecoveryError):
                r.inventory(self.captures, limits)
        for value in [True, 0, -1, 1.5]:
            with self.assertRaises(r.RecoveryError):
                r.Limits(files=value)
        with self.assertRaises(r.RecoveryError):
            r.Limits(seconds=3601)
        sha = self.make_backup()
        with self.assertRaises(r.RecoveryError):
            r.verify(self.bundle, sha, r.Limits(bytes=20))

    def test_connection_strings_and_option_injection_are_rejected(self):
        for database in ['postgres://user:secret@host/db', 'service=prod', '-dpostgres', 'a;DROP TABLE x', 'a b', 'x' * 64, None]:
            with self.subTest(database=database), self.assertRaises(r.RecoveryError):
                r.database_name(database)

    def test_restore_requires_explicit_target_and_trust_before_commands(self):
        sha = self.make_backup()
        with patch('recovery.sql') as sql:
            for target, confirm, trusted in [('production', 'production', True), ('arb_restore_test', 'wrong', True), ('arb_restore_test', 'arb_restore_test', False)]:
                with self.subTest(target=target), self.assertRaises(r.RecoveryError):
                    r.restore(self.bundle, sha, target, confirm, self.root / 'restored', trusted)
            sql.assert_not_called()

    def test_restore_rejects_tampered_bundle_before_database_access(self):
        sha = self.make_backup()
        (self.bundle / 'database.dump').write_bytes(b'bad')
        with patch('recovery.sql') as sql, self.assertRaises(r.RecoveryError):
            r.restore(self.bundle, sha, 'arb_restore_test', 'arb_restore_test', self.root / 'restored', True)
        sql.assert_not_called()

    def test_restore_rejects_nonempty_or_wrong_major(self):
        sha = self.make_backup()
        for outputs in [[b'160001'], [b'170011', b'1']]:
            with patch('recovery.sql', side_effect=outputs), patch('recovery.command') as command, self.assertRaises(r.RecoveryError):
                r.restore(self.bundle, sha, 'arb_restore_test', 'arb_restore_test', self.root / 'restored', True)
            command.assert_not_called()

    def test_restore_refuses_existing_destination_and_source_database(self):
        sha = self.make_backup()
        with patch('recovery.sql') as sql, self.assertRaises(r.RecoveryError):
            r.restore(self.bundle, sha, 'arb_restore_test', 'arb_restore_test', self.captures, True)
        sql.assert_not_called()
        sha = self.rewrite_manifest(lambda m: m.update(source_database='arb_restore_test'))
        with patch('recovery.sql') as sql, self.assertRaises(r.RecoveryError):
            r.restore(self.bundle, sha, 'arb_restore_test', 'arb_restore_test', self.root / 'new', True)
        sql.assert_not_called()

    def test_restore_uses_transaction_and_never_cleans_drops_or_starts_workers(self):
        sha = self.make_backup()
        dest = self.root / 'restored'
        with patch('recovery.sql', side_effect=[b'170011', b'0']), patch('recovery.command') as command, patch('recovery.db_evidence', return_value=EVIDENCE):
            report = r.restore(self.bundle, sha, 'arb_restore_test', 'arb_restore_test', dest, True)
        args = command.call_args.args[0]
        self.assertIn('--single-transaction', args)
        self.assertIn('--exit-on-error', args)
        self.assertNotIn('--clean', args)
        self.assertNotIn('--create', args)
        self.assertEqual(report['workers_started'], 0)
        self.assertFalse(report['production_accepted'])
        self.assertEqual(r.inventory(dest, r.Limits()), r.inventory(self.captures, r.Limits()))

    def test_restore_mismatch_never_reports_success(self):
        sha = self.make_backup()
        with patch('recovery.sql', side_effect=[b'170011', b'0']), patch('recovery.command'), patch('recovery.db_evidence', return_value={'relations': []}), self.assertRaisesRegex(r.RecoveryError, 'differs'):
            r.restore(self.bundle, sha, 'arb_restore_test', 'arb_restore_test', self.root / 'restored', True)

    def test_streaming_process_output_limits_deadlines_and_errors(self):
        result = r.command([sys.executable, '-c', 'print(42)'], r.Limits())
        self.assertEqual(result, {'rows': 1, 'bytes': 3, 'sha256': r.digest(b'42\n')})
        for code, limits in [('print("a" * 100)', r.Limits(bytes=10)), ('import time; time.sleep(5)', r.Limits(seconds=1)), ('raise SystemExit(2)', r.Limits())]:
            with self.subTest(code=code), self.assertRaises(r.RecoveryError):
                r.command([sys.executable, '-c', code], limits)

    def test_no_secret_or_connection_details_in_cli_error(self):
        with patch.object(sys, 'argv', ['recovery.py', 'verify', '--bundle', str(self.root / 'private-secret'), '--manifest-sha256', 'x']), patch('builtins.print') as output:
            self.assertEqual(r.main(), 2)
        self.assertNotIn('private-secret', output.call_args.args[0])


class DiagnosticTests(unittest.TestCase):
    """Exercise actual child-process pipes; no PostgreSQL service is mocked as live."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.store = r.Diagnostics('command', self.root / 'private')

    def run_child(self, code, **kwargs):
        return r.command([sys.executable, '-c', code], r.Limits(seconds=3), diagnostics=self.store, **kwargs)

    def metadata(self, number=1):
        return json.loads((self.store.root / f'command-{number:04d}.json').read_bytes())

    def test_stderr_is_private_and_separate_on_success_and_failure(self):
        value = self.run_child('import sys; print("public-result"); sys.stderr.write("fixture-private-diagnostic")', capture=True)
        self.assertEqual(value, b'public-result\n')
        self.assertEqual((self.store.root / 'command-0001.stderr').read_bytes(), b'fixture-private-diagnostic')
        with self.assertRaises(r.RecoveryError):
            self.run_child('import sys; sys.stderr.write("fixture-private-failure"); sys.exit(2)')
        self.assertEqual((self.store.root / 'command-0002.stderr').read_bytes(), b'fixture-private-failure')
        self.assertEqual(self.metadata(2)['returncode'], 2)
        self.assertFalse(self.metadata(2)['completed'])
        self.assertEqual(stat.S_IMODE(self.store.root.stat().st_mode), 0o700)
        for path in self.store.root.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            if path.suffix == '.json':
                self.assertNotIn(b'fixture-private', path.read_bytes())
                self.assertNotIn(b'argv', path.read_bytes())

    def test_large_stderr_is_drained_after_truncation_without_blocking_stdout(self):
        self.store.max_file_bytes = 1024
        self.store.max_total_bytes = 2048
        result = self.run_child('import sys; sys.stderr.write("x" * 2000000); print("done")', capture=True)
        self.assertEqual(result, b'done\n')
        self.assertEqual((self.store.root / 'command-0001.stderr').stat().st_size, 1024)
        self.assertEqual(self.metadata()['stderr_bytes_seen'], 2000000)
        self.assertTrue(self.metadata()['stderr_truncated'])

    def test_operation_total_limit_is_shared_by_all_commands(self):
        self.store.max_file_bytes = 100
        self.store.max_total_bytes = 150
        for _ in range(3):
            self.run_child('import sys; sys.stderr.write("z" * 200)')
        sizes = [(self.store.root / f'command-{i:04d}.stderr').stat().st_size for i in range(1, 4)]
        self.assertEqual(sizes, [100, 50, 0])
        self.assertEqual(self.store.retained_bytes, 150)
        self.assertTrue(all(self.metadata(i)['stderr_truncated'] for i in range(1, 4)))

    def test_timeout_retains_diagnostic_without_exposing_it(self):
        with self.assertRaises(r.RecoveryError):
            r.command([sys.executable, '-c', 'import sys,time; sys.stderr.write("fixture-before-timeout"); sys.stderr.flush(); time.sleep(10)'], r.Limits(seconds=1), diagnostics=self.store)
        self.assertIn(b'fixture-before-timeout', (self.store.root / 'command-0001.stderr').read_bytes())
        self.assertFalse(self.metadata()['completed'])
        self.assertIsNotNone(self.metadata()['returncode'])

    def test_spawn_failure_still_has_private_metadata(self):
        with self.assertRaises(FileNotFoundError):
            r.command([str(self.root / 'missing-fixture-program')], r.Limits(), diagnostics=self.store)
        self.assertFalse(self.metadata()['completed'])
        self.assertIsNone(self.metadata()['returncode'])
        self.assertEqual((self.store.root / 'command-0001.stderr').read_bytes(), b'')

    def test_overlapping_or_existing_diagnostic_roots_fail_closed(self):
        for path in [self.root, self.store.root, self.store.root / 'nested']:
            with self.subTest(path=path), self.assertRaises(r.RecoveryError):
                self.store.exclude(path)
        self.store.exclude(self.root / 'captures', self.root / 'bundle')
        self.store.root.mkdir()
        with patch('recovery.subprocess.Popen') as child, self.assertRaises(FileExistsError):
            self.run_child('print(1)')
        child.assert_not_called()
        alias = self.root / 'alias'
        alias.symlink_to(self.store.root, target_is_directory=True)
        with self.assertRaises(r.RecoveryError):
            r.Diagnostics('backup', alias)

    def test_diagnostic_command_limit_prevents_new_processes(self):
        self.store.max_commands = 1
        self.run_child('print(1)')
        with patch('recovery.subprocess.Popen') as child, self.assertRaises(r.RecoveryError):
            self.run_child('print(2)')
        child.assert_not_called()

    def test_sql_propagates_private_store_and_readonly_context(self):
        with patch('recovery.command', return_value=b'42') as child:
            self.assertEqual(r.sql('fixture_db', 'SELECT 42', r.Limits(), diagnostics=self.store), b'42')
        self.assertEqual(child.call_args.args[0][0], 'psql')
        self.assertTrue(child.call_args.kwargs['readonly'])
        self.assertIs(child.call_args.kwargs['diagnostics'], self.store)

    def test_backup_and_restore_keep_diagnostics_outside_artifact_inventories(self):
        captures = self.root / 'captures'
        captures.mkdir()
        (captures / 'sample.bin').write_bytes(b'fixture')
        bundle = self.root / 'bundle'
        def dump(args, *_args, **kwargs):
            self.assertIs(kwargs['diagnostics'], self.store)
            self.assertTrue(kwargs['readonly'])
            Path(args[args.index('--file') + 1]).write_bytes(b'PGDMP-fixture')
        with patch('recovery.sql', return_value=b'170011') as sql, patch('recovery.db_evidence', return_value=EVIDENCE) as evidence, patch('recovery.command', side_effect=dump):
            result = r.backup('fixture_db', captures, bundle, HEAD, True, diagnostics=self.store)
        self.assertTrue(all(c.kwargs['diagnostics'] is self.store for c in sql.call_args_list + evidence.call_args_list))
        self.run_child('import sys; sys.stderr.write("private-fixture")')
        manifest, _ = r.verify(bundle, result['manifest_sha256'])
        self.assertFalse(any('stderr' in x['path'] or 'private' in x['path'] for x in manifest['files']))
        with patch('recovery.sql', side_effect=[b'170011', b'0']) as sql, patch('recovery.command') as child, patch('recovery.db_evidence', return_value=EVIDENCE):
            r.restore(bundle, result['manifest_sha256'], 'arb_restore_test', 'arb_restore_test', self.root / 'restored', True, diagnostics=self.store)
        self.assertIs(child.call_args.kwargs['diagnostics'], self.store)
        self.assertTrue(all(c.kwargs['diagnostics'] is self.store for c in sql.call_args_list))
        self.assertEqual(r.inventory(self.root / 'restored', r.Limits()), r.inventory(captures, r.Limits()))

    def test_cli_error_does_not_print_retained_private_stderr(self):
        def fail(*_args, **kwargs):
            r.command([sys.executable, '-c', 'import sys; sys.stderr.write("fixture-secret-never-public"); sys.exit(2)'], r.Limits(), diagnostics=kwargs['diagnostics'])
        args = ['recovery.py', 'backup', '--database', 'fixture_db', '--captures', str(self.root), '--output', str(self.root / 'out'), '--source-commit', HEAD, '--quiesced', '--diagnostics', str(self.root / 'cli-private')]
        with patch.object(sys, 'argv', args), patch('recovery.backup', side_effect=fail), patch('builtins.print') as output:
            self.assertEqual(r.main(), 2)
        public = output.call_args.args[0]
        self.assertNotIn('fixture-secret-never-public', public)
        self.assertNotIn(str(self.root), public)
        self.assertRegex(json.loads(public)['diagnostics_id'], r'^[0-9a-f]{32}$')
        self.assertEqual((self.root / 'cli-private/command-0001.stderr').read_bytes(), b'fixture-secret-never-public')


if __name__ == '__main__':
    unittest.main()
