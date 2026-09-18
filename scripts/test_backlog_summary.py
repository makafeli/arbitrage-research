#!/usr/bin/env python3
"""Synthetic state reconciliation tests; not native GitHub operations."""
import copy
import unittest
from backlog_summary import summarize,markdown
B={'tickets':[{'id':'ARB-001','title':'First','milestone':'M1'}, {'id':'ARB-002','title':'Later','milestone':'M6'}],
   'epics':[{'id':'EPIC-01'}]}
P={'tickets':[{'id':'ARB-001','issue_url':'https://github.com/makafeli/arbitrage-research/issues/14','state':'completed'},
              {'id':'ARB-002','issue_url':'https://github.com/makafeli/arbitrage-research/issues/15','state':'planned'}]}
S={'repository':'makafeli/arbitrage-research','complete':True,'atomic_snapshot':False,
   'read_started_at':'2026-09-18T07:00:00Z','read_finished_at':'2026-09-18T07:00:01Z',
   'issues':[{'number':14,'state':'closed','state_reason':'completed','body':'<!-- arb-ticket:ARB-001 -->'},
             {'number':15,'state':'open','body':'<!-- arb-ticket:ARB-002 -->'},
             {'number':2,'state':'open','body':'<!-- arb-ticket:EPIC-01 -->'},
             {'number':107,'state':'open','body':'Tooling'}]}
class SummaryTests(unittest.TestCase):
    def test_counts_never_double_count_epics(self):
        r=summarize(B,P,S)
        self.assertEqual((r['open_issues'],r['original_tasks_open'],len(r['open_epics']),len(r['open_other_issues'])),(3,1,1,1))
        self.assertEqual(r['open_tasks_by_stage'],{'later_live_expansion_maintenance':1})
        self.assertIsNone(r['completion_percentage']);self.assertFalse(r['closure_authorized'])
    def test_implemented_is_still_open(self):
        p=copy.deepcopy(P);p['tickets'][1]['state']='implemented_pending_acceptance'
        self.assertEqual(summarize(B,p,S)['original_tasks_open'],1)
    def test_native_mismatch_is_reported_not_rewritten(self):
        s=copy.deepcopy(S);s['issues'][0]['state']='open'
        r=summarize(B,P,s);self.assertEqual(len(r['state_discrepancies']),1);self.assertEqual(r['original_tasks_open'],2)
    def test_not_planned_is_not_acceptance(self):
        s=copy.deepcopy(S);s['issues'][0]['state_reason']='not_planned'
        self.assertEqual(len(summarize(B,P,s)['state_discrepancies']),1)
    def test_incomplete_or_wrong_repo_refused(self):
        for patch in [{'complete':False},{'repository':'other/repo'},{'atomic_snapshot':None},{'read_started_at':None}]:
            with self.subTest(patch=patch),self.assertRaises(ValueError): summarize(B,P,{**S,**patch})
    def test_duplicate_native_issue_refused(self):
        with self.assertRaises(ValueError): summarize(B,P,{**S,'issues':S['issues']+[S['issues'][0]]})
    def test_missing_task_refused(self):
        with self.assertRaises(ValueError): summarize(B,P,{**S,'issues':S['issues'][1:]})
    def test_changed_marker_refused(self):
        s=copy.deepcopy(S);s['issues'][0]['body']='<!-- arb-ticket:ARB-002 -->'
        with self.assertRaises(ValueError): summarize(B,P,s)
    def test_duplicate_register_entry_refused(self):
        with self.assertRaises(ValueError): summarize(B,{'tickets':P['tickets']+[P['tickets'][0]]},S)
    def test_markdown_discloses_limits(self):
        s=markdown(summarize(B,P,S));self.assertIn('not a percentage',s);self.assertIn('Atomic snapshot: false',s)
if __name__=='__main__':unittest.main()
