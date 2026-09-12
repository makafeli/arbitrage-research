use super::*;
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
