#!/usr/bin/env python3
"""Offline startup-contract tests; process mocks are not AI workers."""
import contextlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import start_codex_team as team


class TeamTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        shutil.copytree(team.ROOT / '.codex', self.root / '.codex')

    def invoke(self, args):
        output = io.StringIO()
        with mock.patch.object(team, 'ROOT', self.root), contextlib.redirect_stdout(output):
            code = team.main(args)
        return code, output.getvalue()

    def test_real_configuration_has_five_roles_and_nonempty_prompt(self):
        prompt = team.validate_setup(team.ROOT)
        self.assertIn('single primary orchestrator', prompt)
        self.assertEqual(len(team.ROLES), 5)
        self.assertIn('FIVE_WORKERS_RUNNING', prompt)

    def test_default_check_does_not_execute_any_process(self):
        with mock.patch.object(team.shutil, 'which', return_value='/usr/bin/codex'), \
             mock.patch.object(team.subprocess, 'run') as run, \
             mock.patch.object(team.os, 'execv') as execute:
            code, output = self.invoke([])
        self.assertEqual(code, 0)
        report = json.loads(output)
        self.assertEqual(report['agents_started'], 0)
        self.assertEqual(report['worker_capacity'], 5)
        self.assertEqual(report['orchestrator_capacity'], 1)
        self.assertFalse(report['runtime_verified'])
        run.assert_not_called()
        execute.assert_not_called()

    def test_missing_binary_is_visible_without_starting(self):
        with mock.patch.object(team.shutil, 'which', return_value=None):
            code, output = self.invoke([])
        self.assertEqual(code, 0)
        self.assertFalse(json.loads(output)['codex_executable_found'])

    def test_explicit_start_without_runtime_fails_closed(self):
        with mock.patch.object(team.shutil, 'which', return_value=None), \
             mock.patch.object(team.os, 'execv') as execute:
            code, output = self.invoke(['--start'])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(output)['reason'], 'CODEX_RUNTIME_MISSING')
        self.assertEqual(json.loads(output)['agents_started'], 0)
        execute.assert_not_called()

    def test_noninteractive_start_is_refused(self):
        with mock.patch.object(team.shutil, 'which', return_value='/usr/bin/codex'), \
             mock.patch.object(team.sys.stdin, 'isatty', return_value=False), \
             mock.patch.object(team.os, 'execv') as execute:
            code, output = self.invoke(['--start'])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(output)['reason'], 'INTERACTIVE_TERMINAL_REQUIRED')
        execute.assert_not_called()

    def test_capacity_cannot_be_six_or_boolean_or_wrong_type(self):
        path = self.root / '.codex/config.toml'
        original = path.read_text()
        for invalid in ('6', 'true', '5.0', '"5"', '0', '-1'):
            path.write_text(original.replace('= 5', '= ' + invalid))
            with self.subTest(invalid=invalid), self.assertRaises(team.SetupError):
                team.validate_setup(self.root)

    def test_project_cannot_introduce_auth_or_provider_overrides(self):
        path = self.root / '.codex/config.toml'
        path.write_text('model_provider = "paid-test-provider"\n' + path.read_text())
        with self.assertRaisesRegex(team.SetupError, 'UNEXPECTED_PROJECT_SETTINGS'):
            team.validate_setup(self.root)

    def test_extra_worker_or_orchestrator_child_is_rejected(self):
        (self.root / '.codex/agents/orchestrator.toml').write_text('name = "orchestrator"')
        with self.assertRaisesRegex(team.SetupError, 'INVALID_ROLE_SET'):
            team.validate_setup(self.root)

    def test_missing_role_is_rejected(self):
        (self.root / '.codex/agents/base.toml').unlink()
        with self.assertRaisesRegex(team.SetupError, 'INVALID_ROLE_SET'):
            team.validate_setup(self.root)

    def test_role_identity_and_override_are_checked(self):
        path = self.root / '.codex/agents/base.toml'
        original = path.read_text()
        for changed in (original.replace('name = "base"', 'name = "another"'),
                        'model_provider = "paid-provider"\n' + original):
            path.write_text(changed)
            with self.assertRaises(team.SetupError):
                team.validate_setup(self.root)

    def test_blank_bootstrap_is_rejected(self):
        (self.root / '.codex/ORCHESTRATOR.md').write_text(' ')
        with self.assertRaisesRegex(team.SetupError, 'MISSING_ORCHESTRATOR_PROMPT'):
            team.validate_setup(self.root)

    def test_malformed_config_is_redacted(self):
        (self.root / '.codex/config.toml').write_text('[invalid private-source')
        code, output = self.invoke([])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(output)['reason'], 'PREFLIGHT_FAILED')
        self.assertNotIn(str(self.root), output)
        self.assertNotIn('private-source', output)

    def test_git_checks_root_origin_and_clean_worktree(self):
        results = [subprocess.CompletedProcess([], 0, str(self.root), ''),
                   subprocess.CompletedProcess([], 0, 'git@github.com:makafeli/arbitrage-research.git', ''),
                   subprocess.CompletedProcess([], 0, '', '')]
        with mock.patch.object(team.shutil, 'which', return_value='/usr/bin/git'), \
             mock.patch.object(team, 'run_readonly', side_effect=results) as run:
            team.verify_repository(self.root)
        self.assertEqual(run.call_count, 3)

    def test_wrong_origin_and_dirty_worktree_are_refused(self):
        for origin, dirty, expected in (
            ('https://github.com/someone/other.git', '', 'UNEXPECTED_REPOSITORY_ORIGIN'),
            ('https://github.com/makafeli/arbitrage-research.git', ' M README.md', 'WORKTREE_NOT_CLEAN'),
        ):
            results = [subprocess.CompletedProcess([], 0, str(self.root), ''),
                       subprocess.CompletedProcess([], 0, origin, ''),
                       subprocess.CompletedProcess([], 0, dirty, '')]
            with mock.patch.object(team.shutil, 'which', return_value='/usr/bin/git'), \
                 mock.patch.object(team, 'run_readonly', side_effect=results), \
                 self.assertRaisesRegex(team.SetupError, expected):
                team.verify_repository(self.root)

    def test_nonrepo_and_missing_git_are_refused(self):
        with mock.patch.object(team.shutil, 'which', return_value=None), \
             self.assertRaisesRegex(team.SetupError, 'GIT_RUNTIME_MISSING'):
            team.verify_repository(self.root)
        with mock.patch.object(team.shutil, 'which', return_value='/usr/bin/git'), \
             mock.patch.object(team, 'run_readonly', return_value=subprocess.CompletedProcess([], 128, '', 'private')), \
             self.assertRaisesRegex(team.SetupError, 'NOT_REPOSITORY_ROOT'):
            team.verify_repository(self.root)

    def test_only_explicit_chatgpt_login_status_is_accepted(self):
        for code, text, accepted in ((0, 'Logged in using ChatGPT', True),
                                     (0, 'Logged in using an API key', False),
                                     (1, 'Not logged in using ChatGPT', False),
                                     (0, 'unknown auth format', False),
                                     (0, 'ChatGPT api_key', False)):
            result = subprocess.CompletedProcess([], code, '', text)
            with self.subTest(text=text), mock.patch.object(team, 'run_readonly', return_value=result):
                if accepted:
                    team.verify_login('/usr/bin/codex', self.root)
                else:
                    with self.assertRaisesRegex(team.SetupError, 'CHATGPT_LOGIN_NOT_VERIFIED'):
                        team.verify_login('/usr/bin/codex', self.root)

    def test_readonly_process_calls_have_timeout_and_no_shell(self):
        with mock.patch.object(team.subprocess, 'run') as run:
            team.run_readonly(['git', 'status'], self.root)
        self.assertEqual(run.call_args.kwargs['timeout'], 15)
        self.assertTrue(run.call_args.kwargs['capture_output'])
        self.assertNotIn('shell', run.call_args.kwargs)

    def test_launch_uses_one_literal_prompt_without_unsafe_flags(self):
        prompt = 'literal $(touch /tmp/no) ; && prompt'
        command = team.launch_command('/usr/bin/codex', self.root, prompt)
        self.assertEqual(command[0], '/usr/bin/codex')
        self.assertEqual(command[-1], prompt)
        self.assertIn('agents.max_concurrent_threads_per_session=5', command)
        self.assertIn('forced_login_method="chatgpt"', command)
        self.assertIn('model_provider="openai"', command)
        self.assertIn('workspace-write', command)
        self.assertIn('on-request', command)
        self.assertFalse(any(v in command for v in ('--yolo', '--full-auto', '--dangerously-bypass-approvals-and-sandbox', 'exec', 'sh', '&')))

    def test_authorized_start_executes_only_primary_and_does_not_claim_workers(self):
        output = io.StringIO()
        output.isatty = lambda: True
        with mock.patch.object(team, 'ROOT', self.root), \
             mock.patch.object(team.shutil, 'which', return_value='/usr/bin/codex'), \
             mock.patch.object(team.sys.stdin, 'isatty', return_value=True), \
             mock.patch.object(team, 'verify_repository') as repo, \
             mock.patch.object(team, 'verify_login') as auth, \
             mock.patch.object(team.os, 'execv') as execute, \
             contextlib.redirect_stdout(output):
            code = team.main(['--start'])
        self.assertEqual(code, 0)
        repo.assert_called_once_with(self.root)
        auth.assert_called_once_with('/usr/bin/codex', self.root)
        execute.assert_called_once()
        self.assertIn('dispatch is not yet verified', output.getvalue())
        self.assertNotIn('FIVE_WORKERS_RUNNING', output.getvalue())

    def test_real_check_cli_reports_no_started_agents(self):
        result = subprocess.run([sys.executable, str(team.ROOT / 'scripts/start_codex_team.py')],
                                capture_output=True, text=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['status'], 'CONFIGURED_NOT_STARTED')
        self.assertEqual(json.loads(result.stdout)['agents_started'], 0)


if __name__ == '__main__':
    unittest.main()
