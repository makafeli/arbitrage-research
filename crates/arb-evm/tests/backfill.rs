//! Ordered transport transcripts are synthetic recovery fixtures, not mainnet evidence.
//!
//! Call shape pinned by issue #180 (owner decision 2026-09-19, option 1): code
//! identity is verified only at the two ends of a step (Base is Cancun,
//! EIP-6780 means a pre-existing contract's code cannot change block to
//! block), and ranged `eth_getLogs(fromBlock..toBlock)` calls replace the old
//! per-block filter. Since issue #202 (2026-09-19) those ranged calls are
//! chunked to at most `LOG_RANGE_CHUNK_BLOCKS` (10) blocks each, so a step of
//! N blocks issues `ceil(N / LOG_RANGE_CHUNK_BLOCKS)` `eth_getLogs` calls
//! (still 1 for N <= 10) instead of exactly 1. A step of N blocks over P
//! pools therefore costs `6 + 2*(P+1) + N + ceil(N / LOG_RANGE_CHUNK_BLOCKS)`
//! requests when the caller supplies an explicit target
//! (`recover_logs_through`/`recover_logs_bounded`); plain `recover_logs`
//! folds "finalized" and "target resolution" into one call, so it costs one
//! less.
use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc, RpcRecord, TranscriptRpc};
use arb_evm::{
    PoolRegistry, UNISWAP_V3_FACTORY,
    backfill::*,
    events::{BlockHeader, INITIALIZE, PoolLog},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::Cell, collections::VecDeque, rc::Rc};

fn pools() -> Vec<PoolRegistry> {
    let mut pools: Vec<PoolRegistry> =
        serde_json::from_str(include_str!("fixtures/batch-registries.json")).unwrap();
    for p in &mut pools {
        p.pool_runtime_sha256 = format!("sha256:{}", hex::encode(Sha256::digest([0x60, 0x00])));
        p.factory_runtime_sha256 = p.pool_runtime_sha256.clone();
    }
    pools
}
fn addresses() -> Vec<String> {
    pools().iter().map(|p| p.pool.clone()).collect()
}
fn hash(n: u64) -> String {
    format!("0x{n:064x}")
}
fn block(n: u64) -> Value {
    json!({"number":format!("0x{n:x}"),"hash":hash(n),"parentHash":hash(n-1),"timestamp":format!("0x{:x}",n*2)})
}
fn checkpoint() -> BlockHeader {
    BlockHeader::from_rpc(&block(100)).unwrap()
}
fn event(n: u64, index: u64) -> Value {
    json!({"address":pools()[0].pool,"blockNumber":format!("0x{n:x}"),"blockHash":hash(n),
        "transactionHash":hash(900+n),"transactionIndex":"0x0","logIndex":format!("0x{index:x}"),
        "removed":false,"topics":[INITIALIZE],"data":format!("0x{:064x}{:064x}",1_u128<<96,0)})
}
fn default_logs(checkpoint_n: u64, target_n: u64) -> Vec<Value> {
    ((checkpoint_n + 1)..=target_n)
        .map(|n| event(n, 0))
        .collect()
}
/// The block number an `event()`-shaped log fixture carries, used only to
/// sort a step's logs into the `eth_getLogs` chunk that would have returned
/// them.
fn log_block_number(entry: &Value) -> u64 {
    u64::from_str_radix(
        entry["blockNumber"]
            .as_str()
            .and_then(|s| s.strip_prefix("0x"))
            .expect("fixture log entries carry a hex blockNumber"),
        16,
    )
    .expect("fixture blockNumber is valid hex")
}
fn push(records: &mut Vec<RpcRecord>, method: ReadMethod, params: Value, result: Value) -> usize {
    let sequence = records.len() as u64;
    let index = records.len();
    records.push(RpcRecord {
        sequence,
        method,
        params,
        response: json!({"jsonrpc":"2.0","id":sequence,"result":result}).to_string(),
    });
    index
}

