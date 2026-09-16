//! Explicit bounded finalized polling. No signer, quote generation or deployment.
#[path = "base_ingest/reconnect.rs"]
mod reconnect;
#[path = "base_ingest/shutdown.rs"]
mod shutdown;

use arb_adapter_api::{EndpointKind, HttpReadRpc, ReadMethod, ReadRpc, endpoint_kind};
use arb_evm::{
    PoolRegistry, SOURCE_COMMIT, UNISWAP_V3_FACTORY,
    backfill::{BackfillLimits, GapReason, recover_logs},
    events::BlockHeader,
    filters::{FilterError, PoolFilters},
};
use arb_storage::{IngestionBinding, IngestionCursor, IngestionHalt, IngestionHead, Store};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

type Result<T> = std::result::Result<T, &'static str>;
fn setting(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or("REQUIRED_SETTING_MISSING")
}
fn scope(name: &str) -> Result<String> {
    let s = setting(name)?;
    if s.len() > 128
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
    {
        return Err("INVALID_STREAM_SCOPE");
    }
    Ok(s)
}
fn positive_setting(name: &str, default: u64, min: u64, max: u64) -> Result<u64> {
    match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => Ok(default),
        Ok(s) => s
            .parse::<u64>()
            .ok()
            .filter(|v| s == v.to_string() && (min..=max).contains(v))
            .ok_or("INVALID_POLL_BOUND"),
        _ => Err("INVALID_POLL_BOUND"),
    }
}
fn load_pools() -> Result<Vec<PoolRegistry>> {
    let file = std::fs::File::open(setting("ARB_INGEST_REGISTRY_FILE")?)
        .map_err(|_| "REGISTRY_UNAVAILABLE")?;
    if !file
        .metadata()
        .map_err(|_| "REGISTRY_UNAVAILABLE")?
        .is_file()
    {
        return Err("REGISTRY_NOT_REGULAR_FILE");
    }
    let mut bytes = Vec::new();
    file.take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "REGISTRY_UNAVAILABLE")?;
    if bytes.len() > 65_536 {
        return Err("REGISTRY_TOO_LARGE");
    }
    let pools: Vec<PoolRegistry> =
        serde_json::from_slice(&bytes).map_err(|_| "INVALID_REGISTRY")?;
    if pools.is_empty() || pools.len() > 8 || pools.iter().any(|p| p.validate().is_err()) {
        return Err("INVALID_REGISTRY");
    }
    Ok(pools)
}
fn binding(pools: &[PoolRegistry], endpoint: &str) -> Result<IngestionBinding> {
    let mut addresses: Vec<_> = pools.iter().map(|p| p.pool.to_lowercase()).collect();
    addresses.sort();
    let origin = match endpoint_kind(endpoint).map_err(|_| "INVALID_RPC_ENDPOINT")? {
        EndpointKind::LoopbackFixture => "MANUALLY_CONSTRUCTED",
        EndpointKind::HttpsRemote => "RECORDED_LIVE",
    };
    let result = IngestionBinding {
        schema_version: 1,
        network_id: "base-mainnet".into(),
        registry_digest: format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                serde_json::to_vec(pools).map_err(|_| "INVALID_REGISTRY")?
            ))
        ),
        abi_source_commit: SOURCE_COMMIT.into(),
        dataset_origin: origin.into(),
        pool_addresses: addresses,
    };
    result.validate().map_err(|_| "INVALID_REGISTRY_BINDING")?;
    Ok(result)
}
fn head(value: &IngestionHead) -> BlockHeader {
    BlockHeader {
        number: value.number,
        hash: value.hash.clone(),
        parent_hash: value.parent_hash.clone(),
        timestamp_seconds: value.timestamp_seconds,
    }
}
fn stored(value: BlockHeader) -> IngestionHead {
    IngestionHead {
        number: value.number,
        hash: value.hash,
        parent_hash: value.parent_hash,
        timestamp_seconds: value.timestamp_seconds,
    }
}
fn rpc(endpoint: &str) -> Result<HttpReadRpc> {
    HttpReadRpc::new(endpoint, Duration::from_secs(5), 2 * 1024 * 1024, 512)
        .map_err(|_| "RPC_CONFIGURATION_REJECTED")
}
/// Bootstrap explicitly declares that there is no coverage at/before this seed.
/// Verify code and canonical identity before creating any durable cursor.
fn initial(
    endpoint: &str,
    pools: &[PoolRegistry],
    cancelled: &AtomicBool,
) -> Result<IngestionHead> {
    let mut rpc = rpc(endpoint)?;
    let mut call = |method, params| {
        if cancelled.load(Ordering::SeqCst) {
            return Err("CANCELLED");
        }
        let value = rpc
            .call(method, params)
            .map_err(|_| "BOOTSTRAP_PROVIDER_FAILURE")?;
        if cancelled.load(Ordering::SeqCst) {
            return Err("CANCELLED");
        }
        Ok(value)
    };
    if call(ReadMethod::EthChainId, json!([]))? != "0x2105" {
        return Err("WRONG_CHAIN");
    }
    let header = BlockHeader::from_rpc(&call(
        ReadMethod::EthGetBlockByNumber,
        json!(["finalized", false]),
    )?)
    .map_err(|_| "INVALID_BOOTSTRAP_HEADER")?;
    let code_targets = std::iter::once((UNISWAP_V3_FACTORY, &pools[0].factory_runtime_sha256))
        .chain(
            pools
                .iter()
                .map(|p| (p.pool.as_str(), &p.pool_runtime_sha256)),
        );
    for (address, digest) in code_targets {
        if pools.iter().any(|p| {
            !p.factory_runtime_sha256
                .eq_ignore_ascii_case(&pools[0].factory_runtime_sha256)
        }) {
            return Err("CONFLICTING_FACTORY_IDENTITY");
        }
        let code = call(
            ReadMethod::EthGetCode,
            json!([address,{"blockHash":header.hash,"requireCanonical":true}]),
        )?;
        let bytes = code
            .as_str()
            .and_then(|s| s.strip_prefix("0x"))
            .and_then(|s| hex::decode(s).ok())
            .filter(|b| !b.is_empty())
            .ok_or("INVALID_BOOTSTRAP_CODE")?;
        if format!("sha256:{}", hex::encode(Sha256::digest(bytes))) != digest.to_lowercase() {
            return Err("CONTRACT_CODE_CHANGED");
        }
    }
    let checked = BlockHeader::from_rpc(&call(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{:x}", header.number), false]),
    )?)
    .map_err(|_| "INVALID_BOOTSTRAP_HEADER")?;
    if !checked.same_block(&header) {
        return Err("BOOTSTRAP_CONTEXT_CHANGED");
    }
    Ok(stored(header))
}
fn gap(reason: GapReason) -> IngestionHalt {
    match reason {
        GapReason::ProviderFailure => IngestionHalt::ProviderFailure,
        GapReason::BackfillLimitExceeded | GapReason::LogLimitExceeded => {
            IngestionHalt::ResourceLimit
        }
        GapReason::InvalidInput => IngestionHalt::InvalidInput,
        _ => IngestionHalt::ContinuityLost,
    }
}
/// Output is diagnostic, not a provider result. Return write failures through
/// the normal outcome path so owned filters always reach explicit cleanup.
fn write_record(value: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{value}")
        .and_then(|()| stdout.flush())
        .map_err(|_| "OUTPUT_UNAVAILABLE")
}

