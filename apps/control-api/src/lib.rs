//! Authenticated HTTP boundary. Command acceptance is never a worker acknowledgement.
use arb_storage::{
    CommandReceipt, NewCommand, NewSession, SessionPage, SessionRecord, Store, StoreError,
};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Extension, Path, Query, Request, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;

mod research;
mod support;
use research::{PaperAssetChoice, ResearchStore};

const MAX_INFLIGHT_REQUESTS: usize = 64;
// Bulk reads use at most half of Store::connect's eight database connections.
// They acquire this sub-budget before a global slot, preserving control capacity.
const MAX_INFLIGHT_READS: usize = 4;
const RESERVED_CONTROL_REQUESTS_PER_MINUTE: u32 = 60;
const REQUEST_TIMEOUT_SECONDS: u64 = 15;
const SESSION_TTL_SECONDS: u64 = 8 * 60 * 60;
const MAX_SESSIONS: usize = 8;
const LOGIN_ATTEMPTS_PER_MINUTE: u32 = 10;
const SESSION_REQUESTS_PER_MINUTE: u32 = 300;

#[derive(Clone, Serialize)]
pub struct RegisteredConfiguration {
    pub configuration_digest: String,
    pub mode: String,
    pub enabled_networks: Vec<String>,
    pub strategy_ids: Vec<String>,
    pub paper_assets: Vec<PaperAssetChoice>,
    #[serde(skip)]
    snapshot: serde_json::Value,
}

/// Secret material intentionally has no Debug implementation.
pub struct ServerConfig {
    pub listen_address: SocketAddr,
    pub public_origin: String,
    pub allow_insecure_loopback: bool,
    operator_secret_hash: [u8; 32],
    pub configurations: Vec<RegisteredConfiguration>,
    adapter_support: support::AdapterSupport,
}

impl ServerConfig {
    pub async fn register_configurations(&self, store: &Store) -> Result<(), StoreError> {
        for config in &self.configurations {
            store
                .save_configuration(
                    "operator",
                    &config.configuration_digest,
                    config.snapshot.clone(),
                )
                .await?;
        }
        Ok(())
    }
    pub fn from_env() -> Result<Self, String> {
        let secret =
            std::env::var("ARB_OPERATOR_SECRET").map_err(|_| "ARB_OPERATOR_SECRET is required")?;
        if secret.len() < 32 || secret.len() > 1024 {
            return Err("ARB_OPERATOR_SECRET must contain 32 through 1024 bytes".into());
        }
        let configured_origin = std::env::var("ARB_PUBLIC_ORIGIN").ok();
        let origin = configured_origin
            .clone()
            .unwrap_or_else(|| "https://localhost:8443".into());
        let insecure = match std::env::var("ARB_ALLOW_INSECURE_LOOPBACK").as_deref() {
            Ok("true") => true,
            Ok("false") | Err(_) => false,
            _ => return Err("ARB_ALLOW_INSECURE_LOOPBACK must be true or false".into()),
        };
        validate_origin(&origin, insecure)?;
        let port = std::env::var("ARB_API_PORT")
            .or_else(|_| std::env::var("PORT"))
            .unwrap_or_else(|_| "8080".into());
        let bind_ip = std::env::var("ARB_API_BIND_IP").unwrap_or_else(|_| "127.0.0.1".into());
        let listen_address =
            validate_listener(&bind_ip, &port, configured_origin.as_deref(), insecure)?;
        let files = std::env::var("ARB_CONFIG_FILES")
            .unwrap_or_else(|_| "config/research.example.toml".into());
        let mut configurations = Vec::new();
        let mut validated_configurations = Vec::new();
        for path in files.split(',').map(str::trim) {
            if path.is_empty() || configurations.len() >= 16 {
                return Err("ARB_CONFIG_FILES requires 1 through 16 paths".into());
            }
            let source = std::fs::read_to_string(path)
                .map_err(|_| "A registered configuration could not be read")?;
            let config = arb_config::ValidatedConfig::from_toml(&source)
                .map_err(|_| "A registered configuration failed validation")?;
            let mode = serde_json::to_value(config.mode())
                .map_err(|_| "Configuration mode invalid")?
                .as_str()
                .ok_or("Configuration mode invalid")?
                .to_owned();
            let enabled_networks = [
                arb_domain::NetworkId::BaseMainnet,
                arb_domain::NetworkId::SolanaMainnet,
            ]
            .into_iter()
            .filter(|network| config.network_enabled(*network))
            .map(|network| match network {
                arb_domain::NetworkId::BaseMainnet => "base-mainnet".to_owned(),
                arb_domain::NetworkId::SolanaMainnet => "solana-mainnet".to_owned(),
            })
            .collect();
            configurations.push(RegisteredConfiguration {
                configuration_digest: config.digest().to_owned(),
                mode,
                enabled_networks,
                strategy_ids: config.strategy_ids().to_vec(),
                paper_assets: paper_asset_choices(&config),
                snapshot: serde_json::from_str(config.effective_json())
                    .map_err(|_| "Configuration snapshot invalid")?,
            });
            validated_configurations.push(config);
        }
        let registries = support::load_from_env()?;
        let adapter_support = support::build(&validated_configurations, &registries)?;
        Ok(Self {
            adapter_support,
            listen_address,
            public_origin: origin,
            allow_insecure_loopback: insecure,
            operator_secret_hash: hash(&secret),
            configurations,
        })
    }
}

