#!/usr/bin/env python3
"""Validate the contract fixtures and financial/evidence invariants.

Uses a deliberately limited checker for the schema keywords present here;
not a general JSON Schema or OpenAPI conformance validator.
Requires Python 3.11+ and PyYAML.
"""
from pathlib import Path
import copy,json,re,tomllib,yaml,datetime,hashlib
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
  if s.get('uniqueItems') and len({json.dumps(v,sort_keys=True) for v in x})!=len(x):fail('duplicate array values')
  for i,y in enumerate(x):check(s.get('items',{}),y,path+f'[{i}]')
  if 'contains' in s:
   matches=0
   for i,y in enumerate(x):
    try:check(s['contains'],y,path+f'[{i}]')
    except AssertionError:pass
    else:matches+=1
   if matches<s.get('minContains',1) or matches>s.get('maxContains',float('inf')):fail('contains')
 if isinstance(x,str):
  if len(x)<s.get('minLength',0) or len(x)>s.get('maxLength',float('inf')):fail('string bounds')
  if 'pattern' in s and re.search(s['pattern'],x) is None:fail('pattern')
  if s.get('format')=='date-time':datetime.datetime.fromisoformat(x.replace('Z','+00:00'))
 if isinstance(x,(int,float)) and not isinstance(x,bool):
  if x<s.get('minimum',float('-inf')) or x>s.get('maximum',float('inf')):fail('numeric bounds')
 for child in s.get('allOf',[]):check(child,x,path)
 if 'not' in s:
  try:check(s['not'],x,path)
  except AssertionError:pass
  else:fail('not')
 for keyword in ('oneOf','anyOf'):
  if keyword in s:
   matches=0
   for child in s[keyword]:
    try:check(child,x,path)
    except AssertionError:pass
    else:matches+=1
   if (keyword=='oneOf' and matches!=1) or (keyword=='anyOf' and matches==0):fail(keyword)
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
 ('legacy-provided-origin-source-mismatch',lambda x:x.update(dataset_origin='RECORDED_LIVE')),
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
new_example=json.loads((root/'specs/opportunity.v1.1.example.json').read_text())
check(s,new_example)
for label,mutation in [
 ('v1.1-missing-origin',lambda x:x.pop('dataset_origin')),
 ('v1.1-synthetic-claimed-as-market',lambda x:x.update(dataset_origin='SYNTHETIC',source_kind='CAPTURED_MARKET_DATA')),
 ('v1.1-manual-claimed-as-market',lambda x:x.update(dataset_origin='MANUALLY_CONSTRUCTED',source_kind='CAPTURED_MARKET_DATA')),
 ('v1.1-recorded-claimed-as-synthetic',lambda x:x.update(dataset_origin='RECORDED_LIVE',source_kind='SYNTHETIC_FIXTURE')),
 ('v1.1-unknown-cost-numeric-net',lambda x:x.update(net_after_explicit_costs_minor='0')),
 ('v1.1-complete-cost-null-net',lambda x:x['eligibility_checks'].update(costs_complete=True)),
 ('v1.1-estimated-null-net',lambda x:x.update(evidence_label='ESTIMATED_EXECUTABLE'))]:
 bad=copy.deepcopy(new_example);mutation(bad)
 try:check(s,bad)
 except AssertionError:negatives.append(label)
 else:raise AssertionError('negative example accepted: '+label)