/// Preserve a local output error separately from chain/provider gap evidence.
enum RecoveryFailure {
    Gap(GapReason),
    Output,
}
impl From<GapReason> for RecoveryFailure {
    fn from(reason: GapReason) -> Self {
        Self::Gap(reason)
    }
}

fn emit(status: &str, cursor: &IngestionCursor) -> Result<()> {
    write_record(
        &json!({"status":status,"checkpoint":cursor.checkpoint,"revision":cursor.revision.to_string(),
        "state":cursor.state,"halt_reason":cursor.halt_reason,"dataset_origin":cursor.binding.dataset_origin,
        "registry_digest":cursor.binding.registry_digest,"execution_authorized":false,"quote_qualified":false}),
    )
}
async fn database<T>(
    operation: impl std::future::Future<Output = std::result::Result<T, arb_storage::StoreError>>,
) -> Result<T> {
    tokio::time::timeout(Duration::from_secs(15), operation)
        .await
        .map_err(|_| "STORAGE_OUTCOME_UNCERTAIN")?
        .map_err(|e| match e {
            arb_storage::StoreError::NotFound => "STREAM_NOT_INITIALIZED",
            arb_storage::StoreError::Conflict(_) => "STREAM_CONFLICT",
            arb_storage::StoreError::InvalidInput(_) => "STORAGE_INPUT_OR_RETENTION_REJECTED",
            _ => "STORAGE_UNAVAILABLE",
        })
}
/// Close both known node filters once under the existing independent budget.
async fn close_filters(endpoint: &str, filters: &mut Option<PoolFilters>) -> Result<()> {
    let Some(mut active) = filters.take() else {
        return Ok(());
    };
    let endpoint = endpoint.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut rpc = HttpReadRpc::new(&endpoint, Duration::from_secs(1), 1024, 2)
            .map_err(|_| "FILTER_CLEANUP_FAILED")?;
        active.close(&mut rpc).map_err(|_| "FILTER_CLEANUP_FAILED")
    })
    .await
    .map_err(|_| "FILTER_CLEANUP_FAILED")
    .and_then(|result| result)
}

