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
) -> Result<(SessionRecord, bool)> {
    let request = input(config);
    if let Some(session) = store
        .replay_session_creation(operator, CREATION_KEY, &request)
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
    if page.next_cursor.is_some() || page.items.iter().any(|s| s.network_id == "base-mainnet") {
        // A concurrent same-key registration may have committed between the reads.
        if let Some(session) = store
            .replay_session_creation(operator, CREATION_KEY, &request)
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
        .create_session(operator, CREATION_KEY, request)
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
    let (session, registration_requested) =
        register(&store, &operator, &config, args[0] == "--register").await?;
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
}
