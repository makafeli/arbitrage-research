//! PostgreSQL-controlled OBSERVE/PAPER read-only capture and research runtime.
//! Captures feed bounded research-only route evaluation. Gross quotes never imply fills or executable profit.
mod capture_source;
mod managed_ingestion;
mod pipeline_metrics;
mod stage_metrics;
use pipeline_metrics::{Component, PipelineMetrics, persistence};

use arb_adapter_api::{Chain, HttpReadRpc, ReadRpc, RpcRecord, StateContext};
use arb_capture::{CaptureManifest, MAX_BUNDLE_BYTES, Origin, file_digest, write_bundle};
use arb_config::ValidatedConfig;
use arb_control::{ControlWorker, WorkGeneration};
use arb_domain::{Mode, NetworkId};
use arb_registry::{PoolRegistry, RegistryDocument};
use arb_storage::{
    CollectionFinish, CollectionOutcome, CollectionPurpose, CollectionReason, Store, StoreError,
};
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

/// Only bounded public codes cross the acquisition/evaluation boundary. A raw
/// provider response, endpoint or operating-system error is never persisted.
#[derive(Clone, Copy, Debug)]
struct AttemptFailure {
    outcome: CollectionOutcome,
    reason: CollectionReason,
    captured_pools: u32,
    ingestion_halt: Option<arb_storage::IngestionHalt>,
}
impl AttemptFailure {
    fn acquisition(reason: CollectionReason, captured_pools: usize) -> Self {
        Self {
            outcome: if reason == CollectionReason::AcquisitionDeadline {
                CollectionOutcome::DeadlineExceeded
            } else {
                CollectionOutcome::AcquisitionFailed
            },
            reason,
            captured_pools: captured_pools as u32,
            ingestion_halt: None,
        }
    }
    fn evaluation(reason: arb_scheduler::DropReason, captured_pools: u32) -> Self {
        use arb_scheduler::DropReason;
        let (outcome, reason) = match reason {
            DropReason::GateClosed | DropReason::GenerationChanged | DropReason::Cancelled => (
                CollectionOutcome::Suppressed,
                CollectionReason::GenerationFenced,
            ),
            DropReason::DeadlineExpired => (
                CollectionOutcome::DeadlineExceeded,
                CollectionReason::EvaluationDeadline,
            ),
            DropReason::QueueFull => (
                CollectionOutcome::EvaluationFailed,
                CollectionReason::ResourceLimit,
            ),
            DropReason::FutureTimestamp | DropReason::Abandoned => (
                CollectionOutcome::EvaluationFailed,
                CollectionReason::EvaluationRejected,
            ),
        };
        Self {
            outcome,
            reason,
            captured_pools,
            ingestion_halt: None,
        }
    }
}

struct CollectionWork {
    id: String,
    generation: Option<WorkGeneration>,
    started: Instant,
    captured_pools: u32,
    metrics: Arc<PipelineMetrics>,
    correlation: Uuid,
    ingestion_cursor: Option<arb_storage::IngestionCursor>,
}
impl CollectionWork {
    fn finish(
        &self,
        outcome: CollectionOutcome,
        reason: Option<CollectionReason>,
    ) -> CollectionFinish {
        CollectionFinish {
            outcome,
            reason,
            captured_pools: self.captured_pools,
            elapsed_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        }
    }
}

/// Remember whether the adapter failed inside a provider call or while validating
/// successful input. The wrapped transport already has request/byte/deadline bounds.
struct AcquisitionRpc {
    inner: HttpReadRpc,
    failure: Option<CollectionReason>,
    metrics: Arc<PipelineMetrics>,
    correlation: Uuid,
    rpc_elapsed: Duration,
}
impl ReadRpc for AcquisitionRpc {
    fn call(
        &mut self,
        method: arb_adapter_api::ReadMethod,
        params: Value,
    ) -> arb_adapter_api::Result<Value> {
        let started = Instant::now();
        let result = self.inner.call(method, params).inspect_err(|error| {
            self.failure = Some(match error.0 {
                "capture RPC deadline exceeded" => CollectionReason::AcquisitionDeadline,
                "RPC request quota exhausted" | "RPC response or capture exceeds byte quota" => {
                    CollectionReason::ResourceLimit
                }
                _ => CollectionReason::ProviderUnavailable,
            });
        });
        let elapsed = started.elapsed();
        self.rpc_elapsed = self.rpc_elapsed.saturating_add(elapsed);
        self.metrics.record(
            Component::Rpc,
            self.correlation,
            Some(elapsed),
            result.is_ok(),
        );
        result
    }
}