/// One bounded finalized step, built in exactly the request order
/// `recover_logs`/`recover_logs_through`/`recover_logs_bounded` issue it in.
/// `has_resolution` is true for the two functions that take an explicit
/// target (a separate "target resolution" fetch, confirmed-exact or capped:
/// both look identical on the wire); it is false for the plain, unbounded
/// `recover_logs`, where "finalized" doubles as the target and there is no
/// separate resolution call. Named fields let tests mutate or truncate a
/// specific call without guessing positions.
struct Step {
    records: Vec<RpcRecord>,
    checkpoint_header: usize,
    finalized_header: usize,
    target_resolution: Option<usize>,
    /// Two bounds (`from` then `through`), each factory-then-pools: length is
    /// `0` when the step covers no blocks, else `2 * (pools().len() + 1)`.
    code_checks: Vec<usize>,
    /// One `eth_getBlockByNumber` per historical block, `checkpoint_n+1..=target_n`.
    block_headers: Vec<usize>,
    /// The first (and, for a step of `LOG_RANGE_CHUNK_BLOCKS` blocks or
    /// fewer, only) ranged `eth_getLogs` call; absent when the step is
    /// empty. Kept alongside `log_chunks` so single-chunk tests can keep
    /// indexing this field directly.
    logs: Option<usize>,
    /// Every ranged `eth_getLogs` call for the step, in ascending order,
    /// each covering at most `LOG_RANGE_CHUNK_BLOCKS` blocks; empty when the
    /// step is empty.
    log_chunks: Vec<usize>,
    final_target_recheck: usize,
    final_checkpoint_recheck: Option<usize>,
}
fn step(checkpoint_n: u64, finalized_n: u64, target_n: u64, has_resolution: bool) -> Step {
    step_with_logs(
        checkpoint_n,
        finalized_n,
        target_n,
        has_resolution,
        default_logs(checkpoint_n, target_n),
    )
}
fn step_with_logs(
    checkpoint_n: u64,
    finalized_n: u64,
    target_n: u64,
    has_resolution: bool,
    logs: Vec<Value>,
) -> Step {
    let mut records = Vec::new();
    push(
        &mut records,
        ReadMethod::EthChainId,
        json!([]),
        json!("0x2105"),
    );
    let checkpoint_header = push(
        &mut records,
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{checkpoint_n:x}"), false]),
        block(checkpoint_n),
    );
    let finalized_header = push(
        &mut records,
        ReadMethod::EthGetBlockByNumber,
        json!(["finalized", false]),
        block(finalized_n),
    );
    let target_resolution = has_resolution.then(|| {
        push(
            &mut records,
            ReadMethod::EthGetBlockByNumber,
            json!([format!("0x{target_n:x}"), false]),
            block(target_n),
        )
    });
    let distance = target_n
        .checked_sub(checkpoint_n)
        .expect("fixtures only build a forward step");
    let mut code_checks = Vec::new();
    if distance > 0 {
        for bound_n in [checkpoint_n, target_n] {
            code_checks.push(push(
                &mut records,
                ReadMethod::EthGetCode,
                json!([UNISWAP_V3_FACTORY, {"blockHash":hash(bound_n),"requireCanonical":true}]),
                json!("0x6000"),
            ));
            for pool in &pools() {
                code_checks.push(push(
                    &mut records,
                    ReadMethod::EthGetCode,
                    json!([pool.pool, {"blockHash":hash(bound_n),"requireCanonical":true}]),
                    json!("0x6000"),
                ));
            }
        }
    }
    let block_headers: Vec<_> = ((checkpoint_n + 1)..=target_n)
        .map(|n| {
            push(
                &mut records,
                ReadMethod::EthGetBlockByNumber,
                json!([format!("0x{n:x}"), false]),
                block(n),
            )
        })
        .collect();
    // One eth_getLogs call per LOG_RANGE_CHUNK_BLOCKS-sized slice of the
    // step (issue #202): for a 32-block step over checkpoint c that is
    // c+1..c+10, c+11..c+20, c+21..c+30, c+31..c+32. Each chunk's expected
    // response is the subset of `logs` whose blockNumber falls in that
    // chunk's range, mirroring how the production ranged fetch is split.
    let mut log_chunks = Vec::new();
    let mut chunk_start = checkpoint_n + 1;
    while chunk_start <= target_n {
        let chunk_end = (chunk_start + LOG_RANGE_CHUNK_BLOCKS - 1).min(target_n);
        let chunk_logs: Vec<Value> = logs
            .iter()
            .filter(|entry| {
                let number = log_block_number(entry);
                (chunk_start..=chunk_end).contains(&number)
            })
            .cloned()
            .collect();
        log_chunks.push(push(
            &mut records,
            ReadMethod::EthGetLogs,
            json!([{
                "fromBlock": format!("0x{chunk_start:x}"),
                "toBlock": format!("0x{chunk_end:x}"),
                "address": addresses(),
            }]),
            json!(chunk_logs),
        ));
        chunk_start = chunk_end + 1;
    }
    let logs_index = log_chunks.first().copied();
    let final_target_recheck = push(
        &mut records,
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{target_n:x}"), false]),
        block(target_n),
    );
    let final_checkpoint_recheck = (distance > 0).then(|| {
        push(
            &mut records,
            ReadMethod::EthGetBlockByNumber,
            json!([format!("0x{checkpoint_n:x}"), false]),
            block(checkpoint_n),
        )
    });
    Step {
        records,
        checkpoint_header,
        finalized_header,
        target_resolution,
        code_checks,
        block_headers,
        logs: logs_index,
        log_chunks,
        final_target_recheck,
        final_checkpoint_recheck,
    }
}
fn change(record: &mut RpcRecord, mutate: impl FnOnce(&mut Value)) {
    let mut response: Value = serde_json::from_str(&record.response).unwrap();
    mutate(&mut response["result"]);
    record.response = response.to_string();
}
fn run(records: Vec<RpcRecord>) -> Result<BackfillBatch, BackfillError> {
    recover_logs(
        &mut TranscriptRpc::new(records),
        &pools(),
        &checkpoint(),
        BackfillLimits::default(),
        || false,
    )
}

#[test]
fn complete_range_is_hash_pinned_replayable_and_does_not_advance_the_callers_checkpoint() {
    let checkpoint = checkpoint();
    let before = checkpoint.clone();
    let fixture = step(100, 102, 102, false);
    let mut rpc = TranscriptRpc::new(fixture.records);
    let batch = recover_logs(
        &mut rpc,
        &pools(),
        &checkpoint,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc.finish().unwrap();
    assert_eq!(checkpoint, before);
    assert_eq!(batch.blocks.len(), 2);
    assert_eq!(batch.through.number, 102);
    assert!(batch.full_snapshot_required);
    assert_eq!(batch.blocks[1].logs[0].block_hash, hash(102));
    assert_eq!(batch.pool_addresses.len(), 2);
    assert_eq!(batch.schema_version, 1);
    assert_eq!(batch.network_id, "base-mainnet");
    let expected = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serde_json::to_vec(&pools()).unwrap()))
    );
    assert_eq!(batch.registry_digest, expected);
}

