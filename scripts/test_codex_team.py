#!/usr/bin/env python3
"""Offline startup-contract tests; process mocks are not AI workers."""
import contextlib
import io
import json
import os
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
             mock.patch.object(team.os, 'execve') as execute:
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
             mock.patch.object(team.os, 'execve') as execute:
            code, output = self.invoke(['--start'])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(output)['reason'], 'CODEX_RUNTIME_MISSING')
        self.assertEqual(json.loads(output)['agents_started'], 0)
        execute.assert_not_called()

    def test_noninteractive_start_is_refused(self):
        with mock.patch.object(team.shutil, 'which', return_value='/usr/bin/codex'), \
             mock.patch.object(team.sys.stdin, 'isatty', return_value=False), \
             mock.patch.object(team.os, 'execve') as execute:
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

    def test_negative_or_ambiguous_zero_exit_auth_is_rejected(self):
        for message in (
            'Not logged in using ChatGPT',
            'Please sign in with ChatGPT',
            'ChatGPT authentication failed',
            'Logged in using ChatGPT access token',
            'Logged in using ChatGPT\nAuthentication failed',
            'Warning: account expired\nLogged in using ChatGPT',
            'Logged in using ChatGPT\nLogged in using ChatGPT',
        ):
            result = subprocess.CompletedProcess([], 0, '', message)
            with self.subTest(message=message), \
                 mock.patch.object(team, 'run_readonly', return_value=result), \
                 self.assertRaisesRegex(team.SetupError, 'CHATGPT_LOGIN_NOT_VERIFIED'):
                team.verify_login('/usr/bin/codex', self.root)

    def test_auth_requires_one_complete_status_line(self):
        for stdout, stderr, accepted in (
            ('Logged in using ChatGPT\n', '', True),
            ('', 'Logged in using ChatGPT\r\n', True),
            ('\n', '  Logged in using ChatGPT  \n', True),
            ('Logged in us', 'ing ChatGPT', False),
            ('Logged in using ChatGPT', 'Not authenticated', False),
        ):
            result = subprocess.CompletedProcess([], 0, stdout, stderr)
            with self.subTest(stdout=stdout, stderr=stderr), \
                 mock.patch.object(team, 'run_readonly', return_value=result):
                if accepted:
                    team.verify_login('/usr/bin/codex', self.root)
                else:
                    with self.assertRaisesRegex(team.SetupError, 'CHATGPT_LOGIN_NOT_VERIFIED'):
                        team.verify_login('/usr/bin/codex', self.root)

    def test_auth_refusal_does_not_exec_or_reveal_status_output(self):
        output = io.StringIO()
        output.isatty = lambda: True
        message = 'ChatGPT authentication failed: private-account@example.invalid'
        with mock.patch.object(team, 'ROOT', self.root), \
             mock.patch.object(team.shutil, 'which', return_value='/usr/bin/codex'), \
             mock.patch.object(team.sys.stdin, 'isatty', return_value=True), \
             mock.patch.object(team, 'verify_repository'), \
             mock.patch.object(team, 'run_readonly', return_value=subprocess.CompletedProcess([], 0, '', message)), \
             mock.patch.object(team.os, 'execve') as execute, \
             contextlib.redirect_stdout(output):
            code = team.main(['--start'])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(output.getvalue()), {
            'status': 'START_BLOCKED', 'reason': 'CHATGPT_LOGIN_NOT_VERIFIED', 'agents_started': 0,
        })
        self.assertNotIn('private-account', output.getvalue())
        execute.assert_not_called()

    def test_auth_timeout_is_redacted_and_does_not_exec(self):
        output = io.StringIO()
        output.isatty = lambda: True
        with mock.patch.object(team, 'ROOT', self.root), \
             mock.patch.object(team.shutil, 'which', return_value='/usr/bin/codex'), \
             mock.patch.object(team.sys.stdin, 'isatty', return_value=True), \
             mock.patch.object(team, 'verify_repository'), \
             mock.patch.object(team, 'run_readonly', side_effect=subprocess.TimeoutExpired(
                 'private-command', 15, output='private-auth-output')), \
             mock.patch.object(team.os, 'execve') as execute, \
             contextlib.redirect_stdout(output):
            code = team.main(['--start'])
        self.assertEqual(code, 2)
        self.assertEqual(json.loads(output.getvalue())['reason'], 'PREFLIGHT_FAILED')
        self.assertNotIn('private', output.getvalue())
        execute.assert_not_called()

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
             mock.patch.object(team.os, 'execve') as execute, \
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


