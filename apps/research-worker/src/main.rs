//! OBSERVE-only PostgreSQL-controlled read-only capture runtime.
//! Raw input captures are never opportunities, simulations, paper fills, or live trades.
use arb_adapter_api::{Chain, HttpReadRpc, ReadRpc, RpcRecord, StateContext};
use arb_capture::{CaptureManifest, MAX_BUNDLE_BYTES, Origin, digest, file_digest, write_bundle};
use arb_config::ValidatedConfig;
use arb_control::{ControlWorker, WorkGeneration};
use arb_domain::{Mode, NetworkId};
use arb_storage::{Store, StoreError};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::task::JoinHandle;
use uuid::Uuid;

type AnyError = Box<dyn Error + Send + Sync>;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const CAPTURE_INTERVAL: Duration = Duration::from_secs(5);
const CAPTURE_READY_AGE: Duration = Duration::from_secs(90);

#[derive(Clone)]
enum Registry {
    Base(arb_evm::PoolRegistry),
    Solana(arb_solana::PoolRegistry),
}
impl Registry {
    fn load(bytes: &[u8], network: NetworkId, config: &ValidatedConfig) -> Result<Self, AnyError> {
        if !config.network_enabled(network) || config.mode() != Mode::Observe {
            return Err("research-worker requires an enabled OBSERVE network".into());
        }
        if config.registry_qualification_digest(network) != Some(digest(bytes).as_str()) {
            return Err("registry digest differs from immutable configuration".into());
        }
        let (registry, pool, assets) = match network {
            NetworkId::BaseMainnet => {
                let r: arb_evm::PoolRegistry = serde_json::from_slice(bytes)?;
                r.validate()?;
                let pool = r.pool.to_ascii_lowercase();
                let assets = [r.token0.to_ascii_lowercase(), r.token1.to_ascii_lowercase()];
                (Self::Base(r), pool, assets)
            }
            NetworkId::SolanaMainnet => {
                let r: arb_solana::PoolRegistry = serde_json::from_slice(bytes)?;
                r.validate()?;
                if r.expected_genesis_hash != config.expected_genesis_identity() {
                    return Err("registry genesis differs from immutable configuration".into());
                }
                let pool = r.pool.clone();
                let assets = [r.mint_a.clone(), r.mint_b.clone()];
                (Self::Solana(r), pool, assets)
            }
        };
        let canonical = |s: &str| match network {
            NetworkId::BaseMainnet => s.to_ascii_lowercase(),
            NetworkId::SolanaMainnet => s.to_owned(),
        };
        if !config
            .verified_pools(network)
            .iter()
            .any(|p| canonical(p.address()) == pool)
            || assets.iter().any(|asset| {
                !config
                    .verified_assets(network)
                    .iter()
                    .any(|a| canonical(a.address()) == *asset)
            })
        {
            return Err("registry pool or asset is not in the immutable allowlist".into());
        }
        Ok(registry)
    }
    fn capture(
        &self,
        rpc: &mut impl ReadRpc,
        observed: u64,
    ) -> Result<(Value, StateContext, bool), AnyError> {
        match self {
            Self::Base(registry) => {
                let snapshot = arb_evm::capture_pool(rpc, registry, observed)?;
                Ok((
                    serde_json::to_value(&snapshot)?,
                    snapshot.context,
                    snapshot.quality.coherent,
                ))
            }
            Self::Solana(registry) => {
                let snapshot = arb_solana::capture_pool(rpc, registry, observed)?;
                Ok((
                    serde_json::to_value(&snapshot)?,
                    snapshot.context,
                    snapshot.quality.coherent,
                ))
            }
        }
    }
    fn chain(&self) -> Chain {
        match self {
            Self::Base(_) => Chain::BaseMainnet,
            Self::Solana(_) => Chain::SolanaMainnet,
        }
    }
    fn source(&self) -> &'static str {
        match self {
            Self::Base(_) => arb_evm::SOURCE_COMMIT,
            Self::Solana(_) => arb_solana::SOURCE_COMMIT,
        }
    }
    fn adapter(&self) -> &'static str {
        match self {
            Self::Base(_) => "arb_evm-v1",
            Self::Solana(_) => "arb_solana-v1",
        }
    }
}

