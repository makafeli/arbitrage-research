//! Bounded finalized-block log recovery over the existing read-only transport.
//! No retry, endpoint selection, task spawning or automatic cursor advancement.
use crate::events::{BlockHeader, PoolLog, decode_log};
use crate::{
    BASE_CHAIN_ID, MAX_CAPTURE_POOLS, PoolRegistry, SOURCE_COMMIT, UNISWAP_V3_FACTORY, quantity,
};
use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Largest block range this module asks a provider to cover with one
/// `eth_getLogs` call. Set to the free-tier cap observed on the provider
/// behind a live Base worker: a probe (issue #202, 2026-09-19) showed spans
/// over 10 blocks rejected with JSON-RPC `-32600` ("Under the Free tier
/// plan, you can make eth_getLogs requests with up to a 10 block range"),
/// while spans of 10 or fewer answered normally. A paid plan raises the
/// provider's own cap; this constant may be raised to match it, up to
/// `BackfillLimits::max_blocks`, without changing anything else here — the
/// step size, the header walk and every existing check stay exactly as
/// today regardless of how many chunks a step's logs are split across.
pub const LOG_RANGE_CHUNK_BLOCKS: u64 = 10;

/// Explicit admission limits, in addition to HttpReadRpc's unchanged byte/time cap.
#[derive(Clone, Copy, Debug)]
pub struct BackfillLimits {
    pub max_blocks: u64,
    pub max_logs_per_block: usize,
    /// Default 2048 keeps ~64 logs/block of headroom across a 32-block step
    /// (raised from 1024 when the default `max_blocks` doubled from 16 to
    /// 32; owner decision 2026-09-19). The hard ceiling in `validate` below
    /// (4096) is unchanged.
    pub max_total_logs: usize,
}
impl Default for BackfillLimits {
    fn default() -> Self {
        Self {
            max_blocks: 32,
            max_logs_per_block: 256,
            max_total_logs: 2048,
        }
    }
}
impl BackfillLimits {
    fn validate(self) -> bool {
        (1..=32).contains(&self.max_blocks)
            && (1..=512).contains(&self.max_logs_per_block)
            && (1..=4096).contains(&self.max_total_logs)
            && self.max_logs_per_block <= self.max_total_logs
    }
}

/// Reasons identify lost continuity instead of presenting a partial range as whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GapReason {
    InvalidInput,
    WrongChain,
    ProviderFailure,
    MissingOrMalformedBlock,
    CheckpointChanged,
    UnexpectedContractCode,
    FinalityRegressed,
    BackfillLimitExceeded,
    BrokenAncestry,
    TargetChanged,
    LogLimitExceeded,
    MalformedLog,
    ConflictingLog,
    RemovedLog,
    Cancelled,
}

/// A failure retains the starting checkpoint. No recovered prefix is accepted.
#[derive(Clone, Debug)]
pub struct BackfillError {
    pub reason: GapReason,
    pub checkpoint: BlockHeader,
    pub requested_through: Option<u64>,
    pub failed_at: Option<u64>,
    /// Existing transport errors are fixed strings; no upstream body or URL.
    pub transport_error: Option<AdapterError>,
}
impl std::fmt::Display for BackfillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base log recovery failed: {:?}", self.reason)
    }
}
impl std::error::Error for BackfillError {}

/// One exact block, including an explicitly empty provider log result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BlockLogs {
    pub header: BlockHeader,
    pub logs: Vec<PoolLog>,
}

/// Commit the end checkpoint only after downstream persistence succeeds.
/// This evidence describes provider-returned logs, not receipt-root completeness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackfillBatch {
    pub schema_version: u32,
    pub network_id: String,
    /// SHA-256 of the exact serde JSON serialization of the supplied registries.
    pub registry_digest: String,
    pub from_checkpoint: BlockHeader,
    pub through: BlockHeader,
    pub pool_addresses: Vec<String>,
    pub abi_source_commit: String,
    pub blocks: Vec<BlockLogs>,
    /// Updates must invalidate dependent pool snapshots, not fabricate new quotes.
    pub full_snapshot_required: bool,
}

