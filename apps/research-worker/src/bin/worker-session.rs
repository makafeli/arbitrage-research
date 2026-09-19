//! Explicit registration of one anchored Base OBSERVE session. Never starts research.
use arb_config::ValidatedConfig;
use arb_domain::{Mode, NetworkId};
use arb_registry::RegistryDocument;
use arb_storage::{NewSession, SessionRecord, Store, StoreError};
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use std::{fs, io::Read, path::Path, process::ExitCode, str::FromStr, time::Duration};

const CREATION_KEY: &str = "railway-base-profile-v1";
const EXPERIMENT: &str = "railway-base-observe-v1";
type Result<T> = std::result::Result<T, &'static str>;

fn scope(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

/// Read `ARB_BASE_GENERATION`: absent = 1 (today's single stream); `2`..`99`
/// (ASCII digits, no leading zero, no whitespace) selects a later generation
/// succeeding a HALTED source (#193). Anything else is a fixed rejection.
fn generation_from_env(raw: Option<&str>) -> Result<u64> {
    let Some(raw) = raw else { return Ok(1) };
    let digits = !raw.is_empty() && raw.len() <= 2 && raw.bytes().all(|b| b.is_ascii_digit());
    if !digits || raw.starts_with('0') {
        return Err("GENERATION_REJECTED");
    }
    raw.parse().map_err(|_| "GENERATION_REJECTED")
}

/// Idempotency key for a given generation: the original key for generation 1,
/// unchanged; `<key>.g<N>` for a later generation. Pure, never touches storage.
fn session_key(generation: u64) -> String {
    if generation == 1 {
        CREATION_KEY.to_owned()
    } else {
        format!("{CREATION_KEY}.g{generation}")
    }
}

/// An observed lifecycle state (`arb_domain::lifecycle::State`, SCREAMING_SNAKE_CASE)
/// that is safely not running: no admission gate can be currently open.
fn is_non_running_state(observed_state: &str) -> bool {
    matches!(observed_state, "STOPPED" | "FAULTED")
}

/// True when every `base-mainnet` session among `sessions` is in a non-running
/// observed state (vacuously true when there is none). RUNNING or any starting/
/// stopping transition (RECOVERING, PAUSING, PAUSED, DRAINING) makes this false.
fn base_sessions_all_non_running(sessions: &[SessionRecord]) -> bool {
    sessions
        .iter()
        .filter(|session| session.network_id == "base-mainnet")
        .all(|session| is_non_running_state(&session.observed_state))
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    let info = fs::symlink_metadata(path).map_err(|_| "PROFILE_UNAVAILABLE")?;
    if !info.file_type().is_file() || info.len() > 1_048_576 {
        return Err("PROFILE_FILE_REJECTED");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "PROFILE_UNAVAILABLE")?
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| "PROFILE_UNAVAILABLE")?;
    if bytes.len() > 1_048_576 {
        return Err("PROFILE_FILE_REJECTED");
    }
    Ok(bytes)
}

fn profile(root: &Path, expected: &str) -> Result<ValidatedConfig> {
    if !fs::symlink_metadata(root)
        .map_err(|_| "PROFILE_UNAVAILABLE")?
        .file_type()
        .is_dir()
    {
        return Err("PROFILE_DIRECTORY_REJECTED");
    }
    if expected.len() != 71
        || !expected.starts_with("sha256:")
        || !expected[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("PROFILE_ANCHOR_REQUIRED");
    }
    let bytes = read_file(&root.join("configuration.toml"))?;
    let source = std::str::from_utf8(&bytes).map_err(|_| "PROFILE_CONFIG_REJECTED")?;
    let config = ValidatedConfig::from_toml(source).map_err(|_| "PROFILE_CONFIG_REJECTED")?;
    if config.digest() != expected {
        return Err("PROFILE_ANCHOR_MISMATCH");
    }
    if config.mode() != Mode::Observe
        || !config.network_enabled(NetworkId::BaseMainnet)
        || config.network_enabled(NetworkId::SolanaMainnet)
        || config.capture_directory() != "/data/captures"
        || config.capture_quota_bytes() != 134_217_728
        || config.database_secret_reference().name() != "env:ARB_DATABASE_URL"
        || config.rpc_secret_reference(NetworkId::BaseMainnet).name() != "env:ARB_BASE_RPC_URL"
    {
        return Err("PROFILE_SCOPE_REJECTED");
    }
    let registry = RegistryDocument::from_bytes(
        &read_file(&root.join("registry.json"))?,
        NetworkId::BaseMainnet,
    )
    .map_err(|_| "PROFILE_REGISTRY_REJECTED")?;
    registry
        .authorize(&config)
        .map_err(|_| "PROFILE_REGISTRY_UNAUTHORIZED")?;
    if registry.pools().len() != 2 {
        return Err("PROFILE_POOL_COUNT_REJECTED");
    }
    Ok(config)
}

