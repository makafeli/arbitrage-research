//! PostgreSQL-controlled OBSERVE/PAPER read-only capture and research runtime.
//! Captures feed bounded research-only route evaluation. Gross quotes never imply fills or executable profit.
use arb_adapter_api::{Chain, HttpReadRpc, ReadRpc, RpcRecord, StateContext};
use arb_capture::{CaptureManifest, MAX_BUNDLE_BYTES, Origin, file_digest, write_bundle};
use arb_config::ValidatedConfig;
use arb_control::{ControlWorker, WorkGeneration};
use arb_domain::{Mode, NetworkId};
use arb_registry::{PoolRegistry, RegistryDocument};
use arb_storage::{Store, StoreError};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    io::Read,
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
const EVALUATION_DEADLINE: Duration = Duration::from_secs(65);

struct EvaluationPayload {
    pools: Vec<arb_engine::CapturedPool>,
    observed_at_ms: u64,
}
struct ResearchGate {
    cancellation: arb_scheduler::Cancellation,
    started: Instant,
    generation: u64,
}
impl arb_engine::EvaluationGate for ResearchGate {
    fn state(&self) -> arb_engine::GateState {
        let now = Instant::now();
        arb_engine::GateState {
            generation: self.generation,
            admission_open: self.cancellation.check(now).is_ok(),
            now_monotonic_ms: u64::try_from(
                now.saturating_duration_since(self.started).as_millis(),
            )
            .unwrap_or(u64::MAX),
        }
    }
}
fn evaluate_blocking(
    permit: arb_scheduler::WorkPermit<EvaluationPayload, WorkGeneration>,
    config: Arc<ValidatedConfig>,
    session_id: String,
    experiment_id: String,
    strategy_id: String,
) -> Result<Vec<arb_domain::DecisionTrace>, AnyError> {
    let item = permit.item();
    let gate = ResearchGate {
        cancellation: permit.cancellation(),
        started: item.observed_at,
        generation: item.generation.generation(),
    };
    let origin = item
        .payload
        .pools
        .first()
        .ok_or("empty captured batch")?
        .origin;
    let request = arb_engine::EvaluationRequest {
        session_id: &session_id,
        experiment_id: &experiment_id,
        generation: item.generation.generation(),
        configuration: &config,
        strategy_id: &strategy_id,
        network_id: item.network,
        dataset_origin: origin,
        observed_at_unix_ms: item.payload.observed_at_ms,
        observed_monotonic_ms: 0,
        deadline_monotonic_ms: EVALUATION_DEADLINE.as_millis() as u64,
        pools: &item.payload.pools,
    };
    let traces = arb_engine::evaluate(&request, &gate)?;
    if permit.finish(Instant::now())?.is_err() {
        return Err("evaluation fenced or expired before persistence".into());
    }
    Ok(traces)
}

#[derive(Clone)]
enum Registry {
    Base(arb_evm::PoolRegistry),
    Solana(arb_solana::PoolRegistry),
}
impl Registry {
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
}

struct CapturePlan {
    registry: RegistryDocument,
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
    pool: arb_engine::CapturedPool,
}
struct CompletedBatch {
    captures: Vec<CompletedCapture>,
    started: Instant,
    observed_at_ms: u64,
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
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("configuration/registry exceeds 1 MiB".into());
    }
    Ok(bytes)
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

