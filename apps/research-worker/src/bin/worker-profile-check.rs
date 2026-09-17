//! Offline validation of an explicitly prepared Base profile; no database or RPC.
use arb_config::ValidatedConfig;
use arb_domain::{Mode, NetworkId};
use arb_registry::RegistryDocument;
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path, process::ExitCode};

fn read_small(path: &Path) -> Result<Vec<u8>, &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "PROFILE_FILE_UNAVAILABLE")?;
    if !meta.file_type().is_file() || meta.len() > 1024 * 1024 {
        return Err("PROFILE_FILE_REJECTED");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "PROFILE_FILE_UNAVAILABLE")?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "PROFILE_FILE_UNAVAILABLE")?;
    if bytes.len() > 1024 * 1024 {
        return Err("PROFILE_FILE_REJECTED");
    }
    Ok(bytes)
}

fn validate(configuration: &[u8], registry: &[u8]) -> Result<Value, &'static str> {
    let source = std::str::from_utf8(configuration).map_err(|_| "PROFILE_CONFIG_REJECTED")?;
    let config = ValidatedConfig::from_toml(source).map_err(|_| "PROFILE_CONFIG_REJECTED")?;
    let network = NetworkId::BaseMainnet;
    if config.mode() != Mode::Observe
        || !config.network_enabled(network)
        || config.network_enabled(NetworkId::SolanaMainnet)
        || config.capture_directory() != "/data/captures"
        || config.capture_quota_bytes() != 134_217_728
        || config.database_secret_reference().name() != "env:ARB_DATABASE_URL"
        || config.rpc_secret_reference(network).name() != "env:ARB_BASE_RPC_URL"
    {
        return Err("PROFILE_SCOPE_REJECTED");
    }
    let document =
        RegistryDocument::from_bytes(registry, network).map_err(|_| "PROFILE_REGISTRY_REJECTED")?;
    document
        .authorize(&config)
        .map_err(|_| "PROFILE_REGISTRY_UNAUTHORIZED")?;
    if document.pools().len() != 2 {
        return Err("PROFILE_POOL_COUNT_REJECTED");
    }
    Ok(json!({
        "status": "PROFILE_VALIDATED",
        "configuration_digest": config.digest(),
        "registry_digest": arb_capture::digest(registry),
        "network_id": "base-mainnet",
        "mode": "OBSERVE",
        "pool_count": 2,
        "provider_requests": 0,
        "database_requests": 0,
        "execution_authorized": false
    }))
}

fn run() -> Result<Value, &'static str> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 || args[0] != "--check" {
        return Err("PROFILE_CHECK_ARGUMENTS_REJECTED");
    }
    validate(
        &read_small(Path::new(&args[1]))?,
        &read_small(Path::new(&args[2]))?,
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!(
                "{}",
                json!({"status":"PROFILE_CHECK_BLOCKED","reason":reason})
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_template_is_not_an_enabled_profile() {
        let template = include_bytes!("../../../../config/research.example.toml");
        assert_eq!(validate(template, b"{}"), Err("PROFILE_SCOPE_REJECTED"));
    }

    #[test]
    fn malformed_configuration_is_redacted() {
        assert_eq!(
            validate(b"private-invalid-provider-value", b"{}"),
            Err("PROFILE_CONFIG_REJECTED")
        );
    }
}
