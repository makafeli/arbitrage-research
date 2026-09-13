#!/usr/bin/env python3
"""Read-only delivery planner and closeout preflight. Never starts agents or mutates GitHub."""
from __future__ import annotations

import argparse
import hashlib
from datetime import datetime, timezone
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'makafeli/arbitrage-research'
STATES = {'planned', 'in_progress', 'implemented_pending_acceptance', 'blocked', 'completed'}
LANES = ('foundation-operations', 'platform-engine', 'base', 'solana', 'dashboard')
CHECKS = ('specifications', 'rust', 'web', 'containers')
SHA = re.compile(r'^[0-9a-f]{40}$')


class DeliveryError(ValueError):
    """Invalid or incomplete delivery evidence; errors never grant permission."""


def require(condition: Any, message: str) -> None:
    if not condition:
        raise DeliveryError(message)


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        require(key not in result, f'duplicate JSON key: {key}')
        result[key] = value
    return result


def read_json(path: Path) -> Any:
    require(path.is_file() and path.stat().st_size <= 16 * 1024 * 1024,
            f'missing or oversized JSON input: {path.name}')
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=unique_object)


def index(rows: list[dict], key: str, label: str) -> dict:
    require(isinstance(rows, list), f'{label}: list required')
    result = {}
    for row in rows:
        require(isinstance(row, dict) and key in row, f'{label}: missing {key}')
        identity = row[key]
        require(isinstance(identity, (str, int)) and not isinstance(identity, bool),
                f'{label}: invalid identity')
        require(identity not in result, f'{label}: duplicate {identity}')
        result[identity] = row
    return result


def issue_number(row: dict) -> int:
    match = re.fullmatch(r'https://github\.com/' + re.escape(REPOSITORY)
                         + r'/issues/([1-9][0-9]*)', row.get('issue_url', ''))
    require(match, f"{row.get('id')}: invalid canonical issue URL")
    return int(match.group(1))


def load_register(root: Path) -> tuple[dict, dict]:
    backlog = read_json(root / 'planning/backlog.json')
    progress = read_json(root / 'planning/implementation-progress.json')
    tasks = index(backlog['tickets'], 'id', 'backlog')
    rows = index(progress['tickets'], 'id', 'progress')
    require(tasks.keys() == rows.keys(), 'backlog and progress must have identical task IDs')
    numbers = set()
    for ticket_id, task in tasks.items():
        require(re.fullmatch(r'ARB-[0-9]{3}', ticket_id), 'invalid task ID')
        deps = task['dependencies']
        require(isinstance(deps, list) and all(isinstance(d, str) for d in deps), 'invalid dependencies')
        require(len(deps) == len(set(deps)) and set(deps) <= tasks.keys(), 'duplicate or missing dependency')
        row = rows[ticket_id]
        require(row['dependencies'] == deps, f'{ticket_id}: dependency drift')
        require(row['state'] in STATES, f'{ticket_id}: unknown progress state')
        number = issue_number(row)
        require(number not in numbers, f'{ticket_id}: duplicate issue mapping')
        numbers.add(number)
        require(isinstance(task.get('body'), str) and acceptance(task['body']),
                f'{ticket_id}: missing acceptance criteria')
        if row['state'] == 'completed':
            require(row.get('evidence') and row.get('acceptance_review'), f'{ticket_id}: missing acceptance evidence')
            require(not row.get('remaining_acceptance') and not row.get('external_prerequisite'),
                    f'{ticket_id}: completed with unresolved acceptance')
            require(all(rows[d]['state'] == 'completed' for d in deps), f'{ticket_id}: incomplete dependency')
    visiting, visited = set(), set()

    def visit(ticket_id: str) -> None:
        require(ticket_id not in visiting, f'dependency cycle at {ticket_id}')
        if ticket_id in visited:
            return
        visiting.add(ticket_id)
        for dep in tasks[ticket_id]['dependencies']:
            visit(dep)
        visiting.remove(ticket_id)
        visited.add(ticket_id)

    for ticket_id in tasks:
        visit(ticket_id)
    return tasks, rows