valid_estimated_v11=copy.deepcopy(valid_estimated)
valid_estimated_v11.update(schema_version='1.1.0',dataset_origin='SYNTHETIC')
check(s,valid_estimated_v11)
valid_estimated_v11['net_after_explicit_costs_minor']=None
try:check(s,valid_estimated_v11)
except AssertionError:negatives.append('v1.1-fully-qualified-estimated-null-net')
else:raise AssertionError('fully qualified estimated example accepted null net')
collection=json.loads((root/'specs/collection.example.json').read_text())
export=json.loads((root/'specs/research-export.example.json').read_text())
for name,value in [('CollectionAttempt',collection['attempt']),('CollectionCoverage',collection['coverage']),('ResearchExport',export)]:check(api['components']['schemas'][name],value)
canonical=json.dumps({k:export[k] for k in ['snapshot','methodology','data']},sort_keys=True,separators=(',',':'),ensure_ascii=False).encode('utf-8')
assert export['content_sha256']=='sha256:'+hashlib.sha256(canonical).hexdigest()
cost=json.loads((root/'specs/cost-assessment.example.json').read_text())
for name,value in [('StoredDecisionTrace',cost['source_decision']),('NewCostAssessment',cost['request']),('StoredCostAssessment',cost['record'])]:check(api['components']['schemas'][name],value)
def canonical_digest(value):
 return 'sha256:'+hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode('utf-8')).hexdigest()
assessment=cost['record']['assessment']; trace=cost['source_decision']['trace']
assert trace['observation_id']==canonical_digest({k:v for k,v in trace.items() if k!='observation_id'})
assert assessment['binding']['decision_digest']==canonical_digest(trace)
assert assessment['scenario_digest']==canonical_digest(cost['request']['scenario'])
assert assessment['assessment_id']==canonical_digest(dict(assessment,assessment_id=''))
assert assessment['binding']['observation_id']==cost['request']['observation_id']==trace['observation_id']
converted=[]
for entry in assessment['report']['expenses']:
 native=entry['expense']['amount']; valuation=native['valuation']; amount=int(native['amount'])
 value=amount if valuation['kind']=='SAME_ASSET' else (amount*int(valuation['numerator'])+int(valuation['denominator'])-1)//int(valuation['denominator'])
 assert str(value)==entry['in_start_asset']; converted.append(value)
gross=int(trace['result']['quoted_output_minor'])-int(trace['amount_in_minor'])
assert str(gross)==assessment['report']['gross_after_quote_included_costs']
assert str(gross-sum(converted))==assessment['report']['transaction_net']=='-2001'
assert str(gross-sum(converted)-int(assessment['scenario']['overhead']['amount_in_start_asset']))==assessment['report']['fully_allocated_net']=='-2501'
assert assessment['evidence']=='CANDIDATE' and not assessment['report']['incomplete_reasons']
for name,label,source,mutate in [
 ('NewCostAssessment','cost-client-quote-override',cost['request'],lambda x:x.update(quoted_output_minor='999999')),
 ('NewCostAssessment','cost-numeric-amount',cost['request'],lambda x:x['scenario']['expenses'][0]['amount'].update(amount=100)),
 ('NewCostAssessment','cost-provider-url-reference',cost['request'],lambda x:x['scenario'].update(provenance_reference='https://credential.invalid/token')),
 ('NewCostAssessment','cost-malformed-sha-reference',cost['request'],lambda x:x['scenario'].update(provenance_reference='sha256:abc')),
 ('NewCostAssessment','cost-false-recorded-origin',cost['request'],lambda x:x['scenario'].update(origin='RECORDED_LIVE')),
 ('NewCostAssessment','cost-zero-conversion-denominator',cost['request'],lambda x:x['scenario']['expenses'][0]['amount']['valuation'].update(denominator='0')),
 ('NewCostAssessment','cost-token-labelled-native-fee',cost['request'],lambda x:x['scenario']['expenses'][0]['amount'].update(asset={'kind':'TOKEN','identity':assessment['binding']['starting_asset']})),
 ('NewCostAssessment','cost-unbounded-valuation-age',cost['request'],lambda x:x['scenario'].update(valuation_max_age_ms=86400001)),
 ('StoredCostAssessment','cost-false-simulation-evidence',cost['record'],lambda x:x['assessment'].update(evidence='SIMULATED')),
 ('StoredCostAssessment','cost-unexpected-secret-field',cost['record'],lambda x:x['assessment'].update(private_key='REJECTED_FIXTURE_FIELD')),
 ('ResearchExport','export-missing-cost-dataset',export,lambda x:x['data'].pop('cost_assessments')),
 ('ResearchExport','export-missing-cost-count',export,lambda x:x['snapshot']['source_counts'].pop('cost_assessments')),
]:
 bad=copy.deepcopy(source);mutate(bad)
 try:check(api['components']['schemas'][name],bad)
 except AssertionError:negatives.append(label)
 else:raise AssertionError('negative example accepted: '+label)
