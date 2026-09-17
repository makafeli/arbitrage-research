"""One-time exact-base source preparation. Removed from the final feature tree."""
from pathlib import Path
import json
import subprocess

BASE = 'b3867784bafbb304305328b9703aa7fb1540f8b3'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD^'], text=True).strip() == BASE

def replace(path, before, after):
    p = Path(path)
    text = p.read_text()
    assert text.count(before) == 1, (path, before[:80])
    p.write_text(text.replace(before, after, 1))

def append(path, text):
    p = Path(path)
    p.write_text(p.read_text().rstrip() + '\n\n' + text.strip() + '\n')

replace('crates/arb-storage/src/lib.rs', 'mod ingestion;', 'mod ingestion;\nmod continuity_read;\npub use continuity_read::*;')
replace('apps/control-api/src/lib.rs', '.route("/v1/decisions/{observation_id}", get(research::get_decision))', '.route("/v1/decisions/{observation_id}", get(research::get_decision))\n        .route("/v1/sessions/{session_id}/decisions/{observation_id}/continuity", get(research::decision_continuity))')
replace('apps/control-api/src/research.rs', 'pub(crate) trait ResearchStore: Send + Sync {', '''pub(crate) trait ResearchStore: Send + Sync {
    async fn decision_continuity(&self, _operator: &str, _session: &str, _observation: &str) -> Result<arb_storage::DecisionContinuity, StoreError> {
        Err(StoreError::CapabilityUnavailable)
    }''')
replace('apps/control-api/src/research.rs', 'impl ResearchStore for Store {', '''impl ResearchStore for Store {
    async fn decision_continuity(&self, operator: &str, session: &str, observation: &str) -> Result<arb_storage::DecisionContinuity, StoreError> {
        Store::decision_continuity(self, operator, session, observation).await
    }''')
append('apps/control-api/src/research.rs', '''pub(super) async fn decision_continuity(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<(String, String)>, PathRejection>,
    query: Result<Query<EmptyQuery>, QueryRejection>,
) -> Result<Json<arb_storage::DecisionContinuity>, ApiError> {
    let Path((session, observation)) = path.map_err(|_| ApiError::invalid(&id))?;
    query.map_err(|_| ApiError::invalid(&id))?;
    for value in [&session, &observation] {
        if value.is_empty() || value.len() > 128 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b)) {
            return Err(ApiError::invalid(&id));
        }
    }
    state.0.store.decision_continuity("operator", &session, &observation).await
        .map(Json).map_err(|error| ApiError::store(error, &id))
}''')
replace('apps/web/src/api/client.ts', "import { parseAdapterSupport } from './support.ts';", "import { parseAdapterSupport } from './support.ts';\nimport { parseDecisionContinuity } from './continuity.ts';")
replace('apps/web/src/api/client.ts', '  async decisionGroups(sessionId: string, cursor?: string, signal?: AbortSignal) {', '''  async decisionContinuity(sessionId: string, observationId: string, signal?: AbortSignal) {
    const value = parseDecisionContinuity(await this.request('/sessions/' + encodeURIComponent(sessionId) + '/decisions/' + encodeURIComponent(observationId) + '/continuity', { signal }));
    assert(value.session_id === sessionId && value.observation_id === observationId, 'Continuity response has the wrong decision scope.');
    return value;
  }
  async decisionGroups(sessionId: string, cursor?: string, signal?: AbortSignal) {''')
