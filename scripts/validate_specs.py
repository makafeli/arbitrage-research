#!/usr/bin/env python3
"""Validate the contract fixtures and financial/evidence invariants.

Uses a deliberately limited checker for the schema keywords present here;
not a general JSON Schema or OpenAPI conformance validator.
Requires Python 3.11+ and PyYAML.
"""
from pathlib import Path
import copy,json,re,tomllib,yaml,datetime
root=Path(__file__).resolve().parents[1]
api=yaml.safe_load((root/'specs/openapi.yaml').read_text())
def check(s,x,path='$'):
 def fail(m):raise AssertionError(path+': '+m)
 if '$ref' in s:
  ref=s['$ref']
  if ref.startswith('#/'):
   target=api
   for bit in ref[2:].split('/'):target=target[bit.replace('~1','/').replace('~0','~')]
  else:target=json.loads((root/'specs'/ref).read_text())
  check(target,x,path)
 if 'type' in s:
  ts=s['type'] if isinstance(s['type'],list) else [s['type']]
  matches={'object':isinstance(x,dict),'array':isinstance(x,list),'string':isinstance(x,str),'integer':isinstance(x,int) and not isinstance(x,bool),'number':isinstance(x,(int,float)) and not isinstance(x,bool),'boolean':isinstance(x,bool),'null':x is None}
  if not any(matches[t] for t in ts):fail('wrong type')
 if 'const' in s and x!=s['const']:fail('const')
 if 'enum' in s and x not in s['enum']:fail('enum')
 if isinstance(x,dict):
  for k in s.get('required',[]):
   if k not in x:fail('missing '+k)
  props=s.get('properties',{})
  if s.get('additionalProperties') is False and set(x)-set(props):fail('extra fields')
  for k,v in props.items():
   if k in x:check(v,x[k],path+'.'+k)
 if isinstance(x,list):
  if len(x)<s.get('minItems',0) or len(x)>s.get('maxItems',float('inf')):fail('array bounds')
  for i,y in enumerate(x):check(s.get('items',{}),y,path+f'[{i}]')
 if isinstance(x,str):
  if len(x)<s.get('minLength',0) or len(x)>s.get('maxLength',float('inf')):fail('string bounds')
  if 'pattern' in s and re.search(s['pattern'],x) is None:fail('pattern')
  if s.get('format')=='date-time':datetime.datetime.fromisoformat(x.replace('Z','+00:00'))
 if isinstance(x,(int,float)) and not isinstance(x,bool):
  if x<s.get('minimum',float('-inf')) or x>s.get('maximum',float('inf')):fail('numeric bounds')
 for child in s.get('allOf',[]):check(child,x,path)
 if 'if' in s:
  try:check(s['if'],x,path); valid=True
  except AssertionError:valid=False
  if valid and 'then' in s:check(s['then'],x,path)
  if not valid and 'else' in s:check(s['else'],x,path)

def refs(x):
 if isinstance(x,dict):
  if '$ref' in x:
   r=x['$ref']
   if r.startswith('#/'):
    y=api
    for k in r[2:].split('/'):y=y[k.replace('~1','/').replace('~0','~')]
   else:assert (root/'specs'/r).is_file(),r
  for v in x.values():refs(v)
 elif isinstance(x,list):
  for v in x:refs(v)
refs(api)
assert api['openapi']=='3.1.0'
ops=[]
for path,entry in api['paths'].items():
 for method,op in entry.items():
  if method in ['get','post','put','patch','delete']:
   ops.append(op['operationId']); assert 'responses' in op