fn input(config: &ValidatedConfig) -> NewSession {
    NewSession {
        network_id: "base-mainnet".into(),
        mode: "OBSERVE".into(),
        configuration_digest: config.digest().into(),
        experiment_id: EXPERIMENT.into(),
        strategy_ids: config.strategy_ids().to_vec(),
    }
}

fn storage_error(error: StoreError) -> &'static str {
    match error {
        StoreError::Conflict(_) => "SESSION_REGISTRATION_CONFLICT",
        StoreError::NotFound => "SESSION_NOT_FOUND",
        _ => "SESSION_STORAGE_UNAVAILABLE",
    }
}

async fn register(
    store: &Store,
    operator: &str,
    config: &ValidatedConfig,
    write: bool,
    generation: u64,
) -> Result<(SessionRecord, bool)> {
    let request = input(config);
    let key = session_key(generation);
    if let Some(session) = store
        .replay_session_creation(operator, &key, &request)
        .await
        .map_err(storage_error)?
    {
        return Ok((session, false));
    }
    if !write {
        return Err("BASE_SESSION_NOT_REGISTERED");
    }
    // Never silently adopt or duplicate an earlier manually created Base session.
    let page = store
        .list_sessions_page(operator, None, 100)
        .await
        .map_err(storage_error)?;
    // Generation 1 keeps the exact original rule: any existing base-mainnet session
    // blocks a new registration. A later generation only needs every existing
    // base-mainnet session to be safely non-running (#193) — it never reuses,
    // resets or touches those sessions or their streams.
    let blocked = page.next_cursor.is_some()
        || if generation == 1 {
            page.items.iter().any(|s| s.network_id == "base-mainnet")
        } else {
            !base_sessions_all_non_running(&page.items)
        };
    if blocked {
        // A concurrent same-key registration may have committed between the reads.
        if let Some(session) = store
            .replay_session_creation(operator, &key, &request)
            .await
            .map_err(storage_error)?
        {
            return Ok((session, false));
        }
        return Err("EXISTING_BASE_SESSION_REQUIRES_SELECTION");
    }
    let snapshot =
        serde_json::from_str(config.effective_json()).map_err(|_| "PROFILE_CONFIG_REJECTED")?;
    store
        .save_configuration(operator, config.digest(), snapshot)
        .await
        .map_err(storage_error)?;
    // Store serializes equal idempotency keys and rejects changed payloads. A failed
    // session write may retain the immutable configuration, never a partial session.
    let session = store
        .create_session(operator, &key, request)
        .await
        .map_err(storage_error)?;
    Ok((session, true))
}

