//! Producer-side matching of the actual decoded batch to an explicitly selected stream.
use super::*;
use arb_storage::{CaptureSourceBinding, IngestionHead};

fn parse_setting(value: Option<&str>) -> Result<Option<String>, AnyError> {
    match value {
        None => Ok(None),
        Some(value)
            if !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c)) =>
        {
            Ok(Some(value.into()))
        }
        _ => Err("invalid Base ingestion stream setting".into()),
    }
}

pub(super) fn setting() -> Result<Option<String>, AnyError> {
    match std::env::var("ARB_BASE_INGESTION_STREAM") {
        Ok(value) => parse_setting(Some(&value)),
        Err(std::env::VarError::NotPresent) => parse_setting(None),
        Err(_) => Err("invalid Base ingestion stream setting".into()),
    }
}

/// Use the same typed, ordered pool-registry serialization as base-ingest. A shared
/// pool name or observation time is never sufficient to associate independent inputs.
pub(super) fn configured(
    plan: &CapturePlan,
    stream: Option<&str>,
) -> Result<Option<CaptureSourceBinding>, AnyError> {
    let Some(stream) = stream else {
        return Ok(None);
    };
    let pools = plan
        .registry
        .pools()
        .iter()
        .map(|pool| match pool {
            PoolRegistry::Base(pool) => Ok(pool.clone()),
            _ => Err("Base ingestion cannot bind a different network"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut addresses: Vec<_> = pools.iter().map(|p| p.pool.to_lowercase()).collect();
    addresses.sort();
    let origin = match plan.origin {
        Origin::ManuallyConstructed => "MANUALLY_CONSTRUCTED",
        Origin::RecordedLive => "RECORDED_LIVE",
        Origin::Synthetic => return Err("synthetic source binding is unsupported".into()),
    };
    let source = CaptureSourceBinding {
        stream_id: stream.into(),
        binding: arb_storage::IngestionBinding {
            schema_version: 1,
            network_id: "base-mainnet".into(),
            registry_digest: arb_capture::digest(&serde_json::to_vec(&pools)?),
            abi_source_commit: arb_evm::SOURCE_COMMIT.into(),
            dataset_origin: origin.into(),
            pool_addresses: addresses,
        },
    };
    source.validate()?;
    Ok(Some(source))
}

fn checkpoint(context: &StateContext) -> Result<IngestionHead, StoreError> {
    match context {
        StateContext::Evm {
            block_number,
            block_hash,
            parent_hash,
            block_timestamp_seconds,
            finality,
        } if finality == "finalized" => Ok(IngestionHead {
            number: *block_number,
            hash: block_hash.to_lowercase(),
            parent_hash: parent_hash.to_lowercase(),
            timestamp_seconds: *block_timestamp_seconds,
        }),
        _ => Err(StoreError::InvalidInput(
            "capture source requires finalized Base context",
        )),
    }
}

/// All captures below were produced by the validated adapter and fsynced by write_bundle.
/// Match the same complete context and typed registry before any association is written.
pub(super) async fn bind_batch(
    worker: &ControlWorker,
    generation: WorkGeneration,
    plan: &CapturePlan,
    source: &CaptureSourceBinding,
    batch: &CompletedBatch,
) -> Result<(), StoreError> {
    let invalid =
        || StoreError::InvalidInput("capture batch differs from configured ingestion source");
    if batch.captures.is_empty()
        || batch.captures.len() != plan.registry.pools().len()
        || batch.captures.len() > arb_domain::MAX_PUBLICATION_CAPTURE_REFS
    {
        return Err(invalid());
    }
    let mut common = None;
    let mut references = Vec::with_capacity(batch.captures.len());
    for (capture, expected) in batch.captures.iter().zip(plan.registry.pools()) {
        let (arb_engine::PoolState::Base { snapshot, registry }, PoolRegistry::Base(expected)) =
            (&capture.pool.state, expected)
        else {
            return Err(invalid());
        };
        if !snapshot.quality.coherent
            || serde_json::to_value(registry).map_err(|_| invalid())?
                != serde_json::to_value(expected).map_err(|_| invalid())?
            || capture.pool.configuration_digest != plan.config_digest
            || capture.pool.origin.label() != source.binding.dataset_origin
            || capture.pool.capture.capture_id != capture.capture_id
            || capture.pool.capture.manifest_digest != capture.manifest_digest
            || capture.pool.capture.snapshot_id != capture.manifest_digest
        {
            return Err(invalid());
        }
        let head = checkpoint(&snapshot.context)?;
        if common.as_ref().is_some_and(|previous| previous != &head) {
            return Err(invalid());
        }
        common = Some(head);
        references.push(capture.pool.capture.clone());
    }
    worker
        .bind_captures_to_ingestion(
            generation,
            source,
            &common.ok_or_else(invalid)?,
            &references,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_setting_is_explicit_bounded_and_never_echoes_secrets() {
        assert!(parse_setting(None).unwrap().is_none());
        assert_eq!(
            parse_setting(Some("base:research-1")).unwrap().as_deref(),
            Some("base:research-1")
        );
        for value in [
            "",
            " ",
            "https://example.invalid/private-secret",
            "a/b",
            "new\nsource",
        ] {
            let error = parse_setting(Some(value)).unwrap_err().to_string();
            assert_eq!(error, "invalid Base ingestion stream setting");
        }
        assert!(parse_setting(Some(&"a".repeat(129))).is_err());
        assert!(parse_setting(Some(&"a".repeat(128))).is_ok());
    }

    #[test]
    fn complete_finalized_header_is_kept_and_other_contexts_are_rejected() {
        let context = StateContext::Evm {
            block_number: 9_007_199_254_740_993,
            block_hash: format!("0x{}", "AB".repeat(32)),
            parent_hash: format!("0x{}", "CD".repeat(32)),
            block_timestamp_seconds: 100,
            finality: "finalized".into(),
        };
        let head = checkpoint(&context).unwrap();
        assert_eq!(head.number, 9_007_199_254_740_993);
        assert_eq!(head.hash, format!("0x{}", "ab".repeat(32)));
        assert_eq!(head.parent_hash, format!("0x{}", "cd".repeat(32)));
        assert_eq!(head.timestamp_seconds, 100);
        let mut unsafe_context = context;
        if let StateContext::Evm { finality, .. } = &mut unsafe_context {
            *finality = "latest".into();
        }
        assert!(checkpoint(&unsafe_context).is_err());
        assert!(
            checkpoint(&StateContext::Solana {
                slot: 1,
                genesis_hash: "different-network".into(),
                commitment: "finalized".into(),
                account_context: "single-response".into()
            })
            .is_err()
        );
    }
}