replace('apps/web/src/components/DecisionExplorer.tsx', "import { ChainFreshnessEvidence } from './ChainFreshnessEvidence';", "import { ChainFreshnessEvidence } from './ChainFreshnessEvidence';\nimport { DecisionContinuityEvidence } from './DecisionContinuityEvidence';")
replace('apps/web/src/components/DecisionExplorer.tsx', '<DecisionDetail record={detail.data} receivedAt={detail.at} />', '<><DecisionContinuityEvidence key={detail.data.trace_id} api={api} record={detail.data} active={enabled && !disabled && Boolean(observationId)} /><DecisionDetail record={detail.data} receivedAt={detail.at} /></>')
append('crates/arb-storage/tests/capture_continuity.rs', '''#[tokio::test]
async fn diagnostic_read_tracks_halt_without_rewriting_historical_trace() {
    let f = fixture().await;
    capture(&f, "capture").await;
    link(&f, "capture").await.unwrap();
    let original = payload(&f, &["capture"]);
    publish(&f, "read-status", &original).await.unwrap();
    let before = f.store.decision_continuity(&f.operator, &f.session, "read-status").await.unwrap();
    assert_eq!(before.continuity_status, "NO_KNOWN_INVALIDATION");
    assert_eq!(before.capture_count, "1");
    assert_eq!(before.bound_count, "1");
    assert!(!before.authorizes_execution);
    assert_eq!(before.assessment_kind, "CONTINUITY_ONLY");
    assert!(before.invalidation_reasons.is_empty());
    assert!(chrono::DateTime::parse_from_rfc3339(&before.checked_at).is_ok());
    halt(&f).await;
    let after = f.store.decision_continuity(&f.operator, &f.session, "read-status").await.unwrap();
    assert_eq!(after.trace_id, before.trace_id);
    assert_eq!(after.continuity_status, "INVALIDATED");
    assert_eq!(after.invalidation_reasons, vec!["CONTINUITY_LOST"]);
    assert!(!after.authorizes_execution);
    let retained: Value = sqlx::query_scalar("SELECT payload FROM decision_traces WHERE session_id=$1 AND observation_id='read-status'")
        .bind(&f.session).fetch_one(&f.pool).await.unwrap();
    assert_eq!(retained, original);
}

#[tokio::test]
async fn diagnostic_read_does_not_borrow_another_session_or_operator() {
    let f = fixture().await;
    let other = fixture().await;
    capture(&f, "legacy").await;
    publish(&f, "same-observation", &payload(&f, &["legacy"])).await.unwrap();
    let report = f.store.decision_continuity(&f.operator, &f.session, "same-observation").await.unwrap();
    assert_eq!(report.continuity_status, "UNTRACKED");
    assert_eq!(report.bound_count, "0");
    for (operator, session, observation) in [
        (&other.operator, &f.session, "same-observation"),
        (&f.operator, &other.session, "same-observation"),
        (&f.operator, &f.session, "missing"),
    ] {
        assert!(matches!(f.store.decision_continuity(operator, session, observation).await, Err(arb_storage::StoreError::NotFound)));
    }
    assert!(f.store.decision_continuity(&f.operator, &f.session, &"x".repeat(129)).await.is_err());
}

#[tokio::test]
async fn zero_input_diagnostics_do_not_invent_a_healthy_source() {
    let f = fixture().await;
    let mut diagnostic = payload(&f, &[]);
    diagnostic["result"]["status"] = json!("DATA_UNAVAILABLE");
    publish(&f, "zero-input", &diagnostic).await.unwrap();
    let report = f.store.decision_continuity(&f.operator, &f.session, "zero-input").await.unwrap();
    assert_eq!(report.capture_count, "0");
    assert_eq!(report.bound_count, "0");
    assert_eq!(report.continuity_status, "UNTRACKED");
    assert!(report.invalidation_reasons.is_empty());
    assert!(!report.authorizes_execution);
}''')
api_tests = Path('apps/control-api/src/tests.rs')
text = api_tests.read_text()
mock_marker = next(x for x in ['impl ResearchStore for MockStore {', 'impl research::ResearchStore for MockStore {'] if x in text)
replace(str(api_tests), mock_marker, mock_marker + '''
    async fn decision_continuity(&self, operator: &str, session: &str, observation: &str) -> Result<arb_storage::DecisionContinuity, StoreError> {
        assert_eq!(operator, "operator");
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail { return Err(StoreError::CorruptState); }
        if observation == "missing" { return Err(StoreError::NotFound); }
        Ok(arb_storage::DecisionContinuity {
            schema_version: "1.0.0".into(), assessment_kind: "CONTINUITY_ONLY".into(), authorizes_execution: false,
            trace_id: "synthetic-trace".into(), session_id: session.into(), observation_id: observation.into(),
            network_id: "base-mainnet".into(), checked_at: "2026-09-17T08:00:00.000Z".into(),
            policy_version: "base-capture-continuity-v1".into(), continuity_status: "UNTRACKED".into(),
            capture_count: "0".into(), bound_count: "0".into(), invalidation_reasons: vec![],
        })
    }''')
