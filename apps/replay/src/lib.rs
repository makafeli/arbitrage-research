//! Offline capture re-decoding. This module never constructs an HTTP transport.
//! Matching a transcript proves deterministic acquisition decoding, not quote math or P&L.
use arb_adapter_api::{Chain, RpcRecord, TranscriptRpc};
use arb_capture::{CaptureError, LoadedBundle, Origin, load_bundle};
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
    let actual = match bundle.manifest.network {
        Chain::BaseMainnet => {
            if bundle.manifest.adapter_source_commit != arb_evm::SOURCE_COMMIT {
                return Err(CaptureError("unsupported Base adapter source revision"));
            }
            if recorded_config.is_some() && bundle.manifest.adapter_version != "arb_evm-v1" {
                return Err(CaptureError("unsupported Base capture format"));
            }
            let registry: arb_evm::PoolRegistry = serde_json::from_slice(objects["registry.json"])
                .map_err(|_| CaptureError("invalid Base replay registry"))?;
            if let Some((network, config)) = &recorded_config
                && (!config
                    .verified_pools(*network)
                    .iter()
                    .any(|pool| pool.address().eq_ignore_ascii_case(&registry.pool))
                    || [&registry.token0, &registry.token1].iter().any(|address| {
                        !config
                            .verified_assets(*network)
                            .iter()
                            .any(|asset| asset.address().eq_ignore_ascii_case(address))
                    }))
            {
                return Err(CaptureError(
                    "Base replay registry is outside recorded allowlists",
                ));
            }
            let snapshot =
                arb_evm::capture_pool(&mut rpc, &registry, bundle.manifest.created_at_ms)
                    .map_err(|_| CaptureError("Base transcript replay failed"))?;
            serde_json::to_value(snapshot).map_err(|_| CaptureError("snapshot encoding failed"))?
        }
        Chain::SolanaMainnet => {
            if bundle.manifest.adapter_source_commit != arb_solana::SOURCE_COMMIT {
                return Err(CaptureError("unsupported Solana adapter source revision"));
            }
            if recorded_config.is_some() && bundle.manifest.adapter_version != "arb_solana-v1" {
                return Err(CaptureError("unsupported Solana capture format"));
            }
            let registry: arb_solana::PoolRegistry =
                serde_json::from_slice(objects["registry.json"])
                    .map_err(|_| CaptureError("invalid Solana replay registry"))?;
            if let Some((network, config)) = &recorded_config
                && (config.expected_genesis_identity() != registry.expected_genesis_hash
                    || !config
                        .verified_pools(*network)
                        .iter()
                        .any(|pool| pool.address() == registry.pool)
                    || [&registry.mint_a, &registry.mint_b].iter().any(|address| {
                        !config
                            .verified_assets(*network)
                            .iter()
                            .any(|asset| asset.address() == address.as_str())
                    }))
            {
                return Err(CaptureError(
                    "Solana replay registry is outside recorded allowlists or genesis identity",
                ));
            }
            let snapshot =
                arb_solana::capture_pool(&mut rpc, &registry, bundle.manifest.created_at_ms)
                    .map_err(|_| CaptureError("Solana transcript replay failed"))?;
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