struct Attempt<'a, R, C> {
    rpc: &'a mut R,
    cancelled: C,
    checkpoint: &'a BlockHeader,
    target: Option<u64>,
}
impl<R: ReadRpc, C: FnMut() -> bool> Attempt<'_, R, C> {
    fn error(&self, reason: GapReason, at: Option<u64>) -> BackfillError {
        BackfillError {
            reason,
            checkpoint: self.checkpoint.clone(),
            requested_through: self.target,
            failed_at: at,
            transport_error: None,
        }
    }
    fn call(
        &mut self,
        method: ReadMethod,
        params: Value,
        at: Option<u64>,
    ) -> Result<Value, BackfillError> {
        if (self.cancelled)() {
            return Err(self.error(GapReason::Cancelled, at));
        }
        let result = self.rpc.call(method, params);
        // A completed request after cancellation must not advance the recovery.
        if (self.cancelled)() {
            return Err(self.error(GapReason::Cancelled, at));
        }
        result.map_err(|cause| {
            let mut failure = self.error(GapReason::ProviderFailure, at);
            failure.transport_error = Some(cause);
            failure
        })
    }
    fn code(
        &mut self,
        address: &str,
        expected: &str,
        header: &BlockHeader,
    ) -> Result<(), BackfillError> {
        let value = self.call(
            ReadMethod::EthGetCode,
            json!([address,{"blockHash":header.hash,"requireCanonical":true}]),
            Some(header.number),
        )?;
        let code = value.as_str().and_then(|text| crate::hex_bytes(text).ok());
        if code.as_ref().is_none_or(|bytes| {
            bytes.is_empty()
                || format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
                    != expected.to_ascii_lowercase()
        }) {
            return Err(self.error(GapReason::UnexpectedContractCode, Some(header.number)));
        }
        Ok(())
    }
    fn header(&mut self, selector: Value, at: Option<u64>) -> Result<BlockHeader, BackfillError> {
        let value = self.call(
            ReadMethod::EthGetBlockByNumber,
            json!([selector, false]),
            at,
        )?;
        BlockHeader::from_rpc(&value)
            .map_err(|_| self.error(GapReason::MissingOrMalformedBlock, at))
    }
}

/// Fetch a bounded consecutive range after a caller-supplied checkpoint. Fetch
/// the whole step with ranged `eth_getLogs` calls (`fromBlock`/`toBlock` over
/// the pool addresses) instead of one call per block, chunked to at most
/// `LOG_RANGE_CHUNK_BLOCKS` blocks per call and concatenated in ascending
/// order (issue #202, 2026-09-19), verify ancestry and canonical endpoints,
/// confirm factory/pool runtime code only at the step's first and last block
/// (issue #180, 2026-09-19), and return nothing on any failure. The
/// checkpoint is borrowed immutably.
///
/// A notification/disconnect is only a reason to call this function: it supplies
/// no finality proof. A larger gap requires an explicit operator/caller recovery
/// decision; this function never drops history or silently advances to the tip.
pub fn recover_logs(
    rpc: &mut impl ReadRpc,
    pools: &[PoolRegistry],
    checkpoint: &BlockHeader,
    limits: BackfillLimits,
    cancelled: impl FnMut() -> bool,
) -> Result<BackfillBatch, BackfillError> {
    recover_logs_selected(rpc, pools, checkpoint, None, false, limits, cancelled)
}

/// Recover only through an already captured finalized header. The provider must
/// still confirm finality and that exact canonical header. A newer finalized tip
/// does not change the requested range or the snapshot the caller will consume.
pub fn recover_logs_through(
    rpc: &mut impl ReadRpc,
    pools: &[PoolRegistry],
    checkpoint: &BlockHeader,
    through: &BlockHeader,
    limits: BackfillLimits,
    cancelled: impl FnMut() -> bool,
) -> Result<BackfillBatch, BackfillError> {
    recover_logs_selected(
        rpc,
        pools,
        checkpoint,
        Some(through),
        false,
        limits,
        cancelled,
    )
}

/// Recover through an already captured finalized header, the same as
/// `recover_logs_through`, except a range longer than `limits.max_blocks` is
/// never rejected outright: the walk covers exactly the first `max_blocks`
/// blocks after the checkpoint and returns that reached header as `through`,
/// with nothing skipped and no limit raised. A caller that has not reached the
/// requested header resumes from the returned `through` on a later attempt,
/// repeating until the requested header is reached.
pub fn recover_logs_bounded(
    rpc: &mut impl ReadRpc,
    pools: &[PoolRegistry],
    checkpoint: &BlockHeader,
    through: &BlockHeader,
    limits: BackfillLimits,
    cancelled: impl FnMut() -> bool,
) -> Result<BackfillBatch, BackfillError> {
    recover_logs_selected(
        rpc,
        pools,
        checkpoint,
        Some(through),
        true,
        limits,
        cancelled,
    )
}

