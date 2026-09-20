#!/usr/bin/python3 -I
"""Finite Railway pre-deploy gate for this public repository's exact main revision."""
from __future__ import annotations

import json
import os
import re
import signal
import sys
import time
import urllib.error
import urllib.request

REPOSITORY = 'makafeli/arbitrage-research'
REPOSITORY_ID = 1367512682
API = 'https://api.github.com/repos/' + REPOSITORY
REQUIRED = frozenset({
    '.github/workflows/ci.yml',
    '.github/workflows/recovery-review.yml',
    '.github/workflows/delivery-review.yml',
})
MAX_RESPONSE = 2 * 1024 * 1024
MAX_POLLS = 30
POLL_SECONDS = 20
WALL_SECONDS = 660
PENDING = frozenset({'queued', 'in_progress', 'waiting', 'pending', 'requested'})


class GateError(Exception):
    """Only fixed public reason codes are returned to logs."""


def require(condition: bool, code: str = 'CI_METADATA_REJECTED') -> None:
    if not condition:
        raise GateError(code)


def positive_int(value: object) -> bool:
    return type(value) is int and 0 < value < 2**63


def revision(environment: dict[str, str]) -> str:
    require(environment.get('RAILWAY_GIT_REPO_OWNER') == 'makafeli'
            and environment.get('RAILWAY_GIT_REPO_NAME') == 'arbitrage-research'
            and environment.get('RAILWAY_GIT_BRANCH') == 'main', 'CI_SOURCE_REJECTED')
    sha = environment.get('RAILWAY_GIT_COMMIT_SHA', '')
    require(re.fullmatch(r'[0-9a-f]{40}', sha) is not None, 'CI_SOURCE_REJECTED')
    return sha


def no_duplicate_keys(pairs: list[tuple[str, object]]) -> dict:
    value: dict = {}
    for key, item in pairs:
        require(key not in value)
        value[key] = item
    return value


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise GateError('CI_REDIRECT_REJECTED')


def fetch(sha: str) -> dict:
    # Only the optional ARB_CI_GATE_GITHUB_TOKEN credential; no environment
    # proxies, no redirects, no arbitrary API base.
    require(re.fullmatch(r'[0-9a-f]{40}', sha) is not None, 'CI_SOURCE_REJECTED')
    url = API + '/actions/runs?event=push&branch=main&per_page=100&head_sha=' + sha
    headers = {
        'Accept': 'application/vnd.github+json',
        'X-GitHub-Api-Version': '2022-11-28',
        'User-Agent': 'arbitrage-research-worker-ci-gate',
        'Cache-Control': 'no-cache',
    }
    token = os.environ.get('ARB_CI_GATE_GITHUB_TOKEN', '')
    if token:
        require(re.fullmatch(r'[A-Za-z0-9_]{20,255}', token) is not None, 'CI_TOKEN_REJECTED')
        headers['Authorization'] = 'Bearer ' + token
    request = urllib.request.Request(url, headers=headers)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    try:
        with opener.open(request, timeout=10) as response:
            require(response.status == 200, 'CI_API_UNAVAILABLE')
            data = response.read(MAX_RESPONSE + 1)
        require(len(data) <= MAX_RESPONSE, 'CI_RESPONSE_LIMIT')
        return json.loads(data, object_pairs_hook=no_duplicate_keys)
    except (urllib.error.URLError, OSError, ValueError, RecursionError):
        raise GateError('CI_API_UNAVAILABLE') from None


def assess(payload: object, sha: str) -> tuple[bool, dict[str, int]]:
    """Only latest main-push workflow runs count; PR checks cannot authorize deploys."""
    require(isinstance(payload, dict))
    runs, total = payload.get('workflow_runs'), payload.get('total_count')
    require(isinstance(runs, list) and type(total) is int
            and 0 <= total <= 100 and len(runs) == total, 'CI_RESPONSE_INCOMPLETE')
    latest: dict[str, dict] = {}
    identities: set[int] = set()
    for run in runs:
        require(isinstance(run, dict))
        require(run.get('head_sha') == sha and run.get('head_branch') == 'main'
                and run.get('event') == 'push', 'CI_SCOPE_MISMATCH')
        for name in ('repository', 'head_repository'):
            repo = run.get(name)
            require(isinstance(repo, dict) and repo.get('id') == REPOSITORY_ID
                    and repo.get('full_name') == REPOSITORY, 'CI_SCOPE_MISMATCH')
        path, rid = run.get('path'), run.get('id')
        require(isinstance(path, str) and re.fullmatch(r'\.github/workflows/[A-Za-z0-9_.-]+\.ya?ml', path)
                is not None)
        require(positive_int(rid) and positive_int(run.get('run_attempt'))
                and positive_int(run.get('run_number')) and rid not in identities)
        identities.add(rid)
        status, conclusion = run.get('status'), run.get('conclusion')
        require(isinstance(status, str) and (status == 'completed' or status in PENDING))
        require((status == 'completed' and isinstance(conclusion, str) and conclusion in {
            'success', 'failure', 'cancelled', 'timed_out', 'action_required',
            'neutral', 'skipped', 'stale', 'startup_failure',
        }) or (status in PENDING and conclusion is None))
        prior = latest.get(path)
        if prior is None or (run['run_number'], rid, run['run_attempt']) > (
            prior['run_number'], prior['id'], prior['run_attempt']
        ):
            latest[path] = run
    waiting = not REQUIRED.issubset(latest)
    for path, run in latest.items():
        if run['status'] != 'completed':
            waiting = True
        elif run['conclusion'] != 'success':
            # Nonmandatory skipped/neutral jobs are permitted, not mandatory checks.
            require(path not in REQUIRED and run['conclusion'] in {'skipped', 'neutral'},
                    'CI_CHECK_FAILED')
    return not waiting, {path: latest[path]['id'] for path in sorted(REQUIRED) if path in latest}


def check(sha: str, reader=fetch, sleep=time.sleep) -> dict:
    for poll in range(MAX_POLLS):
        ready, checks = assess(reader(sha), sha)
        if ready:
            return {'status': 'CI_GATE_PASSED', 'source_commit': sha,
                    'required_runs': checks, 'provider_requests': 0,
                    'database_requests': 0, 'execution_authorized': False}
        if poll + 1 < MAX_POLLS:
            sleep(POLL_SECONDS)
    raise GateError('CI_WAIT_EXHAUSTED')


def alarm(_signum, _frame):
    raise GateError('CI_WALL_LIMIT')


def main(args: list[str] | None = None) -> int:
    try:
        require(not (sys.argv[1:] if args is None else args), 'CI_ARGUMENTS_REJECTED')
        sha = revision(dict(os.environ))
        signal.signal(signal.SIGALRM, alarm)
        signal.alarm(WALL_SECONDS)
        try:
            result = check(sha)
        finally:
            signal.alarm(0)
        print(json.dumps(result, sort_keys=True))
        return 0
    except GateError as error:
        print(json.dumps({'status': 'CI_GATE_BLOCKED', 'reason': str(error),
                          'execution_authorized': False}), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