assert len(ops)==len(set(ops))
s=json.loads((root/'specs/opportunity.schema.json').read_text()); e=json.loads((root/'specs/opportunity.example.json').read_text()); check(s,e)
cmd=json.loads((root/'specs/command.example.json').read_text())
check(api['components']['schemas']['CommandRequest'],cmd['request'])
for k in ['accepted_receipt','applied_receipt']:check(api['components']['schemas']['CommandReceipt'],cmd[k])
valid_simulated=copy.deepcopy(e)
valid_simulated.update(evidence_label='SIMULATED',simulation_status='PASSED',execution_plan_digest='fixture-plan')
valid_simulated['eligibility_checks']['atomic_route_supported']=True
valid_simulated['eligibility_checks']['simulation_matches_exact_plan']=True
check(s,valid_simulated)
valid_estimated=copy.deepcopy(valid_simulated)
valid_estimated.update(evidence_label='ESTIMATED_EXECUTABLE',inclusion_scenario_id='fixture-scenario')
valid_estimated['eligibility_checks']={k:True for k in valid_estimated['eligibility_checks']}
check(s,valid_estimated)
negatives=[]
for label,mutation in [
 ('paper-realized',lambda x:x.update(evidence_label='REALIZED',transaction_id='fakehash',finality_status='FINALIZED')),
 ('numeric-token-amount',lambda x:x.update(amount_in_minor=100)),
 ('negative-input',lambda x:x.update(amount_in_minor='-1')),
 ('simulation-without-plan',lambda x:x.update(evidence_label='SIMULATED',simulation_status='PASSED')),
 ('estimated-without-checks',lambda x:x.update(evidence_label='ESTIMATED_EXECUTABLE',simulation_status='PASSED',execution_plan_digest='fixture',inclusion_scenario_id='fixture')),
 ('one-leg-route',lambda x:x.update(route=x['route'][:1])),
 ('unknown-network',lambda x:x.update(network_id='ethereum-mainnet')),
 ('duplicate-unexpected-field',lambda x:x.update(private_key='REJECTED_FIXTURE_FIELD'))]:
 bad=copy.deepcopy(e); mutation(bad)
 try:check(s,bad)
 except AssertionError:negatives.append(label)
 else:raise AssertionError('negative example accepted: '+label)
for label,bad in [
 ('negative-native-cost',copy.deepcopy(e)),
 ('simulated-without-atomic-route',copy.deepcopy(valid_simulated)),
 ('simulated-without-matching-plan',copy.deepcopy(valid_simulated)),
 ('estimated-with-inconsistent-snapshot',copy.deepcopy(valid_estimated))]:
 if label=='negative-native-cost':bad['costs'][0]['native_amount']['amount_minor']='-5'
 elif label=='simulated-without-atomic-route':bad['eligibility_checks']['atomic_route_supported']=False
 elif label=='simulated-without-matching-plan':bad['eligibility_checks']['simulation_matches_exact_plan']=False
 else:bad['snapshot']['consistent']=False
 try:check(s,bad)
 except AssertionError:negatives.append(label)
 else:raise AssertionError('negative example accepted: '+label)
assert e['route'][0]['asset_in']==e['start_asset_id']
for first,second in zip(e['route'],e['route'][1:]):assert first['asset_out']==second['asset_in']
assert e['route'][-1]['asset_out']==e['start_asset_id']
assert len({l['pool_id'] for l in e['route']})==len(e['route'])
assert int(e['quoted_output_minor'])-int(e['amount_in_minor'])-sum(int(c['in_start_asset_minor']) for c in e['costs'])==int(e['net_after_explicit_costs_minor'])
big=str(2**255+123); assert json.loads(json.dumps({'amount':big}))['amount']==big
cfg=tomllib.loads((root/'config/research.example.toml').read_text()); assert cfg['deployment']['mode']=='PAPER'; assert cfg['execution']['broadcast_enabled'] is False; assert cfg['execution']['signer_enabled'] is False; assert all(not n['enabled'] and not n['verified_pool_ids'] for n in cfg['networks'].values())
report={'api_operations':len(ops),'api_local_refs':'resolved','opportunity_example':'passed','command_examples':'passed','negative_cases_rejected':negatives,'financial_example_arithmetic':'passed','large_integer_json':'passed','inert_toml':'passed','validation_scope':'Structural parsing and custom checks of the schema keywords used; not a complete OpenAPI or JSON Schema conformance certification.'}
print(json.dumps(report,indent=2))