struct CapturePlan {
    registry: Registry,
    origin: Origin,
    registry_bytes: Vec<u8>,
    config_bytes: Vec<u8>,
    config_digest: String,
    endpoint: String,
    provider_alias: String,
    build_digest: String,
    root: PathBuf,
    quota_bytes: u64,
    retention_days: u16,
}
struct CompletedCapture {
    capture_id: String,
    manifest_digest: String,
    path: PathBuf,
}

fn now_ms() -> Result<u64, AnyError> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}
fn read_small(path: &str) -> Result<Vec<u8>, AnyError> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len() > 1024 * 1024 {
        return Err("configuration/registry must be a regular file at most 1 MiB".into());
    }
    Ok(fs::read(path)?)
}
fn required_env(name: &str) -> Result<String, AnyError> {
    std::env::var(name)
        .map_err(|_| format!("required environment reference {name} is missing").into())
}
fn resolve_reference(reference: &str) -> Result<String, AnyError> {
    let name = reference
        .strip_prefix("env:")
        .ok_or("environment reference is unconfigured")?;
    required_env(name)
}

/// Bound the scan and reject symlinks. The initial deployment owns one capture volume
/// with one worker process; quota is shared across its retained old run directories.
fn directory_bytes(root: &Path) -> Result<u64, AnyError> {
    let mut paths = vec![(root.to_owned(), 0)];
    let mut entries = 0_u32;
    let mut total = 0_u64;
    while let Some((path, depth)) = paths.pop() {
        if depth > 4 {
            return Err("capture directory nesting exceeds supported layout".into());
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            entries = entries
                .checked_add(1)
                .ok_or("capture directory count overflow")?;
            if entries > 100_000 {
                return Err("capture directory entry quota exceeded".into());
            }
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.file_type().is_symlink() {
                return Err("capture volume contains symlink".into());
            }
            if meta.is_dir() {
                paths.push((entry.path(), depth + 1));
            } else if meta.is_file() {
                total = total
                    .checked_add(meta.len())
                    .ok_or("capture byte count overflow")?;
            } else {
                return Err("capture volume contains unsupported file type".into());
            }
        }
    }
    Ok(total)
}

fn capture_blocking(plan: &CapturePlan) -> Result<CompletedCapture, AnyError> {
    let observed = now_ms()?;
    // At most one blocking capture exists. Transport enforces a 60-second total
    // deadline as well as per-request timeout, response bytes and request count.
    let mut rpc = HttpReadRpc::new(
        &plan.endpoint,
        Duration::from_secs(5),
        8 * 1024 * 1024,
        4096,
    )?;
    let (snapshot, context, coherent) = plan.registry.capture(&mut rpc, observed)?;
    let records: Vec<RpcRecord> = rpc.into_records();
    if records.is_empty() {
        return Err("capture returned no recorded inputs".into());
    }
    let capture_id = Uuid::new_v4().to_string();
    let path = plan.root.join(&capture_id);
    let remaining = plan
        .quota_bytes
        .checked_sub(directory_bytes(&plan.root)?)
        .filter(|q| *q > 0)
        .ok_or("capture volume quota exhausted")?;
    let manifest = CaptureManifest {
        schema_version: 1,
        capture_id: capture_id.clone(),
        origin: plan.origin.clone(),
        network: plan.registry.chain(),
        provider_alias: plan.provider_alias.clone(),
        adapter_version: plan.registry.adapter().into(),
        adapter_source_commit: plan.registry.source().into(),
        build_digest: plan.build_digest.clone(),
        config_digest: plan.config_digest.clone(),
        created_at_ms: observed,
        raw_expires_at_ms: Some(
            observed
                .checked_add(u64::from(plan.retention_days) * 86_400_000)
                .ok_or("retention overflow")?,
        ),
        context,
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
        coherent,
        complete_for_quote: false,
        objects: vec![],
    };
    let manifest_digest = write_bundle(
        &path,
        manifest,
        vec![
            ("rpc.json".into(), serde_json::to_vec(&records)?),
            ("snapshot.json".into(), serde_json::to_vec(&snapshot)?),
            ("registry.json".into(), plan.registry_bytes.clone()),
            ("effective-config.json".into(), plan.config_bytes.clone()),
        ],
        remaining.min(MAX_BUNDLE_BYTES),
    )?;
    Ok(CompletedCapture {
        capture_id,
        manifest_digest,
        path,
    })
}

