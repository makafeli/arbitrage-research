#!/usr/bin/env python3
"""Report native issue counts separately from implementation and later-stage work.

Offline only. The supplied GitHub snapshot is evidence with a read window, not
an atomic live query or authority to close issues. No provider/AI jobs are started.
"""
from __future__ import annotations
import argparse
from collections import Counter
import json
from pathlib import Path
import re

REPO = 'makafeli/arbitrage-research'
STAGES = {'M0': 'initial_research_software', 'M1': 'initial_research_software',
          'M2': 'initial_research_software', 'M3': 'initial_research_software',
          'M4': 'initial_research_software', 'M5': 'campaign_and_decision',
          'M6': 'later_live_expansion_maintenance', 'M7': 'later_live_expansion_maintenance'}
STATES = {'completed', 'implemented_pending_acceptance', 'in_progress', 'planned'}

def summarize(backlog: dict, progress: dict, snapshot: dict) -> dict:
    if snapshot.get('repository') != REPO or snapshot.get('complete') is not True:
        raise ValueError('Complete matching native snapshot required')
    if not isinstance(snapshot.get('atomic_snapshot'), bool):
        raise ValueError('Snapshot atomicity must be disclosed')
    if not snapshot.get('read_started_at') or not snapshot.get('read_finished_at'):
        raise ValueError('Snapshot read window required')
    issues = snapshot.get('issues', [])
    if not isinstance(issues, list) or not issues:
        raise ValueError('Native issues required')
    by_number = {}
    for issue in issues:
        n = issue.get('number')
        if type(n) is not int or n < 1 or n in by_number or issue.get('state') not in {'open','closed'}:
            raise ValueError('Invalid or duplicated native issue')
        by_number[n] = issue
    tasks = {t['id']:t for t in backlog['tickets']}
    registers = {t['id']:t for t in progress['tickets']}
    if len(tasks) != len(backlog['tickets']) or len(registers) != len(progress['tickets']) or tasks.keys() != registers.keys():
        raise ValueError('Task register mismatch')
    native_tasks = {}
    rows = []
    discrepancies = []
    for tid, t in tasks.items():
        p = registers[tid]
        match = re.fullmatch(r'https://github\.com/makafeli/arbitrage-research/issues/([1-9][0-9]*)',p['issue_url'])
        if not match or p.get('state') not in STATES or t['milestone'] not in STAGES:
            raise ValueError('Invalid task metadata')
        n = int(match[1])
        if n not in by_number or n in native_tasks:
            raise ValueError('Missing or reused task issue')
        native = by_number[n]
        if f'<!-- arb-ticket:{tid} -->' not in native.get('body',''):
            raise ValueError('Native task identity mismatch')
        native_tasks[n] = tid
        expected = 'closed' if p['state'] == 'completed' else 'open'
        if native['state'] != expected or (expected == 'closed' and native.get('state_reason') != 'completed'):
            discrepancies.append({'ticket':tid,'issue':n,'register':p['state'],'native':native['state'],
                                  'native_state_reason':native.get('state_reason')})
        rows.append({'ticket':tid,'issue':n,'title':t['title'],'milestone':t['milestone'],
                     'stage':STAGES[t['milestone']],'native_state':native['state'],
                     'implementation_state':p['state'],'remaining_acceptance':p.get('remaining_acceptance','')})
    epic_ids = {e['id'] for e in backlog['epics']}
    open_epics=[]; other=[]
    for n,issue in by_number.items():
        if n in native_tasks or issue['state'] != 'open': continue
        found = re.findall(r'<!-- arb-ticket:(EPIC-\d+) -->',issue.get('body',''))
        if len(found)==1 and found[0] in epic_ids: open_epics.append(n)
        else: other.append(n)
    opened = [r for r in rows if r['native_state']=='open']
    return {'schema_version':1,'repository':REPO,
            'snapshot':{k:snapshot[k] for k in ['read_started_at','read_finished_at','atomic_snapshot']},
            'open_issues':sum(i['state']=='open' for i in issues),
            'original_tasks_total':len(rows),'original_tasks_open':len(opened),
            'original_tasks_closed':len(rows)-len(opened),'open_epics':sorted(open_epics),
            'open_other_issues':sorted(other),'open_tasks_by_stage':dict(Counter(r['stage'] for r in opened)),
            'open_tasks_by_milestone':dict(sorted(Counter(r['milestone'] for r in opened).items())),
            'register_states':dict(sorted(Counter(p['state'] for p in registers.values()).items())),
            'state_discrepancies':discrepancies,'tasks':sorted(rows,key=lambda r:r['ticket']),
            'closure_authorized':False,'completion_percentage':None}

def markdown(report: dict) -> str:
    lines=['# Delivery status','',f"Native snapshot: {report['snapshot']['read_started_at']} to {report['snapshot']['read_finished_at']}.",
           f"Atomic snapshot: {str(report['snapshot']['atomic_snapshot']).lower()}. This report never closes issues.",'',
           '| Measure | Count |','|---|---:|',f"| Open issues, excluding pull requests | {report['open_issues']} |",
           f"| Open original tasks | {report['original_tasks_open']} |",f"| Open parent epics | {len(report['open_epics'])} |",
           f"| Other open tooling/issues | {len(report['open_other_issues'])} |",f"| Closed original tasks | {report['original_tasks_closed']} |",'',
           '| Remaining stage | Original tasks |','|---|---:|']
    for stage,count in report['open_tasks_by_stage'].items(): lines.append(f'| {stage} | {count} |')
    lines+=['','Epics contain tasks; they are not additional implementation estimates. Closed-task share is not a percentage of engineering completion.',
            'Implemented-pending-acceptance can still require integration and qualification. It does not mean review alone remains.',
            f"Register/native discrepancies: {len(report['state_discrepancies'])}.",'',
            '| Task | Phase | Native | Implementation |','|---|---|---|---|']
    for r in report['tasks']:
        lines.append(f"| [{r['ticket']} / #{r['issue']}](https://github.com/{REPO}/issues/{r['issue']}) | {r['milestone']} | {r['native_state']} | {r['implementation_state']} |")
    return '\n'.join(lines)+'\n'

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--snapshot',type=Path,required=True)
    parser.add_argument('--backlog',type=Path,default=Path('planning/backlog.json'))
    parser.add_argument('--progress',type=Path,default=Path('planning/implementation-progress.json'))
    parser.add_argument('--format',choices=['json','markdown'],default='json')
    args=parser.parse_args()
    try:
        values=[]
        for path in [args.backlog,args.progress,args.snapshot]:
            with path.open('rb') as f: data=f.read(16*1024*1024+1)
            if len(data)>16*1024*1024: raise ValueError('Input too large')
            values.append(json.loads(data))
        report=summarize(*values)
    except (OSError,ValueError,KeyError,TypeError) as error:
        parser.exit(2,f'Backlog report refused: {type(error).__name__}\n')
    print(markdown(report) if args.format=='markdown' else json.dumps(report,indent=2),end='\n')

if __name__=='__main__': main()