async fn run() -> Result<Value> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Ok(
            json!({"status":"NOT_STARTED","database_requests":0,"provider_requests":0,"execution_authorized":false}),
        );
    }
    if args.len() != 2 || !matches!(args[0].as_str(), "--register" | "--status") {
        return Err("SESSION_ARGUMENTS_REJECTED");
    }
    let generation = generation_from_env(std::env::var("ARB_BASE_GENERATION").ok().as_deref())?;
    let operator = std::env::var("ARB_OPERATOR_ID").map_err(|_| "OPERATOR_REQUIRED")?;
    if !scope(&operator) {
        return Err("OPERATOR_REJECTED");
    }
    let expected =
        std::env::var("ARB_BASE_PROFILE_DIGEST").map_err(|_| "PROFILE_ANCHOR_REQUIRED")?;
    let config = profile(Path::new(&args[1]), &expected)?;
    let url = std::env::var("ARB_DATABASE_URL").map_err(|_| "DATABASE_REQUIRED")?;
    let options = PgConnectOptions::from_str(&url)
        .map_err(|_| "DATABASE_SETTING_REJECTED")?
        .ssl_mode(PgSslMode::VerifyFull);
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|_| "DATABASE_UNAVAILABLE")?;
    let store = Store::from_pool(pool);
    // Existing deployed migrations only. Registration never implicitly migrates.
    let (session, registration_requested) = register(
        &store,
        &operator,
        &config,
        args[0] == "--register",
        generation,
    )
    .await?;
    Ok(json!({
        "status":"BASE_SESSION_REGISTERED",
        "session":session,
        "registration_requested":registration_requested,
        "worker_started":false,
        "provider_requests":0,
        "execution_authorized":false
    }))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let result = tokio::time::timeout(Duration::from_secs(20), run()).await;
    match result {
        Ok(Ok(value)) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        other => {
            let reason = match other {
                Ok(Err(reason)) => reason,
                _ => "SESSION_OUTCOME_UNCERTAIN",
            };
            eprintln!(
                "{}",
                json!({"status":"BASE_SESSION_BLOCKED","reason":reason,"worker_started":false,"execution_authorized":false})
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_scope_is_bounded_and_not_a_url() {
        assert!(scope("operator"));
        assert!(scope("ci:base-1"));
        for value in ["", "https://secret.invalid/key", "a/b", "a\nb"] {
            assert!(!scope(value));
        }
        assert!(!scope(&"x".repeat(129)));
    }

    #[test]
    fn storage_failures_do_not_echo_internal_diagnostics() {
        assert_eq!(
            storage_error(StoreError::Conflict("sensitive")),
            "SESSION_REGISTRATION_CONFLICT"
        );
        assert_eq!(
            storage_error(StoreError::InvalidInput("sensitive")),
            "SESSION_STORAGE_UNAVAILABLE"
        );
    }

    #[test]
    fn generation_absent_or_explicit_one_is_generation_one() {
        assert_eq!(generation_from_env(None), Ok(1));
        assert_eq!(generation_from_env(Some("1")), Ok(1));
    }

    #[test]
    fn generation_two_to_ninety_nine_parses_without_leading_zero() {
        assert_eq!(generation_from_env(Some("2")), Ok(2));
        assert_eq!(generation_from_env(Some("99")), Ok(99));
    }

    #[test]
    fn generation_rejects_zero_leading_zero_overflow_and_whitespace() {
        for raw in ["0", "01", "100", "x", " 2", "2 ", ""] {
            assert_eq!(generation_from_env(Some(raw)), Err("GENERATION_REJECTED"));
        }
    }

    #[test]
    fn session_key_is_unchanged_for_generation_one_and_suffixed_afterward() {
        assert_eq!(session_key(1), CREATION_KEY);
        assert_eq!(session_key(2), format!("{CREATION_KEY}.g2"));
        assert_eq!(session_key(99), format!("{CREATION_KEY}.g99"));
    }

    fn session(network_id: &str, observed_state: &str) -> SessionRecord {
        SessionRecord {
            session_id: "session".into(),
            network_id: network_id.into(),
            mode: "OBSERVE".into(),
            observed_state: observed_state.into(),
            health: "OK".into(),
            desired_revision: "0".into(),
            applied_revision: "0".into(),
            outstanding_attempts: 0,
            execution_authorized: false,
            last_heartbeat_at: None,
            configuration_digest: "sha256:0".into(),
        }
    }

    #[test]
    fn empty_session_list_allows_a_new_base_session() {
        assert!(base_sessions_all_non_running(&[]));
    }

    #[test]
    fn all_stopped_or_faulted_base_sessions_allow_a_new_one() {
        let sessions = [
            session("base-mainnet", "STOPPED"),
            session("base-mainnet", "FAULTED"),
        ];
        assert!(base_sessions_all_non_running(&sessions));
    }

    #[test]
    fn one_running_base_session_refuses_a_new_one() {
        let sessions = [
            session("base-mainnet", "STOPPED"),
            session("base-mainnet", "RUNNING"),
        ];
        assert!(!base_sessions_all_non_running(&sessions));
    }

    #[test]
    fn a_starting_or_stopping_transition_refuses_a_new_session() {
        for observed_state in ["RECOVERING", "PAUSING", "PAUSED", "DRAINING"] {
            let sessions = [session("base-mainnet", observed_state)];
            assert!(!base_sessions_all_non_running(&sessions));
        }
    }

    #[test]
    fn a_non_base_network_session_never_blocks_a_base_registration() {
        let sessions = [session("solana-mainnet", "RUNNING")];
        assert!(base_sessions_all_non_running(&sessions));
    }
}
