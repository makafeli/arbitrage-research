#!/usr/bin/env python3
"""Offline/adversarial coordination tests. No agents, credentials or GitHub writes."""
import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import delivery as d

HEAD = 'a' * 40
BODY = '### Acceptance criteria\n- [ ] Exact result is checked.\n- [ ] Failure is explicit.\n\n### Verification\nEvidence.\n'


def sample():
    task = {'id': 'ARB-008', 'title': 'Fixture task', 'priority': 'P0', 'milestone': 'M1',
            'body': BODY, 'dependencies': []}
    row = {'id': task['id'], 'issue_url': f'https://github.com/{d.REPOSITORY}/issues/22',
           'state': 'implemented_pending_acceptance', 'dependencies': [],
           'evidence': ['fixture-evidence'], 'acceptance_review': 'fixture-reviewed', 'remaining_acceptance': ''}
    return {task['id']: task}, {task['id']: row}


def snapshot(tasks, rows):
    return {'schema_version': 1, 'repository': d.REPOSITORY, 'complete': True, 'issues': [
        {'number': d.issue_number(rows[t]), 'state': 'closed' if rows[t]['state'] == 'completed' else 'open',
         'state_reason': 'completed' if rows[t]['state'] == 'completed' else None,
         'body': f'<!-- arb-ticket:{t} -->\n' + tasks[t]['body'].replace('[ ]', '[x]')}
        for t in tasks]}


def packet(tasks, rows):
    return {'schema_version': 1, 'repository': d.REPOSITORY, 'ticket_id': 'ARB-008',
            'pull_request': {'number': 1, 'url': f'https://github.com/{d.REPOSITORY}/pull/1',
                             'merged': True, 'head_sha': HEAD, 'base_ref': 'main', 'merge_commit_sha': 'b' * 40},
            'checks': [{'name': name, 'head_sha': HEAD, 'status': 'completed', 'conclusion': 'success',
                        'run_url': f'https://github.com/{d.REPOSITORY}/actions/runs/10'} for name in d.CHECKS],
            'review': {'status': 'completed', 'head_sha': HEAD, 'kind': 'solo-maintainer',
                       'reviewer': 'fixture-reviewer', 'unresolved_findings': [], 'unresolved_threads': [],
                       'evidence_url': f'https://github.com/{d.REPOSITORY}/pull/1#issuecomment-1'},
            'criteria': [{'text': text, 'result': 'passed', 'evidence': ['fixture-evidence']}
                         for text, _ in d.acceptance(BODY)], 'github_snapshot': snapshot(tasks, rows)}


def assignment(ticket='ARB-008', path='crates/arb-domain'):
    return {'ticket_id': ticket, 'manager': d.lane(ticket), 'worker': 'worker-' + ticket,
            'branch': f'fix/{ticket}-fixture', 'write_paths': [path]}


