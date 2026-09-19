//! Opt-in ownership of bounded source catch-up inside the existing research worker.
//! An explicitly initialized stream is required; restart never replaces its seed.
use super::*;
use arb_evm::{
    backfill::{BackfillBatch, BackfillLimits, GapReason, recover_logs_bounded},
    events::BlockHeader,
};
use arb_storage::{CaptureSourceBinding, IngestionCursor, IngestionHalt};

fn parse(value: Option<&str>, stream: Option<&str>) -> Result<bool, AnyError> {
    match value {
        None | Some("false") => Ok(false),
        Some("true") if stream.is_some() => Ok(true),
        _ => Err("managed Base ingestion requires true/false and an explicit source stream".into()),
    }
}

pub(super) fn enabled(stream: Option<&str>) -> Result<bool, AnyError> {
    match std::env::var("ARB_BASE_MANAGED_INGESTION") {
        Ok(value) => parse(Some(&value), stream),
        Err(std::env::VarError::NotPresent) => parse(None, stream),
        Err(_) => Err("invalid managed Base ingestion setting".into()),
    }
}

/// Read the original cursor on every attempt. An existing HALTED stream or a
/// changed binding is never repaired, silently restarted or moved to the tip.
pub(super) async fn cursor(
    store: &Store,
    operator: &str,
    source: Option<&CaptureSourceBinding>,
) -> Result<IngestionCursor, StoreError> {
    let source = source.ok_or(StoreError::InvalidInput("managed source is missing"))?;
    source.validate()?;
    let current = store.ingestion_cursor(operator, &source.stream_id).await?;
    if current.binding != source.binding || current.state != "ACTIVE" {
        return Err(StoreError::Conflict("managed source is halted or differs"));
    }
    Ok(current)
}