fn recover_logs_selected(
    rpc: &mut impl ReadRpc,
    pools: &[PoolRegistry],
    checkpoint: &BlockHeader,
    requested: Option<&BlockHeader>,
    bounded: bool,
    limits: BackfillLimits,
    cancelled: impl FnMut() -> bool,
) -> Result<BackfillBatch, BackfillError> {
    let mut attempt = Attempt {
        rpc,
        cancelled,
        checkpoint,
        target: requested.map(|header| header.number),
    };
    if !limits.validate()
        || checkpoint.validate().is_err()
        || requested.is_some_and(|header| header.validate().is_err())
        || pools.is_empty()
        || pools.len() > MAX_CAPTURE_POOLS
    {
        return Err(attempt.error(GapReason::InvalidInput, None));
    }
    let mut by_address = BTreeMap::new();
    for pool in pools {
        if pool.validate().is_err()
            || !pool
                .factory_runtime_sha256
                .eq_ignore_ascii_case(&pools[0].factory_runtime_sha256)
            || by_address.insert(pool.pool.to_lowercase(), pool).is_some()
        {
            return Err(attempt.error(GapReason::InvalidInput, None));
        }
    }
    let addresses: Vec<_> = by_address.keys().cloned().collect();
    let registry_bytes =
        serde_json::to_vec(pools).map_err(|_| attempt.error(GapReason::InvalidInput, None))?;
    let registry_digest = format!("sha256:{}", hex::encode(Sha256::digest(&registry_bytes)));
    let chain = attempt.call(ReadMethod::EthChainId, json!([]), None)?;
    if quantity(&chain).ok() != Some(BASE_CHAIN_ID) {
        return Err(attempt.error(GapReason::WrongChain, None));
    }
    let original = attempt.header(
        json!(format!("0x{:x}", checkpoint.number)),
        Some(checkpoint.number),
    )?;
    if !original.same_block(checkpoint) {
        return Err(attempt.error(GapReason::CheckpointChanged, Some(checkpoint.number)));
    }
    let finalized = attempt.header(json!("finalized"), None)?;
    let target = if let Some(requested) = requested {
        if requested.number > finalized.number {
            return Err(attempt.error(GapReason::FinalityRegressed, Some(requested.number)));
        }
        if requested.number == finalized.number && !requested.same_block(&finalized) {
            return Err(attempt.error(GapReason::TargetChanged, Some(requested.number)));
        }
        if bounded && requested.number.saturating_sub(checkpoint.number) > limits.max_blocks {
            // The requested header is farther away than one attempt may walk. Do
            // not fetch/confirm it at all; instead walk exactly `max_blocks` blocks
            // toward it and let the caller resume from the returned checkpoint.
            let capped = checkpoint.number + limits.max_blocks;
            attempt.header(json!(format!("0x{capped:x}")), Some(capped))?
        } else {
            let confirmed = attempt.header(
                json!(format!("0x{:x}", requested.number)),
                Some(requested.number),
            )?;
            if !confirmed.same_block(requested) {
                return Err(attempt.error(GapReason::TargetChanged, Some(requested.number)));
            }
            confirmed
        }
    } else {
        finalized
    };
    attempt.target = Some(target.number);
    let Some(distance) = target.number.checked_sub(checkpoint.number) else {
        return Err(attempt.error(GapReason::FinalityRegressed, Some(target.number)));
    };
    if distance > limits.max_blocks {
        return Err(attempt.error(GapReason::BackfillLimitExceeded, Some(target.number)));
    }
    if distance == 0 && !target.same_block(checkpoint) {
        return Err(attempt.error(GapReason::CheckpointChanged, Some(target.number)));
    }
    // Code identity is confirmed only at the two ends of the step, not at
    // every intervening block. Precondition: this holds for steps whose
    // `from` is at or after Ecotone (Cancun on Base, activated 2024-03-14);
    // the seeded checkpoint for the registered Base pools is far after that
    // date, so this is a documented, accepted narrowing (issue #180,
    // 2026-09-19), not an open risk for the pools this walks today. Under
    // Ecotone/EIP-6780 the code of a pre-existing contract cannot change
    // between two blocks, so identity confirmed at `original` (`from`) and
    // `target` (`through`) implies identity at every block in between.
    //
    // The check at `original` is load-bearing: it is the only place this
    // function verifies that the contract standing at the checkpoint is
    // still the one on record, and code is now checked AT the checkpoint
    // block itself, not at `from + 1` as the earlier per-block base walk
    // did. A checkpoint seeded before a registered pool's creation block
    // would now fail here with `UnexpectedContractCode` -> `ContinuityLost`
    // instead of walking past it — theoretical for the registered Base
    // pools, whose seeded checkpoints all postdate their creation. The check
    // at `target` is defense in depth: `original`'s check already implies it
    // under the Ecotone precondition above, so `target`'s check only adds
    // value if that precondition is somehow violated.
    if distance > 0 {
        for bound in [&original, &target] {
            attempt.code(UNISWAP_V3_FACTORY, &pools[0].factory_runtime_sha256, bound)?;
            for (address, pool) in &by_address {
                attempt.code(address, &pool.pool_runtime_sha256, bound)?;
            }
        }
    }

    let mut seen_hashes = BTreeSet::from([original.hash.to_ascii_lowercase()]);
    let mut previous = original.clone();
    let mut step_headers = Vec::with_capacity(distance as usize);
    for offset in 1..=distance {
        // distance is target - checkpoint and target is representable as u64.
        let number = checkpoint.number + offset;
        let header = attempt.header(json!(format!("0x{number:x}")), Some(number))?;
        if !seen_hashes.insert(header.hash.to_ascii_lowercase())
            || header.number != number
            || !header.parent_hash.eq_ignore_ascii_case(&previous.hash)
            || header.timestamp_seconds < previous.timestamp_seconds
        {
            return Err(attempt.error(GapReason::BrokenAncestry, Some(number)));
        }
        if number == target.number && !header.same_block(&target) {
            return Err(attempt.error(GapReason::TargetChanged, Some(number)));
        }
        previous = header.clone();
        step_headers.push(header);
    }

    // Ranged eth_getLogs calls replace the old per-block filter, fetched in
    // consecutive chunks of at most LOG_RANGE_CHUNK_BLOCKS blocks each (a
    // step of that size or smaller still issues exactly one call). Chunk
    // results are concatenated in ascending order before anything below
    // sees them, so every log is still attributed to its own
    // already-verified header and every existing bound/ordering/conflict
    // rejection is preserved unchanged.
    //
    // Residual risk: a present log is verified against its own header, but an
    // absent one is not independently verifiable by this request alone — a
    // stale or split-view provider could return an empty range for blocks
    // that later have logs committed on the canonical chain. This is bounded,
    // not proven: `target` is always the `finalized` header (see above,
    // never a provisional tip), and the by-number rechecks of `target` and
    // `checkpoint` after the whole range (below, ~line 450 and ~line 455)
    // would catch a reorg or provider swap that produced a stale empty range.
    // See docs/BASE-LOG-RECOVERY.md, "Residual risk of the ranged fetch".
    let mut blocks = Vec::with_capacity(distance as usize);
    let mut total_logs = 0_usize;
    if distance > 0 {
        let mut raw_logs: Vec<Value> = Vec::new();
        let mut chunk_start = checkpoint.number + 1;
        while chunk_start <= target.number {
            let chunk_end = chunk_start
                .saturating_add(LOG_RANGE_CHUNK_BLOCKS - 1)
                .min(target.number);
            let response = attempt.call(
                ReadMethod::EthGetLogs,
                json!([{
                    "fromBlock": format!("0x{chunk_start:x}"),
                    "toBlock": format!("0x{chunk_end:x}"),
                    "address": addresses,
                }]),
                Some(target.number),
            )?;
            match response {
                Value::Array(chunk) => raw_logs.extend(chunk),
                _ => return Err(attempt.error(GapReason::MalformedLog, Some(target.number))),
            }
            if chunk_end == target.number {
                break;
            }
            chunk_start = chunk_end + 1;
        }
        // Group by block while requiring the provider's own advertised order
        // (ascending by block number); a log for a block outside the step or
        // returned out of order is rejected before anything else is checked.
        let mut grouped: BTreeMap<u64, Vec<&Value>> = BTreeMap::new();
        let mut last_seen_block: Option<u64> = None;
        for entry in &raw_logs {
            let number = quantity(&entry["blockNumber"])
                .map_err(|_| attempt.error(GapReason::MalformedLog, Some(target.number)))?;
            if number < checkpoint.number + 1
                || number > target.number
                || last_seen_block.is_some_and(|last| number < last)
            {
                return Err(attempt.error(GapReason::MalformedLog, Some(target.number)));
            }
            last_seen_block = Some(number);
            grouped.entry(number).or_default().push(entry);
        }
        let mut transaction_blocks = BTreeMap::new();
        for header in step_headers {
            let number = header.number;
            let raw_block_logs = grouped.remove(&number).unwrap_or_default();
            if raw_block_logs.len() > limits.max_logs_per_block
                || raw_block_logs.len() > limits.max_total_logs - total_logs
            {
                return Err(attempt.error(GapReason::LogLimitExceeded, Some(number)));
            }
            let mut logs = Vec::with_capacity(raw_block_logs.len());
            let mut transaction_hashes = BTreeMap::new();
            let mut transaction_positions = BTreeMap::new();
            let mut last_transaction = 0;
            let mut last_log_index: Option<u64> = None;
            for value in raw_block_logs {
                let emitter = value["address"]
                    .as_str()
                    .filter(|s| s.len() == 42)
                    .map(str::to_ascii_lowercase)
                    .ok_or_else(|| attempt.error(GapReason::MalformedLog, Some(number)))?;
                let pool = by_address
                    .get(&emitter)
                    .ok_or_else(|| attempt.error(GapReason::MalformedLog, Some(number)))?;
                let decoded = decode_log(value, pool)
                    .map_err(|_| attempt.error(GapReason::MalformedLog, Some(number)))?;
                if decoded.block_number != header.number
                    || !decoded.block_hash.eq_ignore_ascii_case(&header.hash)
                {
                    return Err(attempt.error(GapReason::MalformedLog, Some(number)));
                }
                if decoded.removed {
                    return Err(attempt.error(GapReason::RemovedLog, Some(number)));
                }
                // A ranged fetch trusts provider order instead of re-sorting:
                // log_index must already strictly increase within a block.
                if last_log_index.is_some_and(|last| decoded.log_index <= last) {
                    return Err(attempt.error(GapReason::ConflictingLog, Some(number)));
                }
                last_log_index = Some(decoded.log_index);
                if transaction_blocks
                    .insert(decoded.transaction_hash.clone(), header.number)
                    .is_some_and(|block| block != header.number)
                    || decoded.transaction_index < last_transaction
                    || transaction_hashes
                        .insert(decoded.transaction_index, decoded.transaction_hash.clone())
                        .is_some_and(|old| old != decoded.transaction_hash)
                    || transaction_positions
                        .insert(decoded.transaction_hash.clone(), decoded.transaction_index)
                        .is_some_and(|old| old != decoded.transaction_index)
                {
                    return Err(attempt.error(GapReason::ConflictingLog, Some(number)));
                }
                last_transaction = decoded.transaction_index;
                logs.push(decoded);
            }
            total_logs += logs.len();
            blocks.push(BlockLogs { header, logs });
        }
    }
    let final_tip = attempt.header(json!(format!("0x{:x}", target.number)), Some(target.number))?;
    if !final_tip.same_block(&target) {
        return Err(attempt.error(GapReason::TargetChanged, Some(target.number)));
    }
    if distance > 0 {
        let final_checkpoint = attempt.header(
            json!(format!("0x{:x}", checkpoint.number)),
            Some(checkpoint.number),
        )?;
        if !final_checkpoint.same_block(checkpoint) {
            return Err(attempt.error(GapReason::CheckpointChanged, Some(checkpoint.number)));
        }
    }
    Ok(BackfillBatch {
        schema_version: 1,
        network_id: "base-mainnet".into(),
        registry_digest,
        from_checkpoint: checkpoint.clone(),
        through: target,
        pool_addresses: addresses,
        abi_source_commit: SOURCE_COMMIT.into(),
        blocks,
        full_snapshot_required: true,
    })
}