fn capture_blocking(plan: &CapturePlan) -> Result<CompletedBatch, AnyError> {
    let started = Instant::now();
    let observed = now_ms()?;
    // At most one blocking capture exists. Transport enforces a 60-second total
    // deadline as well as per-request timeout, response bytes and request count.
    let mut rpc = HttpReadRpc::new(
        &plan.endpoint,
        Duration::from_secs(5),
        8 * 1024 * 1024,
        4096,
    )?;
    let mut captures = Vec::new();
    for selected in plan.registry.pools() {
        let registry = match selected {
            PoolRegistry::Base(r) => Registry::Base(r.clone()),
            PoolRegistry::Solana(r) => Registry::Solana(r.clone()),
        };
        let pool_observed = now_ms()?;
        let (snapshot, context, coherent) = registry.capture(&mut rpc, pool_observed)?;
        let records: Vec<RpcRecord> = rpc.take_records();
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
            network: registry.chain(),
            provider_alias: plan.provider_alias.clone(),
            adapter_version: plan.registry.adapter_version().into(),
            adapter_source_commit: registry.source().into(),
            build_digest: plan.build_digest.clone(),
            config_digest: plan.config_digest.clone(),
            created_at_ms: pool_observed,
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
        let state = match selected {
            PoolRegistry::Base(r) => arb_engine::PoolState::Base {
                snapshot: serde_json::from_value(snapshot)?,
                registry: r.clone(),
            },
            PoolRegistry::Solana(r) => arb_engine::PoolState::Solana {
                snapshot: serde_json::from_value(snapshot)?,
                registry: r.clone(),
            },
        };
        let origin = match plan.origin {
            Origin::Synthetic => arb_domain::DatasetOrigin::Synthetic,
            Origin::ManuallyConstructed => arb_domain::DatasetOrigin::ManuallyConstructed,
            Origin::RecordedLive => arb_domain::DatasetOrigin::RecordedLive,
        };
        let pool = arb_engine::CapturedPool {
            capture: arb_domain::DecisionCaptureRef {
                capture_id: capture_id.clone(),
                manifest_digest: manifest_digest.clone(),
                snapshot_id: manifest_digest.clone(),
            },
            origin,
            configuration_digest: plan.config_digest.clone(),
            state,
        };
        captures.push(CompletedCapture {
            capture_id,
            manifest_digest,
            path,
            pool,
        });
    }
    Ok(CompletedBatch {
        captures,
        started,
        observed_at_ms: observed,
    })
}

