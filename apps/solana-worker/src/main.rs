//! One-shot read-only capture command; not the durable session worker runtime.
use arb_adapter_api::{Chain, HttpReadRpc, RpcRecord, TranscriptRpc};
use arb_capture::{
    CaptureManifest, MAX_BUNDLE_BYTES, Origin, digest, file_digest, load_bundle, write_bundle,
};
use arb_config::ValidatedConfig;
use arb_domain::{Mode, NetworkId};
use arb_solana::{PoolRegistry, SOURCE_COMMIT, capture_pool};
use std::{
    error::Error,
    fs,
    path::Path,
    process::ExitCode,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn now_ms() -> Result<u64, Box<dyn Error>> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}
fn read_small(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if fs::metadata(path)?.len() > 1024 * 1024 {
        return Err("configuration or fixture input exceeds byte limit".into());
    }
    Ok(fs::read(path)?)
}
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "inspect") && (args.len() == 2 || args.len() == 3) {
        let bundle = load_bundle(
            Path::new(&args[1]),
            args.get(2).map(String::as_str),
            now_ms()?,
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"manifest":bundle.manifest,"manifest_digest":bundle.manifest_digest,"object_integrity_verified":true,"manifest_digest_matched":args.len()==3,"quote_ready":false})
            )?
        );
        return Ok(());
    }
    if args.first().is_none_or(|x| x != "record" && x != "fixture") || args.len() != 4 {
        return Err("Usage: solana-worker record CONFIG.toml REGISTRY.json NEW_CAPTURE_DIRECTORY | fixture REGISTRY.json TRANSCRIPT.json NEW_CAPTURE_DIRECTORY | inspect CAPTURE_DIRECTORY [EXPECTED_MANIFEST_SHA256]. One-shot read-only command, independent of dashboard session controls.".into());
    }
    let synthetic = args[0] == "fixture";
    let registry_bytes = read_small(if synthetic { &args[1] } else { &args[2] })?;
    let registry: PoolRegistry = serde_json::from_slice(&registry_bytes)?;
    registry.validate()?;
    let observed = now_ms()?;
    let (snapshot, records, origin, config_digest, config_bytes, provider, quota, retention_days) =
        if synthetic {
            let transcript_bytes = read_small(&args[2])?;
            let records: Vec<RpcRecord> = serde_json::from_slice(&transcript_bytes)?;
            let mut rpc = TranscriptRpc::new(records.clone());
            let snapshot = capture_pool(&mut rpc, &registry, observed)?;
            rpc.finish()?;
            (
                snapshot,
                records,
                Origin::ManuallyConstructed,
                digest(b"{\"mode\":\"offline-fixture\"}"),
                b"{\"mode\":\"offline-fixture\"}".to_vec(),
                "offline-fixture".to_string(),
                MAX_BUNDLE_BYTES,
                30_u16,
            )
        } else {
            let config_input = String::from_utf8(read_small(&args[1])?)?;
            let config = ValidatedConfig::from_toml(&config_input)?;
            let network = NetworkId::SolanaMainnet;
            if !config.network_enabled(network)
                || !matches!(config.mode(), Mode::Observe | Mode::Paper)
            {
                return Err("network disabled or mode does not permit read-only capture".into());
            }
            if config.registry_qualification_digest(network)
                != Some(digest(&registry_bytes).as_str())
            {
                return Err("capture registry digest differs from immutable configuration".into());
            }
            if !config
                .verified_pools(network)
                .iter()
                .any(|p| p.address() == registry.pool)
            {
                return Err("pool is not in configured allowlist".into());
            }
            for address in [registry.mint_a.as_str(), registry.mint_b.as_str()] {
                if !config
                    .verified_assets(network)
                    .iter()
                    .any(|a| a.address() == address)
                {
                    return Err("asset is not in configured allowlist".into());
                }
            }
            if config.expected_genesis_identity() != registry.expected_genesis_hash {
                return Err("registry genesis differs from immutable configuration".into());
            }
            let env_name = config
                .rpc_secret_reference(network)
                .name()
                .strip_prefix("env:")
                .ok_or("RPC environment reference is unconfigured")?;
            let endpoint = std::env::var(env_name)
                .map_err(|_| "RPC endpoint environment variable is missing")?;
            let mut rpc =
                HttpReadRpc::new(&endpoint, Duration::from_secs(15), 8 * 1024 * 1024, 4096)?;
            let snapshot = capture_pool(&mut rpc, &registry, observed)?;
            (
                snapshot,
                rpc.into_records(),
                Origin::RecordedLive,
                config.digest().to_owned(),
                config.effective_json().as_bytes().to_vec(),
                env_name.to_owned(),
                config.capture_quota_bytes().min(MAX_BUNDLE_BYTES),
                config.capture_retention_days(),
            )
        };
    if records.is_empty() {
        return Err("empty capture transcript".into());
    }
    let directory = Path::new(&args[3]);
    let capture_id = directory
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid capture directory name")?
        .to_owned();
    let manifest = CaptureManifest {
        schema_version: 1,
        capture_id,
        origin,
        network: Chain::SolanaMainnet,
        provider_alias: provider,
        adapter_version: "arb_solana-v1".into(),
        adapter_source_commit: SOURCE_COMMIT.into(),
        build_digest: file_digest(&std::env::current_exe()?)?,
        config_digest,
        created_at_ms: observed,
        raw_expires_at_ms: Some(
            observed
                .checked_add(u64::from(retention_days) * 86_400_000)
                .ok_or("retention overflow")?,
        ),
        context: snapshot.context.clone(),
        first_sequence: 0,
        last_sequence: records.len() as u64 - 1,
        required_inputs: vec![
            "pool-state".into(),
            "qualified-quote-range".into(),
            "qualified-quote-math".into(),
        ],
        missing_inputs: vec![
            "qualified-quote-range".into(),
            "qualified-quote-math".into(),
        ],
        coherent: snapshot.quality.coherent,
        complete_for_quote: false,
        objects: vec![],
    };
    let manifest_digest = write_bundle(
        directory,
        manifest,
        vec![
            ("rpc.json".into(), serde_json::to_vec_pretty(&records)?),
            (
                "snapshot.json".into(),
                serde_json::to_vec_pretty(&snapshot)?,
            ),
            ("registry.json".into(), registry_bytes),
            ("effective-config.json".into(), config_bytes),
        ],
        quota,
    )?;
    println!(
        "{}",
        serde_json::json!({"capture_manifest_digest":manifest_digest,"quote_ready":false,"origin":if synthetic {"manually-constructed"}else{"recorded-live"},"command":"one-shot-read-only-capture"})
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