class EnvironmentTests(unittest.TestCase):
    """Real local Git fixtures, without a model process or network access."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture_env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
        self.fixture_env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull)
        self.template = self.root / 'empty-template'
        self.template.mkdir()

    def git(self, root, *args):
        return subprocess.run(['git', *args], cwd=root, env=self.fixture_env,
                              capture_output=True, text=True, check=True, timeout=10)

    def repository(self, name, origin='https://github.com/makafeli/arbitrage-research.git'):
        root = self.root / name
        root.mkdir()
        self.git(root, 'init', '-q', '--template=' + str(self.template))
        self.git(root, 'config', 'user.name', 'Synthetic Test')
        self.git(root, 'config', 'user.email', 'test@example.invalid')
        self.git(root, 'remote', 'add', 'origin', origin)
        (root / 'tracked.txt').write_text('fixture\n')
        self.git(root, 'add', 'tracked.txt')
        self.git(root, 'commit', '-qm', 'synthetic fixture')
        return root

    def test_location_and_index_overrides_are_not_inherited_by_subprocess(self):
        source = {'PATH': '/usr/bin', 'GIT_DIR': '/foreign/.git',
                  'GIT_WORK_TREE': '/foreign', 'GIT_INDEX_FILE': '/foreign/index',
                  'HOME': '/home/example', 'SSH_AUTH_SOCK': '/tmp/test-socket'}
        with mock.patch.dict(os.environ, source, clear=True), \
             mock.patch.object(team.subprocess, 'run') as run:
            team.run_readonly(['git', 'status'], self.root)
        env = run.call_args.kwargs.get('env', {})
        self.assertEqual(env, {k: source[k] for k in ('PATH', 'HOME', 'SSH_AUTH_SOCK')})
        self.assertEqual(run.call_args.kwargs['cwd'], self.root)

    def test_environment_copy_preserves_unrelated_configuration(self):
        source = {'PATH': '/bin', 'CODEX_HOME': '/private/codex', 'HTTP_PROXY': 'http://proxy.invalid',
                  'GIT_AUTHOR_NAME': 'Test Author', 'GIT_DIR': '/wrong', 'GIT_CONFIG_COUNT': '1',
                  'GIT_CONFIG_KEY_0': 'core.worktree', 'GIT_CONFIG_VALUE_0': '/wrong'}
        with mock.patch.dict(os.environ, source, clear=True):
            cleaned = team.clean_environment()
            self.assertEqual(dict(os.environ), source)
        self.assertEqual(cleaned, {k: source[k] for k in ('PATH', 'CODEX_HOME', 'HTTP_PROXY', 'GIT_AUTHOR_NAME')})

    def test_known_git_local_environment_variables_are_removed(self):
        keys = self.git(self.root, 'rev-parse', '--local-env-vars').stdout.splitlines()
        keys += ['GIT_NAMESPACE', 'GIT_CONFIG_KEY_2', 'GIT_CONFIG_VALUE_2', 'GIT_INTERNAL_SUPER_PREFIX']
        with mock.patch.dict(os.environ, {k: 'synthetic' for k in keys}, clear=True):
            self.assertEqual(team.clean_environment(), {})

    def test_real_preflight_ignores_foreign_git_directory_and_index(self):
        intended = self.repository('intended')
        foreign = self.repository('foreign', 'https://github.com/synthetic/other.git')
        index_before = (foreign / '.git/index').read_bytes()
        overrides = dict(self.fixture_env, GIT_DIR=str(foreign / '.git'),
                         GIT_WORK_TREE=str(foreign), GIT_INDEX_FILE=str(foreign / '.git/index'))
        with mock.patch.dict(os.environ, overrides, clear=True):
            team.verify_repository(intended)
        self.assertEqual(index_before, (foreign / '.git/index').read_bytes())

    def test_environment_cannot_spoof_an_allowed_remote(self):
        target = self.repository('wrong-origin', 'https://github.com/synthetic/other.git')
        overrides = dict(self.fixture_env, GIT_CONFIG_COUNT='1', GIT_CONFIG_KEY_0='remote.origin.url',
                         GIT_CONFIG_VALUE_0='https://github.com/makafeli/arbitrage-research.git')
        with mock.patch.dict(os.environ, overrides, clear=True), \
             self.assertRaisesRegex(team.SetupError, 'UNEXPECTED_REPOSITORY_ORIGIN'):
            team.verify_repository(target)

    def test_five_worktrees_resolve_their_own_state_despite_foreign_overrides(self):
        primary = self.repository('primary')
        workers = [self.root / ('worker-' + role) for role in team.ROLES]
        for role, worker in zip(team.ROLES, workers):
            self.git(primary, 'worktree', 'add', '-q', '-b', role, str(worker))
        foreign = self.repository('foreign', 'https://github.com/synthetic/other.git')
        index_before = (foreign / '.git/index').read_bytes()
        overrides = dict(self.fixture_env, GIT_DIR=str(foreign / '.git'),
                         GIT_WORK_TREE=str(foreign), GIT_COMMON_DIR=str(foreign / '.git'),
                         GIT_INDEX_FILE=str(foreign / '.git/index'))
        with mock.patch.dict(os.environ, overrides, clear=True):
            for root in [primary, *workers]:
                with self.subTest(root=root.name):
                    team.verify_repository(root)
            (workers[0] / 'tracked.txt').write_text('changed\n')
            with self.assertRaisesRegex(team.SetupError, 'WORKTREE_NOT_CLEAN'):
                team.verify_repository(workers[0])
            for root in [primary, *workers[1:]]:
                team.verify_repository(root)
        self.assertEqual(index_before, (foreign / '.git/index').read_bytes())

    def test_real_child_receives_clean_environment(self):
        overrides = dict(self.fixture_env, GIT_DIR='/foreign', GIT_WORK_TREE='/foreign',
                         GIT_INDEX_FILE='/foreign/index', TEAM_TEST_SENTINEL='retained')
        source = 'import os,json;print(json.dumps({k:os.environ.get(k) for k in ("GIT_DIR","GIT_WORK_TREE","GIT_INDEX_FILE","TEAM_TEST_SENTINEL")}))'
        with mock.patch.dict(os.environ, overrides, clear=True):
            result = team.run_readonly([sys.executable, '-c', source], self.root)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(json.loads(result.stdout), {'GIT_DIR': None, 'GIT_WORK_TREE': None,
                          'GIT_INDEX_FILE': None, 'TEAM_TEST_SENTINEL': 'retained'})

    def test_final_replacement_uses_sanitized_environment(self):
        output = io.StringIO()
        output.isatty = lambda: True
        overrides = {'PATH': '/bin', 'GIT_DIR': '/foreign', 'GIT_WORK_TREE': '/foreign',
                     'GIT_INDEX_FILE': '/foreign/index', 'TEAM_TEST_SENTINEL': 'retained'}
        with mock.patch.dict(os.environ, overrides, clear=True), \
             mock.patch.object(team.shutil, 'which', return_value='/bin/codex'), \
             mock.patch.object(team.sys.stdin, 'isatty', return_value=True), \
             mock.patch.object(team, 'verify_repository'), mock.patch.object(team, 'verify_login'), \
             mock.patch.object(team.os, 'execv') as unsafe_exec, \
             mock.patch.object(team.os, 'execve') as safe_exec, contextlib.redirect_stdout(output):
            self.assertEqual(team.main(['--start']), 0)
        unsafe_exec.assert_not_called()
        safe_exec.assert_called_once()
        self.assertEqual(safe_exec.call_args.args[2], {'PATH': '/bin', 'TEAM_TEST_SENTINEL': 'retained'})
        self.assertIn('dispatch is not yet verified', output.getvalue())


if __name__ == '__main__':
    unittest.main()