def acceptance(body: str) -> list[tuple[str, bool]]:
    section = re.search(r'^### Acceptance criteria\s*\n(.*?)(?=^### |\Z)', body, re.M | re.S)
    if not section:
        return []
    return [(text.strip(), checked.lower() == 'x')
            for checked, text in re.findall(r'^- \[([ xX])\] (.+)$', section.group(1), re.M)]


def lane(ticket_id: str) -> str:
    number = int(ticket_id[4:])
    if number in {16, 19, 28, 30}:
        return 'base'
    if number in {17, 20, 29, 31}:
        return 'solana'
    if number in {*range(36, 42), 43, 59}:
        return 'dashboard'
    if number in {*range(1, 8), 24, 42, *range(44, 51), 60, 61, 62, 63, 66, 68}:
        return 'foundation-operations'
    return 'platform-engine'


def readiness(task: dict, row: dict, rows: dict) -> str:
    if row['state'] == 'completed':
        return 'accepted'
    if task['milestone'] in {'M6', 'M7'}:
        return 'later-gate'
    if row.get('external_prerequisite') or row['state'] == 'blocked':
        return 'external-or-explicit-block'
    if row['state'] == 'implemented_pending_acceptance':
        return 'acceptance-review'
    # Existing prerequisite code is sufficient for a scoped implementation
    # proposal, never for closing its dependent acceptance gate.
    if all(rows[d]['state'] in {'completed', 'implemented_pending_acceptance'}
           for d in task['dependencies']):
        return 'implementation-proposal'
    return 'dependency-blocked'


