//! Offline capture re-decoding. This module never constructs an HTTP transport.
//! Matching a transcript proves deterministic acquisition decoding, not quote math or P&L.
use arb_adapter_api::{Chain, RpcRecord, TranscriptRpc};
use arb_capture::{CaptureError, LoadedBundle, Origin, load_bundle};
use arb_registry::{PoolRegistry, RegistryDocument};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub fn verify_capture(
    path: &Path,
    expected_manifest_digest: &str,
    now_ms: u64,
) -> Result<Value, CaptureError> {
    let bundle = load_bundle(path, Some(expected_manifest_digest), now_ms)?;
    verify_loaded(&bundle)
}

fn verify_loaded(bundle: &LoadedBundle) -> Result<Value, CaptureError> {
    let objects: BTreeMap<_, _> = bundle
        .objects
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    if objects.keys().copied().collect::<Vec<_>>()
        != [
            "effective-config.json",
            "registry.json",
            "rpc.json",
            "snapshot.json",
        ]
    {
        return Err(CaptureError("unsupported replay object set"));
    }
    let config_source: Value = serde_json::from_slice(objects["effective-config.json"])
        .map_err(|_| CaptureError("invalid recorded effective configuration"))?;
    // Verify original typed serialization bytes without reordering map keys.
    // Recorded configuration is never an instruction to resolve secrets or connect.
    if !config_source.is_object() {
        return Err(CaptureError("empty recorded configuration"));
    }
    if arb_capture::digest(objects["effective-config.json"]) != bundle.manifest.config_digest {
        return Err(CaptureError("effective configuration digest mismatch"));
    }
    let recorded_config = if bundle.manifest.origin == Origin::RecordedLive {
        let source = std::str::from_utf8(objects["effective-config.json"])
            .map_err(|_| CaptureError("invalid effective configuration encoding"))?;
        let config = arb_config::ValidatedConfig::from_effective_json(source)
            .map_err(|_| CaptureError("recorded configuration failed validation"))?;
        let network = match bundle.manifest.network {
            Chain::BaseMainnet => arb_domain::NetworkId::BaseMainnet,
            Chain::SolanaMainnet => arb_domain::NetworkId::SolanaMainnet,
        };
        if config.digest() != bundle.manifest.config_digest
            || !config.network_enabled(network)
            || !matches!(
                config.mode(),
                arb_domain::Mode::Observe | arb_domain::Mode::Paper
            )
            || config.registry_qualification_digest(network)
                != Some(arb_capture::digest(objects["registry.json"]).as_str())
        {
            return Err(CaptureError(
                "recorded configuration does not authorize this acquisition",
            ));
        }
        Some((network, config))
    } else {
        None
    };
    let records: Vec<RpcRecord> = serde_json::from_slice(objects["rpc.json"])
        .map_err(|_| CaptureError("invalid RPC transcript"))?;
    if records.is_empty()
        || records.len() > 4096
        || bundle.manifest.first_sequence != 0
        || bundle.manifest.last_sequence != records.len() as u64 - 1
    {
        return Err(CaptureError("capture sequence coverage mismatch"));
    }
    let count = records.len();
    let mut rpc = TranscriptRpc::new(records);
    let expected: Value = serde_json::from_slice(objects["snapshot.json"])
        .map_err(|_| CaptureError("invalid recorded snapshot"))?;
    let network = match bundle.manifest.network {
        Chain::BaseMainnet => arb_domain::NetworkId::BaseMainnet,
        Chain::SolanaMainnet => arb_domain::NetworkId::SolanaMainnet,
    };
    let document = RegistryDocument::from_bytes(objects["registry.json"], network)
        .map_err(|_| CaptureError("invalid replay registry document"))?;
    if let Some((_, config)) = &recorded_config {
        document
            .authorize(config)
            .map_err(|_| CaptureError("replay registry is outside recorded configuration"))?;
    }
    // Version 3 binds an exact-slot time lookup to a validated frozen policy.
    // Enforce this for synthetic economic fixtures too: rehashing a stripped
    // transcript and changing the adapter label must not remove that lookup.
    let chain_time_v3 = bundle.manifest.adapter_version == "arb_solana-pool-set-v3";
    let policy_config = arb_config::ValidatedConfig::from_effective_json(
        std::str::from_utf8(objects["effective-config.json"])
            .map_err(|_| CaptureError("invalid effective configuration encoding"))?,
    )
    .ok();
    let chain_time_required = network == arb_domain::NetworkId::SolanaMainnet
        && policy_config
            .as_ref()
            .is_some_and(|config| config.chain_freshness(network).is_some());
    if chain_time_v3 != chain_time_required {
        return Err(CaptureError(
            "Solana chain-time policy and capture version differ",
        ));
    }
    if chain_time_v3 {
        document
            .authorize(policy_config.as_ref().unwrap())
            .map_err(|_| CaptureError("chain-time registry is outside frozen configuration"))?;
    }
    if (recorded_config.is_some() || document.format() == arb_registry::DocumentFormat::PoolSetV1)
        && bundle.manifest.adapter_version != document.adapter_version()
        && bundle.manifest.adapter_version != document.legacy_adapter_version()
        && !chain_time_v3
    {
        return Err(CaptureError("unsupported registry capture format"));
    }
    let pool = expected["pool"]
        .as_str()
        .ok_or(CaptureError("snapshot has no pool identity"))?;
    let batch = chain_time_v3
        || (document.format() == arb_registry::DocumentFormat::PoolSetV1
            && bundle.manifest.adapter_version == document.adapter_version());
    let actual = match document
        .select(pool)
        .map_err(|_| CaptureError("captured pool is absent from registry"))?
    {
        PoolRegistry::Base(registry) => {
            if bundle.manifest.adapter_source_commit != arb_evm::SOURCE_COMMIT {
                return Err(CaptureError("unsupported Base adapter source revision"));
            }
            let snapshot = if batch {
                let registries = document
                    .pools()
                    .iter()
                    .map(|pool| match pool {
                        PoolRegistry::Base(registry) => Ok(registry.clone()),
                        _ => Err(CaptureError("mixed replay registry networks")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let snapshots =
                    arb_evm::capture_pools(&mut rpc, &registries, bundle.manifest.created_at_ms)
                        .map_err(|_| CaptureError("Base batch transcript replay failed"))?;
                snapshots
                    .into_iter()
                    .find(|snapshot| snapshot.pool.eq_ignore_ascii_case(pool))
                    .ok_or(CaptureError("captured pool missing from replayed batch"))?
            } else {
                arb_evm::capture_pool(&mut rpc, registry, bundle.manifest.created_at_ms)
                    .map_err(|_| CaptureError("Base transcript replay failed"))?
            };
            serde_json::to_value(snapshot).map_err(|_| CaptureError("snapshot encoding failed"))?
        }
        PoolRegistry::Solana(registry) => {
            if bundle.manifest.adapter_source_commit != arb_solana::SOURCE_COMMIT {
                return Err(CaptureError("unsupported Solana adapter source revision"));
            }
            let snapshot = if batch {
                let registries = document
                    .pools()
                    .iter()
                    .map(|pool| match pool {
                        PoolRegistry::Solana(registry) => Ok(registry.clone()),
                        _ => Err(CaptureError("mixed replay registry networks")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let snapshots = if chain_time_v3 {
                    arb_solana::capture_pools_with_chain_time(
                        &mut rpc,
                        &registries,
                        bundle.manifest.created_at_ms,
                    )
                } else {
                    arb_solana::capture_pools(&mut rpc, &registries, bundle.manifest.created_at_ms)
                }
                .map_err(|_| CaptureError("Solana batch transcript replay failed"))?;
                snapshots
                    .into_iter()
                    .find(|snapshot| snapshot.pool == pool)
                    .ok_or(CaptureError("captured pool missing from replayed batch"))?
            } else {
                arb_solana::capture_pool(&mut rpc, registry, bundle.manifest.created_at_ms)
                    .map_err(|_| CaptureError("Solana transcript replay failed"))?
            };
            serde_json::to_value(snapshot).map_err(|_| CaptureError("snapshot encoding failed"))?
        }
    };
    rpc.finish()
        .map_err(|_| CaptureError("replay left unconsumed RPC records"))?;
    let context = serde_json::to_value(&bundle.manifest.context)
        .map_err(|_| CaptureError("context encoding failed"))?;
    if actual != expected
        || actual["context"] != context
        || actual["quality"]["coherent"] != bundle.manifest.coherent
        || actual["quality"]["complete_for_quote"] != bundle.manifest.complete_for_quote
    {
        return Err(CaptureError(
            "replayed snapshot differs from captured evidence",
        ));
    }
    Ok(json!({
        "schema_version": 1,
        "verification": "ACQUISITION_REDECODE_MATCHED",
        "capture_id": bundle.manifest.capture_id,
        "origin": bundle.manifest.origin,
        "network": bundle.manifest.network,
        "manifest_digest": bundle.manifest_digest,
        "configuration_digest": bundle.manifest.config_digest,
        "protocol_source_commit": bundle.manifest.adapter_source_commit,
        "original_build_digest": bundle.manifest.build_digest,
        "replay_version": env!("CARGO_PKG_VERSION"),
        "rpc_records_consumed": count,
        "context": context,
        "quality": actual["quality"],
        "quote_replay_available": false,
        "paper_pnl_available": false,
        "full_transaction_simulation_available": false,
        "network_requests": 0
    }))
}

/// Validated economic replay inputs. Effective configuration must parse and match
/// the complete registry even for fixtures; legacy decoder-only fixtures cannot
/// become economic evidence by changing their origin label.
pub fn load_evaluation_capture(
    path: &Path,
    expected_manifest_digest: &str,
    now_ms: u64,
) -> Result<(arb_engine::CapturedPool, arb_config::ValidatedConfig), CaptureError> {
    let bundle = load_bundle(path, Some(expected_manifest_digest), now_ms)?;
    verify_loaded(&bundle)?;
    let object = |name: &str| -> Result<&[u8], CaptureError> {
        bundle
            .objects
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, bytes)| bytes.as_slice())
            .ok_or(CaptureError("missing replay object"))
    };
    let configuration = arb_config::ValidatedConfig::from_effective_json(
        std::str::from_utf8(object("effective-config.json")?)
            .map_err(|_| CaptureError("invalid effective configuration encoding"))?,
    )
    .map_err(|_| CaptureError("economic replay requires validated frozen configuration"))?;
    let network = match bundle.manifest.network {
        Chain::BaseMainnet => arb_domain::NetworkId::BaseMainnet,
        Chain::SolanaMainnet => arb_domain::NetworkId::SolanaMainnet,
    };
    let document = RegistryDocument::from_bytes(object("registry.json")?, network)
        .map_err(|_| CaptureError("invalid replay registry document"))?;
    document
        .authorize(&configuration)
        .map_err(|_| CaptureError("economic replay registry is not authorized"))?;
    let snapshot: Value = serde_json::from_slice(object("snapshot.json")?)
        .map_err(|_| CaptureError("invalid snapshot"))?;
    let selected = document
        .select(
            snapshot["pool"]
                .as_str()
                .ok_or(CaptureError("missing pool identity"))?,
        )
        .map_err(|_| CaptureError("pool outside registry"))?;
    let state = match selected {
        PoolRegistry::Base(registry) => arb_engine::PoolState::Base {
            snapshot: serde_json::from_value(snapshot)
                .map_err(|_| CaptureError("invalid Base snapshot"))?,
            registry: registry.clone(),
        },
        PoolRegistry::Solana(registry) => arb_engine::PoolState::Solana {
            snapshot: serde_json::from_value(snapshot)
                .map_err(|_| CaptureError("invalid Solana snapshot"))?,
            registry: registry.clone(),
        },
    };
    let origin = match bundle.manifest.origin {
        Origin::Synthetic => arb_domain::DatasetOrigin::Synthetic,
        Origin::ManuallyConstructed => arb_domain::DatasetOrigin::ManuallyConstructed,
        Origin::RecordedLive => arb_domain::DatasetOrigin::RecordedLive,
    };
    Ok((
        arb_engine::CapturedPool {
            capture: arb_domain::DecisionCaptureRef {
                capture_id: bundle.manifest.capture_id,
                manifest_digest: bundle.manifest_digest.clone(),
                snapshot_id: bundle.manifest_digest,
            },
            origin,
            configuration_digest: bundle.manifest.config_digest,
            state,
        },
        configuration,
    ))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayCaptureInput {
    pub path: String,
    pub manifest_digest: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayEvaluationRequest {
    pub schema_version: u32,
    pub session_id: String,
    pub experiment_id: String,
    pub strategy_id: String,
    pub network_id: arb_domain::NetworkId,
    pub generation: u64,
    pub observed_at_unix_ms: u64,
    /// Explicit historical batch age. Never derive historical freshness from
    /// today's wall clock, and never silently use a zero-age default.
    pub input_age_ms: u64,
    pub captures: Vec<ReplayCaptureInput>,
}

struct HistoricalGate {
    generation: u64,
    age: u64,
}
impl arb_engine::EvaluationGate for HistoricalGate {
    fn state(&self) -> arb_engine::GateState {
        arb_engine::GateState {
            generation: self.generation,
            admission_open: true,
            now_monotonic_ms: self.age,
        }
    }
}
pub fn evaluate_captures(
    request: &ReplayEvaluationRequest,
    now_ms: u64,
) -> Result<Value, CaptureError> {
    if request.schema_version != 1
        || request.captures.is_empty()
        || request.captures.len() > arb_registry::MAX_POOLS
        || request.input_age_ms >= 65_000
    {
        return Err(CaptureError("unsupported replay evaluation bounds"));
    }
    let mut pools = Vec::new();
    let mut frozen = None;
    for input in &request.captures {
        let (pool, configuration) =
            load_evaluation_capture(Path::new(&input.path), &input.manifest_digest, now_ms)?;
        if let Some(previous) = &frozen {
            let previous: &arb_config::ValidatedConfig = previous;
            if previous.digest() != configuration.digest() {
                return Err(CaptureError("capture configurations differ"));
            }
        } else {
            frozen = Some(configuration);
        }
        pools.push(pool);
    }
    let configuration = frozen.ok_or(CaptureError("missing frozen configuration"))?;
    let dataset_origin = pools[0].origin;
    let evaluation = arb_engine::EvaluationRequest {
        session_id: &request.session_id,
        experiment_id: &request.experiment_id,
        generation: request.generation,
        configuration: &configuration,
        strategy_id: &request.strategy_id,
        network_id: request.network_id,
        dataset_origin,
        observed_at_unix_ms: request.observed_at_unix_ms,
        observed_monotonic_ms: 0,
        deadline_monotonic_ms: 65_000,
        pools: &pools,
    };
    let traces = arb_engine::evaluate(
        &evaluation,
        &HistoricalGate {
            generation: request.generation,
            age: request.input_age_ms,
        },
    )
    .map_err(|_| CaptureError("bounded replay evaluation failed"))?;
    Ok(json!({
        "schema_version":1,
        "verification":"ACQUISITION_REDECODE_AND_RESEARCH_EVALUATION",
        "configuration_digest":configuration.digest(),
        "dataset_origin":dataset_origin,
        "operation":"REPLAY",
        "timing_policy":"MODELED_HISTORICAL_BATCH_AGE",
        "timing_evidence":"USER_SUPPLIED_SCENARIO",
        "original_configuration_mode":configuration.mode(),
        "input_age_ms":request.input_age_ms,
        "decisions":traces,
        "evidence_ceiling":"CANDIDATE",
        "paper_pnl_available":false,
        "full_transaction_simulation_available":false,
        "network_requests":0
    }))
}
