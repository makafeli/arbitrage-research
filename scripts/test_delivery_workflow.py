"""Exercise the actual audit shell block with a synthetic CLI, never GitHub."""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / '.github/workflows/delivery-review.yml'


def audit_block() -> tuple[str, str]:
    text = WORKFLOW.read_text(encoding='utf-8')
    marker = '      - name: Audit current GitHub ticket states without mutations\n'
    if text.count(marker) != 1:
        raise AssertionError('one authoritative audit step is required')
    step = text.split(marker, 1)[1].split('      - name:', 1)[0]
    return step, textwrap.dedent(step.split('        run: |\n', 1)[1])


class DeliveryWorkflowTests(unittest.TestCase):
    def test_actual_step_retains_read_only_scope_and_failure_propagation(self):
        step, script = audit_block()
        self.assertIn('        shell: bash\n', step)
        self.assertIn('set -euo pipefail', script)
        self.assertIn('snapshot > delivery-artifacts/github-issues.json', script)
        self.assertIn('audit --input delivery-artifacts/github-issues.json | tee delivery-artifacts/issue-audit.json', script)
        self.assertNotIn('continue-on-error', step)
        self.assertNotIn('|| true', script)
        self.assertIn('  contents: read\n  issues: read\n', WORKFLOW.read_text())

    def run_case(self, snapshot_exit: int, audit_exit: int) -> tuple[int, str, dict | None]:
        _, script = audit_block()
        with tempfile.TemporaryDirectory(prefix='arb-audit-test-') as root:
            directory = Path(root)
            (directory / 'delivery-artifacts').mkdir()
            fake = directory / 'python'
            fake.write_text('#!/bin/sh\n'
                            'if [ "$2" = snapshot ]; then\n'
                            '  printf \'{"issues":[]}\\n\'\n'
                            '  exit "$TEST_SNAPSHOT_EXIT"\n'
                            'fi\n'
                            'if [ "$2" = audit ]; then\n'
                            '  printf \'{"findings":[{"id":"ARB-016","reason":"SYNTHETIC_TEST"}]}\\n\'\n'
                            '  exit "$TEST_AUDIT_EXIT"\n'
                            'fi\nexit 99\n', encoding='utf-8')
            fake.chmod(0o700)
            env = dict(os.environ)
            env.update(PATH=f'{root}{os.pathsep}{os.environ.get("PATH", "")}',
                       TEST_SNAPSHOT_EXIT=str(snapshot_exit), TEST_AUDIT_EXIT=str(audit_exit))
            env.pop('GH_TOKEN', None)
            env.pop('GITHUB_TOKEN', None)
            result = subprocess.run(['bash', '-c', script], cwd=directory, env=env,
                                    capture_output=True, text=True, timeout=10, check=False)
            report = directory / 'delivery-artifacts/issue-audit.json'
            return result.returncode, result.stdout, json.loads(report.read_text()) if report.exists() else None

    def test_failed_audit_is_visible_retained_and_still_fails(self):
        code, output, report = self.run_case(0, 1)
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(output), report)
        self.assertEqual(report['findings'][0]['reason'], 'SYNTHETIC_TEST')

    def test_successful_producer_retains_its_exit_status(self):
        code, output, report = self.run_case(0, 0)
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(output), report)

    def test_failed_snapshot_never_runs_the_audit(self):
        code, output, report = self.run_case(2, 0)
        self.assertEqual(code, 2)
        self.assertEqual(output, '')
        self.assertIsNone(report)


if __name__ == '__main__':
    unittest.main()
