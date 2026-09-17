#!/usr/bin/env python3
"""Offline gate regressions; no API request, token or production deployment."""
import copy
import io
import json
import os
import unittest
from contextlib import redirect_stderr
from unittest.mock import patch

import worker_ci_gate as gate

SHA = 'a' * 40


def fixture():
    runs = []
    for index, path in enumerate(sorted(gate.REQUIRED), start=1):
        runs.append(dict(path=path, id=index, run_number=10, run_attempt=1,
                         event='push', head_branch='main', head_sha=SHA,
                         repository=dict(id=gate.REPOSITORY_ID, full_name=gate.REPOSITORY),
                         head_repository=dict(id=gate.REPOSITORY_ID, full_name=gate.REPOSITORY),
                         status='completed', conclusion='success'))
    return dict(total_count=len(runs), workflow_runs=runs)


class GateTests(unittest.TestCase):
    def test_complete_exact_revision_passes(self):
        result = gate.check(SHA, reader=lambda _: fixture())
        self.assertEqual(result['status'], 'CI_GATE_PASSED')
        self.assertFalse(result['execution_authorized'])
        self.assertEqual(len(result['required_runs']), 3)

    def test_branch_repo_and_sha_metadata_required(self):
        env = dict(RAILWAY_GIT_REPO_OWNER='makafeli',
                   RAILWAY_GIT_REPO_NAME='arbitrage-research',
                   RAILWAY_GIT_BRANCH='main', RAILWAY_GIT_COMMIT_SHA=SHA)
        self.assertEqual(gate.revision(env), SHA)
        for key in env:
            for value in ['', 'other', 'https://invalid/private']:
                with self.subTest(key=key, value=value), self.assertRaises(gate.GateError):
                    gate.revision({**env, key: value})

    def test_required_failure_skip_cancel_neutral_all_block(self):
        for conclusion in ['failure', 'skipped', 'neutral', 'cancelled', 'timed_out']:
            value = fixture()
            value['workflow_runs'][0]['conclusion'] = conclusion
            with self.subTest(conclusion=conclusion), self.assertRaisesRegex(gate.GateError, 'CI_CHECK_FAILED'):
                gate.assess(value, SHA)

    def test_missing_or_running_required_check_waits(self):
        value = fixture()
        value['workflow_runs'].pop()
        value['total_count'] -= 1
        self.assertFalse(gate.assess(value, SHA)[0])
        value = fixture()
        value['workflow_runs'][0].update(status='in_progress', conclusion=None)
        self.assertFalse(gate.assess(value, SHA)[0])

    def test_pr_branch_sha_and_repository_mismatch_rejected(self):
        for change in [dict(event='pull_request'), dict(head_branch='other'),
                       dict(head_sha='b' * 40), dict(repository={}), dict(head_repository={})]:
            value = fixture()
            value['workflow_runs'][0].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(gate.GateError, 'CI_SCOPE_MISMATCH'):
                gate.assess(value, SHA)

    def test_overflow_and_truncation_rejected(self):
        for total in [True, -1, 2, 4, 101, '3']:
            with self.subTest(total=total), self.assertRaises(gate.GateError):
                gate.assess({**fixture(), 'total_count': total}, SHA)

    def test_duplicate_run_ids_and_bad_types_rejected(self):
        for change in [dict(id=2), dict(id=True), dict(run_number=0), dict(run_attempt='1'),
                       dict(path='../other'), dict(status='unknown'), dict(conclusion=None)]:
            value = fixture()
            value['workflow_runs'][0].update(change)
            with self.subTest(change=change), self.assertRaises(gate.GateError):
                gate.assess(value, SHA)

    def test_latest_rerun_cannot_reuse_older_success(self):
        value = fixture()
        new = copy.deepcopy(value['workflow_runs'][0])
        new.update(id=4, run_number=11, status='in_progress', conclusion=None)
        value['workflow_runs'].append(new)
        value['total_count'] += 1
        self.assertFalse(gate.assess(value, SHA)[0])
        new.update(status='completed', conclusion='failure')
        with self.assertRaisesRegex(gate.GateError, 'CI_CHECK_FAILED'):
            gate.assess(value, SHA)
        new['conclusion'] = 'success'
        self.assertEqual(gate.assess(value, SHA)[1][new['path']], 4)

    def test_additional_failure_blocks_but_neutral_optional_allowed(self):
        value = fixture()
        other = copy.deepcopy(value['workflow_runs'][0])
        other.update(id=4, path='.github/workflows/optional.yml', conclusion='neutral')
        value['workflow_runs'].append(other)
        value['total_count'] += 1
        self.assertTrue(gate.assess(value, SHA)[0])
        other['conclusion'] = 'failure'
        with self.assertRaises(gate.GateError):
            gate.assess(value, SHA)

    def test_wait_is_finite_and_does_not_repeat_failure(self):
        reads, pauses = [], []
        def reader(sha):
            reads.append(sha)
            return {'total_count': 0, 'workflow_runs': []}
        with self.assertRaisesRegex(gate.GateError, 'CI_WAIT_EXHAUSTED'):
            gate.check(SHA, reader=reader, sleep=pauses.append)
        self.assertEqual(len(reads), gate.MAX_POLLS)
        self.assertEqual(pauses, [gate.POLL_SECONDS] * (gate.MAX_POLLS - 1))

    def test_pending_then_success_uses_updated_metadata(self):
        values = iter([{'total_count': 0, 'workflow_runs': []}, fixture()])
        pauses = []
        result = gate.check(SHA, reader=lambda _: next(values), sleep=pauses.append)
        self.assertEqual(result['status'], 'CI_GATE_PASSED')
        self.assertEqual(len(pauses), 1)

    def test_error_output_contains_no_environment_values(self):
        output = io.StringIO()
        with patch.dict(os.environ, {'RAILWAY_GIT_BRANCH': 'private-secret'}, clear=True), redirect_stderr(output):
            self.assertEqual(gate.main([]), 2)
        self.assertNotIn('private-secret', output.getvalue())
        self.assertEqual(json.loads(output.getvalue())['reason'], 'CI_SOURCE_REJECTED')

    def test_no_proxy_token_redirect_or_arbitrary_api_base(self):
        class Response(io.BytesIO):
            status = 200
        class Opener:
            def open(self, req, timeout):
                self_test.assertTrue(req.full_url.startswith(gate.API + '/actions/runs?'))
                self_test.assertEqual(timeout, 10)
                self_test.assertFalse(req.has_header('Authorization'))
                return Response(json.dumps(fixture()).encode())
        self_test = self
        with patch('urllib.request.build_opener', return_value=Opener()) as build:
            self.assertEqual(gate.fetch(SHA), fixture())
            handlers = build.call_args.args
            self.assertEqual(handlers[0].proxies, {})
            self.assertIsInstance(handlers[1], gate.NoRedirect)
        with self.assertRaisesRegex(gate.GateError, 'CI_REDIRECT_REJECTED'):
            gate.NoRedirect().redirect_request(None, None, 302, '', {}, 'https://other.invalid')

    def test_response_size_json_and_duplicate_keys_fail_closed(self):
        for raw in [b'x' * (gate.MAX_RESPONSE + 1), b'not-json', b'{"total_count":0,"total_count":3}']:
            class Response(io.BytesIO):
                status = 200
            class Opener:
                def open(self, req, timeout):
                    return Response(raw)
            with self.subTest(size=len(raw)), patch('urllib.request.build_opener', return_value=Opener()), self.assertRaises(gate.GateError):
                gate.fetch(SHA)

    def test_wall_timeout_and_arguments_refuse(self):
        with self.assertRaisesRegex(gate.GateError, 'CI_WALL_LIMIT'):
            gate.alarm(None, None)
        with redirect_stderr(io.StringIO()):
            self.assertEqual(gate.main(['--skip-ci']), 2)


if __name__ == '__main__':
    unittest.main()