append(str(api_tests), '''#[tokio::test]
async fn continuity_http_requires_auth_and_validates_scope_before_reading() {
    let (app, store) = setup();
    let path = "/v1/sessions/session/decisions/observation/continuity";
    let unauthorized = call(&app, Method::GET, path, None, None, None, None, None).await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(unauthorized.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    let (cookie, _) = authenticate(&app).await;
    let valid = call(&app, Method::GET, path, None, Some(&cookie), None, None, None).await;
    assert_eq!(valid.status(), StatusCode::OK);
    assert_eq!(valid.headers()[header::CACHE_CONTROL], "no-store");
    let body = value(valid).await;
    assert_eq!(body["session_id"], "session");
    assert_eq!(body["observation_id"], "observation");
    assert_eq!(body["authorizes_execution"], false);
    assert_eq!(body["continuity_status"], "UNTRACKED");
    let calls = store.calls.load(Ordering::SeqCst);
    for invalid in [format!("{path}?extra=true"), format!("/v1/sessions/session/decisions/{}/continuity", "x".repeat(129))] {
        let response = call(&app, Method::GET, &invalid, None, Some(&cookie), None, None, None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), calls);
    let missing = call(&app, Method::GET, "/v1/sessions/session/decisions/missing/continuity", None, Some(&cookie), None, None, None).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn continuity_http_storage_failure_is_redacted_not_healthy() {
    let store = Arc::new(MockStore { fail: true, ..Default::default() });
    let app = setup_with(store, true);
    let (cookie, _) = authenticate(&app).await;
    let response = call(&app, Method::GET, "/v1/sessions/session/decisions/observation/continuity", None, Some(&cookie), None, None, None).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = value(response).await;
    assert_eq!(body["code"], "DEPENDENCY_UNAVAILABLE");
    assert!(body.get("continuity_status").is_none());
}''')
append('apps/web/tests/browser/research.spec.ts', '''function continuityFixture(status = 'NO_KNOWN_INVALIDATION') {
  const record = records[0];
  return { schema_version: '1.0.0', assessment_kind: 'CONTINUITY_ONLY', authorizes_execution: false,
    trace_id: record.trace_id, session_id: record.trace.session_id, observation_id: record.trace.observation_id,
    network_id: record.trace.network_id, checked_at: '2026-09-17T08:00:00.000Z', policy_version: 'base-capture-continuity-v1',
    continuity_status: status, capture_count: String(record.trace.capture_refs.length),
    bound_count: status === 'UNTRACKED' ? '0' : String(record.trace.capture_refs.length),
    invalidation_reasons: status === 'INVALIDATED' ? ['CONTINUITY_LOST'] : [] };
}
test('source invalidation is visible without changing historical quote evidence', async ({ page }) => {
  let status = 'NO_KNOWN_INVALIDATION';
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/continuity')) return false;
    await route.fulfill({ json: continuityFixture(status) }); return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Source continuity', exact: true });
  await expect(panel.getByText('No known source invalidation', { exact: true })).toBeVisible();
  status = 'INVALIDATED';
  await panel.getByRole('button', { name: 'Refresh source status' }).click();
  await expect(panel.getByText('Source invalidated', { exact: true })).toBeVisible();
  await expect(panel.getByText('CONTINUITY_LOST', { exact: true })).toBeVisible();
  const modal = page.getByRole('dialog', { name: 'Decision evidence detail' });
  await expect(modal.getByText('CANDIDATE · GROSS QUOTE', { exact: true })).toBeVisible();
  await expect(modal.getByText('Unknown — external costs incomplete', { exact: true })).toBeVisible();
});
test('failed source refresh preserves a labelled last-known timestamp and status', async ({ page }) => {
  let failed = false;
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/continuity')) return false;
    if (failed) await route.fulfill({ status: 503, json: { code: 'DEPENDENCY_UNAVAILABLE' } });
    else await route.fulfill({ json: continuityFixture('INVALIDATED') });
    return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Source continuity', exact: true });
  await expect(panel.getByText('Source invalidated', { exact: true })).toBeVisible();
  failed = true; await panel.getByRole('button', { name: 'Refresh source status' }).click();
  await expect(panel.getByText(/Last received source status only/)).toBeVisible();
  await expect(panel.getByText('2026-09-17T08:00:00.000Z', { exact: true })).toBeVisible();
  await expect(panel.getByText('Source invalidated', { exact: true })).toBeVisible();
  await expect(panel.getByText('No known source invalidation', { exact: true })).toHaveCount(0);
});
test('missing source links and wrong-session responses never appear as healthy', async ({ page }) => {
  let wrong = false;
  await stub(page, async (route, url) => {
    if (!url.pathname.endsWith('/continuity')) return false;
    await route.fulfill({ json: wrong ? { ...continuityFixture(), session_id: 'other-session' } : continuityFixture('UNTRACKED') }); return true;
  });
  await openDecisions(page);
  await page.getByRole('button', { name: 'Inspect decision observation-fixture', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Source continuity', exact: true });
  await expect(panel.getByText('Source continuity not tracked', { exact: true })).toBeVisible();
  wrong = true; await panel.getByRole('button', { name: 'Refresh source status' }).click();
  await expect(panel.getByText(/Last received source status only/)).toBeVisible();
  await expect(panel.getByText('No known source invalidation', { exact: true })).toHaveCount(0);
});''')
properties = {k: {'type': 'string'} for k in ['trace_id', 'session_id', 'observation_id', 'checked_at']}
for key in ['trace_id', 'session_id', 'observation_id']:
    properties[key].update(minLength=1, maxLength=128, pattern='^[A-Za-z0-9_.:-]+$')