struct EvaluationPayload {
    metrics: Arc<PipelineMetrics>,
    correlation: Uuid,
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
) -> Result<Vec<arb_domain::DecisionTrace>, AttemptFailure> {
    let item = permit.item();
    let gate = ResearchGate {
        cancellation: permit.cancellation(),
        started: item.observed_at,
        generation: item.generation.generation(),
    };
    let captured_pools = item.payload.pools.len() as u32;
    let rejected = || AttemptFailure {
        outcome: CollectionOutcome::EvaluationFailed,
        reason: CollectionReason::EvaluationRejected,
        captured_pools,
        ingestion_halt: None,
    };
    if let Err(reason) = gate.cancellation.check(Instant::now()) {
        return Err(AttemptFailure::evaluation(reason, captured_pools));
    }
    let origin = item.payload.pools.first().ok_or_else(rejected)?.origin;
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
    let measurement = item
        .payload
        .metrics
        .span(Component::Evaluation, item.payload.correlation);
    let evaluated = arb_engine::evaluate(&request, &gate);
    measurement.finish(evaluated.is_ok());
    let traces = evaluated.map_err(|error| {
        // The engine's generic admission gate also closes on deadline. Preserve
        // the scheduler's exact reason instead of labelling expiry as STOP.
        if let Err(reason) = gate.cancellation.check(Instant::now()) {
            AttemptFailure::evaluation(reason, captured_pools)
        } else if error.code == "DEADLINE_EXPIRED" {
            AttemptFailure::evaluation(arb_scheduler::DropReason::DeadlineExpired, captured_pools)
        } else if error.code == "WORK_GENERATION_CANCELLED" {
            AttemptFailure::evaluation(arb_scheduler::DropReason::GenerationChanged, captured_pools)
        } else {
            rejected()
        }
    })?;
    match permit.finish(Instant::now()).map_err(|_| rejected())? {
        Ok(_) => Ok(traces),
        Err(rejected) => Err(AttemptFailure::evaluation(rejected.reason, captured_pools)),
    }
}

#[derive(Clone)]
enum Registry {
    Base,
    Solana,
}
impl Registry {
    fn chain(&self) -> Chain {
        match self {
            Self::Base => Chain::BaseMainnet,
            Self::Solana => Chain::SolanaMainnet,
        }
    }
    fn source(&self) -> &'static str {
        match self {
            Self::Base => arb_evm::SOURCE_COMMIT,
            Self::Solana => arb_solana::SOURCE_COMMIT,
        }
    }
}