for name,label,source,mutate in [
 ('CollectionAttempt','collection-numeric-generation',collection['attempt'],lambda x:x.update(generation=1)),
 ('CollectionAttempt','collection-in-progress-success-count',collection['attempt'],lambda x:x.update(decision_rows='1')),
 ('CollectionAttempt','collection-error-raw-provider-text',collection['attempt'],lambda x:x.update(reason='https://credential.invalid/token')),
 ('CollectionAttempt','collection-terminal-without-evidence',collection['attempt'],lambda x:x.update(outcome='DECISIONS_RECORDED')),
 ('CollectionCoverage','collection-false-completeness',collection['coverage'],lambda x:x.update(collection_completeness='COMPLETE')),
 ('CollectionCoverage','collection-numeric-count',collection['coverage'],lambda x:x.update(attempts_started=1)),
 ('ResearchExport','export-false-execution',export,lambda x:x['methodology'].update(execution_authorized=True)),
 ('ResearchExport','export-numeric-source-count',export,lambda x:x['snapshot']['source_counts'].update(decisions=0)),
 ('ResearchExport','export-unknown-secret-field',export,lambda x:x['data'].update(provider_api_key='REJECTED_FIXTURE_FIELD')),
 ('ResearchExport','export-raw-artifact-overclaim',export,lambda x:x['methodology'].update(raw_artifacts='ALL_VERIFIED')),
]:
 bad=copy.deepcopy(source);mutate(bad)
 try:check(api['components']['schemas'][name],bad)
 except AssertionError:negatives.append(label)
 else:raise AssertionError('negative example accepted: '+label)
