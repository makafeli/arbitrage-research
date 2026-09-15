"""Read-only native GitHub/git audit; never execute code read from branches."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

REPO = 'makafeli/arbitrage-research'
EXPECTED_MAIN = 'ff61b29e75fe4f368eab521a1c9dfdd8c2d90cb8'
OUT = Path(os.environ['RUNNER_TEMP']) / 'repository-audit'
OUT.mkdir(exist_ok=False)


def api(path):
    result = subprocess.run(['gh', 'api', 'repos/' + REPO + path], text=True,
                            capture_output=True, check=True, timeout=60)
    return json.loads(result.stdout)


def pages(path):
    rows = []
    for page in range(1, 21):
        sep = '&' if '?' in path else '?'
        result = api(path + sep + f'per_page=100&page={page}')
        assert isinstance(result, list)
        rows.extend(result)
        if len(result) < 100:
            return rows
    raise RuntimeError('pagination bound exceeded; incomplete data is not actionable')


def git(*args):
    return subprocess.check_output(['git', *args], text=True, timeout=60).strip()


def ancestor(a, b):
    result = subprocess.run(['git', 'merge-base', '--is-ancestor', a, b], timeout=30)
    assert result.returncode in (0, 1)
    return result.returncode == 0


main = api('/git/ref/heads/main')['object']['sha']
assert main == EXPECTED_MAIN, 'main moved before audit'
branches = pages('/branches')
prs = pages('/pulls?state=all&sort=created&direction=desc')
issues = [v for v in pages('/issues?state=all&sort=created&direction=asc') if 'pull_request' not in v]
subprocess.run(['git', 'fetch', '--no-tags', 'origin',
                '+refs/heads/*:refs/remotes/origin/*'], check=True, timeout=120)
report = []
for branch in branches:
    name, sha = branch['name'], branch['commit']['sha']
    assert git('rev-parse', 'refs/remotes/origin/' + name) == sha, 'branch moved during fetch'
    relevant = [p for p in prs if p['head']['ref'] == name
                and p['head'].get('repo') and p['head']['repo']['full_name'] == REPO]
    active = [p['number'] for p in relevant if p['state'] == 'open']
    exact_merged = [p for p in relevant if p['head']['sha'] == sha and p.get('merged_at')]
    merged = [p for p in exact_merged if ancestor(p['merge_commit_sha'], main)]
    included = ancestor(sha, main)
    if name == 'main' or branch['protected'] or active:
        status = 'KEEP_MAIN_PROTECTED_OR_OPEN_PR'
    elif included:
        status = 'DELETE_INTEGRATED_ANCESTOR'
    elif merged:
        status = 'DELETE_EXACT_HEAD_MERGED_PR'
    else:
        status = 'KEEP_REVIEW_UNINTEGRATED'
    row = {'branch': name, 'sha': sha, 'protected': branch['protected'],
           'open_prs': active, 'ancestor_of_main': included, 'decision': status,
           'prs': [{'number': p['number'], 'state': p['state'], 'head': p['head']['sha'],
                    'merged_at': p.get('merged_at'), 'merge_commit': p.get('merge_commit_sha')}
                   for p in relevant]}
    if merged:
        row['verified_merged_pr'] = merged[0]['number']
        row['verified_merge_commit'] = merged[0]['merge_commit_sha']
        row['tree_equals_merge'] = git('rev-parse', sha+'^{tree}') == git('rev-parse', merged[0]['merge_commit_sha']+'^{tree}')
    if status == 'KEEP_REVIEW_UNINTEGRATED':
        base = git('merge-base', main, sha)
        row['merge_base'] = base
        row['unique_paths'] = git('diff', '--name-only', base, sha).splitlines()
        row['head_message'] = git('show', '-s', '--format=%B', sha)
        patch = subprocess.check_output(['git', 'diff', '--binary', base, sha], timeout=60)
        filename = 'unintegrated-' + sha + '.patch'
        (OUT / filename).write_bytes(patch)
        row['patch_file'] = filename
        row['patch_sha256'] = hashlib.sha256(patch).hexdigest()
    report.append(row)
progress = json.loads(git('show', main + ':planning/implementation-progress.json'))
backlog = json.loads(git('show', main + ':planning/backlog.json'))
by_number = {v['number']: v for v in issues}
drift = []
for task in progress['tickets']:
    number = int(task['issue_url'].rstrip('/').rsplit('/', 1)[-1])
    actual = by_number[number]
    expected_closed = task['state'] == 'completed'
    if expected_closed != (actual['state'] == 'closed' and actual.get('state_reason') == 'completed'):
        drift.append({'ticket': task['id'], 'number': number, 'register_state': task['state'],
                      'native_state': actual['state'], 'reason': actual.get('state_reason')})
summary = {'repository': REPO, 'source_main': main, 'workflow_sha': os.environ['GITHUB_SHA'],
           'run_id': os.environ['GITHUB_RUN_ID'], 'branch_count': len(branches),
           'open_issues': sum(v['state'] == 'open' for v in issues),
           'open_prs': [p['number'] for p in prs if p['state'] == 'open'],
           'original_tasks': len(progress['tickets']),
           'accepted_original_tasks': sum(t['state'] == 'completed' for t in progress['tickets']),
           'register_native_drift': drift, 'branches': report,
           'mutation_performed': False}
assert api('/git/ref/heads/main')['object']['sha'] == main, 'main moved during audit'
for name, data in [('audit.json', summary), ('issues.json', issues), ('pulls.json', prs),
                   ('branches.json', branches), ('progress.json', progress), ('backlog.json', backlog)]:
    (OUT / name).write_text(json.dumps(data, indent=2) + '\n')
subprocess.run(['git', 'archive', '--format=tar.gz', '--output='+str(OUT/'main-source.tar.gz'), main],
               check=True, timeout=60)
subprocess.run(['git', 'bundle', 'create', str(OUT/'branches-before.bundle'), '--all'], check=True, timeout=120)
(OUT/'SHA256SUMS').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.name+'\n'
                                    for p in sorted(OUT.iterdir()) if p.is_file()))
print(json.dumps({k:v for k,v in summary.items() if k != 'branches'}, indent=2))
print(json.dumps([{'branch':r['branch'],'decision':r['decision'],'pr':r.get('verified_merged_pr')}
                  for r in report], indent=2))