fn paper_asset_choices(config: &arb_config::ValidatedConfig) -> Vec<PaperAssetChoice> {
    [
        arb_domain::NetworkId::BaseMainnet,
        arb_domain::NetworkId::SolanaMainnet,
    ]
    .into_iter()
    .filter(|network| config.network_enabled(*network))
    .flat_map(|network| {
        std::iter::once(arb_paper::AccountingAsset::Native(network))
            .chain(
                config
                    .verified_assets(network)
                    .iter()
                    .cloned()
                    .map(arb_paper::AccountingAsset::Token),
            )
            .map(move |asset| PaperAssetChoice {
                network_id: network.to_string(),
                asset,
            })
    })
    .collect()
}

fn validate_listener(
    bind_ip: &str,
    port: &str,
    configured_origin: Option<&str>,
    insecure: bool,
) -> Result<SocketAddr, String> {
    let ip = bind_ip
        .parse::<IpAddr>()
        .map_err(|_| "ARB_API_BIND_IP must be an IP address")?;
    let port = port
        .parse::<u16>()
        .map_err(|_| "ARB_API_PORT or PORT must be an integer port")?;
    if port == 0 {
        return Err("API port must be nonzero".into());
    }
    if !ip.is_loopback() {
        let origin = configured_origin
            .ok_or("A non-loopback listener requires explicit ARB_PUBLIC_ORIGIN")?;
        if insecure {
            return Err(
                "An insecure development cookie is forbidden with a non-loopback listener".into(),
            );
        }
        validate_origin(origin, false)?;
    }
    Ok(SocketAddr::new(ip, port))
}

fn validate_origin(origin: &str, insecure: bool) -> Result<(), String> {
    let uri = origin
        .parse::<axum::http::Uri>()
        .map_err(|_| "ARB_PUBLIC_ORIGIN is invalid")?;
    let host = uri.host().ok_or("ARB_PUBLIC_ORIGIN must contain a host")?;
    if uri
        .authority()
        .is_some_and(|authority| authority.as_str().contains('@'))
        || uri.path() != "/"
        || uri.query().is_some()
        || origin.ends_with('/')
    {
        return Err("ARB_PUBLIC_ORIGIN must be a canonical origin without path, credentials, query, or trailing slash".into());
    }
    match uri.scheme_str() {
        Some("https") if !insecure => Ok(()),
        Some("http") if insecure && matches!(host, "127.0.0.1" | "localhost" | "[::1]") => Ok(()),
        _ => {
            Err("Use HTTPS; insecure development requires an explicit HTTP loopback origin".into())
        }
    }
}

