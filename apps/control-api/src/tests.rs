use super::*;
use arb_storage::{DecisionCoverage, DecisionGroupPage, DecisionTracePage, NewPaperRun, OpportunityFilter, OpportunityPage, PaperJournalPage, PaperReservationPage, PaperRunPage, PaperRunRecord, StoredDecisionTrace};
use axum::body::{Body, to_bytes};
use serde_json::{Value, json};
use std::sync::atomic::AtomicUsize;
use tower::ServiceExt;

const SECRET: &str = "test-operator-secret-at-least-32-bytes";
const ORIGIN: &str = "http://127.0.0.1:5173";
const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Default)]
struct MockStore {
    calls: AtomicUsize,
    fail: bool,
    blocked: bool,
    sessions: Mutex<HashMap<String, (NewSession, SessionRecord)>>,
}
fn record() -> SessionRecord {
    SessionRecord {
        session_id: "6e09f761-8d04-4a8b-8ff0-301bdba75f22".into(),
        network_id: "base-mainnet".into(),
        mode: "PAPER".into(),
        observed_state: "RECOVERING".into(),
        health: "UNKNOWN".into(),
        desired_revision: "0".into(),
        applied_revision: "0".into(),
        outstanding_attempts: 0,
        execution_authorized: false,
        last_heartbeat_at: None,
        configuration_digest: DIGEST.into(),
    }
}
#[async_trait]
impl ControlStore for MockStore {
    async fn replay_session_creation(
        &self,
        _: &str,
        key: &str,
        input: &NewSession,
    ) -> Result<Option<SessionRecord>, StoreError> {
        match self.sessions.lock().unwrap().get(key) {
            Some((saved, result)) if saved == input => Ok(Some(result.clone())),
            Some(_) => Err(StoreError::Conflict("idempotency payload changed")),
            None => Ok(None),
        }
    }
    async fn health(&self) -> Result<(), StoreError> {
        if self.fail {
            Err(StoreError::CorruptState)
        } else {
            Ok(())
        }
    }
    async fn create_session(
        &self,
        _: &str,
        key: &str,
        input: NewSession,
    ) -> Result<SessionRecord, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut sessions = self.sessions.lock().unwrap();
        if let Some((saved, result)) = sessions.get(key) {
            return if *saved == input {
                Ok(result.clone())
            } else {
                Err(StoreError::Conflict("idempotency payload changed"))
            };
        }
        let session = record();
        sessions.insert(key.into(), (input, session.clone()));
        Ok(session)
    }
    async fn list_sessions(
        &self,
        _: &str,
        _: Option<&str>,
        _: u32,
    ) -> Result<SessionPage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.blocked {
            std::future::pending::<()>().await;
        }
        if self.fail {
            return Err(StoreError::CorruptState);
        }
        Ok(SessionPage {
            items: vec![record()],
            next_cursor: None,
        })
    }
    async fn get_session(&self, _: &str, id: &str) -> Result<SessionRecord, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if id == "missing" {
            Err(StoreError::NotFound)
        } else {
            Ok(record())
        }
    }
    async fn issue_command(
        &self,
        _: &str,
        _: &str,
        _: &str,
        input: NewCommand,
    ) -> Result<CommandReceipt, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if input.expected_revision != "0" {
            return Err(StoreError::Conflict("revision conflict"));
        }
        Ok(CommandReceipt {
            command_id: "command-1".into(),
            session_id: record().session_id,
            revision: "1".into(),
            status: "PENDING".into(),
            action: input.action,
            accepted_at: "2026-09-12T00:00:00Z".into(),
            applied_at: None,
            outstanding_attempts: 0,
            fence_effective: false,
            signer_revocation_status: "NOT_APPLICABLE".into(),
        })
    }
    async fn get_command(&self, _: &str, _: &str) -> Result<CommandReceipt, StoreError> {
        Err(StoreError::NotFound)
    }
}
fn setup_with(store: Arc<MockStore>, insecure: bool) -> Router {
    router(AppState::with_store(
        store,
        ServerConfig {
            listen_address: "127.0.0.1:8080".parse().unwrap(),
            public_origin: ORIGIN.into(),
            allow_insecure_loopback: insecure,
            operator_secret_hash: hash(SECRET),
            configurations: vec![RegisteredConfiguration {
                configuration_digest: DIGEST.into(),
                mode: "PAPER".into(),
                enabled_networks: vec!["base-mainnet".into()],
                strategy_ids: vec!["cyclic-exact-in-2leg-v1".into()],
                paper_assets: Vec::new(),
                snapshot: json!({}),
            }],
        },
    ))
}
fn setup() -> (Router, Arc<MockStore>) {
    let store = Arc::new(MockStore::default());
    (setup_with(store.clone(), true), store)
}
#[allow(clippy::too_many_arguments)]
async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: Option<&str>,
    origin: Option<&str>,
    key: Option<&str>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    if let Some(origin) = origin {
        request = request.header("origin", origin);
    }
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    let body = if let Some(body) = body {
        request = request.header(header::CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&body).unwrap())
    } else {
        Body::empty()
    };
    app.clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
}
async fn value(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap()
}
async fn authenticate(app: &Router) -> (String, String) {
    let response = call(
        app,
        Method::POST,
        "/v1/auth/login",
        Some(json!({"operator_secret":SECRET})),
        None,
        None,
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body = value(response).await;
    (cookie, body["csrf_token"].as_str().unwrap().to_owned())
}
fn new_session() -> Value {
    json!({"network_id":"base-mainnet","mode":"PAPER","configuration_digest":DIGEST,"experiment_id":"experiment-1","strategy_ids":["cyclic-exact-in-2leg-v1"]})
}

#[tokio::test]
async fn unauthenticated_requests_are_structured_redacted_and_do_not_touch_storage() {
    let (app, store) = setup();
    let response = call(
        &app,
        Method::GET,
        "/v1/sessions",
        None,
        None,
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let correlation = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = value(response).await;
    assert_eq!(body["request_id"], correlation);
    assert_eq!(body.as_object().unwrap().len(), 3);
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    assert!(!body.to_string().contains(SECRET));
}

#[tokio::test]
async fn cookies_are_secure_by_default_and_login_rotates_tokens() {
    let app = setup_with(Arc::new(MockStore::default()), false);
    let first = call(
        &app,
        Method::POST,
        "/v1/auth/login",
        Some(json!({"operator_secret":SECRET})),
        None,
        None,
        Some(ORIGIN),
        None,
    )
    .await;
    let cookie = first.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("; Secure"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    let second = call(
        &app,
        Method::POST,
        "/v1/auth/login",
        Some(json!({"operator_secret":SECRET})),
        None,
        None,
        Some(ORIGIN),
        None,
    )
    .await;
    assert_ne!(cookie, second.headers()[header::SET_COOKIE]);
}

#[tokio::test]
async fn origin_and_csrf_rejections_cannot_mutate() {
    let (app, store) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    for (origin, csrf) in [
        (Some("https://attacker.invalid"), Some(csrf.as_str())),
        (None, Some(csrf.as_str())),
        (Some(ORIGIN), None),
        (Some(ORIGIN), Some("wrong")),
    ] {
        let response = call(
            &app,
            Method::POST,
            "/v1/sessions",
            Some(new_session()),
            Some(&cookie),
            csrf,
            origin,
            Some("session-create-0001"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn login_has_global_bounded_rate_limit_and_redacts_submitted_secret() {
    let (app, _) = setup();
    for attempt in 0..11 {
        let response = call(
            &app,
            Method::POST,
            "/v1/auth/login",
            Some(json!({"operator_secret":"DO-NOT-LEAK-THIS"})),
            None,
            None,
            Some(ORIGIN),
            None,
        )
        .await;
        assert_eq!(
            response.status(),
            if attempt < 10 {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
        assert!(
            !value(response)
                .await
                .to_string()
                .contains("DO-NOT-LEAK-THIS")
        );
    }
}

#[tokio::test]
async fn authenticated_session_and_logout_do_not_persist_browser_secrets() {
    let (app, _) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    let response = call(
        &app,
        Method::GET,
        "/v1/auth/session",
        None,
        Some(&cookie),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(value(response).await["csrf_token"], csrf);
    let response = call(
        &app,
        Method::POST,
        "/v1/auth/logout",
        None,
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        call(
            &app,
            Method::GET,
            "/v1/sessions",
            None,
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn creation_retries_forward_idempotency_and_reject_changed_payload() {
    let (app, _) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    let first = call(
        &app,
        Method::POST,
        "/v1/sessions",
        Some(new_session()),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some("session-create-0001"),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first = value(first).await;
    assert_eq!(first["observed_state"], "RECOVERING");
    assert_eq!(first["execution_authorized"], false);
    let second = call(
        &app,
        Method::POST,
        "/v1/sessions",
        Some(new_session()),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some("session-create-0001"),
    )
    .await;
    assert_eq!(value(second).await, first);
    let mut changed = new_session();
    changed["experiment_id"] = json!("changed-experiment");
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/sessions",
            Some(changed),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some("session-create-0001")
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn unknown_config_live_mode_and_unsupported_strategies_fail_before_store() {
    let (app, store) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    for (field, replacement, expected) in [
        (
            "configuration_digest",
            json!("sha256:unknown"),
            StatusCode::BAD_REQUEST,
        ),
        ("mode", json!("LIVE"), StatusCode::FORBIDDEN),
        (
            "strategy_ids",
            json!(["unverified-strategy"]),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut body = new_session();
        body[field] = replacement;
        assert_eq!(
            call(
                &app,
                Method::POST,
                "/v1/sessions",
                Some(body),
                Some(&cookie),
                Some(&csrf),
                Some(ORIGIN),
                Some("session-create-0001")
            )
            .await
            .status(),
            expected
        );
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn stop_acceptance_is_pending_and_revision_conflicts_are_explicit() {
    let (app, _) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    let response = call(
        &app,
        Method::POST,
        "/v1/sessions/any/commands",
        Some(json!({"action":"STOP","expected_revision":"0"})),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some("command-stop-0001"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let receipt = value(response).await;
    assert_eq!(receipt["status"], "PENDING");
    assert_eq!(receipt["fence_effective"], false);
    assert!(receipt["applied_at"].is_null());
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/sessions/any/commands",
            Some(json!({"action":"STOP","expected_revision":"10"})),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some("command-stop-0002")
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/sessions/any/commands",
            Some(json!({"action":"DISARM","expected_revision":"0"})),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some("command-stop-0003")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn pages_are_bounded_and_evidence_filters_are_validated() {
    let (app, store) = setup();
    let (cookie, _) = authenticate(&app).await;
    for path in [
        "/v1/sessions?limit=0",
        "/v1/sessions?limit=101",
        "/v1/sessions?limit=abc",
        "/v1/sessions?unexpected=1",
        "/v1/opportunities?network_id=unknown",
        "/v1/opportunities?evidence_label=PROFITABLE",
    ] {
        assert_eq!(
            call(
                &app,
                Method::GET,
                path,
                None,
                Some(&cookie),
                None,
                None,
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    let page = value(
        call(
            &app,
            Method::GET,
            "/v1/opportunities?limit=100",
            None,
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(page, json!({"items":[],"next_cursor":null}));
    let caps = value(
        call(
            &app,
            Method::GET,
            "/v1/capabilities",
            None,
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(caps["opportunity_capture"], false);
    assert_eq!(caps["live_execution"], false);
    assert!(!caps.to_string().contains("snapshot"));
    assert!(!caps.to_string().contains(SECRET));
}

#[tokio::test]
async fn storage_failure_returns_redacted_correlated_dependency_error() {
    let app = setup_with(
        Arc::new(MockStore {
            fail: true,
            ..MockStore::default()
        }),
        true,
    );
    let (cookie, _) = authenticate(&app).await;
    let response = call(
        &app,
        Method::GET,
        "/v1/health",
        None,
        Some(&cookie),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(value(response).await["code"], "DEPENDENCY_UNAVAILABLE");
}

#[tokio::test]
async fn unknown_json_fields_and_oversized_bodies_are_rejected() {
    let (app, store) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    let mut input = new_session();
    input["secret"] = json!("ignored-fields-must-fail");
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/sessions",
            Some(input),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some("session-create-0001")
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let mut input = new_session();
    input["experiment_id"] = json!("x".repeat(20_000));
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/sessions",
            Some(input),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some("session-create-0001")
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn insecure_cookie_exception_is_strictly_loopback_and_opt_in() {
    for origin in [
        "http://example.com",
        "http://127.0.0.1.evil.example",
        "https://localhost/path",
        "https://user:pass@localhost",
        "https://localhost/",
    ] {
        assert!(validate_origin(origin, true).is_err());
    }
    assert!(validate_origin("https://private.example", false).is_ok());
    assert!(validate_origin("http://localhost:5173", true).is_ok());
    assert!(validate_origin("http://localhost:5173", false).is_err());
}

/// PostgreSQL is mandatory for this integration case. Unit-only runners must
/// explicitly filter it out; absence never counts as a passing database test.
#[tokio::test]
async fn postgres_http_contract_survives_api_restart_and_deduplicates_intent() {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL is required for PostgreSQL HTTP integration");
    let store = Store::connect(&url)
        .await
        .expect("test PostgreSQL reachable");
    store.migrate().await.expect("test migrations apply");
    let unique = random_token(&RequestId("test".into())).unwrap();
    let digest = format!("sha256:{unique}");
    store
        .save_configuration(
            "operator",
            &digest,
            json!({"test_fixture":true,"unique":unique}),
        )
        .await
        .unwrap();
    let settings = || ServerConfig {
        listen_address: "127.0.0.1:8080".parse().unwrap(),
        public_origin: ORIGIN.into(),
        allow_insecure_loopback: true,
        operator_secret_hash: hash(SECRET),
        configurations: vec![RegisteredConfiguration {
            configuration_digest: digest.clone(),
            mode: "PAPER".into(),
            enabled_networks: vec!["base-mainnet".into()],
            strategy_ids: vec!["cyclic-exact-in-2leg-v1".into()],
                paper_assets: Vec::new(),
            snapshot: json!({}),
        }],
    };
    let app = router(AppState::new(store.clone(), settings()));
    let (cookie, csrf) = authenticate(&app).await;
    assert_eq!(
        call(
            &app,
            Method::GET,
            "/v1/health",
            None,
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    let mut input = new_session();
    input["configuration_digest"] = json!(digest);
    let key = format!("create-{unique}");
    let response = call(
        &app,
        Method::POST,
        "/v1/sessions",
        Some(input.clone()),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some(&key),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let original = value(response).await;
    let session_id = original["session_id"].as_str().unwrap();
    assert_eq!(original["observed_state"], "RECOVERING");
    assert_eq!(
        value(
            call(
                &app,
                Method::POST,
                "/v1/sessions",
                Some(input.clone()),
                Some(&cookie),
                Some(&csrf),
                Some(ORIGIN),
                Some(&key)
            )
            .await
        )
        .await,
        original
    );
    input["experiment_id"] = json!("changed");
    assert_eq!(
        call(
            &app,
            Method::POST,
            "/v1/sessions",
            Some(input),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some(&key)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let worker = arb_control::ControlWorker::claim(
        store.clone(),
        "operator",
        session_id,
        "base-mainnet",
        &format!("worker-{unique}"),
        30,
    )
    .await
    .unwrap();
    worker.complete_recovery().await.unwrap();
    let path = format!("/v1/sessions/{session_id}/commands");
    let key = format!("stop-{unique}");
    let command = json!({"action":"STOP","expected_revision":"0"});
    let response = call(
        &app,
        Method::POST,
        &path,
        Some(command.clone()),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some(&key),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let receipt = value(response).await;
    assert_eq!(receipt["status"], "PENDING");
    assert_eq!(receipt["fence_effective"], false);
    assert_eq!(
        value(
            call(
                &app,
                Method::POST,
                &path,
                Some(command),
                Some(&cookie),
                Some(&csrf),
                Some(ORIGIN),
                Some(&key)
            )
            .await
        )
        .await,
        receipt
    );
    assert_eq!(
        call(
            &app,
            Method::POST,
            &path,
            Some(json!({"action":"STOP","expected_revision":"0"})),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some(&format!("stale-{unique}"))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let update = worker.tick(false).await.unwrap();
    let applied = update.command.expect("worker acknowledges STOP");
    assert_eq!(applied.status, "APPLIED");
    assert!(applied.fence_effective);
    let command_path = format!("/v1/commands/{}", receipt["command_id"].as_str().unwrap());
    let applied_http = value(
        call(
            &app,
            Method::GET,
            &command_path,
            None,
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(applied_http["status"], "APPLIED");
    assert_eq!(applied_http["fence_effective"], true);
    store
        .save_configuration("other-operator", &digest, json!({"scope":"other"}))
        .await
        .unwrap();
    let mut other_input: NewSession = serde_json::from_value(new_session()).unwrap();
    other_input.configuration_digest = digest.clone();
    let other_session = store
        .create_session("other-operator", &format!("other-{unique}"), other_input)
        .await
        .unwrap();
    assert_eq!(
        call(
            &app,
            Method::GET,
            &format!("/v1/sessions/{}", other_session.session_id),
            None,
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    // A fresh API process has no old browser session; intent remains durable.
    let restarted = router(AppState::new(store, settings()));
    assert_eq!(
        call(
            &restarted,
            Method::GET,
            "/v1/auth/session",
            None,
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let (new_cookie, _) = authenticate(&restarted).await;
    let path = format!("/v1/commands/{}", receipt["command_id"].as_str().unwrap());
    assert_eq!(
        value(
            call(
                &restarted,
                Method::GET,
                &path,
                None,
                Some(&new_cookie),
                None,
                None,
                None
            )
            .await
        )
        .await,
        applied_http
    );
}

#[tokio::test]
async fn accepted_create_retry_survives_removal_of_active_configuration() {
    let (app, store) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    let first = value(
        call(
            &app,
            Method::POST,
            "/v1/sessions",
            Some(new_session()),
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some("session-create-0001"),
        )
        .await,
    )
    .await;
    let settings = ServerConfig {
        listen_address: "127.0.0.1:8080".parse().unwrap(),
        public_origin: ORIGIN.into(),
        allow_insecure_loopback: true,
        operator_secret_hash: hash(SECRET),
        configurations: vec![],
    };
    let restarted = router(AppState::with_store(store, settings));
    let (cookie, csrf) = authenticate(&restarted).await;
    let retried = call(
        &restarted,
        Method::POST,
        "/v1/sessions",
        Some(new_session()),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some("session-create-0001"),
    )
    .await;
    assert_eq!(retried.status(), StatusCode::CREATED);
    assert_eq!(value(retried).await, first);
    let new_intent = call(
        &restarted,
        Method::POST,
        "/v1/sessions",
        Some(new_session()),
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        Some("session-create-0002"),
    )
    .await;
    assert_eq!(new_intent.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn malformed_path_and_wrong_method_errors_keep_the_json_contract() {
    let (app, _) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    let response = call(
        &app,
        Method::GET,
        "/v1/sessions/%FF",
        None,
        Some(&cookie),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(value(response).await["code"], "INVALID_INPUT");
    let response = call(
        &app,
        Method::POST,
        "/v1/health",
        None,
        Some(&cookie),
        Some(&csrf),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(value(response).await["code"], "METHOD_NOT_ALLOWED");
}

#[test]
fn container_listener_requires_explicit_https_origin_and_secure_cookies() {
    assert!(validate_listener("127.0.0.1", "8080", None, true).is_ok());
    assert!(validate_listener("::1", "8080", None, false).is_ok());
    assert!(validate_listener("0.0.0.0", "8080", None, false).is_err());
    assert!(validate_listener("0.0.0.0", "8080", Some("http://localhost:5173"), true).is_err());
    assert!(validate_listener("0.0.0.0", "8080", Some("http://dashboard.example"), false).is_err());
    assert_eq!(
        validate_listener("0.0.0.0", "8080", Some("https://dashboard.example"), false)
            .unwrap()
            .to_string(),
        "0.0.0.0:8080"
    );
    assert!(validate_listener("::", "8080", Some("https://dashboard.example"), false).is_ok());
    assert!(validate_listener("127.0.0.1", "0", None, false).is_err());
    assert!(validate_listener("hostname.example", "8080", None, false).is_err());
}

#[tokio::test(start_paused = true)]
async fn request_deadline_returns_correlated_uncertainty_without_waiting_real_time() {
    let store = Arc::new(MockStore {
        blocked: true,
        ..MockStore::default()
    });
    let app = setup_with(store, true);
    let (cookie, _) = authenticate(&app).await;
    let response = call(
        &app,
        Method::GET,
        "/v1/sessions",
        None,
        Some(&cookie),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let body = value(response).await;
    assert_eq!(body["code"], "REQUEST_TIMEOUT");
    assert_eq!(body["request_id"], request_id);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("same idempotency key")
    );
}

#[tokio::test(start_paused = true)]
async fn request_capacity_is_bounded_and_cancellation_releases_owned_permits() {
    let store = Arc::new(MockStore {
        blocked: true,
        ..MockStore::default()
    });
    let app = setup_with(store.clone(), true);
    let (cookie, _) = authenticate(&app).await;
    let mut requests = Vec::new();
    for _ in 0..MAX_INFLIGHT_REQUESTS {
        let app = app.clone();
        let cookie = cookie.clone();
        requests.push(tokio::spawn(async move {
            call(
                &app,
                Method::GET,
                "/v1/sessions",
                None,
                Some(&cookie),
                None,
                None,
                None,
            )
            .await
        }));
    }
    for _ in 0..1000 {
        if store.calls.load(Ordering::SeqCst) == MAX_INFLIGHT_REQUESTS {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), MAX_INFLIGHT_REQUESTS);
    let response = call(
        &app,
        Method::GET,
        "/v1/capabilities",
        None,
        Some(&cookie),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(value(response).await["code"], "CAPACITY_EXCEEDED");
    for request in requests {
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
    }
    assert_eq!(
        call(
            &app,
            Method::GET,
            "/v1/capabilities",
            None,
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[async_trait]
impl ResearchStore for MockStore {
    async fn list_decisions(&self, _operator: &str, _session: &str, _cursor: Option<&str>, _limit: u32) -> Result<DecisionTracePage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn get_decision(&self, _operator: &str, _observation: &str) -> Result<StoredDecisionTrace, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn decision_groups(&self, _operator: &str, _session: &str, _cursor: Option<&str>, _limit: u32) -> Result<DecisionGroupPage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn decision_coverage(&self, _operator: &str, _session: &str) -> Result<DecisionCoverage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn opportunities(&self, _operator: &str, _filter: OpportunityFilter, _cursor: Option<&str>, _limit: u32) -> Result<OpportunityPage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(OpportunityPage { items: Vec::new(), next_cursor: None })
    }
    async fn replay_paper_creation(&self, _operator: &str, _session: &str, _key: &str, _input: &NewPaperRun) -> Result<Option<PaperRunRecord>, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    async fn create_paper_run(&self, _operator: &str, _session: &str, _key: &str, _input: NewPaperRun) -> Result<PaperRunRecord, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn get_paper_run(&self, _operator: &str, _run: &str) -> Result<PaperRunRecord, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn list_paper_runs(&self, _operator: &str, _session: &str, _cursor: Option<&str>, _limit: u32) -> Result<PaperRunPage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn paper_journal(&self, _operator: &str, _run: &str, _cursor: Option<&str>, _limit: u32) -> Result<PaperJournalPage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
    async fn paper_reservations(&self, _operator: &str, _run: &str, _cursor: Option<&str>, _limit: u32) -> Result<PaperReservationPage, StoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(StoreError::NotFound)
    }
}

#[tokio::test]
async fn research_routes_require_auth_and_keep_mutation_guards() {
    let (app, store) = setup();
    for path in [
        "/v1/decisions?session_id=x", "/v1/decisions/x",
        "/v1/decision-groups?session_id=x", "/v1/decision-coverage?session_id=x",
        "/v1/sessions/x/paper-runs", "/v1/paper-runs/x",
        "/v1/paper-runs/x/journal", "/v1/paper-runs/x/reservations",
    ] {
        let response = call(&app, Method::GET, path, None, None, None, None, None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    let (cookie, csrf) = authenticate(&app).await;
    for (origin, token) in [(Some("https://foreign.example"), Some(csrf.as_str())), (Some(ORIGIN), None)] {
        let response = call(&app, Method::POST, "/v1/sessions/x/paper-runs",
            Some(json!({"initial_balances":[]})), Some(&cookie), token, origin, Some("paper-create-key-01")).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    for path in ["/v1/paper-runs/x/settle", "/v1/paper-runs/x/reserve", "/v1/decisions/x/realize"] {
        let response = call(&app, Method::POST, path, Some(json!({})), Some(&cookie), Some(&csrf), Some(ORIGIN), Some("paper-create-key-01")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}
#[tokio::test]
async fn research_page_and_paper_payload_bounds_fail_before_read_queries() {
    let (app, store) = setup();
    let (cookie, csrf) = authenticate(&app).await;
    for path in [
        "/v1/decisions", "/v1/decisions?session_id=x&limit=101",
        "/v1/decision-groups?session_id=x&limit=0",
        "/v1/decision-coverage?session_id=x&limit=10",
        "/v1/paper-runs/x/journal?limit=101",
        "/v1/paper-runs/x/reservations?cursor=",
        "/v1/opportunities?source_kind=REAL_PROFIT",
    ] {
        let response = call(&app, Method::GET, path, None, Some(&cookie), None, None, None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(value(response).await["code"], "INVALID_INPUT");
    }
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    let forged = call(&app, Method::POST, "/v1/sessions/x/paper-runs",
        Some(json!({"initial_balances":[], "settlement":{"profit":"100"}})),
        Some(&cookie), Some(&csrf), Some(ORIGIN), Some("paper-create-key-01")).await;
    assert_eq!(forged.status(), StatusCode::BAD_REQUEST);
    assert_eq!(store.calls.load(Ordering::SeqCst), 0);
    let caps = value(call(&app, Method::GET, "/v1/capabilities", None, Some(&cookie), None, None, None).await).await;
    assert_eq!(caps["decision_history"], true);
    assert_eq!(caps["paper_ledger"], true);
    assert_eq!(caps["paper_run_creation"], true);
    assert_eq!(caps["live_execution"], false);
    assert_eq!(caps["registered_configurations"][0]["paper_assets"], json!([]));
}

#[tokio::test]
async fn postgres_paper_http_immutable_runs_survive_restart_and_enforce_scope() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required for PostgreSQL HTTP integration");
    let store = Store::connect(&url).await.expect("test PostgreSQL reachable");
    store.migrate().await.unwrap();
    let unique = random_token(&RequestId("paper-http".into())).unwrap();
    let digest = format!("sha256:{unique}");
    let asset: arb_domain::AssetId = "base-mainnet:0x1111111111111111111111111111111111111111".parse().unwrap();
    let snapshot = json!({"deployment":{"mode":"PAPER"}, "networks":{"base":{"enabled":true,"verified_asset_ids":[asset.to_string()]}}});
    store.save_configuration("operator", &digest, snapshot.clone()).await.unwrap();
    let session = store.create_session("operator", &format!("paper-session-{unique}"), NewSession {
        network_id:"base-mainnet".into(), mode:"PAPER".into(), configuration_digest:digest.clone(),
        experiment_id:format!("paper-http-{unique}"), strategy_ids:vec!["cyclic-exact-in-2leg-v1".into()],
    }).await.unwrap();
    let worker = arb_control::ControlWorker::claim(store.clone(), "operator", &session.session_id, "base-mainnet", &format!("paper-worker-{unique}"), 60).await.unwrap();
    worker.complete_recovery().await.unwrap();
    let configuration = RegisteredConfiguration {
        configuration_digest:digest.clone(), mode:"PAPER".into(), enabled_networks:vec!["base-mainnet".into()],
        strategy_ids:vec!["cyclic-exact-in-2leg-v1".into()],
        paper_assets:vec![
            PaperAssetChoice { network_id:"base-mainnet".into(), asset:arb_paper::AccountingAsset::Token(asset.clone()) },
            PaperAssetChoice { network_id:"base-mainnet".into(), asset:arb_paper::AccountingAsset::Native(arb_domain::NetworkId::BaseMainnet) },
        ],
        snapshot:snapshot.clone(),
    };
    let settings = |include_config:bool| ServerConfig {
        listen_address:"127.0.0.1:8080".parse().unwrap(), public_origin:ORIGIN.into(),
        allow_insecure_loopback:true, operator_secret_hash:hash(SECRET),
        configurations:if include_config { vec![configuration.clone()] } else { vec![] },
    };
    let app=router(AppState::new(store.clone(),settings(true)));
    let (cookie,csrf)=authenticate(&app).await;
    let path=format!("/v1/sessions/{}/paper-runs",session.session_id);
    let input=json!({"initial_balances":[
        {"asset":{"kind":"TOKEN","identity":asset.to_string()},"amount":"100000000"},
        {"asset":{"kind":"NATIVE","identity":"base-mainnet"},"amount":"1000000000000000"}
    ]});
    let key=format!("paper-run-{unique}");
    let response=call(&app,Method::POST,&path,Some(input.clone()),Some(&cookie),Some(&csrf),Some(ORIGIN),Some(&key)).await;
    assert_eq!(response.status(),StatusCode::CREATED);
    let first=value(response).await;
    assert_eq!(first["evidence_label"],"HYPOTHETICAL");
    assert_eq!(first["execution_authorized"],false);
    assert_eq!(first["configuration_digest"],digest);
    assert_eq!(first["outstanding_reservations"],0);
    assert_eq!(first["initial_balances"],input["initial_balances"]);
    let repeat=value(call(&app,Method::POST,&path,Some(input.clone()),Some(&cookie),Some(&csrf),Some(ORIGIN),Some(&key)).await).await;
    assert_eq!(repeat,first);
    let mut changed=input.clone(); changed["initial_balances"][0]["amount"]=json!("200000000");
    assert_eq!(call(&app,Method::POST,&path,Some(changed),Some(&cookie),Some(&csrf),Some(ORIGIN),Some(&key)).await.status(),StatusCode::CONFLICT);
    let mut forbidden=input.clone(); forbidden["initial_balances"][0]["asset"]["identity"]=json!("base-mainnet:0x2222222222222222222222222222222222222222");
    assert_eq!(call(&app,Method::POST,&path,Some(forbidden),Some(&cookie),Some(&csrf),Some(ORIGIN),Some(&format!("forbidden-{unique}"))).await.status(),StatusCode::BAD_REQUEST);
    let second=value(call(&app,Method::POST,&path,Some(input.clone()),Some(&cookie),Some(&csrf),Some(ORIGIN),Some(&format!("second-{unique}"))).await).await;
    assert_ne!(first["run_id"],second["run_id"]);
    let page=value(call(&app,Method::GET,&format!("{path}?limit=1"),None,Some(&cookie),None,None,None).await).await;
    assert_eq!(page["items"].as_array().unwrap().len(),1);
    let cursor=page["next_cursor"].as_str().expect("two runs require another page");
    let next=value(call(&app,Method::GET,&format!("{path}?limit=1&cursor={cursor}"),None,Some(&cookie),None,None,None).await).await;
    assert_eq!(next["items"].as_array().unwrap().len(),1);
    assert_ne!(page["items"][0]["run_id"],next["items"][0]["run_id"]);
    assert_eq!(next["next_cursor"],Value::Null);
    let run_id=first["run_id"].as_str().unwrap();
    let journal=value(call(&app,Method::GET,&format!("/v1/paper-runs/{run_id}/journal"),None,Some(&cookie),None,None,None).await).await;
    assert_eq!(journal["items"].as_array().unwrap().len(),1);
    assert_eq!(journal["items"][0]["event"]["command"]["kind"],"INITIALIZE");
    assert_eq!(journal["items"][0]["event"]["sequence"],"0");
    let reservations=value(call(&app,Method::GET,&format!("/v1/paper-runs/{run_id}/reservations"),None,Some(&cookie),None,None,None).await).await;
    assert_eq!(reservations["items"],json!([]));

    let other=format!("other-paper-{unique}");
    store.save_configuration(&other,&digest,snapshot).await.unwrap();
    let hidden_session=store.create_session(&other,&format!("hidden-{unique}"),NewSession {
        network_id:"base-mainnet".into(),mode:"PAPER".into(),configuration_digest:digest,
        experiment_id:format!("hidden-{unique}"),strategy_ids:vec!["cyclic-exact-in-2leg-v1".into()],
    }).await.unwrap();
    let hidden_worker=arb_control::ControlWorker::claim(store.clone(),&other,&hidden_session.session_id,"base-mainnet",&format!("hidden-worker-{unique}"),60).await.unwrap();
    hidden_worker.complete_recovery().await.unwrap();
    let hidden=store.create_paper_run(&other,&hidden_session.session_id,&format!("hidden-run-{unique}"),serde_json::from_value(input.clone()).unwrap()).await.unwrap();
    for resource in [
        format!("/v1/paper-runs/{}",hidden.run_id),
        format!("/v1/paper-runs/{}/journal",hidden.run_id),
        format!("/v1/paper-runs/{}/reservations",hidden.run_id),
        format!("/v1/sessions/{}/paper-runs",hidden_session.session_id),
    ] {
        assert_eq!(call(&app,Method::GET,&resource,None,Some(&cookie),None,None,None).await.status(),StatusCode::NOT_FOUND);
    }
    assert_eq!(call(&app,Method::POST,&format!("/v1/sessions/{}/paper-runs",hidden_session.session_id),Some(input.clone()),Some(&cookie),Some(&csrf),Some(ORIGIN),Some(&key)).await.status(),StatusCode::NOT_FOUND);

    let restarted=router(AppState::new(store.clone(),settings(false)));
    assert_eq!(call(&restarted,Method::GET,&format!("/v1/paper-runs/{run_id}"),None,Some(&cookie),None,None,None).await.status(),StatusCode::UNAUTHORIZED);
    let (new_cookie,new_csrf)=authenticate(&restarted).await;
    let replay=call(&restarted,Method::POST,&path,Some(input.clone()),Some(&new_cookie),Some(&new_csrf),Some(ORIGIN),Some(&key)).await;
    assert_eq!(replay.status(),StatusCode::CREATED);
    assert_eq!(value(replay).await,first);
    assert_eq!(call(&restarted,Method::POST,&path,Some(input),Some(&new_cookie),Some(&new_csrf),Some(ORIGIN),Some(&format!("unregistered-{unique}"))).await.status(),StatusCode::BAD_REQUEST);
}

fn http_trace_fixture(session: &SessionRecord, experiment: &str, generation: u64) -> arb_domain::DecisionTrace {
        use arb_domain::*;
        let a=AssetId::new(NetworkId::BaseMainnet,"0x0000000000000000000000000000000000000001").unwrap();
        let b=AssetId::new(NetworkId::BaseMainnet,"0x0000000000000000000000000000000000000002").unwrap();
        let captures=(3..=4).map(|n| {
            let digest=format!("sha256:{}", n.to_string().repeat(64));
            DecisionCaptureRef {capture_id:format!("capture-{n}"),manifest_digest:digest.clone(),snapshot_id:digest}
        }).collect();
        DecisionTrace {
            schema_version:DECISION_SCHEMA_VERSION.into(),observation_id:String::new(),
            session_id:session.session_id.clone(),experiment_id:experiment.into(),generation,
            configuration_digest:session.configuration_digest.clone(),
            calculation_version:"research-math-v1".into(),strategy_id:"cyclic-exact-in-2leg-v1".into(),
            network_id:NetworkId::BaseMainnet,mode:Mode::Observe,source_kind:SourceKind::SyntheticFixture,
            dataset_origin:DatasetOrigin::ManuallyConstructed,observed_at_unix_ms:1_700_000_000_100,
            input_age_ms:Some(17),capture_refs:captures,
            route:vec![
                DecisionLeg {pool_id:PoolId::new(NetworkId::BaseMainnet,"0x0000000000000000000000000000000000000003").unwrap(),asset_in:a.clone(),asset_out:b.clone(),venue_family:"uniswap-v3".into()},
                DecisionLeg {pool_id:PoolId::new(NetworkId::BaseMainnet,"0x0000000000000000000000000000000000000004").unwrap(),asset_in:b,asset_out:a,venue_family:"uniswap-v3".into()},
            ],
            amount_in_minor:Some(AtomicAmount::from(100)),result:DecisionResult::Quoted {
                quoted_output_minor:AtomicAmount::from(99),gross_delta_minor:"-1".parse().unwrap(),
                included_pool_fees:vec![AtomicAmount::from(1),AtomicAmount::from(1)],
            },
            grouping:DecisionGrouping {version:String::new(),key:String::new(),window_ms:1000,window_start_ms:0},
            diagnostics:vec!["RESEARCH_MATH_ONLY".into()],
        }.seal().unwrap()
    }

#[tokio::test]
async fn postgres_decision_http_counts_rejections_without_undercounting_eligible_pages() {
    use arb_domain::DecisionResult;
    let url=std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required for PostgreSQL HTTP integration");
    let store=Store::connect(&url).await.expect("test PostgreSQL reachable");
    store.migrate().await.unwrap();
    let unique=random_token(&RequestId("trace-http".into())).unwrap();
    let digest=format!("sha256:{unique}");
    let experiment=format!("trace-http-{unique}");
    store.save_configuration("operator",&digest,json!({"fixture":true,"deployment":{"mode":"OBSERVE"}})).await.unwrap();
    let session=store.create_session("operator",&format!("trace-session-{unique}"),NewSession {
        network_id:"base-mainnet".into(),mode:"OBSERVE".into(),configuration_digest:digest.clone(),
        experiment_id:experiment.clone(),strategy_ids:vec!["cyclic-exact-in-2leg-v1".into()],
    }).await.unwrap();
    let settings=||ServerConfig {
        listen_address:"127.0.0.1:8080".parse().unwrap(),public_origin:ORIGIN.into(),allow_insecure_loopback:true,
        operator_secret_hash:hash(SECRET),configurations:vec![],
    };
    let app=router(AppState::new(store.clone(),settings()));
    let (cookie,_csrf)=authenticate(&app).await;
    let coverage_path=format!("/v1/decision-coverage?session_id={}",session.session_id);
    let empty=value(call(&app,Method::GET,&coverage_path,None,Some(&cookie),None,None,None).await).await;
    assert_eq!(empty["raw_observations"],"0");
    assert_eq!(empty["coverage_window_start_ms"],Value::Null);
    assert_eq!(empty["coverage_window_end_ms"],Value::Null);
    assert_eq!(empty["collection_completeness"],"UNKNOWN");
    assert_eq!(empty["eligible_attempts"],Value::Null);
    assert_eq!(empty["execution_accounting_available"],false);
    let worker=arb_control::ControlWorker::claim(store.clone(),"operator",&session.session_id,"base-mainnet",&format!("trace-worker-{unique}"),60).await.unwrap();
    worker.complete_recovery().await.unwrap();
    store.issue_command("operator",&session.session_id,&format!("start-trace-{unique}"),NewCommand {
        action:"START".into(),expected_revision:"0".into(),reason:None,
    }).await.unwrap();
    worker.tick(true).await.unwrap();
    let work=worker.generation().await.unwrap();
    let quote=http_trace_fixture(&session,&experiment,work.generation());
    // Explicit manually constructed fixtures: this tests durable admission and HTTP,
    // not raw capture authenticity, provider readiness or recorded live market data.
    for capture in &quote.capture_refs {
        worker.admit_capture_manifest(work,&capture.capture_id,&capture.manifest_digest,"/synthetic-http-fixture/manifest.json").await.unwrap();
    }
    let mut zero=quote.clone();zero.observed_at_unix_ms+=1;
    zero.result=DecisionResult::Quoted {quoted_output_minor:100_u64.into(),gross_delta_minor:"0".parse().unwrap(),included_pool_fees:vec![1_u64.into(),1_u64.into()]};
    let zero=zero.seal().unwrap();
    assert_eq!(quote.grouping.key,zero.grouping.key);
    let mut rejected=quote.clone();rejected.observed_at_unix_ms+=2;
    rejected.result=DecisionResult::Rejected{reason_codes:vec!["MATH_INPUT_REJECTED".into()]};
    let rejected=rejected.seal().unwrap();
    let mut no_route=quote.clone();no_route.observed_at_unix_ms+=3;no_route.route.clear();no_route.amount_in_minor=None;
    no_route.result=DecisionResult::NoRoute{reason_codes:vec!["NO_ELIGIBLE_POOL_PAIRS".into()]};
    let no_route=no_route.seal().unwrap();
    let mut unavailable=quote.clone();unavailable.observed_at_unix_ms+=4;unavailable.route.clear();unavailable.amount_in_minor=None;unavailable.input_age_ms=None;unavailable.capture_refs.clear();
    unavailable.result=DecisionResult::DataUnavailable{reason_codes:vec!["NO_CAPTURE_INPUTS".into()]};
    let unavailable=unavailable.seal().unwrap();
    let traces=vec![rejected.clone(),unavailable.clone(),quote.clone(),no_route,zero.clone()];
    let accepted=worker.admit_decision_traces(work,&traces).await.unwrap();
    let repeated=worker.admit_decision_traces(work,&traces).await.unwrap();
    assert_eq!(accepted.iter().map(|r|&r.trace_id).collect::<Vec<_>>(),repeated.iter().map(|r|&r.trace_id).collect::<Vec<_>>());
    let coverage=value(call(&app,Method::GET,&coverage_path,None,Some(&cookie),None,None,None).await).await;
    for (field,count) in [("raw_observations","5"),("quoted_candidates","2"),("rejected","1"),("no_route","1"),("data_unavailable","1"),("unique_opportunity_groups","1")] {
        assert_eq!(coverage[field],count,"{field}");
    }
    assert_eq!(coverage["eligible_attempts"],Value::Null);
    assert_eq!(coverage["reconciled_transactions"],Value::Null);
    assert_eq!(coverage["execution_accounting_available"],false);

    let mut cursor=None::<String>;let mut seen=std::collections::HashSet::new();
    for _ in 0..8 {
        let path=format!("/v1/opportunities?session_id={}&source_kind=SYNTHETIC_FIXTURE&limit=1{}",session.session_id,cursor.as_ref().map(|c|format!("&cursor={c}")).unwrap_or_default());
        let response=call(&app,Method::GET,&path,None,Some(&cookie),None,None,None).await;
        assert_eq!(response.status(),StatusCode::OK);
        let page=value(response).await;
        assert_eq!(page["items"].as_array().unwrap().len(),1,"eligible paging cannot produce empty intermediate pages");
        for item in page["items"].as_array().unwrap() {
            assert!(seen.insert(item["opportunity_id"].as_str().unwrap().to_owned()));
            assert_eq!(item["evidence_label"],"CANDIDATE");
            assert_eq!(item["dataset_origin"],"MANUALLY_CONSTRUCTED");
            assert_eq!(item["net_after_explicit_costs_minor"],Value::Null);
            assert_eq!(item["snapshot"]["age_ms"],17);
            assert!(item["start_asset_id"].as_str().unwrap().starts_with("fixture:base:"));
        }
        cursor=page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none(){break;}
    }
    assert!(cursor.is_none(),"bounded fixture pagination must terminate");
    assert_eq!(seen,std::collections::HashSet::from([quote.observation_id.clone(),zero.observation_id.clone()]));
    for filter in ["source_kind=CAPTURED_MARKET_DATA","evidence_label=SIMULATED","network_id=solana-mainnet"] {
        let filtered=value(call(&app,Method::GET,&format!("/v1/opportunities?session_id={}&{filter}",session.session_id),None,Some(&cookie),None,None,None).await).await;
        assert_eq!(filtered["items"],json!([]));
    }
    let detail=value(call(&app,Method::GET,&format!("/v1/decisions/{}",rejected.observation_id),None,Some(&cookie),None,None,None).await).await;
    assert_eq!(detail["trace"]["result"]["status"],"REJECTED");
    assert_eq!(detail["trace"]["observation_id"],rejected.observation_id);
    assert!(detail["recorded_at"].is_string());
    let mut trace_cursor=None::<String>;let mut observations=std::collections::HashSet::new();
    for _ in 0..8 {
        let path=format!("/v1/decisions?session_id={}&limit=2{}",session.session_id,trace_cursor.as_ref().map(|c|format!("&cursor={c}")).unwrap_or_default());
        let page=value(call(&app,Method::GET,&path,None,Some(&cookie),None,None,None).await).await;
        for item in page["items"].as_array().unwrap(){assert!(observations.insert(item["trace"]["observation_id"].as_str().unwrap().to_owned()));}
        trace_cursor=page["next_cursor"].as_str().map(str::to_owned);
        if trace_cursor.is_none(){break;}
    }
    assert_eq!(observations.len(),5);
    assert!(trace_cursor.is_none());
    let groups=value(call(&app,Method::GET,&format!("/v1/decision-groups?session_id={}&limit=1",session.session_id),None,Some(&cookie),None,None,None).await).await;
    assert_eq!(groups["items"].as_array().unwrap().len(),1);
    assert!(groups["next_cursor"].is_string());
    assert_eq!(groups["items"][0]["dataset_origin"],"MANUALLY_CONSTRUCTED");

    let other=format!("other-trace-{unique}");
    store.save_configuration(&other,&digest,json!({"fixture":true})).await.unwrap();
    let hidden_session=store.create_session(&other,&format!("hidden-trace-{unique}"),NewSession {
        network_id:"base-mainnet".into(),mode:"OBSERVE".into(),configuration_digest:digest,
        experiment_id:experiment.clone(),strategy_ids:vec!["cyclic-exact-in-2leg-v1".into()],
    }).await.unwrap();
    let hidden_worker=arb_control::ControlWorker::claim(store.clone(),&other,&hidden_session.session_id,"base-mainnet",&format!("hidden-trace-worker-{unique}"),60).await.unwrap();
    hidden_worker.complete_recovery().await.unwrap();
    store.issue_command(&other,&hidden_session.session_id,&format!("hidden-start-{unique}"),NewCommand {action:"START".into(),expected_revision:"0".into(),reason:None}).await.unwrap();
    hidden_worker.tick(true).await.unwrap();
    let hidden_work=hidden_worker.generation().await.unwrap();
    let mut hidden=unavailable;hidden.session_id=hidden_session.session_id.clone();hidden.generation=hidden_work.generation();
    let hidden=hidden.seal().unwrap();
    hidden_worker.admit_decision_traces(hidden_work,std::slice::from_ref(&hidden)).await.unwrap();
    for path in [
        format!("/v1/decisions/{}",hidden.observation_id),
        format!("/v1/decisions?session_id={}",hidden_session.session_id),
        format!("/v1/decision-groups?session_id={}",hidden_session.session_id),
        format!("/v1/decision-coverage?session_id={}",hidden_session.session_id),
        format!("/v1/opportunities?session_id={}",hidden_session.session_id),
    ] {
        assert_eq!(call(&app,Method::GET,&path,None,Some(&cookie),None,None,None).await.status(),StatusCode::NOT_FOUND,"{path}");
    }
    let restarted=router(AppState::new(store,settings()));
    let (new_cookie,_)=authenticate(&restarted).await;
    let restored=value(call(&restarted,Method::GET,&coverage_path,None,Some(&new_cookie),None,None,None).await).await;
    assert_eq!(restored,coverage);
}

#[test]
fn opportunity_examples_preserve_legacy_and_explicit_unknown_net() {
    for source in [
        include_str!("../../../specs/opportunity.example.json"),
        include_str!("../../../specs/opportunity.v1.1.example.json"),
    ] {
        let expected: Value=serde_json::from_str(source).unwrap();
        let record: arb_domain::OpportunityRecord=serde_json::from_str(source).unwrap();
        record.validate_research().unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(),expected);
    }
    let record: arb_domain::OpportunityRecord=serde_json::from_str(include_str!("../../../specs/opportunity.v1.1.example.json")).unwrap();
    assert!(record.net_after_explicit_costs_minor.is_none());
    assert_eq!(record.dataset_origin,Some(arb_domain::DatasetOrigin::Synthetic));
}