#[test]
fn empty_log_result_is_bound_to_the_requested_range_not_assumed_missing() {
    let mut fixture = step_with_logs(100, 102, 102, false, vec![]);
    change(&mut fixture.records[fixture.logs.unwrap()], |v| {
        *v = json!([])
    });
    let batch = run(fixture.records).unwrap();
    assert_eq!(batch.blocks.len(), 2);
    assert!(batch.blocks.iter().all(|b| b.logs.is_empty()));
    assert_eq!(batch.through.number, 102);
}

#[test]
fn caught_up_checkpoint_is_rechecked_without_log_or_code_queries() {
    let fixture = step(100, 100, 100, false);
    assert!(
        !fixture
            .records
            .iter()
            .any(|r| r.method == ReadMethod::EthGetLogs || r.method == ReadMethod::EthGetCode)
    );
    let mut rpc = TranscriptRpc::new(fixture.records);
    let batch = recover_logs(
        &mut rpc,
        &pools(),
        &checkpoint(),
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc.finish().unwrap();
    assert!(batch.blocks.is_empty());
    assert_eq!(batch.through, checkpoint());
}

#[test]
fn excessive_gap_does_not_skip_old_blocks_or_begin_unbounded_recovery() {
    let mut fixture = step(100, 101, 101, false);
    change(&mut fixture.records[fixture.finalized_header], |v| {
        *v = block(200)
    });
    // Truncate to exactly the 3 calls that must happen before the limit check
    // (chain id, checkpoint header, finalized header) and require the whole
    // transcript to be consumed: this pins that recovery does not begin
    // fetching code, block headers or logs once the gap is already too large.
    fixture.records.truncate(3);
    let mut rpc = TranscriptRpc::new(fixture.records);
    let error = recover_logs(
        &mut rpc,
        &pools(),
        &checkpoint(),
        BackfillLimits::default(),
        || false,
    )
    .unwrap_err();
    rpc.finish().unwrap();
    assert_eq!(error.reason, GapReason::BackfillLimitExceeded);
    assert_eq!(error.requested_through, Some(200));
    assert_eq!(error.checkpoint.number, 100);
}

#[test]
fn regressing_finality_or_replaced_starting_checkpoint_fails() {
    let mut fixture = step(100, 101, 101, false);
    change(&mut fixture.records[fixture.finalized_header], |v| {
        *v = block(99)
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::FinalityRegressed
    );
    for pick in [
        |f: &Step| f.checkpoint_header,
        |f: &Step| f.finalized_header,
    ] {
        let mut fixture = step(100, 100, 100, false);
        let index = pick(&fixture);
        change(&mut fixture.records[index], |v| {
            v["hash"] = json!(hash(888))
        });
        assert_eq!(
            run(fixture.records).unwrap_err().reason,
            GapReason::CheckpointChanged
        );
    }
}

#[test]
fn missing_block_height_or_parent_chain_is_not_accepted() {
    for (key, value) in [
        ("number", json!("0x67")),
        ("parentHash", json!(hash(777))),
        ("timestamp", json!("0x1")),
    ] {
        let mut fixture = step(100, 102, 102, false);
        let index = fixture.block_headers[0];
        change(&mut fixture.records[index], |v| v[key] = value);
        assert_eq!(
            run(fixture.records).unwrap_err().reason,
            GapReason::BrokenAncestry
        );
    }
    let mut fixture = step(100, 102, 102, false);
    let index = fixture.block_headers[0];
    change(&mut fixture.records[index], |v| *v = Value::Null);
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::MissingOrMalformedBlock
    );
}

#[test]
fn a_late_reorg_discards_the_whole_recovered_prefix() {
    let mut fixture = step(100, 102, 102, false);
    let index = fixture.final_target_recheck;
    change(&mut fixture.records[index], |v| {
        v["hash"] = json!(hash(123))
    });
    let error = run(fixture.records).unwrap_err();
    assert_eq!(error.reason, GapReason::TargetChanged);
    assert_eq!(error.checkpoint, checkpoint());

    let mut fixture = step(100, 102, 102, false);
    let index = fixture.final_checkpoint_recheck.unwrap();
    change(&mut fixture.records[index], |v| {
        v["parentHash"] = json!(hash(123))
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::CheckpointChanged
    );
}

#[test]
fn changed_target_header_during_walk_is_rejected_before_its_logs() {
    let mut fixture = step(100, 101, 101, false);
    let index = fixture.block_headers[0];
    change(&mut fixture.records[index], |v| {
        v["hash"] = json!(hash(666))
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::TargetChanged
    );
}

/// Pins issue #180: code identity is checked only at the two ends of a step
/// (the checkpoint block `from`, and the target block `through`) — never at
/// an interior block — because Base runs Cancun and EIP-6780 forbids a
/// pre-existing contract's code from changing between two blocks.
#[test]
fn code_identity_is_verified_only_at_the_step_bounds() {
    let p = pools().len();
    let fixture = step(100, 103, 103, false);
    assert_eq!(
        fixture.code_checks.len(),
        2 * (p + 1),
        "factory+pools at 2 bounds"
    );
    assert_eq!(
        fixture
            .records
            .iter()
            .filter(|r| r.method == ReadMethod::EthGetCode)
            .count(),
        2 * (p + 1),
        "no eth_getCode for an interior block, only the two step bounds"
    );
    for &index in &fixture.code_checks {
        for replacement in [json!("0x"), json!("0x6001"), Value::Null] {
            let mut fixture = step(100, 103, 103, false);
            change(&mut fixture.records[index], |v| *v = replacement.clone());
            assert_eq!(
                run(fixture.records).unwrap_err().reason,
                GapReason::UnexpectedContractCode
            );
        }
    }
}

#[test]
fn removed_wrong_pool_or_other_block_logs_never_enter_a_recovered_batch() {
    for (key, value, expected) in [
        ("removed", json!(true), GapReason::RemovedLog),
        (
            "address",
            json!("0x0909090909090909090909090909090909090909"),
            GapReason::MalformedLog,
        ),
        ("blockHash", json!(hash(999)), GapReason::MalformedLog),
        ("blockNumber", json!("0x66"), GapReason::MalformedLog),
        ("topics", json!([]), GapReason::MalformedLog),
    ] {
        let mut fixture = step(100, 101, 101, false);
        let index = fixture.logs.unwrap();
        change(&mut fixture.records[index], |v| v[0][key] = value);
        assert_eq!(run(fixture.records).unwrap_err().reason, expected);
    }
}

/// A reorg landing between the header fetch and the ranged log fetch shows up
/// as a log whose `blockHash` no longer matches the header already verified
/// for that height; it is rejected before any log is admitted.
#[test]
fn a_reorg_between_the_header_and_log_fetch_is_rejected() {
    let mut fixture = step(100, 101, 101, false);
    let index = fixture.logs.unwrap();
    change(&mut fixture.records[index], |v| {
        v[0]["blockHash"] = json!(hash(555))
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::MalformedLog
    );
}

#[test]
fn a_log_for_a_block_number_outside_the_step_is_rejected() {
    let mut fixture = step(100, 102, 102, false);
    let index = fixture.logs.unwrap();
    change(&mut fixture.records[index], |v| {
        *v = json!([event(101, 0), event(104, 0)])
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::MalformedLog
    );
}

#[test]
fn logs_returned_out_of_block_order_across_the_step_are_rejected() {
    let mut fixture = step(100, 103, 103, false);
    let index = fixture.logs.unwrap();
    change(&mut fixture.records[index], |v| {
        *v = json!([event(102, 0), event(101, 0), event(103, 0)])
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::MalformedLog
    );
}

/// A ranged fetch trusts the provider's own order instead of re-sorting: an
/// eth_getLogs response is expected ascending by (blockNumber, logIndex), so
/// a lower logIndex following a higher one within the same block is rejected
/// rather than silently reordered.
#[test]
fn out_of_order_log_index_within_a_block_is_rejected() {
    let mut fixture = step(100, 101, 101, false);
    let index = fixture.logs.unwrap();
    change(&mut fixture.records[index], |v| {
        *v = json!([event(101, 12), event(101, 3)])
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::ConflictingLog
    );
}

#[test]
fn duplicate_indexes_or_conflicting_transactions_fail_instead_of_being_deduplicated() {
    let duplicate = event(101, 0);
    let mut differing_hash = event(101, 1);
    differing_hash["transactionHash"] = json!(hash(800));
    let mut differing_index = event(101, 1);
    differing_index["transactionIndex"] = json!("0x1");
    let mut reversed_index = event(101, 0);
    reversed_index["transactionIndex"] = json!("0x1");
    reversed_index["transactionHash"] = json!(hash(800));
    for values in [
        json!([duplicate.clone(), duplicate.clone()]),
        json!([duplicate.clone(), differing_hash]),
        json!([duplicate.clone(), differing_index]),
        json!([reversed_index, event(101, 1)]),
    ] {
        let mut fixture = step(100, 101, 101, false);
        let index = fixture.logs.unwrap();
        change(&mut fixture.records[index], |v| *v = values);
        assert_eq!(
            run(fixture.records).unwrap_err().reason,
            GapReason::ConflictingLog
        );
    }
}

#[test]
fn per_block_and_total_log_bounds_remain_independent() {
    let mut fixture = step(100, 101, 101, false);
    let index = fixture.logs.unwrap();
    change(&mut fixture.records[index], |v| {
        *v = json!([event(101, 0), event(101, 1)])
    });
    let limits = BackfillLimits {
        max_blocks: 2,
        max_logs_per_block: 1,
        max_total_logs: 2,
    };
    assert_eq!(
        recover_logs(
            &mut TranscriptRpc::new(fixture.records),
            &pools(),
            &checkpoint(),
            limits,
            || false
        )
        .unwrap_err()
        .reason,
        GapReason::LogLimitExceeded
    );
    let limits = BackfillLimits {
        max_blocks: 2,
        max_logs_per_block: 1,
        max_total_logs: 1,
    };
    let fixture = step(100, 102, 102, false);
    let error = recover_logs(
        &mut TranscriptRpc::new(fixture.records),
        &pools(),
        &checkpoint(),
        limits,
        || false,
    )
    .unwrap_err();
    assert_eq!(error.reason, GapReason::LogLimitExceeded);
    assert_eq!(error.failed_at, Some(102));
}

struct CountingRpc {
    calls: usize,
    records: VecDeque<RpcRecord>,
    fail_at: Option<usize>,
    cancel: Option<Rc<Cell<bool>>>,
}
impl ReadRpc for CountingRpc {
    fn call(&mut self, method: ReadMethod, params: Value) -> arb_adapter_api::Result<Value> {
        let index = self.calls;
        self.calls += 1;
        if self.fail_at == Some(index) {
            return Err(AdapterError(
                "RPC HTTP error: rate limited (details redacted)",
            ));
        }
        let row = self.records.pop_front().expect("unexpected extra request");
        assert_eq!(method, row.method);
        assert_eq!(params, row.params);
        if let Some(flag) = &self.cancel {
            flag.set(true);
        }
        let value: Value = serde_json::from_str(&row.response).unwrap();
        Ok(value["result"].clone())
    }
}
fn counting() -> CountingRpc {
    CountingRpc {
        calls: 0,
        records: step(100, 102, 102, false).records.into(),
        fail_at: None,
        cancel: None,
    }
}

#[test]
fn one_provider_failure_stops_without_retry_and_preserves_trusted_checkpoint() {
    // Fixed call, a bound code check, a per-block header, the ranged log
    // call, and the final target recheck after the range.
    let fixture = step(100, 102, 102, false);
    for failure in [
        0,
        fixture.code_checks[0],
        fixture.block_headers[0],
        fixture.logs.unwrap(),
        fixture.final_target_recheck,
    ] {
        let mut rpc = counting();
        rpc.fail_at = Some(failure);
        let error = recover_logs(
            &mut rpc,
            &pools(),
            &checkpoint(),
            BackfillLimits::default(),
            || false,
        )
        .unwrap_err();
        assert_eq!(rpc.calls, failure + 1);
        assert_eq!(error.reason, GapReason::ProviderFailure);
        assert_eq!(error.checkpoint, checkpoint());
        assert!(error.transport_error.unwrap().0.contains("rate limited"));
    }
}

#[test]
fn invalid_input_and_cancellation_are_refused_before_any_request() {
    for limits in [
        BackfillLimits {
            max_blocks: 0,
            ..Default::default()
        },
        BackfillLimits {
            max_blocks: 33,
            ..Default::default()
        },
        BackfillLimits {
            max_logs_per_block: 513,
            ..Default::default()
        },
        BackfillLimits {
            max_total_logs: 4097,
            ..Default::default()
        },
    ] {
        let mut rpc = counting();
        assert_eq!(
            recover_logs(&mut rpc, &pools(), &checkpoint(), limits, || false)
                .unwrap_err()
                .reason,
            GapReason::InvalidInput
        );
        assert_eq!(rpc.calls, 0);
    }
    for registries in [
        vec![],
        vec![pools()[0].clone(); 2],
        vec![pools()[0].clone(); 9],
    ] {
        let mut rpc = counting();
        assert!(
            recover_logs(
                &mut rpc,
                &registries,
                &checkpoint(),
                Default::default(),
                || false
            )
            .is_err()
        );
        assert_eq!(rpc.calls, 0);
    }
    let mut rpc = counting();
    let mut invalid = checkpoint();
    invalid.hash = "invalid".into();
    assert!(recover_logs(&mut rpc, &pools(), &invalid, Default::default(), || false).is_err());
    assert_eq!(rpc.calls, 0);
    let mut rpc = counting();
    assert_eq!(
        recover_logs(
            &mut rpc,
            &pools(),
            &checkpoint(),
            Default::default(),
            || true
        )
        .unwrap_err()
        .reason,
        GapReason::Cancelled
    );
    assert_eq!(rpc.calls, 0);
}

#[test]
fn cancellation_after_inflight_response_does_not_accept_that_response() {
    let flag = Rc::new(Cell::new(false));
    let mut rpc = counting();
    rpc.cancel = Some(flag.clone());
    let error = recover_logs(
        &mut rpc,
        &pools(),
        &checkpoint(),
        Default::default(),
        || flag.get(),
    )
    .unwrap_err();
    assert_eq!(error.reason, GapReason::Cancelled);
    assert_eq!(rpc.calls, 1);
}

#[test]
fn wrong_chain_never_starts_header_or_log_reads() {
    let mut fixture = step(100, 101, 101, false);
    change(&mut fixture.records[0], |v| *v = json!("0x1"));
    let mut rpc = CountingRpc {
        records: fixture.records.into(),
        ..counting()
    };
    assert_eq!(
        recover_logs(
            &mut rpc,
            &pools(),
            &checkpoint(),
            Default::default(),
            || false
        )
        .unwrap_err()
        .reason,
        GapReason::WrongChain
    );
    assert_eq!(rpc.calls, 1);
}

#[test]
fn a_repeated_hash_cannot_represent_another_recovered_height() {
    let mut fixture = step(100, 102, 102, false);
    let index = fixture.block_headers[0];
    change(&mut fixture.records[index], |v| {
        v["hash"] = json!(hash(100))
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::BrokenAncestry
    );
}

#[test]
fn the_same_transaction_cannot_appear_in_two_finalized_blocks() {
    let mut fixture = step(100, 102, 102, false);
    let index = fixture.logs.unwrap();
    change(&mut fixture.records[index], |v| {
        v[1]["transactionHash"] = json!(hash(1001))
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::ConflictingLog
    );
}

/// Pins the request shape and log-to-header attribution of issue #180's
/// ranged fetch across a mix of empty and non-empty blocks, and pins the
/// resulting batch byte-for-byte against hand-decoded expectations — the
/// same content the old per-block path produced for this fixture, since
/// `arb-storage::ingestion::validate_batch` depends on the shape staying
/// identical.
#[test]
fn ranged_log_fetch_shape_and_attribution_across_mixed_blocks() {
    let logs = vec![event(101, 0), event(103, 0), event(103, 1)];
    let fixture = step_with_logs(100, 103, 103, false, logs.clone());
    let logs_record = &fixture.records[fixture.logs.unwrap()];
    assert_eq!(
        logs_record.params,
        json!([{
            "fromBlock": "0x65",
            "toBlock": "0x67",
            "address": addresses(),
        }])
    );
    let batch = run(fixture.records).unwrap();
    assert_eq!(batch.blocks.len(), 3);
    assert_eq!(
        batch
            .blocks
            .iter()
            .map(|b| b.header.number)
            .collect::<Vec<_>>(),
        [101, 102, 103]
    );
    let decode = |value: &Value| arb_evm::events::decode_log(value, &pools()[0]).unwrap();
    let expected: Vec<Vec<PoolLog>> = vec![
        vec![decode(&logs[0])],
        vec![],
        vec![decode(&logs[1]), decode(&logs[2])],
    ];
    for (block, expected) in batch.blocks.iter().zip(expected) {
        assert_eq!(block.logs, expected);
    }
    assert!(batch.blocks[1].logs.is_empty(), "block 102 had no events");
}

#[test]
fn a_single_step_costs_exactly_six_plus_two_pools_plus_one_plus_blocks_plus_one_requests() {
    let p = pools().len();
    let fixture = step(100, 110, 105, true);
    let expected = 6 + 2 * (p + 1) + 5 + 1;
    assert_eq!(fixture.records.len(), expected);
    let count = |method: ReadMethod| {
        fixture
            .records
            .iter()
            .filter(|r| r.method == method)
            .count()
    };
    assert_eq!(count(ReadMethod::EthChainId), 1);
    assert_eq!(count(ReadMethod::EthGetCode), 2 * (p + 1));
    assert_eq!(count(ReadMethod::EthGetLogs), 1);
    // checkpoint + finalized + target resolution + 5 per-block headers +
    // final target recheck + final checkpoint recheck.
    assert_eq!(count(ReadMethod::EthGetBlockByNumber), 3 + 5 + 2);

    let mut rpc = CountingRpc {
        calls: 0,
        records: fixture.records.into(),
        fail_at: None,
        cancel: None,
    };
    let batch = recover_logs_through(
        &mut rpc,
        &pools(),
        &checkpoint(),
        &BlockHeader::from_rpc(&block(105)).unwrap(),
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    assert_eq!(batch.blocks.len(), 5);
    assert_eq!(rpc.calls, expected);
}

#[test]
fn hard_bounds_accept_their_exact_edge_and_refuse_the_next_block_or_log() {
    let p = pools().len();
    let limits = BackfillLimits {
        max_blocks: 32,
        max_logs_per_block: 512,
        max_total_logs: 4096,
    };
    let fixture = step(100, 132, 132, false);
    // A 32-block step chunks its logs into ceil(32 / 10) = 4 eth_getLogs
    // calls (issue #202) instead of the single call a step this long used
    // to issue.
    assert_eq!(fixture.log_chunks.len(), 4);
    let expected_calls = 5 + 2 * (p + 1) + 32 + fixture.log_chunks.len();
    let mut rpc = CountingRpc {
        calls: 0,
        records: fixture.records.into(),
        fail_at: None,
        cancel: None,
    };
    let result = recover_logs(&mut rpc, &pools(), &checkpoint(), limits, || false).unwrap();
    assert_eq!(result.blocks.len(), 32);
    assert_eq!(rpc.calls, expected_calls);

    for last in [108, 109] {
        let logs: Vec<Value> = (101..=last)
            .flat_map(|n| (0..512).map(move |index| event(n, index)))
            .collect();
        let fixture = step_with_logs(100, last, last, false, logs);
        let result = recover_logs(
            &mut TranscriptRpc::new(fixture.records),
            &pools(),
            &checkpoint(),
            limits,
            || false,
        );
        if last == 108 {
            assert_eq!(
                result
                    .unwrap()
                    .blocks
                    .iter()
                    .map(|b| b.logs.len())
                    .sum::<usize>(),
                4096
            );
        } else {
            assert_eq!(result.unwrap_err().reason, GapReason::LogLimitExceeded);
        }
    }
}

#[test]
fn exact_capture_target_does_not_follow_a_newer_finalized_tip() {
    let start = checkpoint();
    let through = BlockHeader::from_rpc(&block(102)).unwrap();
    let fixture = step(100, 200, 102, true);
    let mut rpc = TranscriptRpc::new(fixture.records);
    let batch = recover_logs_through(
        &mut rpc,
        &pools(),
        &start,
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc.finish().unwrap();
    assert_eq!(batch.through, through);
    assert_eq!(batch.blocks.len(), 2);
    assert_eq!(start, checkpoint());
}

#[test]
fn exact_target_requires_matching_finalized_canonical_header_and_ancestry() {
    let through = BlockHeader::from_rpc(&block(102)).unwrap();
    let mut fixture = step(100, 101, 102, true);
    fixture.records.truncate(3);
    let mut rpc = TranscriptRpc::new(fixture.records);
    assert_eq!(
        recover_logs_through(
            &mut rpc,
            &pools(),
            &checkpoint(),
            &through,
            BackfillLimits::default(),
            || false
        )
        .unwrap_err()
        .reason,
        GapReason::FinalityRegressed
    );
    rpc.finish().unwrap();
    for pick in [
        |f: &Step| f.finalized_header,
        |f: &Step| f.target_resolution.unwrap(),
    ] {
        let mut fixture = step(100, 102, 102, true);
        let index = pick(&fixture);
        change(&mut fixture.records[index], |v| {
            v["hash"] = json!(hash(777))
        });
        // Only the mutated call and everything before it can be reached
        // before the mismatch is detected; truncate so `finish()` proves
        // nothing beyond it was requested.
        fixture.records.truncate(index + 1);
        let mut rpc = TranscriptRpc::new(fixture.records);
        let error = recover_logs_through(
            &mut rpc,
            &pools(),
            &checkpoint(),
            &through,
            BackfillLimits::default(),
            || false,
        )
        .unwrap_err();
        rpc.finish().unwrap();
        assert_eq!(error.reason, GapReason::TargetChanged);
        assert_eq!(error.requested_through, Some(102));
    }
    let mut fixture = step(100, 103, 102, true);
    let index = fixture.block_headers[0];
    change(&mut fixture.records[index], |v| {
        v["parentHash"] = json!(hash(777))
    });
    fixture.records.truncate(index + 1);
    let mut rpc = TranscriptRpc::new(fixture.records);
    assert_eq!(
        recover_logs_through(
            &mut rpc,
            &pools(),
            &checkpoint(),
            &through,
            BackfillLimits::default(),
            || false
        )
        .unwrap_err()
        .reason,
        GapReason::BrokenAncestry
    );
    rpc.finish().unwrap();
}

#[test]
fn exact_target_keeps_gap_limits_and_caught_up_rechecks() {
    let through = checkpoint();
    let fixture = step(100, 150, 100, true);
    let mut rpc = TranscriptRpc::new(fixture.records);
    let batch = recover_logs_through(
        &mut rpc,
        &pools(),
        &through,
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    assert!(batch.blocks.is_empty());
    rpc.finish().unwrap();

    let target = BlockHeader::from_rpc(&block(133)).unwrap();
    let mut fixture = step(100, 134, 133, true);
    fixture.records.truncate(4);
    let error = recover_logs_through(
        &mut TranscriptRpc::new(fixture.records),
        &pools(),
        &checkpoint(),
        &target,
        BackfillLimits::default(),
        || false,
    )
    .unwrap_err();
    assert_eq!(error.reason, GapReason::BackfillLimitExceeded);
}

fn bounded_walk(checkpoint_n: u64, finalized_n: u64, cap: u64) -> Step {
    step(checkpoint_n, finalized_n, cap, true)
}

#[test]
fn bounded_recovery_walks_only_the_first_max_blocks_of_a_long_range() {
    let checkpoint = checkpoint();
    let through = BlockHeader::from_rpc(&block(200)).unwrap();
    let fixture = bounded_walk(100, 200, 132);
    let mut rpc = TranscriptRpc::new(fixture.records);
    let batch = recover_logs_bounded(
        &mut rpc,
        &pools(),
        &checkpoint,
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc.finish().unwrap();
    assert_eq!(batch.through.number, 132);
    assert_eq!(batch.blocks.len(), 32);
    assert_eq!(batch.from_checkpoint, checkpoint);
    assert_eq!(
        batch
            .blocks
            .iter()
            .map(|b| b.header.number)
            .collect::<Vec<_>>(),
        (101..=132).collect::<Vec<_>>()
    );
}

#[test]
fn bounded_recovery_with_a_short_range_matches_recover_logs_through() {
    let through = BlockHeader::from_rpc(&block(104)).unwrap();
    let fixture = step(100, 120, 104, true);
    let mut expected_rpc = TranscriptRpc::new(fixture.records.clone());
    let expected = recover_logs_through(
        &mut expected_rpc,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    expected_rpc.finish().unwrap();
    let mut actual_rpc = TranscriptRpc::new(fixture.records);
    let actual = recover_logs_bounded(
        &mut actual_rpc,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    actual_rpc.finish().unwrap();
    assert_eq!(actual, expected);
}

/// At exactly `max_blocks` distance the walk must take the exact path
/// (confirm the requested header by number) in a single attempt, not the
/// capped path used for a range *longer* than `max_blocks`. This pins the
/// `>` vs `>=` comparison against `limits.max_blocks` (now 32).
#[test]
fn bounded_recovery_at_exactly_max_blocks_takes_the_exact_path() {
    let through = BlockHeader::from_rpc(&block(132)).unwrap();
    let fixture = step(100, 140, 132, true);
    assert_eq!(
        fixture.records[fixture.target_resolution.unwrap()].params,
        json!(["0x84", false]),
        "transcript must contain the requested-header confirmation of 0x84"
    );
    let mut expected_rpc = TranscriptRpc::new(fixture.records.clone());
    let expected = recover_logs_through(
        &mut expected_rpc,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    expected_rpc.finish().unwrap();
    let mut actual_rpc = TranscriptRpc::new(fixture.records);
    let actual = recover_logs_bounded(
        &mut actual_rpc,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    actual_rpc.finish().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn bounded_recovery_still_rejects_a_target_beyond_finality() {
    let through = BlockHeader::from_rpc(&block(125)).unwrap();
    let mut fixture = step(100, 120, 125, true);
    fixture.records.truncate(3);
    let mut rpc = TranscriptRpc::new(fixture.records);
    let error = recover_logs_bounded(
        &mut rpc,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap_err();
    rpc.finish().unwrap();
    assert_eq!(error.reason, GapReason::FinalityRegressed);
}

#[test]
fn exact_target_preserves_cancellation_and_late_reorg_rejection() {
    let through = BlockHeader::from_rpc(&block(102)).unwrap();
    let mut fixture = step(100, 200, 102, true);
    let index = fixture.final_target_recheck;
    change(&mut fixture.records[index], |v| {
        v["timestamp"] = json!("0x0")
    });
    assert_eq!(
        recover_logs_through(
            &mut TranscriptRpc::new(fixture.records),
            &pools(),
            &checkpoint(),
            &through,
            BackfillLimits::default(),
            || false
        )
        .unwrap_err()
        .reason,
        GapReason::TargetChanged
    );
    let mut rpc = TranscriptRpc::new(vec![]);
    assert_eq!(
        recover_logs_through(
            &mut rpc,
            &pools(),
            &checkpoint(),
            &through,
            BackfillLimits::default(),
            || true
        )
        .unwrap_err()
        .reason,
        GapReason::Cancelled
    );
    rpc.finish().unwrap();
}

/// Issue #202: the provider's free tier rejects a ranged `eth_getLogs` call
/// wider than 10 blocks, so a 32-block step must fetch its logs in
/// consecutive chunks of at most `LOG_RANGE_CHUNK_BLOCKS` blocks instead of
/// one call over the whole step. Pins the exact chunk boundaries and that
/// concatenating the chunk results still yields every block's logs in order.
#[test]
fn a_32_block_step_chunks_its_logs_into_four_calls_with_the_exact_ranges_and_returns_them_in_order()
{
    let logs = default_logs(100, 132);
    let fixture = step_with_logs(100, 132, 132, false, logs.clone());
    assert_eq!(fixture.log_chunks.len(), 4);
    for (chunk_index, (from, to)) in
        fixture
            .log_chunks
            .iter()
            .zip([(101u64, 110u64), (111, 120), (121, 130), (131, 132)])
    {
        assert_eq!(
            fixture.records[*chunk_index].params,
            json!([{
                "fromBlock": format!("0x{from:x}"),
                "toBlock": format!("0x{to:x}"),
                "address": addresses(),
            }])
        );
    }
    let batch = run(fixture.records).unwrap();
    assert_eq!(batch.blocks.len(), 32);
    let expected: Vec<_> = logs
        .iter()
        .map(|value| arb_evm::events::decode_log(value, &pools()[0]).unwrap())
        .collect();
    let actual: Vec<_> = batch
        .blocks
        .iter()
        .flat_map(|block| block.logs.clone())
        .collect();
    assert_eq!(actual, expected);
}

/// A step at or under the chunk size still issues exactly one `eth_getLogs`
/// call — chunking must not split a step that already fits.
#[test]
fn a_ten_block_step_still_issues_exactly_one_log_call() {
    let fixture = step(100, 110, 110, false);
    assert_eq!(fixture.log_chunks.len(), 1);
    assert_eq!(
        fixture.records[fixture.log_chunks[0]].params,
        json!([{
            "fromBlock": "0x65",
            "toBlock": "0x6e",
            "address": addresses(),
        }])
    );
    let batch = run(fixture.records).unwrap();
    assert_eq!(batch.blocks.len(), 10);
}

/// A provider failure partway through the chunked fetch stops the attempt
/// immediately: no retry, no partial batch, and the caller's checkpoint is
/// reported unchanged.
#[test]
fn a_failure_on_the_third_log_chunk_stops_without_a_partial_batch_and_keeps_the_checkpoint() {
    let fixture = step(100, 132, 132, false);
    assert_eq!(fixture.log_chunks.len(), 4);
    let failure = fixture.log_chunks[2];
    let mut rpc = CountingRpc {
        calls: 0,
        records: fixture.records.into(),
        fail_at: Some(failure),
        cancel: None,
    };
    let error = recover_logs(
        &mut rpc,
        &pools(),
        &checkpoint(),
        BackfillLimits::default(),
        || false,
    )
    .unwrap_err();
    assert_eq!(rpc.calls, failure + 1);
    assert_eq!(error.reason, GapReason::ProviderFailure);
    assert_eq!(error.checkpoint, checkpoint());
    assert!(error.transport_error.unwrap().0.contains("rate limited"));
}

/// A log a later chunk returns for a block that belongs to an earlier
/// chunk's already-consumed range is still caught by the existing
/// ascending-order check: chunking does not weaken it.
#[test]
fn a_log_a_later_chunk_returns_for_an_earlier_chunks_block_is_rejected() {
    let mut fixture = step(100, 115, 115, false);
    assert_eq!(fixture.log_chunks.len(), 2);
    let second_chunk = fixture.log_chunks[1];
    change(&mut fixture.records[second_chunk], |v| {
        *v = json!([event(105, 0)])
    });
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::MalformedLog
    );
}

/// A chunk whose result is not a JSON array is a malformed answer, even
/// when the earlier chunks were fine; the attempt stops right there.
#[test]
fn a_chunk_that_is_not_an_array_is_rejected_as_malformed() {
    let mut fixture = step(100, 115, 115, false);
    assert_eq!(fixture.log_chunks.len(), 2);
    let second_chunk = fixture.log_chunks[1];
    change(&mut fixture.records[second_chunk], |v| *v = json!(null));
    assert_eq!(
        run(fixture.records).unwrap_err().reason,
        GapReason::MalformedLog
    );
}
