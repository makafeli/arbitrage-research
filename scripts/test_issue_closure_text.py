"""Offline regression checks for PR metadata; no API, account or issue writes."""
import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_issue_closure_text as guard


def event(body='', title='Scoped maintenance'):
    return {'repository': {'full_name': guard.REPOSITORY},
            'pull_request': {'number': 138, 'title': title, 'body': body}}


class ClosureTextTests(unittest.TestCase):
    def test_all_documented_keywords_and_reference_forms(self):
        for word in ('close', 'closes', 'closed', 'fix', 'fixes', 'fixed',
                     'resolve', 'resolves', 'resolved'):
            for reference in ('#999999', 'example/repository#999999',
                              'https://github.com/example/repository/issues/999999'):
                for separator in (' ', ': ', '\n'):
                    with self.subTest(word=word, reference=reference, separator=separator):
                        self.assertEqual(guard.fields_with_closure(event(word.upper() + separator + reference)), ['body'])

    def test_negative_statement_and_quoted_example_are_not_exempt(self):
        for body in ('This never closes #999999.', 'This does not fix #999999.',
                     '`Resolves #999999`', '**Closes:** [#999999](https://example.invalid)',
                     '> CLOSED #999999'):
            self.assertEqual(guard.fields_with_closure(event(body)), ['body'])

    def test_plain_references_and_status_prose_are_allowed(self):
        for body in (None, '', 'Refs #999999. No task completion is claimed.',
                     '#999999 remains open.', 'Fix the audit output; related task: #999999.',
                     'A post-merge check will verify the branch list.'):
            self.assertEqual(guard.fields_with_closure(event(body)), [])

    def test_title_is_checked_and_only_field_names_are_returned(self):
        self.assertEqual(guard.fields_with_closure(event('Fixes #999999', 'Close #999999')), ['title', 'body'])

    def test_malformed_scope_or_text_fails(self):
        for value in (None, [], {}, {'repository': {}},
                      event(title=None), event(title=123), event(body=['text']),
                      event(body='x' * (guard.MAX_TEXT + 1))):
            with self.subTest(value_type=type(value).__name__):
                with self.assertRaises(ValueError):
                    guard.fields_with_closure(value)
        wrong = event()
        wrong['pull_request']['number'] = True
        with self.assertRaises(ValueError):
            guard.fields_with_closure(wrong)

    def test_cli_failure_is_redacted_and_has_no_mutations(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / 'event.json'
            path.write_text(json.dumps(event('SENSITIVE_SENTINEL; never closes #999999')))
            output = io.StringIO()
            with patch.dict(os.environ, {'GITHUB_EVENT_NAME': 'pull_request', 'GITHUB_EVENT_PATH': str(path)}), contextlib.redirect_stdout(output):
                self.assertEqual(guard.main(), 1)
            self.assertNotIn('SENSITIVE_SENTINEL', output.getvalue())
            report = json.loads(output.getvalue())
            self.assertFalse(report['github_mutations'])
            self.assertEqual(report['fields'], ['body'])

    def test_duplicate_event_keys_are_rejected(self):
        with self.assertRaises(ValueError):
            json.loads('{"body":"first","body":"second"}', object_pairs_hook=guard.unique_object)

    def test_ci_checks_edits_without_a_write_token(self):
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/delivery-review.yml').read_text()
        self.assertIn('types: [opened, synchronize, reopened, edited]', workflow)
        section = workflow.split('      - name: Preserve explicit issue acceptance before merging\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn("if: github.event_name == 'pull_request'", section)
        self.assertIn('run: python scripts/check_issue_closure_text.py', section)
        self.assertNotIn('GH_TOKEN', section)
        self.assertNotIn('continue-on-error', section)


if __name__ == '__main__':
    unittest.main()