#[async_trait]
trait ControlStore: ResearchStore + Send + Sync {
    async fn replay_session_creation(
        &self,
        operator: &str,
        key: &str,
        input: &NewSession,
    ) -> Result<Option<SessionRecord>, StoreError>;
    async fn health(&self) -> Result<(), StoreError>;
    async fn create_session(
        &self,
        operator: &str,
        key: &str,
        input: NewSession,
    ) -> Result<SessionRecord, StoreError>;
    async fn list_sessions(
        &self,
        operator: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionPage, StoreError>;
    async fn get_session(&self, operator: &str, session: &str)
    -> Result<SessionRecord, StoreError>;
    async fn issue_command(
        &self,
        operator: &str,
        session: &str,
        key: &str,
        input: NewCommand,
    ) -> Result<CommandReceipt, StoreError>;
    async fn get_command(
        &self,
        operator: &str,
        command: &str,
    ) -> Result<CommandReceipt, StoreError>;
}
#[async_trait]
impl ControlStore for Store {
    async fn replay_session_creation(
        &self,
        operator: &str,
        key: &str,
        input: &NewSession,
    ) -> Result<Option<SessionRecord>, StoreError> {
        self.replay_session_creation(operator, key, input).await
    }
    async fn health(&self) -> Result<(), StoreError> {
        self.health().await
    }
    async fn create_session(
        &self,
        operator: &str,
        key: &str,
        input: NewSession,
    ) -> Result<SessionRecord, StoreError> {
        self.create_session(operator, key, input).await
    }
    async fn list_sessions(
        &self,
        operator: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<SessionPage, StoreError> {
        self.list_sessions_page(operator, cursor, limit).await
    }
    async fn get_session(
        &self,
        operator: &str,
        session: &str,
    ) -> Result<SessionRecord, StoreError> {
        self.get_session(operator, session).await
    }
    async fn issue_command(
        &self,
        operator: &str,
        session: &str,
        key: &str,
        input: NewCommand,
    ) -> Result<CommandReceipt, StoreError> {
        self.issue_command(operator, session, key, input).await
    }
    async fn get_command(
        &self,
        operator: &str,
        command: &str,
    ) -> Result<CommandReceipt, StoreError> {
        self.get_command(operator, command).await
    }
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);
struct Inner {
    store: Arc<dyn ControlStore>,
    config: ServerConfig,
    auth: Mutex<AuthState>,
    sequence: AtomicU64,
    inflight: Arc<Semaphore>,
    reads: Arc<Semaphore>,
    exports: Semaphore,
}
#[derive(Default)]
struct AuthState {
    sessions: HashMap<[u8; 32], AuthSession>,
    login_window: u64,
    login_attempts: u32,
}
#[derive(Clone)]
struct AuthSession {
    csrf: String,
    expires_at: u64,
    request_window: u64,
    requests: u32,
    read_requests: u32,
}
#[derive(Clone)]
struct Identity {
    token_hash: [u8; 32],
    csrf: String,
    expires_at: u64,
}
#[derive(Clone)]
struct RequestId(String);
impl AppState {
    pub fn new(store: Store, config: ServerConfig) -> Self {
        Self::with_store(Arc::new(store), config)
    }
    fn with_store(store: Arc<dyn ControlStore>, config: ServerConfig) -> Self {
        Self(Arc::new(Inner {
            store,
            config,
            auth: Mutex::new(AuthState::default()),
            sequence: AtomicU64::new(1),
            inflight: Arc::new(Semaphore::new(MAX_INFLIGHT_REQUESTS)),
            reads: Arc::new(Semaphore::new(MAX_INFLIGHT_READS)),
            exports: Semaphore::new(2),
        }))
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { Json(serde_json::json!({"status":"OK","service":"control-api","trading_available":false})) }))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/session", get(auth_session))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/adapter-support", get(support::adapter_support))
        .route("/v1/sessions", get(list_sessions).post(create_session))
        .route("/v1/sessions/{session_id}", get(get_session))
        .route("/v1/sessions/{session_id}/cost-assessments", get(research::list_cost_assessments).post(research::create_cost_assessment))
        .route("/v1/sessions/{session_id}/cost-assessments/{record_id}", get(research::get_cost_assessment))
        .route("/v1/sessions/{session_id}/export", get(research::export_session))
        .route("/v1/sessions/{session_id}/collection-coverage", get(research::collection_coverage))
        .route("/v1/sessions/{session_id}/collection-attempts", get(research::collection_attempts))
        .route("/v1/sessions/{session_id}/commands", post(issue_command))
        .route("/v1/commands/{command_id}", get(get_command))
        .route("/v1/opportunities", get(list_opportunities))
        .route("/v1/decisions", get(research::list_decisions))
        .route("/v1/decisions/{observation_id}", get(research::get_decision))
        .route("/v1/decision-groups", get(research::decision_groups))
        .route("/v1/decision-coverage", get(research::decision_coverage))
        .route("/v1/sessions/{session_id}/paper-runs", get(research::list_paper_runs).post(research::create_paper_run))
        .route("/v1/paper-runs/{run_id}", get(research::get_paper_run))
        .route("/v1/paper-runs/{run_id}/journal", get(research::paper_journal))
        .route("/v1/paper-runs/{run_id}/reservations", get(research::paper_reservations))
        .fallback(fallback)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .with_state(state)
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
    request_id: String,
}
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: &'static str, id: &RequestId) -> Self {
        Self {
            status,
            body: ErrorBody {
                code,
                message,
                request_id: id.0.clone(),
            },
        }
    }
    fn invalid(id: &RequestId) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "INVALID_INPUT",
            "Request does not satisfy the API contract",
            id,
        )
    }
    fn unavailable(id: &RequestId) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "A required control dependency is unavailable",
            id,
        )
    }
    fn store(error: StoreError, id: &RequestId) -> Self {
        match error {
            StoreError::NotFound => Self::new(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Requested resource was not found",
                id,
            ),
            StoreError::Conflict(_) => Self::new(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Idempotency, revision, or lifecycle conflict",
                id,
            ),
            StoreError::InvalidInput(_) => Self::invalid(id),
            StoreError::ExportLimitExceeded => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "EXPORT_LIMIT_EXCEEDED",
                "Complete session export exceeds 10000 source rows or 8 MiB; no partial export was produced",
                id,
            ),
            StoreError::CapabilityUnavailable => Self::new(
                StatusCode::FORBIDDEN,
                "CAPABILITY_UNAVAILABLE",
                "This action is unavailable in research mode",
                id,
            ),
            StoreError::Database(_) | StoreError::Migration(_) | StoreError::CorruptState => {
                Self::unavailable(id)
            }
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.body)).into_response();
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        }
        response
    }
}
fn hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
fn random_token(id: &RequestId) -> Result<String, ApiError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| ApiError::unavailable(id))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn cookie(headers: &HeaderMap) -> Option<&str> {
    let values: Vec<_> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|part| part.trim().strip_prefix("arb_session="))
        .collect();
    if values.len() == 1
        && values[0].len() == 64
        && values[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        Some(values[0])
    } else {
        None
    }
}
fn matches_header(headers: &HeaderMap, name: &str, expected: &str) -> bool {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next().and_then(|value| value.to_str().ok()) else {
        return false;
    };
    values.next().is_none() && bool::from(hash(value).ct_eq(&hash(expected)))
}

