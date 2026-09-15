#!/usr/bin/env python3
"""Guard the offline pipeline workflow's context, evidence and test boundaries.

These checks cover this repository's pipeline workflow, not the complete GitHub
Actions schema. They perform no network calls and do not run the Rust workload.
"""
from __future__ import annotations

from pathlib import Path
import re
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / '.github/workflows/pipeline-observations.yml'
CI = ROOT / '.github/workflows/ci.yml'
# GitHub's context-availability contract for jobs.<job_id>.env. The runner and
# env contexts become available in step-level keys, not in this mapping.
JOB_ENV_CONTEXTS = {'github', 'needs', 'strategy', 'matrix', 'vars', 'secrets', 'inputs'}
EXPRESSION_CONTEXT = re.compile(r'\$\{\{\s*([A-Za-z_][A-Za-z_0-9]*)\s*(?:\.|\[)')


def load_workflow(path: Path) -> dict:
    """Read mappings without YAML 1.1 interpreting the key 'on' as a boolean."""
    with path.open(encoding='utf-8') as stream:
        return yaml.load(stream, Loader=yaml.BaseLoader)


class PipelineWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.document = load_workflow(WORKFLOW)
        self.job = self.document['jobs']['process-observations']
        self.steps = self.job['steps']

    def test_job_environment_uses_only_available_contexts(self) -> None:
        """Reject a runner.temp expression before the workflow can be dispatched."""
        for name, value in self.job.get('env', {}).items():
            with self.subTest(variable=name):
                used = set(EXPRESSION_CONTEXT.findall(value))
                self.assertLessEqual(used, JOB_ENV_CONTEXTS)

    def test_export_and_collection_share_one_evidence_directory(self) -> None:
        """Do not repair the collection location but silently upload another path."""
        self.assertEqual(self.job['env']['ARB_TEST_STAGE_EVIDENCE_DIR'],
                         '${{ github.workspace }}/pipeline-evidence')
        uploads = [step for step in self.steps
                   if step.get('uses', '').startswith('actions/upload-artifact@')]
        self.assertEqual(len(uploads), 1)
        self.assertEqual(uploads[0]['with']['path'],
                         '${{ env.ARB_TEST_STAGE_EVIDENCE_DIR }}/')
        self.assertEqual(uploads[0]['if'], 'always()')
        self.assertEqual(uploads[0]['with']['if-no-files-found'], 'error')

    def test_original_rust_and_process_checks_remain_mandatory(self) -> None:
        """Retain each original workload and propagate cargo/tee failures."""
        required = [
            'cargo build --locked -p control-api',
            'cargo test --locked -p arb-scheduler',
            'cargo test --locked -p research-worker --bin research-worker',
            'cargo test --locked -p control-api --lib',
            'cargo test --locked -p research-worker --test controlled_capture',
            'test -s "$ARB_TEST_STAGE_EVIDENCE_DIR/two-pool-profile.json"',
            'test -s "$ARB_TEST_STAGE_EVIDENCE_DIR/stale-state-profile.json"',
        ]
        commands = '\n'.join(step.get('run', '') for step in self.steps)
        for command in required:
            with self.subTest(command=command):
                self.assertIn(command, commands)
        self.assertNotEqual(self.job.get('continue-on-error'), 'true')
        for step in self.steps:
            if 'cargo ' in step.get('run', ''):
                self.assertEqual(step.get('shell'), 'bash')
                self.assertIn('set -euo pipefail', step['run'])
                self.assertNotEqual(step.get('continue-on-error'), 'true')
                self.assertNotIn('|| true', step['run'])

    def test_workflow_remains_read_only_and_without_provider_secrets(self) -> None:
        """The profile uses disposable PostgreSQL and loopback data, not RPC keys."""
        self.assertEqual(self.document['permissions'], {'contents': 'read'})
        self.assertEqual(set(self.document['on']), {'pull_request'})
        self.assertNotIn('permissions', self.job)
        self.assertNotRegex(WORKFLOW.read_text(), r'\$\{\{\s*secrets(?:\.|\[)')
        self.assertNotIn('ARB_BASE_RPC_URL', WORKFLOW.read_text())

    def test_independent_main_ci_runs_this_guard(self) -> None:
        """A workflow rejected by GitHub cannot execute its own regression test."""
        document = load_workflow(CI)
        commands = [step.get('run', '')
                    for step in document['jobs']['specifications']['steps']]
        check = 'python scripts/test_pipeline_workflow.py'
        install = 'python -m pip install -r requirements-dev.txt'
        self.assertIn(check, commands)
        self.assertLess(commands.index(install), commands.index(check))


if __name__ == '__main__':
    unittest.main()
