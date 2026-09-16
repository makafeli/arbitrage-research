"""Conservative read-only PR title/body check for accidental auto-closure.

Original task acceptance is an explicit native action after criterion review.
A negation or a quoted example does not make closing syntax safe in PR text.
Prefer plain references with the ticket's number before any status explanation.

This is not GitHub's Markdown parser or a complete merge authorization check.
Sidebar links, commit messages and later metadata races still require final
native review. No event text is executed, printed, or used as a shell argument.
Keyword reference: https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import re

REPOSITORY = 'makafeli/arbitrage-research'
MAX_EVENT_BYTES = 8 * 1024 * 1024
MAX_TEXT = 256 * 1024
REFERENCE = (r'(?:[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)?#[1-9][0-9]*'
             r'|https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(?:issues|pull)/[1-9][0-9]*')
CLOSURE = re.compile(r'\b(?:close[sd]?|fix(?:es|ed)?|resolve[sd]?)\b'
                     r'[\s:*_`]*\[?(?:' + REFERENCE + ')', re.IGNORECASE | re.ASCII)


def fields_with_closure(event: dict) -> list[str]:
    if not isinstance(event, dict) or not isinstance(event.get('repository'), dict):
        raise ValueError('INVALID_EVENT_SHAPE')
    if event['repository'].get('full_name') != REPOSITORY:
        raise ValueError('UNEXPECTED_REPOSITORY')
    pr = event.get('pull_request')
    if not isinstance(pr, dict) or type(pr.get('number')) is not int or pr['number'] <= 0:
        raise ValueError('INVALID_PULL_REQUEST')
    fields = []
    for name in ('title', 'body'):
        text = pr.get(name)
        if name == 'body' and text is None:
            text = ''
        if not isinstance(text, str) or len(text) > MAX_TEXT:
            raise ValueError('INVALID_PR_TEXT')
        if CLOSURE.search(text):
            fields.append(name)
    return fields


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('DUPLICATE_EVENT_KEY')
        result[key] = value
    return result


def main() -> int:
    if os.environ.get('GITHUB_EVENT_NAME') != 'pull_request':
        print(json.dumps({'check': 'NOT_A_PULL_REQUEST', 'github_mutations': False}))
        return 0
    try:
        with Path(os.environ['GITHUB_EVENT_PATH']).open('rb') as source:
            payload = source.read(MAX_EVENT_BYTES + 1)
        if len(payload) > MAX_EVENT_BYTES:
            raise ValueError('EVENT_SIZE_LIMIT')
        event = json.loads(payload, object_pairs_hook=unique_object)
        fields = fields_with_closure(event)
    except (KeyError, OSError, ValueError, TypeError, RecursionError):
        print(json.dumps({'check': 'INVALID_PR_EVENT', 'github_mutations': False}))
        return 1
    print(json.dumps({'check': 'MANUAL_ISSUE_ACCEPTANCE_REQUIRED' if fields else 'NO_CLOSING_TEXT',
                      'fields': fields, 'github_mutations': False}))
    return int(bool(fields))


if __name__ == '__main__':
    raise SystemExit(main())