properties['checked_at']['format'] = 'date-time'
properties.update({
    'schema_version': {'const': '1.0.0'}, 'assessment_kind': {'const': 'CONTINUITY_ONLY'},
    'authorizes_execution': {'const': False}, 'network_id': {'enum': ['base-mainnet', 'solana-mainnet']},
    'policy_version': {'const': 'base-capture-continuity-v1'},
    'continuity_status': {'enum': ['NO_KNOWN_INVALIDATION', 'INVALIDATED', 'UNTRACKED', 'UNVERIFIABLE']},
    'capture_count': {'type': 'string', 'pattern': '^(0|[1-5]?[0-9]|6[0-4])$'},
    'bound_count': {'type': 'string', 'pattern': '^(0|[1-5]?[0-9]|6[0-4])$'},
    'invalidation_reasons': {'type': 'array', 'maxItems': 4, 'uniqueItems': True, 'items': {'enum': ['CONTINUITY_LOST', 'PROVIDER_FAILURE', 'RESOURCE_LIMIT', 'INVALID_INPUT']}},
})
schema = {'type': 'object', 'additionalProperties': False, 'properties': properties, 'required': list(properties)}
rendered = '\n'.join('      ' + line for line in json.dumps(schema, indent=2).splitlines())
replace('specs/openapi.yaml', '  schemas:\n', '  schemas:\n    DecisionContinuity:\n' + rendered + '\n')
operation = {'get': {
    'operationId': 'getDecisionContinuity',
    'description': 'Read-only continuity diagnostics at one database read snapshot. This does not grant execution eligibility, refresh market data, or rewrite historical evidence. No query parameters are accepted.',
    'parameters': [{'name': k, 'in': 'path', 'required': True, 'schema': {'type': 'string', 'minLength': 1, 'maxLength': 128, 'pattern': '^[A-Za-z0-9_.:-]+$'}} for k in ['session_id', 'observation_id']],
    'responses': {'200': {'description': 'Current source projection; no-store, never an execution authorization.', 'content': {'application/json': {'schema': {'$ref': '#/components/schemas/DecisionContinuity'}}}}},
}}
for status in ['400', '401', '404', '429', '503', '504']:
    operation['get']['responses'][status] = {'description': 'Redacted request, authentication, scope or dependency failure; never a healthy status.', 'content': {'application/json': {'schema': {'$ref': '#/components/schemas/Error'}}}}
rendered = '\n'.join('    ' + line for line in json.dumps(operation, indent=2).splitlines())
replace('specs/openapi.yaml', '\ncomponents:\n', '\n  /v1/sessions/{session_id}/decisions/{observation_id}/continuity:\n' + rendered + '\ncomponents:\n')
replace('specs/openapi.yaml', '  version: 0.8.0\n', '  version: 0.9.0\n')
replace('GOAL.md', '## Current delivery checkpoint\n', '''## Current delivery checkpoint

### Authenticated decision continuity view, 17 September 2026

PR141 is already integrated at `b3867784`. Existing #32/#51 now connect its source
projection to an authenticated diagnostic endpoint and the decision inspector.
See [decision continuity](docs/DECISION-CONTINUITY-UI.md). This read is not an
execution gate, a deployed worker, fresh market data or paper settlement. Exact CI,
review, merge and deployment results remain recorded on the PR and existing issues.
No new task issues, credentials or real-trading activation are needed.
''')
p = Path('planning/implementation-progress.json')
progress = json.loads(p.read_text())
assert len(progress['tickets']) == 68
progress['updated_on'] = '2026-09-17'
for ticket in progress['tickets']:
    if ticket['id'] in ['ARB-018', 'ARB-037']:
        ticket['evidence'].append('docs/DECISION-CONTINUITY-UI.md: additive current-source diagnostic endpoint and decision-inspector panel. Original scope/dependency acceptance is unchanged; actual exact-source CI and review evidence is recorded on the associated PR and existing #32/#51.')
p.write_text(json.dumps(progress, indent=2, ensure_ascii=False) + '\n')
print('Prepared bounded continuity diagnostic implementation and regression tests; no test pass is implied.')