async fn run() -> Result<(), AnyError> {
    let config = ValidatedConfig::from_toml(&String::from_utf8(read_small(&required_env(
        "ARB_WORKER_CONFIG",
    )?)?)?)?;
    if config.mode() != Mode::Observe {
        return Err("research-worker supports OBSERVE sessions only".into());
    }
    let operator = required_env("ARB_OPERATOR_ID")?;
    let session_id = required_env("ARB_SESSION_ID")?;
    let store = Store::connect(&resolve_reference(
        config.database_secret_reference().name(),
    )?)
    .await?;
    store.migrate().await?;
    let session = store.get_session(&operator, &session_id).await?;
    if session.mode != "OBSERVE" || session.configuration_digest != config.digest() {
        return Err("worker configuration or mode differs from immutable session".into());
    }
    let network: NetworkId = session.network_id.parse()?;
    let registry_bytes = read_small(&required_env("ARB_POOL_REGISTRY")?)?;
    let registry = Registry::load(&registry_bytes, network, &config)?;
    let reference = config.rpc_secret_reference(network).name();
    let endpoint = resolve_reference(reference)?;
    let provider_alias = reference
        .strip_prefix("env:")
        .ok_or("unconfigured RPC reference")?
        .to_owned();
    let root = PathBuf::from(config.capture_directory());
    fs::create_dir_all(&root)?;
    if !fs::symlink_metadata(&root)?.file_type().is_dir() {
        return Err("capture root must be a directory".into());
    }
    let root = root.canonicalize()?;
    if directory_bytes(&root)? >= config.capture_quota_bytes() {
        return Err("capture volume quota exhausted".into());
    }
    // HTTP transport is accepted only for loopback fixtures by HttpReadRpc.
    // Such captures must never be labelled recorded market input.
    let origin = if endpoint.starts_with("http://") {
        Origin::ManuallyConstructed
    } else {
        Origin::RecordedLive
    };
    let plan = Arc::new(CapturePlan {
        registry,
        origin,
        registry_bytes,
        config_bytes: config.effective_json().as_bytes().to_vec(),
        config_digest: config.digest().into(),
        endpoint,
        provider_alias,
        build_digest: file_digest(&std::env::current_exe()?)?,
        root,
        quota_bytes: config.capture_quota_bytes(),
        retention_days: config.capture_retention_days(),
    });
    let worker = ControlWorker::claim(
        store.clone(),
        &operator,
        &session_id,
        network.as_str(),
        &Uuid::new_v4().to_string(),
        15,
    )
    .await?;
    worker.complete_recovery().await?;
    println!(
        "{}",
        json!({"event":"worker-ready","session_id":session_id,"mode":"OBSERVE","state":"STOPPED","capability":"raw-read-only-capture","quote_ready":false})
    );
    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut job: Option<JoinHandle<Result<CompletedCapture, AnyError>>> = None;
    let mut queued_generation: Option<WorkGeneration> = None;
    let mut last_good_capture: Option<Instant> = None;
    let mut next_capture = Instant::now();
    let mut stopping = false;
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown, if !stopping => {
                // Close local admission immediately. An already started HTTP request
                // cannot be recalled; its eventual artifact remains unadmitted.
                stopping = true;
                worker.fence_local().await;
                println!("{}",json!({"event":"shutdown-fenced","capture_inflight":job.is_some()}));
                if job.is_none() { break; }
            }
            _ = poll.tick() => {
                if job.as_ref().is_some_and(JoinHandle::is_finished) {
                    let completed = job.take().expect("finished job exists").await?;
                    match completed {
                        Ok(capture) => {
                            last_good_capture=Some(Instant::now());
                            let admission = if stopping { None } else if let Some(generation)=queued_generation.take() {
                                match worker.admit_capture_manifest(generation,&capture.capture_id,&capture.manifest_digest,capture.path.to_str().ok_or("capture path is not UTF-8")?).await {
                                    Ok(update)=>update.attempt_id,
                                    Err(StoreError::Conflict(_))=>None,
                                    Err(error)=>return Err(error.into()),
                                }
                            } else { None };
                            println!("{}",json!({"event":"capture-written","capture_id":capture.capture_id,"manifest_digest":capture.manifest_digest,"admission":if admission.is_some(){"ADMITTED_RAW_CAPTURE"}else{"UNADMITTED_RAW_CAPTURE"},"research_attempt_id":admission,"quote_ready":false}));
                        }
                        Err(_) => {
                            // Transport details are intentionally absent; endpoint/credentials
                            // are never copied into logs. Full validation remains fail-closed.
                            last_good_capture=None;
                            println!("{}",json!({"event":"capture-failed","details":"capture unavailable or validation/quota failed","quote_ready":false}));
                        }
                    }
                    next_capture=Instant::now()+CAPTURE_INTERVAL;
                    if stopping { break; }
                }
                if stopping { continue; }
                let ready=last_good_capture.is_some_and(|at|at.elapsed()<CAPTURE_READY_AGE);
                let update=worker.tick(ready).await?;
                if update.session.observed_state=="FAULTED" {
                    worker.fence_local().await;
                    return Err("capture readiness lost; worker faulted and requires explicit recovery".into());
                }
                if job.is_none() && Instant::now()>=next_capture {
                    queued_generation=worker.generation().await.ok();
                    let task_plan=Arc::clone(&plan);
                    job=Some(tokio::task::spawn_blocking(move||capture_blocking(&task_plan)));
                }
            }
        }
    }
    Ok(())
}
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            eprintln!("research-worker runtime unavailable");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(run());
    // An in-flight read is bounded by the transport deadline. Shutdown does not
    // claim network cancellation or admit late results into a stopped session.
    runtime.shutdown_timeout(Duration::from_secs(65));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("research-worker: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_adapter_api::{RpcRecord, TranscriptRpc};

    #[test]
    fn disabled_config_cannot_qualify_worker_or_registry() {
        let config =
            ValidatedConfig::from_toml(include_str!("../../../config/research.example.toml"))
                .unwrap();
        let bytes = include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json");
        assert!(Registry::load(bytes, NetworkId::BaseMainnet, &config).is_err());
    }

    #[test]
    fn capture_dispatch_preserves_raw_context_without_quote_claims() {
        let registry = Registry::Base(
            serde_json::from_str(include_str!(
                "../../../crates/arb-evm/tests/fixtures/registry.json"
            ))
            .unwrap(),
        );
        let records: Vec<RpcRecord> = serde_json::from_str(include_str!(
            "../../../crates/arb-evm/tests/fixtures/rpc.json"
        ))
        .unwrap();
        let mut rpc = TranscriptRpc::new(records);
        let (snapshot, context, coherent) = registry.capture(&mut rpc, 100).unwrap();
        rpc.finish().unwrap();
        assert!(coherent);
        assert_eq!(snapshot["quality"]["complete_for_quote"], false);
        assert!(matches!(context, StateContext::Evm { .. }));
        assert_eq!(registry.chain(), Chain::BaseMainnet);
    }

    #[test]
    fn quota_includes_preexisting_captures_and_rejects_symlinks() {
        let root = std::env::temp_dir().join(format!("arb-worker-quota-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let old = root.join("old");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("raw.json"), b"12345").unwrap();
        assert_eq!(directory_bytes(&root).unwrap(), 5);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(old.join("raw.json"), root.join("link")).unwrap();
            assert!(directory_bytes(&root).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