def plan(tasks: dict, rows: dict, capacity: int = 25) -> dict:
    require(type(capacity) is int and 1 <= capacity <= 50, 'capacity must be an integer in 1..50')
    entries = []
    for ticket_id, task in sorted(tasks.items(), key=lambda item: (item[1]['priority'], item[1]['milestone'], item[0])):
        row = rows[ticket_id]
        entries.append({'id': ticket_id, 'issue_number': issue_number(row), 'title': task['title'],
                        'manager': lane(ticket_id), 'state': row['state'],
                        'work_kind': readiness(task, row, rows),
                        'acceptance_blockers': [d for d in task['dependencies'] if rows[d]['state'] != 'completed'],
                        'remaining_acceptance': row.get('remaining_acceptance', ''),
                        'external_prerequisite': row.get('external_prerequisite', '')})
    queues = {name: [e for e in entries if e['manager'] == name and e['work_kind'] in
                     {'implementation-proposal', 'acceptance-review'}] for name in LANES}
    per_manager = {name: capacity // len(LANES) + (i < capacity % len(LANES)) for i, name in enumerate(LANES)}
    proposed = []
    used = Counter()
    # Round-robin proposals prevent one lane consuming all capacity. These are
    # NOT claims: ownership manifests must be checked before starting work.
    while len(proposed) < capacity and any(queues[name] and used[name] < per_manager[name] for name in LANES):
        for name in LANES:
            if queues[name] and used[name] < per_manager[name] and len(proposed) < capacity:
                proposed.append(queues[name].pop(0)['id'])
                used[name] += 1
    return {'schema_version': 1, 'repository': REPOSITORY,
            'execution': 'PLAN_ONLY_NO_AGENTS_STARTED', 'github_mutations': False,
            'orchestrators': 1, 'managers': list(LANES), 'worker_capacity': capacity,
            'active_workers': 0, 'manager_capacity': per_manager, 'proposed_reviews_or_implementations': proposed,
            'ownership_validated': False, 'state_counts': dict(sorted(Counter(r['state'] for r in rows.values()).items())),
            'work_counts': dict(sorted(Counter(e['work_kind'] for e in entries).items())), 'tickets': entries}


def owned_path(path: str) -> str:
    require(isinstance(path, str) and path and '\\' not in path and not any(c in path for c in '*?[]'),
            'ownership uses literal relative paths, not globs')
    normalized = path.rstrip('/')
    require(normalized not in {'', '.'} and not path.startswith('/') and
            all(p not in {'', '.', '..'} for p in normalized.split('/')), 'invalid ownership path')
    require(not any(part == '.git' for part in PurePosixPath(normalized).parts), 'git internals are never worker-owned')
    return normalized


def validate_assignments(packet: dict, tasks: dict, rows: dict, head: str, capacity: int) -> dict:
    require(SHA.fullmatch(head), 'exact source head SHA required')
    topology = plan(tasks, rows, capacity)
    require(isinstance(packet, dict), 'assignment object required')
    require(packet.get('schema_version') == 1 and packet.get('base_sha') == head,
            'assignment version or base SHA mismatch')
    assignments = packet.get('assignments')
    by_ticket = index(assignments, 'ticket_id', 'assignments')
    require(len(by_ticket) <= capacity, 'worker capacity exceeded')
    owners, branches, paths = set(), set(), []
    manager_load = Counter()
    for ticket_id, assignment in by_ticket.items():
        require(ticket_id in tasks, 'assignment references unknown ticket')
        require(readiness(tasks[ticket_id], rows[ticket_id], rows) in
                {'implementation-proposal', 'acceptance-review'}, f'{ticket_id}: not eligible for this work wave')
        require(assignment.get('manager') == lane(ticket_id), 'assignment manager mismatch')
        manager_load[lane(ticket_id)] += 1
        require(manager_load[lane(ticket_id)] <= topology['manager_capacity'][lane(ticket_id)], 'manager capacity exceeded')
        worker = assignment.get('worker')
        branch = assignment.get('branch')
        require(isinstance(worker, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,64}', worker), 'invalid worker identity')
        require(worker not in owners, 'worker already assigned in this wave')
        require(isinstance(branch, str) and re.fullmatch(r'(feat|fix|docs)/ARB-[0-9]{3}-[a-z0-9][a-z0-9-]*', branch)
                and branch.startswith(tuple(f'{prefix}/{ticket_id}-' for prefix in ('feat', 'fix', 'docs'))), 'invalid ticket branch')
        require(branch not in branches, 'branch already assigned')
        owners.add(worker); branches.add(branch)
        writes = assignment.get('write_paths')
        require(isinstance(writes, list) and writes and len(writes) <= 128, 'bounded explicit write paths required')
        for path in writes:
            current = owned_path(path)
            # Shared contract/build/integration changes have one orchestrator.
            require(not (current in {'Cargo.toml', 'Cargo.lock', 'GOAL.md', 'AGENTS.md'}
                         or current.split('/')[0] in {'.github', 'planning', 'specs'}),
                    'shared integration path requires orchestrator ownership')
            for prior, prior_ticket in paths:
                require(not (current == prior or current.startswith(prior + '/') or prior.startswith(current + '/')),
                        f'ownership conflict: {ticket_id} and {prior_ticket}')
            paths.append((current, ticket_id))
    return {'schema_version': 1, 'validation': 'ASSIGNMENTS_COMPATIBLE', 'base_sha': head,
            'assignments': len(assignments), 'agents_started': 0,
            'scope': 'Static ownership preflight, not a distributed lock or worker launcher.'}


def audit(tasks: dict, rows: dict, snapshot: dict) -> dict:
    require(isinstance(snapshot, dict), 'snapshot object required')
    require(snapshot.get('repository') == REPOSITORY and snapshot.get('complete') is True,
            'complete repository-scoped issue snapshot required')
    issues = index(snapshot.get('issues'), 'number', 'GitHub issues')
    findings = []
    marker_owners = {}
    for number, issue in issues.items():
        for marker in re.findall(r'<!-- arb-ticket:(ARB-[0-9]{3}) -->', issue.get('body') or ''):
            if marker in marker_owners:
                findings.append({'id': marker, 'reason': 'DUPLICATE_REMOTE_MARKER'})
            marker_owners[marker] = number
    for ticket_id in tasks:
        row = rows[ticket_id]; number = issue_number(row); remote = issues.get(number)
        if remote is None:
            findings.append({'id': ticket_id, 'reason': 'ISSUE_MISSING'}); continue
        markers = re.findall(r'<!-- arb-ticket:(ARB-[0-9]{3}) -->', remote.get('body', ''))
        if markers != [ticket_id]:
            findings.append({'id': ticket_id, 'reason': 'ISSUE_MARKER_MISMATCH'})
        completed = remote.get('state') == 'closed' and remote.get('state_reason') == 'completed'
        if remote.get('state') not in {'open', 'closed'}:
            findings.append({'id': ticket_id, 'reason': 'UNKNOWN_REMOTE_STATE'})
        if (row['state'] == 'completed') != completed:
            findings.append({'id': ticket_id, 'reason': 'ACCEPTANCE_STATE_DRIFT'})
        if remote.get('state') == 'closed' and not completed:
            findings.append({'id': ticket_id, 'reason': 'CLOSED_WITHOUT_COMPLETION_SCOPE_DECISION_REQUIRED'})
    return {'schema_version': 1, 'checked_tickets': len(tasks), 'findings': findings,
            'github_mutations': False, 'snapshot_freshness': 'CALLER_MUST_VERIFY_CAPTURE_TIME'}


def closeout(ticket_id: str, packet: dict, tasks: dict, rows: dict, expected_head: str) -> dict:
    require(ticket_id in tasks and SHA.fullmatch(expected_head), 'known ticket and exact head SHA required')
    task, row = tasks[ticket_id], rows[ticket_id]
    require(isinstance(packet, dict), 'closeout object required')
    require(packet.get('schema_version') == 1 and packet.get('repository') == REPOSITORY
            and packet.get('ticket_id') == ticket_id, 'closeout identity mismatch')
    require(not row.get('remaining_acceptance') and not row.get('external_prerequisite'),
            'progress still records unmet acceptance or external prerequisites')
    require(row.get('evidence') and row.get('acceptance_review'), 'missing registered acceptance review')
    require(all(rows[d]['state'] == 'completed' for d in task['dependencies']), 'dependencies not accepted')
    require(task['milestone'] not in {'M6', 'M7'}, 'later operator/independent gates are outside automatic preflight')
    pr = packet.get('pull_request', {})
    require(pr.get('merged') is True and pr.get('base_ref') == 'main' and pr.get('head_sha') == expected_head
            and isinstance(pr.get('merge_commit_sha'), str) and SHA.fullmatch(pr['merge_commit_sha']),
            'merged main-targeted PR and exact tested source head required')
    number = pr.get('number')
    require(type(number) is int and number > 0 and pr.get('url') == f'https://github.com/{REPOSITORY}/pull/{number}',
            'canonical PR identity required')
    checks = index(packet.get('checks'), 'name', 'checks')
    require(set(CHECKS) <= checks.keys(), 'missing required CI job')
    for name in CHECKS:
        check = checks[name]
        require(check.get('head_sha') == expected_head and check.get('status') == 'completed'
                and check.get('conclusion') == 'success', f'{name}: missing, failed or wrong-head CI')
        require(re.fullmatch(r'https://github\.com/' + re.escape(REPOSITORY) + r'/actions/runs/[1-9][0-9]*',
                            check.get('run_url', '')), 'canonical CI evidence URL required')
    review = packet.get('review', {})
    require(review.get('head_sha') == expected_head and review.get('status') == 'completed'
            and review.get('kind') in {'solo-maintainer', 'independent'}
            and isinstance(review.get('reviewer'), str) and review['reviewer'].strip()
            and review.get('unresolved_findings') == [] and review.get('unresolved_threads') == [],
            'completed exact-head review with no outstanding findings required')
    require(bool(review.get('evidence_url')), 'review evidence required')
    criteria = packet.get('criteria')
    expected = [text for text, _ in acceptance(task['body'])]
    require(isinstance(criteria, list) and all(isinstance(c, dict) for c in criteria)
            and [c.get('text') for c in criteria] == expected,
            'all original acceptance criteria must appear in original order')
    for criterion in criteria:
        evidence = criterion.get('evidence')
        require(criterion.get('result') == 'passed' and isinstance(evidence, list) and evidence
                and all(isinstance(item, str) and item.strip() for item in evidence), 'unchecked or unsupported acceptance criterion')
    snapshot = packet.get('github_snapshot', {})
    require(not audit(tasks, rows, snapshot)['findings'], 'GitHub/progress drift must be reconciled before final closeout')
    remote = index(snapshot['issues'], 'number', 'GitHub issues')[issue_number(row)]
    require(acceptance(remote['body']) == [(text, True) for text in expected],
            'native issue checklist differs or retains unchecked criteria')
    return {'schema_version': 1, 'ticket_id': ticket_id, 'source_head_sha': expected_head,
            'verdict': 'EVIDENCE_PACKET_CONSISTENT', 'github_mutations': False,
            'warning': 'Supplied evidence is not authenticated by this offline check. Re-read GitHub PR, latest CI, reviews, issue and dependencies before any mutation. No ticket is automatically closed.'}


def collect_snapshot() -> dict:
    """Read-only paginated REST collection through the operator's existing gh auth."""
    issues = []
    started_at = datetime.now(timezone.utc).isoformat()
    for page in range(1, 1001):
        command = ['gh', 'api', '--method', 'GET',
                   f'repos/{REPOSITORY}/issues?state=all&per_page=100&page={page}']
        result = subprocess.run(command, check=False, capture_output=True, text=True, timeout=60)
        require(result.returncode == 0, f'GitHub snapshot read failed on page {page}; no complete snapshot produced')
        batch = json.loads(result.stdout, object_pairs_hook=unique_object)
        require(isinstance(batch, list), 'GitHub did not return an issue array')
        issues.extend({key: issue.get(key) for key in ('number', 'state', 'state_reason', 'body', 'updated_at')}
                      for issue in batch if 'pull_request' not in issue)
        if len(batch) < 100:
            index(issues, 'number', 'collected issues')
            return {'schema_version': 1, 'repository': REPOSITORY, 'complete': True,
                    'read_started_at': started_at, 'read_finished_at': datetime.now(timezone.utc).isoformat(),
                    'atomic_snapshot': False, 'issues': issues}
    raise DeliveryError('GitHub snapshot page bound exceeded; no complete snapshot produced')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['plan', 'assignments', 'audit', 'closeout', 'snapshot'])
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--capacity', type=int, default=25)
    parser.add_argument('--input', type=Path)
    parser.add_argument('--head-sha')
    parser.add_argument('--ticket')
    args = parser.parse_args()
    try:
        if args.command == 'snapshot':
            report = collect_snapshot()
        else:
            tasks, rows = load_register(args.root)
            if args.command == 'plan':
                report = plan(tasks, rows, args.capacity)
                report['register_sha256'] = hashlib.sha256((args.root / 'planning/implementation-progress.json').read_bytes()).hexdigest()
            else:
                require(args.input is not None, '--input required')
                packet = read_json(args.input)
                if args.command == 'audit':
                    report = audit(tasks, rows, packet)
                elif args.command == 'assignments':
                    require(args.head_sha is not None, '--head-sha required')
                    report = validate_assignments(packet, tasks, rows, args.head_sha, args.capacity)
                else:
                    require(args.head_sha is not None and args.ticket is not None, '--head-sha and --ticket required')
                    report = closeout(args.ticket, packet, tasks, rows, args.head_sha)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1 if report.get('findings') else 0
    except (DeliveryError, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        print(json.dumps({'error': str(error), 'github_mutations': False}), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