/// Expensive/dashboard reads share a strict sub-budget. Cheap capability/auth
/// status and command receipts remain available to follow a control action.
/// Unknown read routes are conservative: they also consume the read budget.
fn bulk_read(request: &Request) -> bool {
    matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) && !matches!(
        request.uri().path(),
        "/healthz" | "/v1/health" | "/v1/capabilities" | "/v1/auth/session" | "/v1/adapter-support"
    ) && !request.uri().path().starts_with("/v1/commands/")
}

async fn security(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let id = RequestId(format!(
        "req-{}-{}",
        now(),
        state.0.sequence.fetch_add(1, Ordering::Relaxed)
    ));
    request.extensions_mut().insert(id.clone());
    let result = authorize(&state, &mut request, &id).and_then(|()| {
        // Try the read sub-budget first: refused reads cannot occupy a global
        // slot while waiting for capacity. Both owned permits release on cancel.
        let read = if bulk_read(&request) {
            Some(state.0.reads.clone().try_acquire_owned().map_err(|_| {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "CAPACITY_EXCEEDED",
                    "Read capacity is exhausted; control capacity is reserved",
                    &id,
                )
            })?)
        } else {
            None
        };
        let global = state.0.inflight.clone().try_acquire_owned().map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "CAPACITY_EXCEEDED",
                "Control request capacity is exhausted; retry later",
                &id,
            )
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
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&id.0).expect("generated request ID is a valid header"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}
fn authorize(state: &AppState, request: &mut Request, id: &RequestId) -> Result<(), ApiError> {
    if request.uri().path() == "/healthz" && request.method() == Method::GET {
        return Ok(());
    }
    let is_mutation = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if is_mutation && !matches_header(request.headers(), "origin", &state.0.config.public_origin) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "ORIGIN_DENIED",
            "Mutation origin is not allowed",
            id,
        ));
    }
    if request.uri().path() == "/v1/auth/login" && request.method() == Method::POST {
        return Ok(());
    }
    let token = cookie(request.headers()).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "AUTHENTICATION_REQUIRED",
            "Operator authentication is required",
            id,
        )
    })?;
    let token_hash = hash(token);
    let mut auth = state.0.auth.lock().map_err(|_| ApiError::unavailable(id))?;
    let time = now();
    auth.sessions.retain(|_, session| session.expires_at > time);
    let session = auth.sessions.get_mut(&token_hash).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "AUTHENTICATION_REQUIRED",
            "Operator authentication is required",
            id,
        )
    })?;
    if session.request_window != time / 60 {
        session.request_window = time / 60;
        session.requests = 0;
        session.read_requests = 0;
    }
    let is_read = bulk_read(request);
    if session.requests >= SESSION_REQUESTS_PER_MINUTE
        || (is_read
            && session.read_requests
                >= SESSION_REQUESTS_PER_MINUTE - RESERVED_CONTROL_REQUESTS_PER_MINUTE)
    {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Request rate limit exceeded; retry after a minute",
            id,
        ));
    }
    // A denied bulk read does not consume the reserve, even if a client retries
    // aggressively. The overall session limit remains 300 authorized attempts.
    session.requests += 1;
    if is_read {
        session.read_requests += 1;
    }
    if is_mutation && !matches_header(request.headers(), "x-csrf-token", &session.csrf) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "CSRF_DENIED",
            "A valid CSRF token is required",
            id,
        ));
    }
    request.extensions_mut().insert(Identity {
        token_hash,
        csrf: session.csrf.clone(),
        expires_at: session.expires_at,
    });
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    operator_secret: String,
}
#[derive(Serialize)]
struct AuthResponse {
    operator_id: &'static str,
    csrf_token: String,
    expires_at: String,
}
fn auth_response(identity: Identity) -> AuthResponse {
    AuthResponse {
        operator_id: "operator",
        csrf_token: identity.csrf,
        expires_at: identity.expires_at.to_string(),
    }
}
async fn login(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    body: Result<Json<Login>, JsonRejection>,
) -> Result<Response, ApiError> {
    let time = now();
    {
        let mut auth = state
            .0
            .auth
            .lock()
            .map_err(|_| ApiError::unavailable(&id))?;
        if auth.login_window != time / 60 {
            auth.login_window = time / 60;
            auth.login_attempts = 0;
        }
        auth.login_attempts = auth.login_attempts.saturating_add(1);
        if auth.login_attempts > LOGIN_ATTEMPTS_PER_MINUTE {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Login rate limit exceeded; retry after a minute",
                &id,
            ));
        }
    }
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    if input.operator_secret.len() > 1024
        || !bool::from(hash(&input.operator_secret).ct_eq(&state.0.config.operator_secret_hash))
    {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "AUTHENTICATION_FAILED",
            "Operator authentication failed",
            &id,
        ));
    }
    let token = random_token(&id)?;
    let csrf = random_token(&id)?;
    let expires_at = time + SESSION_TTL_SECONDS;
    {
        let mut auth = state
            .0
            .auth
            .lock()
            .map_err(|_| ApiError::unavailable(&id))?;
        auth.sessions.retain(|_, session| session.expires_at > time);
        if auth.sessions.len() >= MAX_SESSIONS {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "SESSION_LIMIT",
                "Operator session limit reached",
                &id,
            ));
        }
        auth.sessions.insert(
            hash(&token),
            AuthSession {
                csrf: csrf.clone(),
                expires_at,
                request_window: time / 60,
                requests: 0,
                read_requests: 0,
            },
        );
    }
    let secure = if state.0.config.allow_insecure_loopback {
        ""
    } else {
        "; Secure"
    };
    let value = format!(
        "arb_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_TTL_SECONDS}{secure}"
    );
    let mut response = Json(auth_response(Identity {
        token_hash: hash(&token),
        csrf,
        expires_at,
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).map_err(|_| ApiError::unavailable(&id))?,
    );
    Ok(response)
}
async fn auth_session(Extension(identity): Extension<Identity>) -> Json<AuthResponse> {
    Json(auth_response(identity))
}
async fn logout(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    Extension(identity): Extension<Identity>,
) -> Result<Response, ApiError> {
    state
        .0
        .auth
        .lock()
        .map_err(|_| ApiError::unavailable(&id))?
        .sessions
        .remove(&identity.token_hash);
    let secure = if state.0.config.allow_insecure_loopback {
        ""
    } else {
        "; Secure"
    };
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "arb_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}"
        ))
        .map_err(|_| ApiError::unavailable(&id))?,
    );
    Ok(response)
}
async fn health(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Readiness checks durable storage; process liveness is a separate endpoint.
    state
        .0
        .store
        .health()
        .await
        .map_err(|error| ApiError::store(error, &id))?;
    Ok(Json(
        serde_json::json!({"status":"OK", "version":env!("CARGO_PKG_VERSION")}),
    ))
}
async fn capabilities(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"modes":["OBSERVE","PAPER","REPLAY"], "live_execution":false, "market_data":false, "opportunity_capture":false, "decision_history":true, "collection_telemetry":true, "session_export":true, "cost_assessments":true, "adapter_support":true, "paper_ledger":true, "paper_run_creation":true, "command_application":"WORKER_ACK_REQUIRED", "registered_configurations":state.0.config.configurations}),
    )
}
fn idempotency(headers: &HeaderMap, id: &RequestId) -> Result<String, ApiError> {
    let mut values = headers.get_all("idempotency-key").iter();
    let value = values
        .next()
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::invalid(id))?;
    if values.next().is_some()
        || !(16..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(ApiError::invalid(id));
    }
    Ok(value.to_owned())
}
async fn create_session(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    headers: HeaderMap,
    body: Result<Json<NewSession>, JsonRejection>,
) -> Result<(StatusCode, Json<SessionRecord>), ApiError> {
    let key = idempotency(&headers, &id)?;
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    if input.mode == "LIVE" {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "CAPABILITY_UNAVAILABLE",
            "LIVE sessions are unavailable in research mode",
            &id,
        ));
    }
    if let Some(existing) = state
        .0
        .store
        .replay_session_creation("operator", &key, &input)
        .await
        .map_err(|error| ApiError::store(error, &id))?
    {
        return Ok((StatusCode::CREATED, Json(existing)));
    }
    let config = state
        .0
        .config
        .configurations
        .iter()
        .find(|config| config.configuration_digest == input.configuration_digest)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "UNKNOWN_CONFIGURATION",
                "Configuration digest is not registered",
                &id,
            )
        })?;
    if input.mode != config.mode
        || !config.enabled_networks.contains(&input.network_id)
        || input.strategy_ids.is_empty()
        || input.strategy_ids.len() > 16
        || input
            .strategy_ids
            .iter()
            .any(|strategy| !config.strategy_ids.contains(strategy))
        || input.experiment_id.is_empty()
        || input.experiment_id.len() > 128
        || input.experiment_id.chars().any(char::is_control)
    {
        return Err(ApiError::invalid(&id));
    }
    let session = state
        .0
        .store
        .create_session("operator", &key, input)
        .await
        .map_err(|error| ApiError::store(error, &id))?;
    Ok((StatusCode::CREATED, Json(session)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pagination {
    cursor: Option<String>,
    limit: Option<u32>,
}
fn validate_page(query: &Pagination, id: &RequestId) -> Result<u32, ApiError> {
    let limit = query.limit.unwrap_or(25);
    if !(1..=100).contains(&limit)
        || query
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 128)
    {
        return Err(ApiError::invalid(id));
    }
    Ok(limit)
}
async fn list_sessions(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    query: Result<Query<Pagination>, QueryRejection>,
) -> Result<Json<SessionPage>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = validate_page(&query, &id)?;
    state
        .0
        .store
        .list_sessions("operator", query.cursor.as_deref(), limit)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
async fn get_session(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<SessionRecord>, ApiError> {
    let Path(session) = path.map_err(|_| ApiError::invalid(&id))?;
    state
        .0
        .store
        .get_session("operator", &session)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
async fn issue_command(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    body: Result<Json<NewCommand>, JsonRejection>,
) -> Result<(StatusCode, Json<CommandReceipt>), ApiError> {
    let Path(session) = path.map_err(|_| ApiError::invalid(&id))?;
    let key = idempotency(&headers, &id)?;
    let Json(input) = body.map_err(|_| ApiError::invalid(&id))?;
    if input.action == "DISARM" {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "CAPABILITY_UNAVAILABLE",
            "DISARM is unavailable in research mode",
            &id,
        ));
    }
    let receipt = state
        .0
        .store
        .issue_command("operator", &session, &key, input)
        .await
        .map_err(|error| ApiError::store(error, &id))?;
    Ok((StatusCode::ACCEPTED, Json(receipt)))
}
async fn get_command(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    path: Result<Path<String>, PathRejection>,
) -> Result<Json<CommandReceipt>, ApiError> {
    let Path(command) = path.map_err(|_| ApiError::invalid(&id))?;
    state
        .0
        .store
        .get_command("operator", &command)
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpportunityQuery {
    session_id: Option<String>,
    network_id: Option<arb_domain::NetworkId>,
    evidence_label: Option<arb_domain::Evidence>,
    source_kind: Option<arb_domain::SourceKind>,
    cursor: Option<String>,
    limit: Option<u32>,
}
async fn list_opportunities(
    State(state): State<AppState>,
    Extension(id): Extension<RequestId>,
    query: Result<Query<OpportunityQuery>, QueryRejection>,
) -> Result<Json<arb_storage::OpportunityPage>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::invalid(&id))?;
    let limit = validate_page(
        &Pagination {
            cursor: query.cursor.clone(),
            limit: query.limit,
        },
        &id,
    )?;
    if query
        .session_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.len() > 128)
    {
        return Err(ApiError::invalid(&id));
    }
    state
        .0
        .store
        .opportunities(
            "operator",
            arb_storage::OpportunityFilter {
                session_id: query.session_id,
                network_id: query.network_id,
                evidence_label: query.evidence_label,
                source_kind: query.source_kind,
            },
            query.cursor.as_deref(),
            limit,
        )
        .await
        .map(Json)
        .map_err(|error| ApiError::store(error, &id))
}
async fn method_not_allowed(Extension(id): Extension<RequestId>) -> ApiError {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "METHOD_NOT_ALLOWED",
        "HTTP method is not allowed for this resource",
        &id,
    )
}
async fn fallback(Extension(id): Extension<RequestId>) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
        "Requested resource was not found",
        &id,
    )
}

#[cfg(test)]
mod tests;
