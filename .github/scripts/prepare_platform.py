"""Reproduce reviewed platform source and publish immutable objects only."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys

BASE='5e824ba7d57a1df24d6fb65e67590dd4ce822ba4'
PATHS=['apps/control-api/src/lib.rs','apps/control-api/src/tests.rs','crates/arb-config/tests/platform_contract.rs','crates/arb-control/tests/platform_recovery.rs','docs/EPIC-02-ACCEPTANCE.md','.github/workflows/platform-acceptance.yml','planning/implementation-progress.json','GOAL.md']
def git(*args): return subprocess.check_output(['git',*args],text=True).strip()
if sys.argv[1:] == ['publish']:
    def post(endpoint,payload):
        r=subprocess.run(['gh','api','--method','POST','repos/makafeli/arbitrage-research/'+endpoint,'--input','-'],input=json.dumps(payload),text=True,capture_output=True,check=True)
        return json.loads(r.stdout)
    manifest=json.loads((Path(os.environ['RUNNER_TEMP'])/'platform-source/manifest.json').read_text())
    assert git('write-tree')==manifest['tree']
    entries=[]
    for path in PATHS:
        blob=post('git/blobs',{'encoding':'utf-8','content':Path(path).read_text()})['sha']
        assert blob==git('hash-object',path)
        entries.append({'path':path,'mode':'100644','type':'blob','sha':blob})
    tree=post('git/trees',{'base_tree':'de8277bfc1f6c809dc0a27a8f4458d0defadfb89','tree':entries})['sha']
    assert tree==manifest['tree']
    print(json.dumps({'published_tree':tree,'issue_mutations':False,'ref_mutations':False}))
    raise SystemExit(0)
assert not sys.argv[1:]
assert git('rev-parse','HEAD^')==BASE
p=Path('apps/control-api/src/lib.rs');s=p.read_text()
def edit(old,new):
    global s
    assert s.count(old)==1,(old,s.count(old));s=s.replace(old,new)
edit('const MAX_INFLIGHT_REQUESTS: usize = 64;', '''const MAX_INFLIGHT_REQUESTS: usize = 64;
// Bulk reads use at most half of Store::connect's eight database connections.
// They acquire this sub-budget before a global slot, preserving control capacity.
const MAX_INFLIGHT_READS: usize = 4;
const RESERVED_CONTROL_REQUESTS_PER_MINUTE: u32 = 60;''')
edit('    inflight: Arc<Semaphore>,','    inflight: Arc<Semaphore>,\n    reads: Arc<Semaphore>,')
edit('    requests: u32,','    requests: u32,\n    read_requests: u32,')
edit('            inflight: Arc::new(Semaphore::new(MAX_INFLIGHT_REQUESTS)),','            inflight: Arc::new(Semaphore::new(MAX_INFLIGHT_REQUESTS)),\n            reads: Arc::new(Semaphore::new(MAX_INFLIGHT_READS)),')
old='async fn security(State(state): State<AppState>, mut request: Request, next: Next) -> Response {'
edit(old,'''/// Expensive/dashboard reads share a strict sub-budget. Cheap capability/auth
/// status and command receipts remain available to follow a control action.
/// Unknown read routes are conservative: they also consume the read budget.
fn bulk_read(request: &Request) -> bool {
    matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS)
        && !matches!(request.uri().path(),
            "/healthz" | "/v1/health" | "/v1/capabilities" | "/v1/auth/session" | "/v1/adapter-support")
        && !request.uri().path().starts_with("/v1/commands/")
}

'''+old)
a=s.index('    let result = authorize(&state, &mut request, &id);');b=s.index('    response.headers_mut().insert(',a)
s=s[:a]+'''    let result = authorize(&state, &mut request, &id).and_then(|()| {
        // Try the read sub-budget first: refused reads cannot occupy a global
        // slot while waiting for capacity. Both owned permits release on cancel.
        let read = if bulk_read(&request) {
            Some(state.0.reads.clone().try_acquire_owned().map_err(|_| {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "CAPACITY_EXCEEDED",
                    "Read capacity is exhausted; control capacity is reserved", &id)
            })?)
        } else { None };
        let global = state.0.inflight.clone().try_acquire_owned().map_err(|_| {
            ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "CAPACITY_EXCEEDED",
                "Control request capacity is exhausted; retry later", &id)
        })?;
        Ok((global, read))
    });
    let mut response = match result {
        Ok((_global, _read)) => match tokio::time::timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS), next.run(request),
        ).await {
            Ok(response) => response,
            Err(_) => ApiError::new(StatusCode::GATEWAY_TIMEOUT, "REQUEST_TIMEOUT", "Request completion is uncertain; retry mutations with the same idempotency key and payload", &id).into_response(),
        },
        Err(error) => error.into_response(),
    };
'''+s[b:]
edit('        session.requests = 0;','        session.requests = 0;\n        session.read_requests = 0;')
edit('    session.requests = session.requests.saturating_add(1);\n    if session.requests > SESSION_REQUESTS_PER_MINUTE {','''    let is_read = bulk_read(request);
    if session.requests >= SESSION_REQUESTS_PER_MINUTE
        || (is_read && session.read_requests >= SESSION_REQUESTS_PER_MINUTE - RESERVED_CONTROL_REQUESTS_PER_MINUTE) {''')
edit('    if is_mutation && !matches_header(request.headers(), "x-csrf-token", &session.csrf) {','''    // A denied bulk read does not consume the reserve, even if a client retries
    // aggressively. The overall session limit remains 300 authorized attempts.
    session.requests += 1;
    if is_read { session.read_requests += 1; }
    if is_mutation && !matches_header(request.headers(), "x-csrf-token", &session.csrf) {''')
edit('                requests: 0,','                requests: 0,\n                read_requests: 0,')
p.write_text(s)
p=Path('apps/control-api/src/tests.rs');s=p.read_text()
a=s.index('async fn request_capacity_is_bounded_and_cancellation_releases_owned_permits()');b=s.index('\n#[async_trait]',a)
old=s[a:b];new=old.replace('MAX_INFLIGHT_REQUESTS','MAX_INFLIGHT_READS').replace('"/v1/capabilities"','"/v1/sessions"',1)
s=s[:a]+new+s[b:]
s+='''

#[tokio::test(start_paused = true)]
async fn saturated_dashboard_reads_preserve_stop_and_its_rate_reserve() {
    let store = Arc::new(MockStore { blocked: true, ..MockStore::default() });
    let app = setup_with(store.clone(), true);
    let (cookie, csrf) = authenticate(&app).await;
    let mut blocked = Vec::new();
    for _ in 0..MAX_INFLIGHT_READS {
        let app = app.clone(); let cookie = cookie.clone();
        blocked.push(tokio::spawn(async move {
            call(&app, Method::GET, "/v1/sessions", None, Some(&cookie), None, None, None).await
        }));
    }
    for _ in 0..1000 {
        if store.calls.load(Ordering::SeqCst) == MAX_INFLIGHT_READS { break; }
        tokio::task::yield_now().await;
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), MAX_INFLIGHT_READS);
    let excess = call(&app, Method::GET, "/v1/sessions", None, Some(&cookie), None, None, None).await;
    assert_eq!(excess.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(value(excess).await["code"], "CAPACITY_EXCEEDED");
    // More than the complete per-minute budget of rejected read retries cannot
    // consume the reserved control budget or reach the blocked storage method.
    for _ in 0..SESSION_REQUESTS_PER_MINUTE {
        let response = call(&app, Method::GET, "/v1/sessions", None, Some(&cookie), None, None, None).await;
        assert!(matches!(response.status(), StatusCode::SERVICE_UNAVAILABLE | StatusCode::TOO_MANY_REQUESTS));
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), MAX_INFLIGHT_READS);
    let stop = call(&app, Method::POST,
        &format!("/v1/sessions/{}/commands", record().session_id),
        Some(json!({"action":"STOP", "expected_revision":"0"})),
        Some(&cookie), Some(&csrf), Some(ORIGIN), Some("stop-under-read-load"),
    ).await;
    assert_eq!(stop.status(), StatusCode::ACCEPTED);
    let receipt = value(stop).await;
    assert_eq!(receipt["status"], "PENDING");
    assert_eq!(receipt["fence_effective"], false);
    // The middleware reserves API admission, not a fabricated worker ACK.
    assert_eq!(store.calls.load(Ordering::SeqCst), MAX_INFLIGHT_READS + 1);
    for request in blocked { request.abort(); assert!(request.await.unwrap_err().is_cancelled()); }
}

#[tokio::test]
async fn read_reservation_keeps_the_original_global_bound_and_releases_failed_admission() {
    let store = Arc::new(MockStore::default());
    let state = AppState::with_store(store.clone(), ServerConfig {
        listen_address: "127.0.0.1:8080".parse().unwrap(), public_origin: ORIGIN.into(),
        allow_insecure_loopback: true, operator_secret_hash: hash(SECRET),
        adapter_support: support::AdapterSupport::empty(), configurations: vec![],
    });
    let app = router(state.clone()); let (cookie, _) = authenticate(&app).await;
    let reserved = state.0.inflight.clone().acquire_many_owned(MAX_INFLIGHT_REQUESTS as u32).await.unwrap();
    let response = call(&app, Method::GET, "/v1/sessions", None, Some(&cookie), None, None, None).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(state.0.reads.available_permits(), MAX_INFLIGHT_READS);
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    drop(reserved);
    let response = call(&app, Method::GET, "/v1/sessions", None, Some(&cookie), None, None, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(state.0.inflight.available_permits(), MAX_INFLIGHT_REQUESTS);
    assert_eq!(state.0.reads.available_permits(), MAX_INFLIGHT_READS);
}
'''
p.write_text(s)
p=Path('planning/implementation-progress.json');data=json.loads(p.read_text())
ids={'ARB-009','ARB-010','ARB-011','ARB-012','ARB-014'}
for row in data['tickets']:
    if row['id'] in ids:
        row.setdefault('acceptance_reconciliation_history',[]).append({
            'date':'2026-09-14','previous_remaining_acceptance':row.get('remaining_acceptance',''),
            'previous_external_prerequisite':row.get('external_prerequisite',''),
            'reason':'EPIC-02 original configuration/journal/lifecycle/API/manifest contracts; actual deployment, economic fills and recorded market demonstration remain with their existing operational/simulation/ARB-015 gates.'})
        row['state']='implemented_pending_acceptance'
        row['remaining_acceptance']='Complete original-criterion source review and current-source CI, then accept original predecessors and update native checklist before the existing closeout preflight. No platform task is closed by this publication.'
        row['external_prerequisite']=''
        row['evidence'].extend(['docs/EPIC-02-ACCEPTANCE.md','.github/workflows/platform-acceptance.yml'])
    elif row['id']=='ARB-013':
        row['evidence'].append('apps/control-api/src/lib.rs: bounded bulk-read concurrency/rate sub-budgets reserve control admission without weakening the overall cap. Tests preserve PENDING until worker acknowledgement and release permits on cancellation/refusal. Actual current-source execution remains required.')
p.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
p=Path('GOAL.md');s=p.read_text();key='## Current delivery checkpoint\n';assert s.count(key)==1
s=s.replace(key,key+'''
### EPIC-02 platform acceptance in progress

The original #23/#24/#25/#26/#28 contracts are being reconciled against current
source and service-backed tests, with a real process-loss fixture and a correction
to API read/control admission isolation. [The evidence map](docs/EPIC-02-ACCEPTANCE.md)
separates those contracts from actual stage instrumentation and the still-required
recorded #29 observation demonstration. No new task issue or whole-epic acceptance
is implied. Original M2 adapter/snapshot/route dependencies remain in force.

''');p.write_text(s)
subprocess.run(['git','rm','--cached','--','.github/scripts/prepare_platform.py','.github/workflows/prepare-platform-source.yml'],check=True)
subprocess.run(['git','add','--',*PATHS],check=True)
assert git('write-tree')=='245236884b7797d4c2f7039347611b6963a43b0c',git('write-tree')
subprocess.run(['cargo','fmt','--all'],check=True)
subprocess.run(['git','add','--',*PATHS],check=True)
assert set(git('diff','--cached','--name-only',BASE).splitlines())==set(PATHS)
assert not git('diff','--name-only')
subprocess.run(['git','diff','--cached','--check'],check=True)
subprocess.run(['python3','scripts/validate_project.py'],check=True)
subprocess.run(['python3','scripts/test_delivery.py'],check=True)
out=Path(os.environ['RUNNER_TEMP'])/'platform-source';out.mkdir()
tree=git('write-tree')
subprocess.run(['git','archive','--format=tar.gz','--output='+str(out/'source.tar.gz'),tree],check=True)
(out/'manifest.json').write_text(json.dumps({'base':BASE,'tree':tree,'paths':PATHS,'archive_sha256':hashlib.sha256((out/'source.tar.gz').read_bytes()).hexdigest()},indent=2)+'\n')
print((out/'manifest.json').read_text())