/// Decoding stays atomic across a pool set. Legacy single-pool captures preserve
/// their original RPC ordering and adapter identity for offline compatibility.
fn capture_document(
    document: &RegistryDocument,
    rpc: &mut impl ReadRpc,
    observed: u64,
    chain_time_enabled: bool,
) -> Result<Vec<(Value, StateContext, bool)>, AnyError> {
    let batch = document.format() == arb_registry::DocumentFormat::PoolSetV1;
    match document.network() {
        NetworkId::BaseMainnet => {
            let pools = document
                .pools()
                .iter()
                .map(|pool| match pool {
                    PoolRegistry::Base(pool) => Ok(pool.clone()),
                    _ => Err("mixed registry networks"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let snapshots = if batch {
                arb_evm::capture_pools(rpc, &pools, observed)?
            } else {
                vec![arb_evm::capture_pool(rpc, &pools[0], observed)?]
            };
            snapshots
                .into_iter()
                .map(|snapshot| {
                    Ok((
                        serde_json::to_value(&snapshot)?,
                        snapshot.context,
                        snapshot.quality.coherent,
                    ))
                })
                .collect()
        }
        NetworkId::SolanaMainnet => {
            let pools = document
                .pools()
                .iter()
                .map(|pool| match pool {
                    PoolRegistry::Solana(pool) => Ok(pool.clone()),
                    _ => Err("mixed registry networks"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let snapshots = if chain_time_enabled {
                arb_solana::capture_pools_with_chain_time(rpc, &pools, observed)?
            } else if batch {
                arb_solana::capture_pools(rpc, &pools, observed)?
            } else {
                vec![arb_solana::capture_pool(rpc, &pools[0], observed)?]
            };
            snapshots
                .into_iter()
                .map(|snapshot| {
                    Ok((
                        serde_json::to_value(&snapshot)?,
                        snapshot.context,
                        snapshot.quality.coherent,
                    ))
                })
                .collect()
        }
    }
}

fn capture_adapter_version(document: &RegistryDocument, chain_time_enabled: bool) -> &'static str {
    if chain_time_enabled && document.network() == NetworkId::SolanaMainnet {
        "arb_solana-pool-set-v3"
    } else {
        document.adapter_version()
    }
}

struct CapturePlan {
    metrics: Arc<PipelineMetrics>,
    registry: RegistryDocument,
    chain_time_enabled: bool,
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
    ingestion: Option<arb_evm::backfill::BackfillBatch>,
    // True once the managed source has walked all the way to this capture's own
    // anchor block. A bounded catch-up batch that only partially closes a long
    // finalized step leaves this false; the caller must not admit that capture.
    source_caught_up: bool,
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

fn capture_blocking(
    plan: &CapturePlan,
    correlation: Uuid,
    ingestion_cursor: Option<&arb_storage::IngestionCursor>,
) -> Result<CompletedBatch, AttemptFailure> {
    let started = Instant::now();
    let observed = now_ms()
        .map_err(|_| AttemptFailure::acquisition(CollectionReason::AcquisitionUnavailable, 0))?;
    // At most one blocking capture exists. Transport enforces a 60-second total
    // deadline as well as per-request timeout, response bytes and request count.
    let mut rpc = AcquisitionRpc {
        inner: HttpReadRpc::new(
            &plan.endpoint,
            Duration::from_secs(5),
            8 * 1024 * 1024,
            4096,
        )
        .map_err(|_| AttemptFailure::acquisition(CollectionReason::AcquisitionUnavailable, 0))?,
        failure: None,
        metrics: Arc::clone(&plan.metrics),
        correlation,
        rpc_elapsed: Duration::ZERO,
    };
    // Pool-set v2 acquires and validates every member before writing artifacts.
    // Every bundle retains the actual complete batch transcript, including the
    // common anchor/union response. Never synthesize per-pool provider responses.
    let decoding_started = Instant::now();
    let decoded = capture_document(&plan.registry, &mut rpc, observed, plan.chain_time_enabled);
    // Calls are serial and fully inside this interval. Exclude their measured
    // time from decode/context validation; missing subtraction is unknown.
    plan.metrics.record(
        Component::SnapshotDecode,
        correlation,
        decoding_started.elapsed().checked_sub(rpc.rpc_elapsed),
        decoded.is_ok(),
    );
    let snapshots = decoded.map_err(|_| {
        AttemptFailure::acquisition(
            rpc.failure
                .unwrap_or(CollectionReason::InputValidationFailed),
            0,
        )
    })?;
    let records: Vec<RpcRecord> = rpc.inner.take_records();
    if records.is_empty() {
        return Err(AttemptFailure::acquisition(
            CollectionReason::InputValidationFailed,
            0,
        ));
    }
    let transcript_bytes = serde_json::to_vec(&records)
        .map_err(|_| AttemptFailure::acquisition(CollectionReason::InputValidationFailed, 0))?;
    if transcript_bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err(AttemptFailure::acquisition(
            CollectionReason::ResourceLimit,
            0,
        ));
    }
    // Keep the original quote transcript unchanged. Recovery uses the SAME
    // transport and its cumulative request, byte, pacing and 60-second budget.
    // A failed recovery writes no pool artifacts or partial ingestion batch.
    let ingestion = ingestion_cursor
        .map(|cursor| managed_ingestion::recover(plan, &snapshots, cursor, &mut rpc))
        .transpose()?;
    // Every snapshot shares one anchor block (recover() already validates that).
    // A bounded catch-up batch that stops short of it must not be admitted.
    let source_caught_up = ingestion.as_ref().is_none_or(|batch| {
        snapshots.first().is_some_and(|(_, context, _)| {
            matches!(context, StateContext::Evm { block_number, .. }
                if *block_number == batch.through.number)
        })
    });
    let mut captures = Vec::new();
    for (selected, (snapshot, context, coherent)) in plan.registry.pools().iter().zip(snapshots) {
        let registry = match selected {
            PoolRegistry::Base(_) => Registry::Base,
            PoolRegistry::Solana(_) => Registry::Solana,
        };
        let mut artifact_reason = CollectionReason::CaptureStorageUnavailable;
        let measurement = plan.metrics.span(Component::Persistence, correlation);
        let result = (|| -> Result<CompletedCapture, AnyError> {
            let capture_id = Uuid::new_v4().to_string();
            let path = plan.root.join(&capture_id);
            let used = directory_bytes(&plan.root)?;
            let remaining = plan
                .quota_bytes
                .checked_sub(used)
                .filter(|q| *q > 0)
                .ok_or_else(|| {
                    artifact_reason = CollectionReason::ResourceLimit;
                    "capture volume quota exhausted"
                })?;
            let manifest = CaptureManifest {
                schema_version: 1,
                capture_id: capture_id.clone(),
                origin: plan.origin.clone(),
                network: registry.chain(),
                provider_alias: plan.provider_alias.clone(),
                adapter_version: capture_adapter_version(&plan.registry, plan.chain_time_enabled)
                    .into(),
                adapter_source_commit: registry.source().into(),
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
                    ("rpc.json".into(), transcript_bytes.clone()),
                    ("snapshot.json".into(), serde_json::to_vec(&snapshot)?),
                    ("registry.json".into(), plan.registry_bytes.clone()),
                    ("effective-config.json".into(), plan.config_bytes.clone()),
                ],
                remaining.min(MAX_BUNDLE_BYTES),
            )
            .inspect_err(|error| {
                if matches!(
                    error.0,
                    "invalid capture quota"
                        | "capture byte quota exceeded"
                        | "missing objects or object quota exceeded"
                ) {
                    artifact_reason = CollectionReason::ResourceLimit;
                }
            })?;
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
            Ok(CompletedCapture {
                capture_id,
                manifest_digest,
                path,
                pool,
            })
        })();
        measurement.finish(result.is_ok());
        captures.push(
            result.map_err(|_| AttemptFailure::acquisition(artifact_reason, captures.len()))?,
        );
    }
    Ok(CompletedBatch {
        captures,
        ingestion,
        source_caught_up,
        started,
        observed_at_ms: observed,
    })
}

fn is_generation_fence(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::Conflict(
            "stale or fenced collection decisions"
                | "decision generation is fenced"
                | "stale or fenced capture"
                | "stale or fenced capture binding"
                | "capture generation is fenced"
                | "worker lease lost"
        )
    )
}

async fn finish_collection(
    worker: &ControlWorker,
    collection: &CollectionWork,
    outcome: CollectionOutcome,
    reason: Option<CollectionReason>,
) -> Result<(), StoreError> {
    persistence(
        &collection.metrics,
        collection.correlation,
        worker.finish_collection_attempt(&collection.id, collection.finish(outcome, reason)),
    )
    .await?;
    println!(
        "{}",
        json!({"event":"collection-finished","collection_attempt_id":collection.id,"outcome":outcome,"reason":reason,"captured_pools":collection.captured_pools})
    );
    Ok(())
}

async fn run() -> Result<(), AnyError> {
    let ingestion_stream = capture_source::setting()?;
    let managed_ingestion = managed_ingestion::enabled(ingestion_stream.as_deref())?;
    let metrics_enabled = stage_metrics::enabled()?;
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
    let pipeline = PipelineMetrics::new(network);
    let plan = Arc::new(CapturePlan {
        metrics: Arc::clone(&pipeline),
        chain_time_enabled: config.chain_freshness(network).is_some(),
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
    let bound_source = capture_source::configured(&plan, ingestion_stream.as_deref())?;
    if managed_ingestion {
        // Never create, reset or re-arm a source merely because a process starts.
        // The operator-approved source seed remains the coverage boundary.
        managed_ingestion::cursor(&store, &operator, bound_source.as_ref()).await?;
    }
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
    worker
        .configure_capture_ingestion_source(bound_source.as_ref())
        .await?;
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
    let mut metrics = stage_metrics::start(metrics_enabled, Arc::clone(&pipeline))?;
    let mut metrics_poll = tokio::time::interval(Duration::from_secs(1));
    metrics_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let configuration = Arc::new(config);
    let mut evaluation: Option<JoinHandle<Result<Vec<arb_domain::DecisionTrace>, AttemptFailure>>> =
        None;
    let mut active_collection: Option<CollectionWork> = None;
    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut job: Option<JoinHandle<Result<CompletedBatch, AttemptFailure>>> = None;
    let mut last_good_capture: Option<Instant> = None;
    let mut next_capture = Instant::now();
    let mut stopping = false;
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = metrics_poll.tick(), if metrics.is_some() && !stopping => {
                let outcome = metrics.as_mut().expect("enabled metrics publisher")
                    .try_publish(&scheduler, Instant::now())?;
                if outcome == arb_scheduler::telemetry::PublishOutcome::ConsumerDisconnected {
                    // Disable a failed optional sink; never alter control/journal state.
                    metrics = None;
                }
            }
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
                    let result = evaluation.take().expect("finished evaluation exists").await;
                    let collection = active_collection.take().expect("evaluation has durable collection");
                    let generation = collection.generation.expect("evaluation has generation");
                    match result {
                        Ok(Ok(traces)) if !stopping => {
                            let finish = collection.finish(CollectionOutcome::DecisionsRecorded, None);
                            let admitted = match persistence(&pipeline, collection.correlation, worker.admit_collection_decision_traces(generation, &collection.id, &traces, finish)).await {
                                Ok(_) => true,
                                Err(error) if is_generation_fence(&error) => {
                                    finish_collection(&worker, &collection, CollectionOutcome::Suppressed, Some(CollectionReason::GenerationFenced)).await?;
                                    false
                                }
                                Err(error @ (StoreError::Conflict(_) | StoreError::InvalidInput(_))) => {
                                    finish_collection(&worker, &collection, CollectionOutcome::EvaluationFailed, Some(CollectionReason::EvaluationRejected)).await?;
                                    return Err(error.into());
                                }
                                Err(error) => return Err(error.into()),
                            };
                            println!("{}", json!({"event":"decision-batch","collection_attempt_id":collection.id,"count":traces.len(),"admitted":admitted,"evidence_ceiling":"CANDIDATE","full_transaction_simulation":false}));
                        }
                        Ok(Ok(_)) => {
                            finish_collection(&worker, &collection, CollectionOutcome::WorkerCancelled, Some(CollectionReason::WorkerShutdown)).await?;
                        }
                        Ok(Err(_)) if stopping => {
                            finish_collection(&worker, &collection, CollectionOutcome::WorkerCancelled, Some(CollectionReason::WorkerShutdown)).await?;
                        }
                        Ok(Err(failure)) => {
                            finish_collection(&worker, &collection, failure.outcome, Some(failure.reason)).await?;
                        }
                        Err(_) => {
                            finish_collection(&worker, &collection, CollectionOutcome::EvaluationFailed, Some(CollectionReason::TaskFailed)).await?;
                            return Err("evaluation task failed; typed terminal collection evidence retained".into());
                        }
                    }
                }
                if job.as_ref().is_some_and(JoinHandle::is_finished) {
                    let completed = job.take().expect("finished job exists").await;
                    let mut collection = active_collection.take().expect("capture has durable collection");
                    match completed {
                        Ok(Ok(batch)) => {
                            let mut source_ready = true;
                            if !stopping && let Some(recovered) = &batch.ingestion {
                                let source = bound_source.as_ref().ok_or("managed source is missing")?;
                                let expected = collection.ingestion_cursor.as_ref().ok_or("managed cursor is missing")?;
                                match managed_ingestion::commit(&store, &operator, source, expected, recovered).await {
                                    Ok(ready) => source_ready = ready,
                                    Err(error) => {
                                        finish_collection(&worker, &collection, CollectionOutcome::AcquisitionFailed, Some(CollectionReason::InputValidationFailed)).await?;
                                        worker.fence_local().await;
                                        scheduler.fence(network)?;
                                        worker.fault("MANAGED_SOURCE_PERSISTENCE_FAILED").await?;
                                        return Err(error.into());
                                    }
                                }
                            }
                            last_good_capture=source_ready.then(Instant::now);
                            // A RUNNING session stays ready while the source catches up; only
                            // admission for research (not readiness) requires it to have
                            // reached this capture's own anchor block.
                            let admit = source_ready && batch.source_caught_up;
                            collection.captured_pools = batch.captures.len() as u32;
                            let generation=collection.generation;
                            let mut all_admitted=!stopping && generation.is_some() && admit;
                            for capture in &batch.captures {
                                let admission = if stopping || !admit { None } else if let Some(generation)=generation {
                                    match persistence(&pipeline, collection.correlation, worker.admit_capture_manifest(generation,&capture.capture_id,&capture.manifest_digest,capture.path.to_str().ok_or("capture path is not UTF-8")?)).await {
                                        Ok(update)=>update.attempt_id,
                                        Err(error) if is_generation_fence(&error)=>None,
                                        Err(error @ (StoreError::Conflict(_) | StoreError::InvalidInput(_)))=>{
                                            finish_collection(&worker, &collection, CollectionOutcome::AcquisitionFailed, Some(CollectionReason::TaskFailed)).await?;
                                            return Err(error.into());
                                        }
                                        Err(error)=>return Err(error.into()),
                                    }
                                } else { None };
                                all_admitted &= admission.is_some();
                                println!("{}",json!({"event":"capture-written","collection_attempt_id":collection.id,"capture_id":capture.capture_id,"manifest_digest":capture.manifest_digest,"admission":if admission.is_some(){"ADMITTED_RAW_CAPTURE"}else{"UNADMITTED_RAW_CAPTURE"},"research_attempt_id":admission,"quote_ready":false,"source_caught_up":batch.source_caught_up}));
                            }
                            if all_admitted && let Some(source) = &bound_source {
                                let work = generation.expect("admitted capture has generation");
                                match capture_source::bind_batch(&worker, work, &plan, source, &batch).await {
                                    Ok(()) => {},
                                    Err(error) if is_generation_fence(&error) => all_admitted = false,
                                    Err(error) => {
                                        finish_collection(&worker, &collection, CollectionOutcome::AcquisitionFailed, Some(CollectionReason::InputValidationFailed)).await?;
                                        return Err(error.into());
                                    }
                                }
                            }
                            scheduler.set_gate(network, worker.generation().await.ok())?;
                            if all_admitted {
                                let generation=generation.expect("admitted capture has generation");
                                let item=arb_scheduler::WorkItem {
                                    network, stage:arb_scheduler::Stage::Quote, generation,
                                    correlation_id:arb_scheduler::CorrelationId::new(&collection.id)?,
                                    observed_at:batch.started,
                                    payload:EvaluationPayload {
                                        metrics: Arc::clone(&pipeline),
                                        correlation: collection.correlation,
                                        pools:batch.captures.into_iter().map(|c|c.pool).collect(),
                                        observed_at_ms:batch.observed_at_ms,
                                    },
                                };
                                let unavailable = || AttemptFailure {
                                    outcome:CollectionOutcome::EvaluationFailed,
                                    reason:CollectionReason::TaskFailed,
                                    captured_pools:collection.captured_pools,
                                    ingestion_halt:None,
                                };
                                let scheduled = match scheduler.try_enqueue(item,Instant::now()) {
                                    Ok(Ok(())) => match scheduler.dispatch(Instant::now()) {
                                        Ok(Some(permit)) => Ok(permit),
                                        Ok(None) => Err(AttemptFailure::evaluation(
                                            if batch.started.elapsed() >= EVALUATION_DEADLINE { arb_scheduler::DropReason::DeadlineExpired } else { arb_scheduler::DropReason::QueueFull },
                                            collection.captured_pools,
                                        )),
                                        Err(_) => Err(unavailable()),
                                    },
                                    Ok(Err(rejected)) => Err(AttemptFailure::evaluation(rejected.reason,collection.captured_pools)),
                                    Err(_) => Err(unavailable()),
                                };
                                match scheduled {
                                    Ok(permit) => {
                                        let config=Arc::clone(&configuration);
                                        let session_id=session_id.clone();
                                        let experiment_id=experiment_id.clone();
                                        let strategy=configuration.strategy_ids()[0].clone();
                                        active_collection=Some(collection);
                                        evaluation=Some(tokio::task::spawn_blocking(move||evaluate_blocking(permit,config,session_id,experiment_id,strategy)));
                                    }
                                    Err(failure) => {
                                        scheduler.fence(network)?;
                                        finish_collection(&worker, &collection, failure.outcome, Some(failure.reason)).await?;
                                    }
                                }
                            } else if stopping {
                                finish_collection(&worker, &collection, CollectionOutcome::WorkerCancelled, Some(CollectionReason::WorkerShutdown)).await?;
                            } else if generation.is_none() {
                                finish_collection(&worker, &collection, CollectionOutcome::ReadinessCompleted, None).await?;
                            } else if !batch.source_caught_up {
                                // ponytail: still walking a long finalized step; this research
                                // attempt could not admit anything, not a fenced generation.
                                finish_collection(&worker, &collection, CollectionOutcome::AcquisitionFailed, Some(CollectionReason::AcquisitionUnavailable)).await?;
                            } else {
                                finish_collection(&worker, &collection, CollectionOutcome::Suppressed, Some(CollectionReason::GenerationFenced)).await?;
                            }
                        }
                        Ok(Err(failure)) => {
                            last_good_capture=None;
                            collection.captured_pools=failure.captured_pools;
                            // Terminal source faults invalidate dependent history. Never
                            // overwrite a concurrently advanced cursor or halt on shutdown.
                            if !stopping && let Some(reason) = failure.ingestion_halt {
                                worker.fence_local().await;
                                scheduler.fence(network)?;
                                if let (Some(source), Some(expected)) = (&bound_source, &collection.ingestion_cursor) {
                                    persistence(&pipeline, collection.correlation,
                                        store.halt_ingestion(&operator, &source.stream_id, expected, reason)).await?;
                                }
                            }
                            finish_collection(&worker, &collection, failure.outcome, Some(failure.reason)).await?;
                            if !stopping && failure.ingestion_halt.is_some() {
                                // Readiness failure must not leave an apparently live
                                // owner that can subsequently acknowledge START.
                                worker.fault("MANAGED_SOURCE_RECOVERY_FAILED").await?;
                                return Err("managed Base source recovery failed".into());
                            }
                        }
                        Err(_) => {
                            finish_collection(&worker, &collection, CollectionOutcome::AcquisitionFailed, Some(CollectionReason::TaskFailed)).await?;
                            return Err("capture task failed; typed terminal collection evidence retained".into());
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
                    let generation=worker.generation().await.ok();
                    let purpose=if generation.is_some() { CollectionPurpose::Research } else { CollectionPurpose::Readiness };
                    let correlation=Uuid::now_v7();
                    let id=correlation.to_string();
                    // Commit before scheduling blocking work. A failed start writes no
                    // acquisition evidence and performs no external provider request.
                    let ingestion_cursor = if managed_ingestion {
                        Some(managed_ingestion::cursor(&store, &operator, bound_source.as_ref()).await?)
                    } else { None };
                    persistence(&pipeline, correlation, worker.begin_collection_attempt(&id, update.generation, purpose)).await?;
                    active_collection=Some(CollectionWork { id, generation, started:Instant::now(), captured_pools:0, correlation, metrics:Arc::clone(&pipeline), ingestion_cursor:ingestion_cursor.clone() });
                    let task_plan=Arc::clone(&plan);
                    job=Some(tokio::task::spawn_blocking(move||capture_blocking(&task_plan, correlation, ingestion_cursor.as_ref())));
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
    fn admission_invariant_conflicts_are_not_misreported_as_operator_suppression() {
        for message in [
            "stale or fenced collection decisions",
            "decision generation is fenced",
            "stale or fenced capture",
            "capture generation is fenced",
            "worker lease lost",
        ] {
            assert!(is_generation_fence(&StoreError::Conflict(message)));
        }
        for message in [
            "decision scope differs from immutable session",
            "decision identity payload changed",
            "decision capture is not admitted in this generation",
            "capture admission payload changed",
            "collection terminal evidence changed",
            "decision already belongs to another collection attempt",
        ] {
            assert!(!is_generation_fence(&StoreError::Conflict(message)));
        }
        assert!(!is_generation_fence(&StoreError::InvalidInput(
            "invalid sealed decision trace"
        )));
    }

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
        let registry = RegistryDocument::from_bytes(
            include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json"),
            NetworkId::BaseMainnet,
        )
        .unwrap();
        let records: Vec<RpcRecord> = serde_json::from_str(include_str!(
            "../../../crates/arb-evm/tests/fixtures/rpc.json"
        ))
        .unwrap();
        let mut rpc = TranscriptRpc::new(records);
        let (snapshot, context, coherent) = capture_document(&registry, &mut rpc, 100, false)
            .unwrap()
            .remove(0);
        rpc.finish().unwrap();
        assert!(coherent);
        assert_eq!(snapshot["quality"]["complete_for_quote"], false);
        assert!(matches!(context, StateContext::Evm { .. }));
        assert_eq!(registry.network(), NetworkId::BaseMainnet);
    }

    #[test]
    fn pool_set_dispatch_uses_shared_batch_transcripts_on_both_networks() {
        for (network, registries, transcript) in [
            (
                NetworkId::BaseMainnet,
                include_str!("../../../crates/arb-evm/tests/fixtures/batch-registries.json"),
                include_str!("../../../crates/arb-evm/tests/fixtures/batch-rpc.json"),
            ),
            (
                NetworkId::SolanaMainnet,
                include_str!("../../../crates/arb-solana/tests/fixtures/batch-registries.json"),
                include_str!("../../../crates/arb-solana/tests/fixtures/batch-rpc.json"),
            ),
        ] {
            let document = serde_json::to_vec(&json!({
                "schema_version": 1, "network_id": network,
                "pools": serde_json::from_str::<Value>(registries).unwrap(),
            }))
            .unwrap();
            let registry = RegistryDocument::from_bytes(&document, network).unwrap();
            let mut rpc = TranscriptRpc::new(serde_json::from_str(transcript).unwrap());
            let snapshots = capture_document(&registry, &mut rpc, 100, false).unwrap();
            rpc.finish().unwrap();
            assert_eq!(snapshots.len(), 2);
            assert_eq!(snapshots[0].1, snapshots[1].1);
            for (pool, (snapshot, _, _)) in registry.pools().iter().zip(snapshots) {
                assert_eq!(snapshot["pool"], pool.pool());
                assert_eq!(snapshot["quality"]["observed_at_ms"], 100);
                assert_eq!(snapshot["quality"]["complete_for_quote"], false);
                assert_eq!(snapshot["quality"]["quote_implementation_qualified"], false);
            }
        }
    }

    #[test]
    fn solana_policy_dispatch_records_exact_slot_time_for_single_and_batch_registry() {
        for batch in [false, true] {
            let (registries, transcript) = if batch {
                (
                    include_str!("../../../crates/arb-solana/tests/fixtures/batch-registries.json"),
                    include_str!("../../../crates/arb-solana/tests/fixtures/batch-rpc.json"),
                )
            } else {
                (
                    include_str!("../../../crates/arb-solana/tests/fixtures/registry.json"),
                    include_str!("../../../crates/arb-solana/tests/fixtures/rpc.json"),
                )
            };
            let mut registry: Value = serde_json::from_str(registries).unwrap();
            if batch {
                registry =
                    json!({"schema_version":1,"network_id":"solana-mainnet","pools":registry});
            }
            let document = RegistryDocument::from_bytes(
                &serde_json::to_vec(&registry).unwrap(),
                NetworkId::SolanaMainnet,
            )
            .unwrap();
            let mut records: Vec<RpcRecord> = serde_json::from_str(transcript).unwrap();
            let accounts: Value = serde_json::from_str(&records[1].response).unwrap();
            let sequence = records.len() as u64;
            records.push(RpcRecord {
                sequence,
                method: arb_adapter_api::ReadMethod::GetBlockTime,
                params: json!([accounts["result"]["context"]["slot"]]),
                response: json!({"jsonrpc":"2.0","id":sequence,"result":90}).to_string(),
            });
            let mut rpc = TranscriptRpc::new(records.clone());
            let snapshots = capture_document(&document, &mut rpc, 100000, true).unwrap();
            rpc.finish().unwrap();
            assert_eq!(snapshots.len(), if batch { 2 } else { 1 });
            assert_eq!(
                capture_adapter_version(&document, true),
                "arb_solana-pool-set-v3"
            );
            assert_eq!(
                capture_adapter_version(&document, false),
                document.adapter_version()
            );
            for (snapshot, _, _) in snapshots {
                assert_eq!(snapshot["block_time_seconds"], 90);
                assert_eq!(snapshot["quality"]["coherent"], false);
                assert_eq!(snapshot["quality"]["quote_implementation_qualified"], false);
            }
            records.pop();
            let mut missing_time = TranscriptRpc::new(records);
            assert!(capture_document(&document, &mut missing_time, 100000, true).is_err());
        }
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