async fn run() -> Result<(), AnyError> {
    let config = ValidatedConfig::from_toml(&String::from_utf8(read_small(&required_env(
        "ARB_WORKER_CONFIG",
    )?)?)?)?;
    if !matches!(config.mode(), Mode::Observe | Mode::Paper) {
        return Err("research-worker supports OBSERVE and PAPER research sessions only".into());
    }
    let operator = required_env("ARB_OPERATOR_ID")?;
    let session_id = required_env("ARB_SESSION_ID")?;
    let store = Store::connect(&resolve_reference(
        config.database_secret_reference().name(),
    )?)
    .await?;
    store.migrate().await?;
    let session = store.get_session(&operator, &session_id).await?;
    let experiment_id = store
        .get_session_experiment_id(&operator, &session_id)
        .await?;
    let expected_mode = match config.mode() {
        Mode::Observe => "OBSERVE",
        Mode::Paper => "PAPER",
        _ => return Err("unsupported research mode".into()),
    };
    if session.mode != expected_mode || session.configuration_digest != config.digest() {
        return Err("worker configuration or mode differs from immutable session".into());
    }
    let network: NetworkId = session.network_id.parse()?;
    let registry_bytes = read_small(&required_env("ARB_POOL_REGISTRY")?)?;
    let registry = RegistryDocument::from_bytes(&registry_bytes, network)?;
    registry.authorize(&config)?;
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
    let origin = match arb_adapter_api::endpoint_kind(&endpoint)? {
        arb_adapter_api::EndpointKind::LoopbackFixture => Origin::ManuallyConstructed,
        arb_adapter_api::EndpointKind::HttpsRemote => Origin::RecordedLive,
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
        json!({"event":"worker-ready","session_id":session_id,"mode":expected_mode,"state":"STOPPED","capability":"capture-and-candidate-research","quote_ready":false})
    );
    // This process owns one network and one capture/evaluation at a time.
    // Host-wide resource isolation across Railway services is a deployment concern.
    let scheduler = arb_scheduler::Scheduler::<EvaluationPayload, WorkGeneration>::new(
        arb_scheduler::Limits {
            queue_per_network: 1,
            in_flight_per_network: 1,
            global_in_flight: 2,
            stage_deadlines: [EVALUATION_DEADLINE; 6],
        },
    )?;
    let configuration = Arc::new(config);
    let mut evaluation: Option<JoinHandle<Result<Vec<arb_domain::DecisionTrace>, AnyError>>> = None;
    let mut evaluation_generation: Option<WorkGeneration> = None;
    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut job: Option<JoinHandle<Result<CompletedBatch, AnyError>>> = None;
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
                scheduler.fence(network)?;
                println!("{}",json!({"event":"shutdown-fenced","capture_inflight":job.is_some()}));
                if job.is_none() && evaluation.is_none() { break; }
            }
            _ = poll.tick() => {
                if evaluation.as_ref().is_some_and(JoinHandle::is_finished) {
                    let result = evaluation.take().expect("finished evaluation exists").await?;
                    let generation = evaluation_generation.take().expect("evaluation has generation");
                    match result {
                        Ok(traces) if !stopping => {
                            let admitted = match worker.admit_decision_traces(generation, &traces).await {
                                Ok(_) => true,
                                Err(StoreError::Conflict(_)) => false,
                                Err(error) => return Err(error.into()),
                            };
                            println!("{}", json!({"event":"decision-batch","count":traces.len(),"admitted":admitted,"evidence_ceiling":"CANDIDATE","full_transaction_simulation":false}));
                        }
                        _ => println!("{}",json!({"event":"decision-batch-discarded","reason":"FENCED_EXPIRED_OR_UNAVAILABLE"})),
                    }
                }
                if job.as_ref().is_some_and(JoinHandle::is_finished) {
                    let completed = job.take().expect("finished job exists").await?;
                    match completed {
                        Ok(batch) => {
                            last_good_capture=Some(Instant::now());
                            let generation=queued_generation.take();
                            let mut all_admitted=!stopping && generation.is_some();
                            for capture in &batch.captures {
                                let admission = if stopping { None } else if let Some(generation)=generation {
                                    match worker.admit_capture_manifest(generation,&capture.capture_id,&capture.manifest_digest,capture.path.to_str().ok_or("capture path is not UTF-8")?).await {
                                        Ok(update)=>update.attempt_id,
                                        Err(StoreError::Conflict(_))=>None,
                                        Err(error)=>return Err(error.into()),
                                    }
                                } else { None };
                                all_admitted &= admission.is_some();
                                println!("{}",json!({"event":"capture-written","capture_id":capture.capture_id,"manifest_digest":capture.manifest_digest,"admission":if admission.is_some(){"ADMITTED_RAW_CAPTURE"}else{"UNADMITTED_RAW_CAPTURE"},"research_attempt_id":admission,"quote_ready":false}));
                            }
                            scheduler.set_gate(network, worker.generation().await.ok())?;
                            if all_admitted {
                                let generation=generation.expect("admitted capture has generation");
                                let item=arb_scheduler::WorkItem {
                                    network, stage:arb_scheduler::Stage::Quote, generation,
                                    correlation_id:arb_scheduler::CorrelationId::new(&Uuid::new_v4().to_string())?,
                                    observed_at:batch.started,
                                    payload:EvaluationPayload {
                                        pools:batch.captures.into_iter().map(|c|c.pool).collect(),
                                        observed_at_ms:batch.observed_at_ms,
                                    },
                                };
                                if scheduler.try_enqueue(item,Instant::now())?.is_ok()
                                    && let Some(permit)=scheduler.dispatch(Instant::now())? {
                                    let config=Arc::clone(&configuration);
                                    let session_id=session_id.clone();
                                    let experiment_id=experiment_id.clone();
                                    let strategy=configuration.strategy_ids()[0].clone();
                                    evaluation_generation=Some(generation);
                                    evaluation=Some(tokio::task::spawn_blocking(move||evaluate_blocking(permit,config,session_id,experiment_id,strategy)));
                                }
                            }
                        }
                        Err(_) => {
                            // Transport details are intentionally absent; endpoint/credentials
                            // are never copied into logs. Full validation remains fail-closed.
                            last_good_capture=None;
                            println!("{}",json!({"event":"capture-failed","details":"capture unavailable or validation/quota failed","quote_ready":false}));
                        }
                    }
                    next_capture=Instant::now()+CAPTURE_INTERVAL;
                    if stopping && evaluation.is_none() { break; }
                }
                if stopping {
                    if job.is_none() && evaluation.is_none() { break; }
                    continue;
                }
                let ready=last_good_capture.is_some_and(|at|at.elapsed()<CAPTURE_READY_AGE);
                let update=match worker.tick(ready).await {
                    Ok(update)=>update,
                    Err(error)=>{ scheduler.fence(network)?; return Err(error.into()); }
                };
                scheduler.set_gate(network,worker.generation().await.ok())?;
                if update.session.observed_state=="FAULTED" {
                    worker.fence_local().await;
                    scheduler.fence(network)?;
                    return Err("capture readiness lost; worker faulted and requires explicit recovery".into());
                }
                if job.is_none() && evaluation.is_none() && Instant::now()>=next_capture {
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
        let registry = RegistryDocument::from_bytes(bytes, NetworkId::BaseMainnet).unwrap();
        assert!(registry.authorize(&config).is_err());
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