/// Called after capture, before artifacts are written, using the same transport.
/// Only the pool transcript is retained in each replay bundle; no recovery call
/// is rewritten into a quote response. Transport limits remain cumulative.
/// A finalized step larger than the bounded per-attempt range is walked over
/// consecutive attempts: no block is skipped and no limit is raised, so a
/// capture whose anchor is not yet reached commits a partial batch and the
/// next attempt resumes from the returned checkpoint. A provider failure is
/// not terminal: the untouched checkpoint is simply retried on the next attempt.
pub(super) fn recover(
    plan: &CapturePlan,
    snapshots: &[(Value, StateContext, bool)],
    cursor: &IngestionCursor,
    rpc: &mut AcquisitionRpc,
) -> Result<BackfillBatch, AttemptFailure> {
    let invalid = || AttemptFailure::acquisition(CollectionReason::InputValidationFailed, 0);
    if snapshots.is_empty() || snapshots.len() != plan.registry.pools().len() {
        return Err(invalid());
    }
    let pools = plan
        .registry
        .pools()
        .iter()
        .map(|pool| match pool {
            PoolRegistry::Base(pool) => Ok(pool.clone()),
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut target = None;
    for (_, context, coherent) in snapshots {
        let StateContext::Evm {
            block_number,
            block_hash,
            parent_hash,
            block_timestamp_seconds,
            finality,
        } = context
        else {
            return Err(invalid());
        };
        if !coherent || finality != "finalized" {
            return Err(invalid());
        }
        let header = BlockHeader {
            number: *block_number,
            hash: block_hash.to_lowercase(),
            parent_hash: parent_hash.to_lowercase(),
            timestamp_seconds: *block_timestamp_seconds,
        };
        if target
            .as_ref()
            .is_some_and(|previous: &BlockHeader| !previous.same_block(&header))
        {
            return Err(invalid());
        }
        target = Some(header);
    }
    let start = BlockHeader {
        number: cursor.checkpoint.number,
        hash: cursor.checkpoint.hash.clone(),
        parent_hash: cursor.checkpoint.parent_hash.clone(),
        timestamp_seconds: cursor.checkpoint.timestamp_seconds,
    };
    recover_logs_bounded(
        rpc,
        &pools,
        &start,
        &target.ok_or_else(invalid)?,
        BackfillLimits::default(),
        || false,
    )
    .map_err(|error| attempt_failure_for(error.reason, rpc.failure))
}

/// A provider failure during the bounded backfill is retried on the next
/// attempt from the untouched checkpoint, so it is a plain attempt failure,
/// not a halt. Every other reason keeps halting exactly as before.
fn attempt_failure_for(
    reason: GapReason,
    provider_failure: Option<CollectionReason>,
) -> AttemptFailure {
    if reason == GapReason::ProviderFailure {
        return AttemptFailure::acquisition(
            provider_failure.unwrap_or(CollectionReason::ProviderUnavailable),
            0,
        );
    }
    let halt = match reason {
        GapReason::BackfillLimitExceeded | GapReason::LogLimitExceeded => {
            IngestionHalt::ResourceLimit
        }
        GapReason::InvalidInput => IngestionHalt::InvalidInput,
        _ => IngestionHalt::ContinuityLost,
    };
    let reason = match halt {
        IngestionHalt::ResourceLimit => CollectionReason::ResourceLimit,
        _ => CollectionReason::InputValidationFailed,
    };
    let mut failure = AttemptFailure::acquisition(reason, 0);
    failure.ingestion_halt = Some(halt);
    failure
}

/// Source persistence precedes capture associations and decision publication.
/// A seed with no accepted event batch is not ready for protected quoting yet.
pub(super) async fn commit(
    store: &Store,
    operator: &str,
    source: &CaptureSourceBinding,
    expected: &IngestionCursor,
    recovered: &BackfillBatch,
) -> Result<bool, StoreError> {
    if expected.binding != source.binding {
        return Err(StoreError::Conflict("managed source binding differs"));
    }
    let payload = serde_json::to_value(recovered)
        .map_err(|_| StoreError::InvalidInput("invalid managed recovery batch"))?;
    let current = store
        .commit_ingestion(operator, &source.stream_id, expected, payload)
        .await?;
    Ok(current.revision > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_adapter_api::{ReadMethod, TranscriptRpc};

    #[test]
    fn managed_ingestion_is_opt_in_and_cannot_drop_its_source_requirement() {
        assert!(!parse(None, None).unwrap());
        assert!(!parse(Some("false"), Some("source")).unwrap());
        assert!(parse(Some("true"), Some("source")).unwrap());
        assert!(parse(Some("true"), None).is_err());
        for value in ["", "TRUE", "1", "false ", "https://private.invalid/secret"] {
            let error = parse(Some(value), Some("source")).unwrap_err().to_string();
            assert_eq!(
                error,
                "managed Base ingestion requires true/false and an explicit source stream"
            );
        }
    }

    fn fixture_pool() -> arb_evm::PoolRegistry {
        serde_json::from_str(include_str!(
            "../../../crates/arb-evm/tests/fixtures/registry.json"
        ))
        .unwrap()
    }
    fn fixture_checkpoint() -> BlockHeader {
        BlockHeader {
            number: 100,
            hash: format!("0x{:064x}", 100),
            parent_hash: format!("0x{:064x}", 99),
            timestamp_seconds: 1,
        }
    }

    /// No fake RPC answers any call, so the very first one (`eth_chainId`)
    /// fails; `arb_evm::backfill::Attempt::call` maps every RPC error to
    /// `GapReason::ProviderFailure` regardless of which call actually failed
    /// (an `eth_getLogs` failure lands here identically). This is exactly the
    /// gap reason a failed `eth_getLogs` produced in issue #197.
    #[test]
    fn provider_failure_is_retried_next_attempt_not_halted() {
        let checkpoint = fixture_checkpoint();
        let mut rpc = TranscriptRpc::new(vec![]);
        let error = recover_logs_bounded(
            &mut rpc,
            &[fixture_pool()],
            &checkpoint,
            &checkpoint,
            BackfillLimits::default(),
            || false,
        )
        .unwrap_err();
        assert_eq!(error.reason, GapReason::ProviderFailure);
        // The checkpoint recover() would resume from next attempt is untouched.
        assert_eq!(error.checkpoint, checkpoint);

        let failure =
            attempt_failure_for(error.reason, Some(CollectionReason::ProviderUnavailable));
        assert!(failure.ingestion_halt.is_none());
        assert_eq!(failure.reason, CollectionReason::ProviderUnavailable);

        // No recorded RPC failure reason still falls back to ProviderUnavailable.
        let fallback = attempt_failure_for(GapReason::ProviderFailure, None);
        assert!(fallback.ingestion_halt.is_none());
        assert_eq!(fallback.reason, CollectionReason::ProviderUnavailable);
    }

    /// A continuity-style error (here `WrongChain`, raised by the same early
    /// `eth_chainId` check as above once it actually answers) still halts the
    /// source exactly as before this change.
    #[test]
    fn continuity_and_other_reasons_still_halt_exactly_as_before() {
        let checkpoint = fixture_checkpoint();
        let wrong_chain = chain_id_answers("0x1");
        let mut rpc = TranscriptRpc::new(vec![wrong_chain]);
        let error = recover_logs_bounded(
            &mut rpc,
            &[fixture_pool()],
            &checkpoint,
            &checkpoint,
            BackfillLimits::default(),
            || false,
        )
        .unwrap_err();
        assert_eq!(error.reason, GapReason::WrongChain);

        let failure = attempt_failure_for(error.reason, None);
        assert!(matches!(
            failure.ingestion_halt,
            Some(IngestionHalt::ContinuityLost)
        ));
        assert_eq!(failure.reason, CollectionReason::InputValidationFailed);

        assert!(matches!(
            attempt_failure_for(GapReason::InvalidInput, None).ingestion_halt,
            Some(IngestionHalt::InvalidInput)
        ));
        let resource = attempt_failure_for(GapReason::BackfillLimitExceeded, None);
        assert!(matches!(
            resource.ingestion_halt,
            Some(IngestionHalt::ResourceLimit)
        ));
        assert_eq!(resource.reason, CollectionReason::ResourceLimit);
    }

    fn chain_id_answers(result: &str) -> RpcRecord {
        RpcRecord {
            sequence: 0,
            method: ReadMethod::EthChainId,
            params: serde_json::json!([]),
            response: serde_json::json!({"jsonrpc":"2.0","id":0,"result":result}).to_string(),
        }
    }
}
