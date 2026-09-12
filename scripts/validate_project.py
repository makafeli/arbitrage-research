#!/usr/bin/env python3
"""Check the handoff's dependency graph, local links and workspace structure.

This does not compile Rust, build the browser application or certify security.
"""
from __future__ import annotations

import ast
import json
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IGNORE = {'.git', 'node_modules', 'target', 'dist', '__pycache__'}


def files(suffix: str):
    return [p for p in ROOT.rglob('*' + suffix)
            if not any(part in IGNORE for part in p.relative_to(ROOT).parts)]


def main() -> None:
    for path in files('.json'):
        json.loads(path.read_text())
    for path in files('.toml'):
        tomllib.loads(path.read_text())
    for path in files('.py'):
        ast.parse(path.read_text(), filename=str(path))

    backlog = json.loads((ROOT / 'planning/backlog.json').read_text())
    entries = backlog['epics'] + backlog['tickets']
    by_id = {entry['id']: entry for entry in entries}
    assert len(entries) == len(by_id), 'Duplicate ticket IDs'
    assert len(backlog['epics']) == 8, 'Expected eight delivery epics'
    assert {entry['milestone'] for entry in entries} == {f'M{i}' for i in range(8)}
    for entry in entries:
        for key in ['id', 'title', 'body', 'milestone', 'labels', 'role', 'priority', 'estimate_days']:
            assert entry.get(key), f"{entry['id']}: missing {key}"
        assert len(entry['body']) >= 400, f"{entry['id']}: insufficient ticket detail"
        for dep in entry.get('dependencies', []):
            assert dep in by_id, f"{entry['id']}: unknown dependency {dep}"
            assert dep != entry['id'], f"{entry['id']}: self dependency"
    done: set[str] = set()
    visiting: set[str] = set()

    def visit(ticket_id: str) -> None:
        assert ticket_id not in visiting, f'Dependency cycle at {ticket_id}'
        if ticket_id in done:
            return
        visiting.add(ticket_id)
        for dep in by_id[ticket_id].get('dependencies', []):
            visit(dep)
        visiting.remove(ticket_id)
        done.add(ticket_id)

    for ticket_id in by_id:
        visit(ticket_id)

    missing = []
    github_prefix = 'https://github.com/makafeli/arbitrage-research/blob/main/'
    for path in files('.md'):
        source = path.read_text()
        # Ignore planned file trees and command examples within fenced blocks.
        source = re.sub(r'^```[^\n]*\n.*?^```\s*$', '', source, flags=re.M | re.S)
        for link in re.findall(r'\[[^\]]+\]\(([^)]+)\)', source):
            target = link.split('#')[0]
            if not target or target.startswith(('mailto:', 'app:', 'sandbox:')):
                continue
            if target.startswith(github_prefix):
                resolved = ROOT / target.removeprefix(github_prefix)
            elif '://' in target:
                continue
            else:
                resolved = path.parent / target
            if not resolved.exists():
                missing.append(f'{path.relative_to(ROOT)} -> {target}')
    assert not missing, 'Missing document links:\n' + '\n'.join(missing)

    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']
    for member in workspace['members']:
        assert (ROOT / member / 'Cargo.toml').is_file(), f'Missing workspace member {member}'

    print(json.dumps({
        'epics': len(backlog['epics']),
        'tickets': len(backlog['tickets']),
        'dependency_graph': 'acyclic with resolved IDs',
        'document_links': 'resolved',
        'json_toml_python': 'parsed',
        'workspace_members': len(workspace['members']),
        'scope': 'Structural verification; no runtime or security certification',
    }, indent=2))


if __name__ == '__main__':
    main()