async fn run(action: String) -> Result<()> {
    let operator = scope("ARB_INGEST_OPERATOR_ID")?;
    let stream = scope("ARB_INGEST_STREAM_ID")?;
    let store = database(Store::connect(&setting("ARB_INGEST_DATABASE_URL")?)).await?;
    // Schema changes are explicit, not a side effect of every poll or status read.
    if action == "--migrate" {
        database(store.migrate()).await?;
        return write_record(&json!({"status":"MIGRATED"}));
    }
    if action == "--status" {
        return emit(
            "STATUS",
            &database(store.ingestion_cursor(&operator, &stream)).await?,
        );
    }
    let polls = positive_setting("ARB_INGEST_MAX_POLLS", 1, 1, 100)?;
    let interval = positive_setting("ARB_INGEST_POLL_MS", 2000, 1000, 60_000)?;
    let pools = load_pools()?;
    let endpoint = setting("ARB_BASE_RPC_URL")?;
    let expected_binding = binding(&pools, &endpoint)?;
    if expected_binding.dataset_origin == "RECORDED_LIVE" {
        let pacing = setting("ARB_RPC_MIN_INTERVAL_MS")?;
        if pacing
            .parse::<u64>()
            .ok()
            .is_none_or(|n| !(75..=1000).contains(&n) || n.to_string() != pacing)
        {
            return Err("EXPLICIT_RPC_PACING_REQUIRED");
        }
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let listener = shutdown::install(cancelled.clone())?;
    let mut filters: Option<PoolFilters> = None;
    let following = action == "--follow";
    let outcome = async {
        if action == "--initialize" {
            // Do not even probe a provider to reseed an already-existing stream.
            match database(store.ingestion_cursor(&operator, &stream)).await {
                Err("STREAM_NOT_INITIALIZED") => (),
                Ok(_) => return Err("STREAM_ALREADY_EXISTS"),
                Err(e) => return Err(e),
            }
            let cancelled_copy = cancelled.clone();
            let endpoint = endpoint.clone();
            let seed =
                tokio::task::spawn_blocking(move || initial(&endpoint, &pools, &cancelled_copy))
                    .await
                    .map_err(|_| "BOOTSTRAP_TASK_FAILED")??;
            if cancelled.load(Ordering::SeqCst) {
                return Err("CANCELLED");
            }
            emit(
                "INITIALIZED_NO_PRIOR_COVERAGE",
                &database(store.create_ingestion(&operator, &stream, &expected_binding, &seed))
                    .await?,
            )?;
            return Ok(());
        }
        // This budget spans the entire invocation and is not reset by success.
        let mut reconnects = reconnect::Budget::from_environment()?;
        for iteration in 0..polls {
            if cancelled.load(Ordering::SeqCst) {
                return Err("CANCELLED");
            }
            let cursor = database(store.ingestion_cursor(&operator, &stream)).await?;
            if cursor.binding != expected_binding {
                return Err("REGISTRY_OR_ORIGIN_CHANGED");
            }
            if cursor.state != "ACTIVE" {
                emit("HALTED_REQUIRES_REVIEW", &cursor)?;
                return Err("STREAM_HALTED");
            }
            let recovered = loop {
                let task_endpoint = endpoint.clone();
                let pools = pools.clone();
                let checkpoint = head(&cursor.checkpoint);
                let cancel = cancelled.clone();
                let mut active = filters.take();
                let (returned_filters, recovered, transient) =
                    tokio::task::spawn_blocking(move || {
                        let mut rpc = match rpc(&task_endpoint) {
                            Ok(rpc) => reconnect::ObservedRpc::new(rpc),
                            Err(_) => {
                                return (
                                    active,
                                    Err(RecoveryFailure::Gap(GapReason::InvalidInput)),
                                    false,
                                );
                            }
                        };
                        let recovered = (|| {
                            if following {
                                let failure = |error| match error {
                                    FilterError::Cancelled => GapReason::Cancelled,
                                    FilterError::Provider => GapReason::ProviderFailure,
                                    FilterError::LimitExceeded => GapReason::LogLimitExceeded,
                                    _ => GapReason::InvalidInput,
                                };
                                if active.is_none() {
                                    active = Some(
                                        PoolFilters::open(&mut rpc, &pools, &cancel)
                                            .map_err(failure)?,
                                    );
                                }
                                let hints = active
                                    .as_mut()
                                    .expect("opened filters")
                                    .poll(&mut rpc, &cancel)
                                    .map_err(failure)?;
                                write_record(&json!({"status":"FILTER_HINTS",
                            "block_notifications":hints.block_hashes.len(),
                            "pool_notifications":hints.logs.len(),
                            "removed_notifications":hints.logs.iter().filter(|l| l.removed).count(),
                            "authoritative":false}))
                                .map_err(|_| RecoveryFailure::Output)?;
                            }
                            // Quiet, lost or non-final notifications can never skip the
                            // authoritative finalized reconciliation from the saved cursor.
                            recover_logs(
                                &mut rpc,
                                &pools,
                                &checkpoint,
                                BackfillLimits::default(),
                                || cancel.load(Ordering::SeqCst),
                            )
                            .map_err(|e| RecoveryFailure::Gap(e.reason))
                        })();
                        (active, recovered, rpc.transient())
                    })
                    .await
                    .map_err(|_| "RECOVERY_TASK_FAILED")?;
                filters = returned_filters;
                if cancelled.load(Ordering::SeqCst) {
                    return Err("CANCELLED");
                }
                if transient
                    && matches!(
                        &recovered,
                        Err(RecoveryFailure::Gap(GapReason::ProviderFailure))
                    )
                    && let Some(delay) = reconnects.next_delay()
                {
                    // Failed filter pairs are never reused. Cleanup failure ends
                    // reconnect rather than allocating more unknown resources.
                    if close_filters(&endpoint, &mut filters).await.is_err() {
                        break recovered;
                    }
                    write_record(&json!({"status":"RECONNECT_SCHEDULED",
                        "reconnect":reconnects.used(),"backoff_ms":delay.as_millis(),
                        "checkpoint":cursor.checkpoint,"revision":cursor.revision.to_string(),
                        "execution_authorized":false}))?;
                    reconnect::wait(delay, &cancelled).await?;
                    // A competing process or operator halt invalidates this
                    // invocation before any further external request is sent.
                    if database(store.ingestion_cursor(&operator, &stream)).await? != cursor {
                        return Err("STREAM_CONFLICT");
                    }
                    continue;
                }
                break recovered;
            };
            match recovered {
                Ok(batch) => {
                    let value =
                        serde_json::to_value(batch).map_err(|_| "INVALID_RECOVERED_BATCH")?;
                    let committed =
                        database(store.commit_ingestion(&operator, &stream, &cursor, value))
                            .await?;
                    emit(
                        if committed.revision == cursor.revision {
                            "NO_NEW_FINALIZED_BLOCKS"
                        } else {
                            "BATCH_COMMITTED"
                        },
                        &committed,
                    )?;
                }
                Err(RecoveryFailure::Output) => return Err("OUTPUT_UNAVAILABLE"),
                Err(RecoveryFailure::Gap(GapReason::Cancelled)) => return Err("CANCELLED"),
                Err(RecoveryFailure::Gap(reason)) => {
                    let halted =
                        database(store.halt_ingestion(&operator, &stream, &cursor, gap(reason)))
                            .await?;
                    emit("GAP_REQUIRES_REVIEW", &halted)?;
                    return Err("RECOVERY_GAP");
                }
            }
            if iteration + 1 < polls {
                // Small cancellation-aware waits, without new tasks or busy polling.
                for _ in 0..interval.div_ceil(100) {
                    if cancelled.load(Ordering::SeqCst) {
                        return Err("CANCELLED");
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        Ok(())
    }
    .await;
    let cleanup = close_filters(&endpoint, &mut filters).await;
    listener.abort();
    let _ = listener.await;
    // Never turn an acquisition failure into success. A cleanup error after an
    // accepted batch is reported without invalidating that atomic commit.
    outcome.and(cleanup)
}
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] || args == ["--check"] {
        return if write_record(&json!({"status":"NOT_STARTED","provider_requests":0,
            "actions":["--migrate","--initialize","--run","--follow","--status"],"execution_authorized":false})).is_ok() {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::from(2)
        };
    }
    if args.len() != 1
        || !matches!(
            args[0].as_str(),
            "--migrate" | "--initialize" | "--run" | "--follow" | "--status"
        )
    {
        let _ = writeln!(std::io::stderr().lock(), "INVALID_ARGUMENTS");
        return std::process::ExitCode::from(2);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build();
    let result = runtime
        .map_err(|_| "RUNTIME_UNAVAILABLE")
        .and_then(|rt| rt.block_on(run(args[0].clone())));
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(reason) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "{}",
                json!({"status":"STOPPED_WITH_ERROR","reason":reason,"execution_authorized":false})
            );
            std::process::ExitCode::from(2)
        }
    }
}