class DeliveryTests(unittest.TestCase):
    def setUp(self):
        self.tasks, self.rows = sample()

    def test_real_register_is_complete_and_does_not_change(self):
        source = d.ROOT / 'planning/implementation-progress.json'
        before = source.read_bytes()
        tasks, rows = d.load_register(d.ROOT)
        report = d.plan(tasks, rows)
        self.assertEqual(len(tasks), 68)
        self.assertEqual(sum(report['state_counts'].values()), 68)
        self.assertEqual(source.read_bytes(), before)
        self.assertEqual(report['active_workers'], 0)
        self.assertFalse(report['ownership_validated'])
        self.assertFalse(report['github_mutations'])
        self.assertEqual(set(report['manager_capacity'].values()), {5})
        for lane in d.LANES:
            self.assertLessEqual(sum(d.lane(t) == lane for t in report['proposed_reviews_or_implementations']), 5)

    def test_capacity_bounds_and_no_launch_claim(self):
        for capacity in [1, 20, 25, 50]:
            report = d.plan(self.tasks, self.rows, capacity)
            self.assertEqual(report['execution'], 'PLAN_ONLY_NO_AGENTS_STARTED')
            self.assertEqual(sum(report['manager_capacity'].values()), capacity)
        for capacity in [True, 0, -1, 51, 1.5]:
            with self.subTest(capacity=capacity), self.assertRaises(d.DeliveryError):
                d.plan(self.tasks, self.rows, capacity)

    def test_dependency_code_never_means_acceptance(self):
        tasks, rows = copy.deepcopy(self.tasks), copy.deepcopy(self.rows)
        tasks['ARB-009'] = dict(tasks['ARB-008'], id='ARB-009', dependencies=['ARB-008'])
        rows['ARB-009'] = dict(rows['ARB-008'], id='ARB-009', state='planned', dependencies=['ARB-008'],
                              issue_url=f'https://github.com/{d.REPOSITORY}/issues/23')
        report = d.plan(tasks, rows)
        child = next(t for t in report['tickets'] if t['id'] == 'ARB-009')
        self.assertEqual(child['work_kind'], 'implementation-proposal')
        self.assertEqual(child['acceptance_blockers'], ['ARB-008'])
        rows['ARB-008']['state'] = 'planned'
        self.assertEqual(d.readiness(tasks['ARB-009'], rows['ARB-009'], rows), 'dependency-blocked')

    def test_external_and_later_gates_are_not_launched(self):
        self.rows['ARB-008']['external_prerequisite'] = 'Provider account review'
        self.assertEqual(d.plan(self.tasks, self.rows)['proposed_reviews_or_implementations'], [])
        del self.rows['ARB-008']['external_prerequisite']
        self.tasks['ARB-008']['milestone'] = 'M6'
        self.assertEqual(d.plan(self.tasks, self.rows)['proposed_reviews_or_implementations'], [])

    def test_invalid_registers_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); (root / 'planning').mkdir()
            for kind in ['missing', 'duplicate', 'dependency', 'cycle', 'false-completion', 'duplicate-issue']:
                tasks, rows = sample()
                if kind == 'missing': rows = {}
                if kind in {'dependency', 'cycle'}:
                    tasks['ARB-008']['dependencies'] = ['ARB-099' if kind == 'dependency' else 'ARB-008']
                    rows['ARB-008']['dependencies'] = tasks['ARB-008']['dependencies']
                if kind == 'false-completion':
                    rows['ARB-008'].update(state='completed', remaining_acceptance='not done')
                if kind == 'duplicate-issue':
                    tasks['ARB-009'] = dict(tasks['ARB-008'], id='ARB-009')
                    rows['ARB-009'] = dict(rows['ARB-008'], id='ARB-009')
                task_list = list(tasks.values())
                if kind == 'duplicate': task_list *= 2
                (root / 'planning/backlog.json').write_text(json.dumps({'tickets': task_list}))
                (root / 'planning/implementation-progress.json').write_text(json.dumps({'tickets': list(rows.values())}))
                with self.subTest(kind=kind), self.assertRaises(d.DeliveryError): d.load_register(root)

    def test_duplicate_json_fields_are_rejected(self):
        with self.assertRaises(d.DeliveryError): json.loads('{"state":"open","state":"closed"}', object_pairs_hook=d.unique_object)

    def test_static_assignment_is_not_a_distributed_lock(self):
        report = d.validate_assignments({'schema_version': 1, 'base_sha': HEAD, 'assignments': [assignment()]},
                                       self.tasks, self.rows, HEAD, 25)
        self.assertEqual(report['agents_started'], 0)
        self.assertIn('not a distributed lock', report['scope'])

    def test_assignment_rejects_stale_head_and_duplicate_workers(self):
        data = {'schema_version': 1, 'base_sha': 'b' * 40, 'assignments': [assignment()]}
        with self.assertRaises(d.DeliveryError): d.validate_assignments(data, self.tasks, self.rows, HEAD, 25)
        data['base_sha'] = HEAD; data['assignments'] *= 2
        with self.assertRaises(d.DeliveryError): d.validate_assignments(data, self.tasks, self.rows, HEAD, 25)

    def test_file_and_directory_conflicts_are_rejected(self):
        tasks, rows = copy.deepcopy(self.tasks), copy.deepcopy(self.rows)
        tasks['ARB-009'] = dict(tasks['ARB-008'], id='ARB-009')
        rows['ARB-009'] = dict(rows['ARB-008'], id='ARB-009')
        for second in ['crates/arb-domain', 'crates/arb-domain/src/lib.rs', 'crates']:
            data = {'schema_version': 1, 'base_sha': HEAD, 'assignments': [assignment(), assignment('ARB-009', second)]}
            with self.subTest(second=second), self.assertRaisesRegex(d.DeliveryError, 'ownership conflict'):
                d.validate_assignments(data, tasks, rows, HEAD, 25)
        data['assignments'][1]['write_paths'] = ['crates/arb-domain-extra']
        d.validate_assignments(data, tasks, rows, HEAD, 25)

    def test_shared_and_unsafe_paths_are_rejected(self):
        for path in ['.', '../src', '/tmp', '.git/config', 'a//b', 'a/../b', 'apps/*', 'Cargo.lock', '.github', 'specs/openapi.yaml', 'planning']:
            data = {'schema_version': 1, 'base_sha': HEAD, 'assignments': [assignment(path=path)]}
            with self.subTest(path=path), self.assertRaises(d.DeliveryError):
                d.validate_assignments(data, self.tasks, self.rows, HEAD, 25)

    def test_issue_audit_is_complete_scoped_and_distinguishes_not_planned(self):
        native = snapshot(self.tasks, self.rows)
        self.assertFalse(d.audit(self.tasks, self.rows, native)['findings'])
        for field, value in [('complete', False), ('repository', 'someone/else')]:
            with self.assertRaises(d.DeliveryError): d.audit(self.tasks, self.rows, dict(native, **{field: value}))
        native['issues'][0].update(state='closed', state_reason='not_planned')
        self.assertIn('CLOSED_WITHOUT_COMPLETION_SCOPE_DECISION_REQUIRED', [f['reason'] for f in d.audit(self.tasks, self.rows, native)['findings']])

    def test_issue_audit_detects_missing_wrong_and_duplicate_markers(self):
        native = snapshot(self.tasks, self.rows)
        native['issues'] = []
        self.assertEqual(d.audit(self.tasks, self.rows, native)['findings'][0]['reason'], 'ISSUE_MISSING')
        native = snapshot(self.tasks, self.rows)
        native['issues'].append(dict(native['issues'][0], number=999))
        self.assertIn('DUPLICATE_REMOTE_MARKER', [f['reason'] for f in d.audit(self.tasks, self.rows, native)['findings']])
        native['issues'][0]['body'] = '<!-- arb-ticket:ARB-009 -->'
        self.assertIn('ISSUE_MARKER_MISMATCH', [f['reason'] for f in d.audit(self.tasks, self.rows, native)['findings']])

    def test_closeout_accepts_only_consistent_packet_without_mutation(self):
        data = packet(self.tasks, self.rows)
        before = copy.deepcopy(data)
        result = d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)
        self.assertEqual(result['verdict'], 'EVIDENCE_PACKET_CONSISTENT')
        self.assertFalse(result['github_mutations'])
        self.assertEqual(data, before)
        self.assertIn('not authenticated', result['warning'])

    def test_closeout_rejects_each_ci_failure_or_missing_check(self):
        for name in d.CHECKS:
            for field, value in [('head_sha', 'c' * 40), ('conclusion', 'failure'), ('conclusion', 'skipped'), ('status', 'in_progress')]:
                data = packet(self.tasks, self.rows)
                next(c for c in data['checks'] if c['name'] == name)[field] = value
                with self.subTest(name=name, field=field, value=value), self.assertRaises(d.DeliveryError):
                    d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)
            data = packet(self.tasks, self.rows)
            data['checks'] = [c for c in data['checks'] if c['name'] != name]
            with self.assertRaises(d.DeliveryError): d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)

    def test_closeout_rejects_duplicate_check_name(self):
        data = packet(self.tasks, self.rows); data['checks'].append(data['checks'][0])
        with self.assertRaises(d.DeliveryError): d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)

    def test_closeout_rejects_unmerged_wrong_target_and_source(self):
        for field, value in [('merged', False), ('merged', 'true'), ('base_ref', 'dev'), ('head_sha', 'b' * 40), ('merge_commit_sha', ''), ('url', 'https://example.invalid')]:
            data = packet(self.tasks, self.rows); data['pull_request'][field] = value
            with self.subTest(field=field), self.assertRaises(d.DeliveryError): d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)

    def test_closeout_requires_actual_review_and_resolved_findings(self):
        for field, value in [('status', 'pending'), ('reviewer', ''), ('unresolved_findings', ['bug']), ('unresolved_threads', ['thread']), ('head_sha', 'c' * 40), ('evidence_url', '')]:
            data = packet(self.tasks, self.rows); data['review'][field] = value
            with self.subTest(field=field), self.assertRaises(d.DeliveryError): d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)

    def test_closeout_cannot_drop_or_reword_criteria_or_skip_evidence(self):
        for kind in ['removed', 'changed', 'unchecked', 'no-evidence', 'native-unchecked']:
            data = packet(self.tasks, self.rows)
            if kind == 'removed': data['criteria'].pop()
            elif kind == 'changed': data['criteria'][0]['text'] = 'Easier scope'
            elif kind == 'unchecked': data['criteria'][0]['result'] = 'pending'
            elif kind == 'no-evidence': data['criteria'][0]['evidence'] = []
            else: data['github_snapshot']['issues'][0]['body'] = '<!-- arb-ticket:ARB-008 -->\n' + BODY
            with self.subTest(kind=kind), self.assertRaises(d.DeliveryError): d.closeout('ARB-008', data, self.tasks, self.rows, HEAD)

    def test_closeout_cannot_override_remaining_acceptance(self):
        for field in ['remaining_acceptance', 'external_prerequisite']:
            rows = copy.deepcopy(self.rows); rows['ARB-008'][field] = 'Unmet gate'
            with self.assertRaises(d.DeliveryError): d.closeout('ARB-008', packet(self.tasks, rows), self.tasks, rows, HEAD)

    def test_snapshot_uses_paginated_read_only_requests_and_fails_on_error(self):
        import subprocess
        page1 = [{'number': n, 'state': 'open', 'body': ''} for n in range(1, 101)]
        page2 = [{'number': 101, 'state': 'open', 'body': '', 'pull_request': {}}]
        with patch('delivery.subprocess.run', side_effect=[subprocess.CompletedProcess([], 0, json.dumps(p), '') for p in [page1, page2]]) as run:
            result = d.collect_snapshot()
            self.assertEqual(len(result['issues']), 100)
            self.assertTrue(result['complete'])
            for call in run.call_args_list:
                self.assertIn('GET', call.args[0]); self.assertNotIn('POST', call.args[0])
            self.assertIn('page=2', run.call_args_list[1].args[0][-1])
        with patch('delivery.subprocess.run', return_value=subprocess.CompletedProcess([], 1, '', 'secret provider text')):
            with self.assertRaisesRegex(d.DeliveryError, 'no complete snapshot'): d.collect_snapshot()


if __name__ == '__main__':
    unittest.main()