freshness=json.loads((root/'specs/chain-freshness.example.json').read_text())
def check_freshness_semantics(trace):
 """Independent fixture checks beyond static JSON Schema; not the domain validator.

 Hashes are checked separately so semantic negative cases cannot pass merely
 because their original content digest became stale after mutation.
 """
 report=trace['chain_freshness'];now=report['reference_observed_at_unix_ms']+report['evaluation_elapsed_ms']
 assert report['reference_observed_at_unix_ms']==trace['observed_at_unix_ms'],'reference clock binding'
 assert report['evaluation_elapsed_ms']==trace['input_age_ms'],'elapsed clock binding'
 assert 0<now<=253402300799999,'combined clock range'
 captures=[r['capture_id'] for r in trace['capture_refs']]
 assert len(captures)==len(set(captures)),'distinct capture identities'
 assert [s['capture_id'] for s in report['sources']]==captures,'ordered capture binding'
 assert all(r['manifest_digest']==r['snapshot_id'] for r in trace['capture_refs']),'snapshot binding'
 for source in report['sources']:
  context=source['source']
  kind='BASE_FINALIZED_BLOCK_TIMESTAMP' if trace['network_id']=='base-mainnet' else 'SOLANA_ESTIMATED_BLOCK_TIME'
  assert context['kind']==kind,'source network binding'
  height=context['block_number'] if kind=='BASE_FINALIZED_BLOCK_TIMESTAMP' else context['slot']
  assert str(int(height))==height and 0<=int(height)<=2**64-1,'source height exact u64'
  if kind=='SOLANA_ESTIMATED_BLOCK_TIME':
   alphabet='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
   encoded=context['genesis_hash'];value=0
   for char in encoded:value=value*58+alphabet.index(char)
   decoded=b'\0'*(len(encoded)-len(encoded.lstrip('1')))+value.to_bytes((value.bit_length()+7)//8,'big')
   assert len(decoded)==32 and any(decoded),'canonical nonzero 32-byte genesis identity'
  stamp=source['chain_time_seconds'];age=None if stamp is None or stamp*1000>now else now-stamp*1000
  status='UNKNOWN' if stamp is None else 'FUTURE' if age is None else 'STALE' if age>report['policy']['max_chain_age_ms'] else 'WITHIN_POLICY'
  assert source['age_ms']==age,'source age arithmetic'
  assert source['status']==status,'source status arithmetic'
 aggregate=next((status for status in ['FUTURE','UNKNOWN','STALE'] if any(s['status']==status for s in report['sources'])),'WITHIN_POLICY' if report['sources'] else 'UNKNOWN')
 assert report['status']==aggregate,'aggregate status'
for case in freshness['cases']:
 record=case['record'];check(api['components']['schemas']['StoredDecisionTrace'],record)
 trace=record['trace'];check_freshness_semantics(trace)
 assert trace['observation_id']==canonical_digest({k:v for k,v in trace.items() if k!='observation_id'})==record['trace_id']
support=json.loads((root/'specs/adapter-support.example.json').read_text())
check(api['components']['schemas']['AdapterSupportCatalog'],support)
fresh=freshness['cases'][0]['record']
for name,label,source,mutate in [
 ('StoredDecisionTrace','freshness-legacy-schema-with-report',fresh,lambda x:x['trace'].update(schema_version='1.0.0')),
 ('StoredDecisionTrace','freshness-report-removed',fresh,lambda x:x['trace'].pop('chain_freshness')),
 ('StoredDecisionTrace','freshness-report-null',fresh,lambda x:x['trace'].update(chain_freshness=None)),
 ('StoredDecisionTrace','freshness-zero-policy-limit',fresh,lambda x:x['trace']['chain_freshness']['policy'].update(max_chain_age_ms=0)),
 ('StoredDecisionTrace','freshness-unsupported-policy',fresh,lambda x:x['trace']['chain_freshness']['policy'].update(version='future-v999')),
 ('StoredDecisionTrace','freshness-numeric-block-height',fresh,lambda x:x['trace']['chain_freshness']['sources'][0]['source'].update(block_number=1234)),
 ('StoredDecisionTrace','freshness-floating-timestamp',fresh,lambda x:x['trace']['chain_freshness']['sources'][0].update(chain_time_seconds=0.5)),
 ('StoredDecisionTrace','freshness-future-has-fabricated-age',freshness['cases'][2]['record'],lambda x:x['trace']['chain_freshness']['sources'][0].update(age_ms=0)),
 ('StoredDecisionTrace','freshness-unknown-has-fabricated-time',freshness['cases'][3]['record'],lambda x:x['trace']['chain_freshness']['sources'][0].update(chain_time_seconds=0)),
 ('StoredDecisionTrace','freshness-quote-claims-stale',fresh,lambda x:x['trace']['chain_freshness'].update(status='STALE')),
 ('AdapterSupportCatalog','support-submit-promoted',support,lambda x:x['configurations'][0]['networks'][0]['capability'].update(submit=True)),
 ('AdapterSupportCatalog','support-quote-qualified',support,lambda x:x['configurations'][0]['networks'][0]['capability'].update(qualified_quote=True)),
 ('AdapterSupportCatalog','support-unloaded-authorized',support,lambda x:x['configurations'][0]['networks'][0]['registry'].update(status='LOADED_AUTHORIZED')),
 ('AdapterSupportCatalog','support-policy-missing',support,lambda x:x['configurations'][0]['networks'][0]['chain_freshness'].update(status='CONFIGURED')),
 ('AdapterSupportCatalog','support-private-path-exposed',support,lambda x:x['configurations'][0]['networks'][0]['registry'].update(path='/private/registry.json')),
 ('AdapterSupportCatalog','support-unexpanded-without-explanation',support,lambda x:x['configurations'][0]['networks'][0]['declared_scope'].update(identities_expanded=False)),
]:
 bad=copy.deepcopy(source);mutate(bad)
 try:check(api['components']['schemas'][name],bad)
 except AssertionError:negatives.append(label)
 else:raise AssertionError('negative example accepted: '+label)
optin=tomllib.loads((root/'config/chain-freshness.example.toml').read_text())
assert all(not n['enabled'] and not n['verified_pool_ids'] for n in optin['networks'].values())
for network in optin['networks'].values():check(api['components']['schemas']['ChainFreshnessPolicy'],network['chain_freshness'])

# Contract regressions exercise the published schema itself, separately from the
# semantic checks above. These modified fixtures do not claim canonical hashes.
freshness_positive_cases=[]
freshness_semantic_negatives=[]
def trace_contract_case(label,source,mutate,expected):
 trace=copy.deepcopy(source);mutate(trace)
 try:check(api['components']['schemas']['DecisionTrace'],trace)
 except AssertionError:
  if expected=='structural-rejection':negatives.append(label);return
  raise
 if expected=='structural-rejection':raise AssertionError('negative trace accepted: '+label)
 if expected=='semantic-rejection':
  try:check_freshness_semantics(trace)
  except AssertionError:freshness_semantic_negatives.append(label);return
  raise AssertionError('semantic negative trace accepted: '+label)
 if 'chain_freshness' in trace:check_freshness_semantics(trace)
 freshness_positive_cases.append(label)

fresh_trace=fresh['trace']
legacy_trace=cost['source_decision']['trace']
def set_result(trace,status,reasons=None):
 if status=='QUOTED':trace['result']=copy.deepcopy(fresh_trace['result'])
 else:trace['result']={'status':status,'reason_codes':reasons or ['NO_ELIGIBLE_POOL_PAIRS']}
 if status in ('QUOTED','REJECTED'):
  trace['route']=copy.deepcopy(fresh_trace['route']);trace['amount_in_minor']=fresh_trace['amount_in_minor']
 else:trace['route']=[];trace['amount_in_minor']=None

chain_reason={'UNKNOWN':'CHAIN_TIME_UNAVAILABLE','FUTURE':'CHAIN_TIME_FUTURE','STALE':'CHAIN_TIME_STALE'}
for source_label,source in [('legacy',legacy_trace)]+[(c['name'],c['record']['trace']) for c in freshness['cases']]:
 status=source.get('chain_freshness',{}).get('status','WITHIN_POLICY')
 expected_reason=chain_reason.get(status)
 for result_status in ['REJECTED','NO_ROUTE','DATA_UNAVAILABLE']+(['QUOTED'] if not expected_reason else []):
  def mutation(trace):
   set_result(trace,result_status,[expected_reason] if expected_reason else ['CHAIN_TIME_INVALID'])
   trace['diagnostics'].append(expected_reason or 'CHAIN_TIME_INVALID')
  trace_contract_case(f'freshness-compatible-{source_label}-{result_status.lower()}',source,mutation,'accept')
 for origin,kind in [('RECORDED_LIVE','CAPTURED_MARKET_DATA'),('SYNTHETIC','SYNTHETIC_FIXTURE'),('MANUALLY_CONSTRUCTED','SYNTHETIC_FIXTURE')]:
  trace_contract_case(f'freshness-origin-compatible-{source_label}-{origin.lower()}',source,lambda x:x.update(dataset_origin=origin,source_kind=kind),'accept')
 for code in chain_reason.values():
  if code==expected_reason:continue
  trace_contract_case(f'freshness-{source_label}-contradictory-diagnostic-{code}',source,lambda x:x['diagnostics'].append(code),'structural-rejection')
  def mutation(trace):
   set_result(trace,'DATA_UNAVAILABLE',[expected_reason,code] if expected_reason else [code])
  trace_contract_case(f'freshness-{source_label}-contradictory-result-{code}',source,mutation,'structural-rejection')
 if expected_reason:
  for result_status in ['REJECTED','NO_ROUTE','DATA_UNAVAILABLE']:
   trace_contract_case(f'freshness-{source_label}-{result_status.lower()}-missing-matching-reason',source,lambda x:set_result(x,result_status,['CAPTURE_UNAVAILABLE']),'structural-rejection')

for result_status in ['QUOTED','REJECTED']:
 source=copy.deepcopy(fresh_trace);set_result(source,result_status)
 for suffix,mutate in [
  ('missing-route',lambda x:x.update(route=[])),
  ('single-leg',lambda x:x.update(route=x['route'][:1])),
  ('missing-captures',lambda x:x.update(capture_refs=[])),
  ('single-capture',lambda x:x.update(capture_refs=x['capture_refs'][:1])),
  ('missing-input',lambda x:x.update(amount_in_minor=None)),
  ('zero-input',lambda x:x.update(amount_in_minor='0')),
  ('missing-age',lambda x:x.update(input_age_ms=None)),
 ]:trace_contract_case(f'freshness-{result_status.lower()}-{suffix}',source,mutate,'structural-rejection')
for result_status in ['NO_ROUTE','DATA_UNAVAILABLE']:
 source=copy.deepcopy(fresh_trace);set_result(source,result_status)
 for suffix,mutate in [
  ('fabricated-route',lambda x:x.update(route=copy.deepcopy(fresh_trace['route']))),
  ('fabricated-input',lambda x:x.update(amount_in_minor='1')),
  ('missing-age',lambda x:x.update(input_age_ms=None)),
 ]:trace_contract_case(f'freshness-{result_status.lower()}-{suffix}',source,mutate,'structural-rejection')
 if result_status=='NO_ROUTE':
  trace_contract_case('freshness-no-route-without-capture',source,lambda x:x.update(capture_refs=[]),'structural-rejection')

for suffix,mutate in [
 ('recorded-origin-labelled-fixture',lambda x:x.update(dataset_origin='RECORDED_LIVE')),
 ('synthetic-origin-labelled-market',lambda x:x.update(dataset_origin='SYNTHETIC',source_kind='CAPTURED_MARKET_DATA')),
 ('manual-origin-labelled-market',lambda x:x.update(source_kind='CAPTURED_MARKET_DATA')),
 ('sources-shorter-than-captures',lambda x:x['chain_freshness']['sources'].pop()),
 ('base-source-in-solana-trace',lambda x:x.update(network_id='solana-mainnet')),
 ('capture-control-character',lambda x:x['capture_refs'][0].update(capture_id='capture\nsecret')),
 ('source-control-character',lambda x:x['chain_freshness']['sources'][0].update(capture_id='capture\u0085secret')),
]:trace_contract_case('freshness-'+suffix,fresh_trace,mutate,'structural-rejection')
trace_contract_case('legacy-using-freshness-calculation-version',legacy_trace,lambda x:x.update(calculation_version=fresh_trace['calculation_version']),'structural-rejection')

def resize_captures(trace,count):
 capture=copy.deepcopy(fresh_trace['capture_refs'][0]);source=copy.deepcopy(fresh_trace['chain_freshness']['sources'][0])
 trace['capture_refs']=[dict(capture,capture_id=f'capture-{i}') for i in range(count)]
 trace['chain_freshness']['sources']=[dict(source,capture_id=f'capture-{i}') for i in range(count)]
 set_result(trace,'DATA_UNAVAILABLE',['NO_ELIGIBLE_POOL_PAIRS'] if count else ['CHAIN_TIME_UNAVAILABLE'])
 trace['chain_freshness']['status']='WITHIN_POLICY' if count else 'UNKNOWN'
for count in range(10):
 trace_contract_case(f'freshness-capture-source-count-{count}',fresh_trace,lambda x:resize_captures(x,count),'accept' if count<=8 else 'structural-rejection')
for count in [0,1,8,9,64,65]:
 def mutation(trace):
  trace['capture_refs']=[dict(legacy_trace['capture_refs'][0],capture_id=f'legacy-{i}') for i in range(count)]
  set_result(trace,'DATA_UNAVAILABLE')
 trace_contract_case(f'legacy-capture-count-{count}',legacy_trace,mutation,'accept' if count<=64 else 'structural-rejection')
for status in ['FUTURE','STALE','WITHIN_POLICY']:
 source=copy.deepcopy(fresh_trace);resize_captures(source,0)
 def mutation(trace):
  trace['chain_freshness']['status']=status
  set_result(trace,'DATA_UNAVAILABLE',[chain_reason.get(status,'NO_ELIGIBLE_POOL_PAIRS')])
 trace_contract_case('freshness-empty-source-'+status.lower(),source,mutation,'structural-rejection')
for case in freshness['cases']:
 source=case['record']['trace'];current=source['chain_freshness']['status']
 for incorrect in ['FUTURE','UNKNOWN','STALE','WITHIN_POLICY']:
  if incorrect==current:continue
  def mutation(trace):
   trace['chain_freshness']['status']=incorrect
   set_result(trace,'DATA_UNAVAILABLE',[chain_reason.get(incorrect,'NO_ELIGIBLE_POOL_PAIRS')])
  trace_contract_case(f'freshness-aggregate-{current.lower()}-as-{incorrect.lower()}',source,mutation,'structural-rejection')

source_by_status={c['record']['trace']['chain_freshness']['status']:c['record']['trace']['chain_freshness']['sources'][0] for c in freshness['cases']}
for first in source_by_status:
 for second in source_by_status:
  aggregate=next((v for v in ['FUTURE','UNKNOWN','STALE'] if v in (first,second)),'WITHIN_POLICY')
  mixed=copy.deepcopy(fresh_trace)
  mixed['chain_freshness']['sources']=[dict(copy.deepcopy(source_by_status[v]),capture_id=fresh_trace['capture_refs'][i]['capture_id']) for i,v in enumerate([first,second])]
  mixed['chain_freshness']['status']=aggregate
  set_result(mixed,'DATA_UNAVAILABLE',[chain_reason.get(aggregate,'NO_ELIGIBLE_POOL_PAIRS')])
  trace_contract_case(f'freshness-mixed-{first.lower()}-{second.lower()}',mixed,lambda x:None,'accept')
  for incorrect in source_by_status:
   if incorrect==aggregate:continue
   def mutation(trace):
    trace['chain_freshness']['status']=incorrect
    set_result(trace,'DATA_UNAVAILABLE',[chain_reason.get(incorrect,'NO_ELIGIBLE_POOL_PAIRS')])
   trace_contract_case(f'freshness-mixed-{first.lower()}-{second.lower()}-as-{incorrect.lower()}',mixed,mutation,'structural-rejection')

solana_trace=copy.deepcopy(fresh_trace);solana_trace['network_id']='solana-mainnet'
set_result(solana_trace,'DATA_UNAVAILABLE')
for source in solana_trace['chain_freshness']['sources']:
 source['source']={'kind':'SOLANA_ESTIMATED_BLOCK_TIME','slot':'1234567','genesis_hash':'YMN9Qj5jPNp7j14VPcML1B6xGgcPWVZUGLFU3Mnyfaf','account_context':'fixture-finalized-accounts'}
trace_contract_case('freshness-solana-canonical-source',solana_trace,lambda x:None,'accept')
trace_contract_case('freshness-solana-source-in-base-trace',solana_trace,lambda x:x.update(network_id='base-mainnet'),'structural-rejection')
for genesis in ['1'*32,'1'*33,'z'*44]:
 trace_contract_case('freshness-semantic-solana-invalid-genesis-'+genesis,solana_trace,lambda x:x['chain_freshness']['sources'][0]['source'].update(genesis_hash=genesis),'semantic-rejection')
trace_contract_case('freshness-semantic-solana-slot-overflows-u64',solana_trace,lambda x:x['chain_freshness']['sources'][0]['source'].update(slot=str(2**64)),'semantic-rejection')

# Each of these passes the structural schema. The independent semantic oracle
# rejects it for the named relationship, before any digest verification.
for suffix,mutate in [
 ('clock-reference-disagrees',lambda x:x['chain_freshness'].update(reference_observed_at_unix_ms=x['observed_at_unix_ms']+1)),
 ('clock-elapsed-disagrees',lambda x:x['chain_freshness'].update(evaluation_elapsed_ms=x['input_age_ms']+1)),
 ('capture-order-disagrees',lambda x:x['chain_freshness']['sources'].reverse()),
 ('capture-identity-disagrees',lambda x:x['chain_freshness']['sources'][0].update(capture_id='other-capture')),
 ('snapshot-digest-disagrees',lambda x:x['capture_refs'][0].update(snapshot_id='sha256:'+'f'*64)),
 ('source-age-disagrees',lambda x:x['chain_freshness']['sources'][0].update(age_ms=0)),
 ('source-status-arithmetic-disagrees',lambda x:x['chain_freshness']['sources'][0].update(chain_time_seconds=0)),
 ('source-height-overflows-u64',lambda x:x['chain_freshness']['sources'][0]['source'].update(block_number=str(2**64))),
]:trace_contract_case('freshness-semantic-'+suffix,fresh_trace,mutate,'semantic-rejection')
def duplicate_identity(trace):
 trace['capture_refs'][1]['capture_id']=trace['capture_refs'][0]['capture_id']
 trace['chain_freshness']['sources'][1]['capture_id']=trace['chain_freshness']['sources'][0]['capture_id']
trace_contract_case('freshness-semantic-duplicate-capture-identity',fresh_trace,duplicate_identity,'semantic-rejection')
def overflowing_clock(trace):
 trace['observed_at_unix_ms']=253402300799999
 trace['chain_freshness']['reference_observed_at_unix_ms']=trace['observed_at_unix_ms']
trace_contract_case('freshness-semantic-combined-clock-overflow',fresh_trace,overflowing_clock,'semantic-rejection')

assert e['route'][0]['asset_in']==e['start_asset_id']
for first,second in zip(e['route'],e['route'][1:]):assert first['asset_out']==second['asset_in']
assert e['route'][-1]['asset_out']==e['start_asset_id']
assert len({l['pool_id'] for l in e['route']})==len(e['route'])
assert int(e['quoted_output_minor'])-int(e['amount_in_minor'])-sum(int(c['in_start_asset_minor']) for c in e['costs'])==int(e['net_after_explicit_costs_minor'])
big=str(2**255+123); assert json.loads(json.dumps({'amount':big}))['amount']==big
cfg=tomllib.loads((root/'config/research.example.toml').read_text()); assert cfg['deployment']['mode']=='PAPER'; assert cfg['execution']['broadcast_enabled'] is False; assert cfg['execution']['signer_enabled'] is False; assert all(not n['enabled'] and not n['verified_pool_ids'] for n in cfg['networks'].values())
report={'api_operations':len(ops),'api_local_refs':'resolved','opportunity_example':'passed','command_examples':'passed','negative_cases_rejected':negatives,'freshness_positive_cases':freshness_positive_cases,'freshness_semantic_negative_cases_rejected':freshness_semantic_negatives,'financial_example_arithmetic':'passed','large_integer_json':'passed','inert_toml':'passed','validation_scope':'Structural parsing and custom checks of the schema keywords used; not a complete OpenAPI or JSON Schema conformance certification. Freshness semantic cases separately test clock and capture bindings, source identity bounds and exact ages; those cases pass static schema and must be rejected by application validation.'}
print(json.dumps(report,indent=2))
