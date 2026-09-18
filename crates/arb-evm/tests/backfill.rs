//! Ordered transport transcripts are synthetic recovery fixtures, not mainnet evidence.
use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc, RpcRecord, TranscriptRpc};
use arb_evm::{
    PoolRegistry, UNISWAP_V3_FACTORY,
    backfill::*,
    events::{BlockHeader, INITIALIZE},
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
fn records(last: u64) -> Vec<RpcRecord> {
    let mut records = Vec::new();
    let mut push = |method, params, result| {
        let sequence = records.len() as u64;
        records.push(RpcRecord {
            sequence,
            method,
            params,
            response: json!({"jsonrpc":"2.0","id":sequence,"result":result}).to_string(),
        });
    };
    push(ReadMethod::EthChainId, json!([]), json!("0x2105"));
    push(
        ReadMethod::EthGetBlockByNumber,
        json!(["0x64", false]),
        block(100),
    );
    push(
        ReadMethod::EthGetBlockByNumber,
        json!(["finalized", false]),
        block(last),
    );
    let pools = pools();
    let addresses: Vec<_> = pools.iter().map(|p| p.pool.clone()).collect();
    for n in 101..=last {
        push(
            ReadMethod::EthGetBlockByNumber,
            json!([format!("0x{n:x}"), false]),
            block(n),
        );
        for address in [
            UNISWAP_V3_FACTORY.to_string(),
            pools[0].pool.clone(),
            pools[1].pool.clone(),
        ] {
            push(
                ReadMethod::EthGetCode,
                json!([address,{"blockHash":hash(n),"requireCanonical":true}]),
                json!("0x6000"),
            );
        }
        push(
            ReadMethod::EthGetLogs,
            json!([{"blockHash":hash(n),"address":addresses}]),
            json!([event(n, 0)]),
        );
    }
    push(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{last:x}"), false]),
        block(last),
    );
    if last > 100 {
        push(
            ReadMethod::EthGetBlockByNumber,
            json!(["0x64", false]),
            block(100),
        );
    }
    records
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
    let mut rpc = TranscriptRpc::new(records(102));
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
fn empty_log_result_is_bound_to_the_requested_hash_not_assumed_missing() {
    let mut input = records(102);
    for record in &mut input {
        if record.method == ReadMethod::EthGetLogs {
            change(record, |v| *v = json!([]));
        }
    }
    let batch = run(input).unwrap();
    assert_eq!(batch.blocks.len(), 2);
    assert!(batch.blocks.iter().all(|b| b.logs.is_empty()));
    assert_eq!(batch.through.number, 102);
}

#[test]
fn caught_up_checkpoint_is_rechecked_without_log_queries() {
    let input = records(100);
    assert!(!input.iter().any(|r| r.method == ReadMethod::EthGetLogs));
    let mut rpc = TranscriptRpc::new(input);
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
    let mut input = records(101);
    change(&mut input[2], |v| *v = block(200));
    let mut rpc = TranscriptRpc::new(input[..3].to_vec());
    let error = recover_logs(
        &mut rpc,
        &pools(),
        &checkpoint(),
        BackfillLimits::default(),
        || false,
    )
    .unwrap_err();
    assert_eq!(error.reason, GapReason::BackfillLimitExceeded);
    assert_eq!(error.requested_through, Some(200));
    assert_eq!(error.checkpoint.number, 100);
    rpc.finish().unwrap();
}

#[test]
fn regressing_finality_or_replaced_starting_checkpoint_fails() {
    let mut input = records(101);
    change(&mut input[2], |v| *v = block(99));
    assert_eq!(run(input).unwrap_err().reason, GapReason::FinalityRegressed);
    for index in [1, 2] {
        let mut input = records(100);
        change(&mut input[index], |v| v["hash"] = json!(hash(888)));
        assert_eq!(run(input).unwrap_err().reason, GapReason::CheckpointChanged);
    }
}

#[test]
fn missing_block_height_or_parent_chain_is_not_accepted() {
    for (key, value) in [
        ("number", json!("0x67")),
        ("parentHash", json!(hash(777))),
        ("timestamp", json!("0x1")),
    ] {
        let mut input = records(102);
        change(&mut input[3], |v| v[key] = value);
        assert_eq!(run(input).unwrap_err().reason, GapReason::BrokenAncestry);
    }
    let mut input = records(102);
    change(&mut input[3], |v| *v = Value::Null);
    assert_eq!(
        run(input).unwrap_err().reason,
        GapReason::MissingOrMalformedBlock
    );
}

#[test]
fn a_late_reorg_discards_the_whole_recovered_prefix() {
    let mut input = records(102);
    let count = input.len();
    change(&mut input[count - 2], |v| v["hash"] = json!(hash(123)));
    let error = run(input).unwrap_err();
    assert_eq!(error.reason, GapReason::TargetChanged);
    assert_eq!(error.checkpoint, checkpoint());
    let mut input = records(102);
    let count = input.len();
    change(&mut input[count - 1], |v| {
        v["parentHash"] = json!(hash(123))
    });
    assert_eq!(run(input).unwrap_err().reason, GapReason::CheckpointChanged);
}

#[test]
fn changed_target_header_during_walk_is_rejected_before_its_logs() {
    let mut input = records(101);
    change(&mut input[3], |v| v["hash"] = json!(hash(666)));
    assert_eq!(run(input).unwrap_err().reason, GapReason::TargetChanged);
}

#[test]
fn code_identity_is_verified_at_each_historical_block() {
    for index in [4, 5, 6] {
        for replacement in [json!("0x"), json!("0x6001"), Value::Null] {
            let mut input = records(101);
            change(&mut input[index], |v| *v = replacement);
            assert_eq!(
                run(input).unwrap_err().reason,
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
        let mut input = records(101);
        change(&mut input[7], |v| v[0][key] = value);
        assert_eq!(run(input).unwrap_err().reason, expected);
    }
}

#[test]
fn out_of_order_logs_are_sorted_without_reordering_transaction_identity() {
    let mut input = records(101);
    change(&mut input[7], |v| {
        *v = json!([event(101, 12), event(101, 3)])
    });
    let batch = run(input).unwrap();
    let logs = &batch.blocks[0].logs;
    assert_eq!(
        logs.iter().map(|l| l.log_index).collect::<Vec<_>>(),
        [3, 12]
    );
    // Gaps between indexes can belong to other contracts, and are not fabricated omissions.
    assert_eq!(logs[0].transaction_hash, logs[1].transaction_hash);
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
        let mut input = records(101);
        change(&mut input[7], |v| *v = values);
        assert_eq!(run(input).unwrap_err().reason, GapReason::ConflictingLog);
    }
}

#[test]
fn per_block_and_total_log_bounds_remain_independent() {
    let mut input = records(101);
    change(&mut input[7], |v| {
        *v = json!([event(101, 0), event(101, 1)])
    });
    let limits = BackfillLimits {
        max_blocks: 2,
        max_logs_per_block: 1,
        max_total_logs: 2,
    };
    assert_eq!(
        recover_logs(
            &mut TranscriptRpc::new(input),
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
    let error = recover_logs(
        &mut TranscriptRpc::new(records(102)),
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
        records: records(102).into(),
        fail_at: None,
        cancel: None,
    }
}

#[test]
fn one_provider_failure_stops_without_retry_and_preserves_trusted_checkpoint() {
    for failure in [0, 4, 7, 13] {
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
    let mut input = records(101);
    change(&mut input[0], |v| *v = json!("0x1"));
    let mut rpc = CountingRpc {
        records: input.into(),
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
    let mut input = records(102);
    change(&mut input[3], |v| v["hash"] = json!(hash(100)));
    assert_eq!(run(input).unwrap_err().reason, GapReason::BrokenAncestry);
}

#[test]
fn the_same_transaction_cannot_appear_in_two_finalized_blocks() {
    let mut input = records(102);
    change(&mut input[12], |v| {
        v[0]["transactionHash"] = json!(hash(1001))
    });
    assert_eq!(run(input).unwrap_err().reason, GapReason::ConflictingLog);
}

#[test]
fn hard_bounds_accept_their_exact_edge_and_refuse_the_next_block_or_log() {
    let limits = BackfillLimits {
        max_blocks: 32,
        max_logs_per_block: 512,
        max_total_logs: 4096,
    };
    let mut rpc = CountingRpc {
        records: records(132).into(),
        ..counting()
    };
    let result = recover_logs(&mut rpc, &pools(), &checkpoint(), limits, || false).unwrap();
    assert_eq!(result.blocks.len(), 32);
    assert_eq!(rpc.calls, 5 + 32 * 5);
    for last in [108, 109] {
        let mut input = records(last);
        for record in &mut input {
            if record.method == ReadMethod::EthGetLogs {
                let old: Value = serde_json::from_str(&record.response).unwrap();
                let n = u64::from_str_radix(
                    old["result"][0]["blockNumber"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap();
                change(record, |v| {
                    *v = json!((0..512).map(|index| event(n, index)).collect::<Vec<_>>())
                });
            }
        }
        let result = recover_logs(
            &mut TranscriptRpc::new(input),
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

// Targeted recovery uses genuine responses for the captured height, even when
// the finalized tip has moved. These remain manually constructed transcripts.
fn targeted_records(target: u64, finalized: u64) -> Vec<RpcRecord> {
    let mut input = records(target);
    change(&mut input[2], |v| *v = block(finalized));
    input.insert(
        3,
        RpcRecord {
            sequence: 0,
            method: ReadMethod::EthGetBlockByNumber,
            params: json!([format!("0x{target:x}"), false]),
            response: json!({"jsonrpc":"2.0","id":0,"result":block(target)}).to_string(),
        },
    );
    for (index, item) in input.iter_mut().enumerate() {
        item.sequence = index as u64;
        let mut response: Value = serde_json::from_str(&item.response).unwrap();
        response["id"] = json!(index);
        item.response = response.to_string();
    }
    input
}

#[test]
fn exact_capture_target_does_not_follow_a_newer_finalized_tip() {
    let start = checkpoint();
    let through = BlockHeader::from_rpc(&block(102)).unwrap();
    let mut rpc = TranscriptRpc::new(targeted_records(102, 200));
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
    let mut input = targeted_records(102, 101);
    input.truncate(3);
    let mut rpc = TranscriptRpc::new(input);
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
    for index in [2, 3] {
        let mut input = targeted_records(102, 102);
        change(&mut input[index], |v| v["hash"] = json!(hash(777)));
        let error = recover_logs_through(
            &mut TranscriptRpc::new(input),
            &pools(),
            &checkpoint(),
            &through,
            BackfillLimits::default(),
            || false,
        )
        .unwrap_err();
        assert_eq!(error.reason, GapReason::TargetChanged);
        assert_eq!(error.requested_through, Some(102));
    }
    let mut input = targeted_records(102, 103);
    change(&mut input[4], |v| v["parentHash"] = json!(hash(777)));
    assert_eq!(
        recover_logs_through(
            &mut TranscriptRpc::new(input),
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
}

#[test]
fn exact_target_keeps_gap_limits_and_caught_up_rechecks() {
    let mut input = targeted_records(100, 150);
    let through = checkpoint();
    let mut rpc = TranscriptRpc::new(input.clone());
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
    input = targeted_records(117, 118);
    let target = BlockHeader::from_rpc(&block(117)).unwrap();
    input.truncate(4);
    let mut rpc = TranscriptRpc::new(input);
    assert_eq!(
        recover_logs_through(
            &mut rpc,
            &pools(),
            &checkpoint(),
            &target,
            BackfillLimits::default(),
            || false
        )
        .unwrap_err()
        .reason,
        GapReason::BackfillLimitExceeded
    );
    rpc.finish().unwrap();
}

fn bounded_walk_records(checkpoint_n: u64, finalized_n: u64, cap: u64) -> Vec<RpcRecord> {
    let mut records = Vec::new();
    let mut push = |method, params, result| {
        let sequence = records.len() as u64;
        records.push(RpcRecord {
            sequence,
            method,
            params,
            response: json!({"jsonrpc":"2.0","id":sequence,"result":result}).to_string(),
        });
    };
    push(ReadMethod::EthChainId, json!([]), json!("0x2105"));
    push(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{checkpoint_n:x}"), false]),
        block(checkpoint_n),
    );
    push(
        ReadMethod::EthGetBlockByNumber,
        json!(["finalized", false]),
        block(finalized_n),
    );
    push(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{cap:x}"), false]),
        block(cap),
    );
    let pools = pools();
    let addresses: Vec<_> = pools.iter().map(|p| p.pool.clone()).collect();
    for n in (checkpoint_n + 1)..=cap {
        push(
            ReadMethod::EthGetBlockByNumber,
            json!([format!("0x{n:x}"), false]),
            block(n),
        );
        for address in [
            UNISWAP_V3_FACTORY.to_string(),
            pools[0].pool.clone(),
            pools[1].pool.clone(),
        ] {
            push(
                ReadMethod::EthGetCode,
                json!([address,{"blockHash":hash(n),"requireCanonical":true}]),
                json!("0x6000"),
            );
        }
        push(
            ReadMethod::EthGetLogs,
            json!([{"blockHash":hash(n),"address":addresses}]),
            json!([event(n, 0)]),
        );
    }
    push(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{cap:x}"), false]),
        block(cap),
    );
    push(
        ReadMethod::EthGetBlockByNumber,
        json!([format!("0x{checkpoint_n:x}"), false]),
        block(checkpoint_n),
    );
    records
}

#[test]
fn bounded_recovery_walks_only_the_first_max_blocks_of_a_long_range() {
    let checkpoint = checkpoint();
    let through = BlockHeader::from_rpc(&block(120)).unwrap();
    let mut rpc = TranscriptRpc::new(bounded_walk_records(100, 120, 116));
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
    assert_eq!(batch.through.number, 116);
    assert_eq!(batch.blocks.len(), 16);
    assert_eq!(batch.from_checkpoint, checkpoint);
    assert_eq!(
        batch
            .blocks
            .iter()
            .map(|b| b.header.number)
            .collect::<Vec<_>>(),
        (101..=116).collect::<Vec<_>>()
    );
}

#[test]
fn bounded_recovery_with_a_short_range_matches_recover_logs_through() {
    let through = BlockHeader::from_rpc(&block(104)).unwrap();
    let input = targeted_records(104, 120);
    let mut rpc_through = TranscriptRpc::new(input.clone());
    let expected = recover_logs_through(
        &mut rpc_through,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc_through.finish().unwrap();
    let mut rpc_bounded = TranscriptRpc::new(input);
    let actual = recover_logs_bounded(
        &mut rpc_bounded,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc_bounded.finish().unwrap();
    assert_eq!(actual, expected);
}

/// At exactly `max_blocks` distance the walk must take the exact path (confirm
/// the requested header by number) in a single attempt, not the capped path
/// used for a range that is *longer* than `max_blocks`. This pins the `>` vs
/// `>=` comparison against `limits.max_blocks`.
#[test]
fn bounded_recovery_at_exactly_max_blocks_takes_the_exact_path() {
    let through = BlockHeader::from_rpc(&block(116)).unwrap();
    let input = targeted_records(116, 120);
    assert!(
        input
            .iter()
            .any(|r| r.method == ReadMethod::EthGetBlockByNumber
                && r.params == json!(["0x74", false])),
        "transcript must contain the requested-header confirmation of 0x74"
    );
    let mut rpc_through = TranscriptRpc::new(input.clone());
    let expected = recover_logs_through(
        &mut rpc_through,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc_through.finish().unwrap();
    let mut rpc_bounded = TranscriptRpc::new(input);
    let actual = recover_logs_bounded(
        &mut rpc_bounded,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap();
    rpc_bounded.finish().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn bounded_recovery_still_rejects_a_target_beyond_finality() {
    let through = BlockHeader::from_rpc(&block(125)).unwrap();
    let mut input = targeted_records(125, 120);
    input.truncate(3);
    let mut rpc = TranscriptRpc::new(input);
    let error = recover_logs_bounded(
        &mut rpc,
        &pools(),
        &checkpoint(),
        &through,
        BackfillLimits::default(),
        || false,
    )
    .unwrap_err();
    assert_eq!(error.reason, GapReason::FinalityRegressed);
    rpc.finish().unwrap();
}

#[test]
fn exact_target_preserves_cancellation_and_late_reorg_rejection() {
    let through = BlockHeader::from_rpc(&block(102)).unwrap();
    let mut input = targeted_records(102, 200);
    let last = input.len() - 2;
    change(&mut input[last], |v| v["timestamp"] = json!("0x0"));
    assert_eq!(
        recover_logs_through(
            &mut TranscriptRpc::new(input),
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
