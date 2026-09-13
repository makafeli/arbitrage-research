use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc, Result, RpcRecord, TranscriptRpc};
use arb_solana::{
    PoolRegistry, PoolSnapshot, capture_pool, capture_pools, capture_pools_with_chain_time,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn fixture(time: Value) -> (Vec<PoolRegistry>, Vec<RpcRecord>) {
    let registries = serde_json::from_str(include_str!("fixtures/batch-registries.json")).unwrap();
    let mut records: Vec<RpcRecord> =
        serde_json::from_str(include_str!("fixtures/batch-rpc.json")).unwrap();
    let response: Value = serde_json::from_str(&records[1].response).unwrap();
    records.push(RpcRecord {
        sequence: 2,
        method: ReadMethod::GetBlockTime,
        params: json!([response["result"]["context"]["slot"]]),
        response: format!("{{\n\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{time}\n}}"),
    });
    (registries, records)
}

#[test]
fn time_lookup_uses_exact_shared_context_and_preserves_whole_batch_transcript() {
    let (registries, mut records) = fixture(json!(1_700_000_000));
    // Deliberately change the context to catch hard-coded, latest-slot or
    // minContextSlot lookups. The recorded call must select this exact slot.
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    response["result"]["context"]["slot"] = json!(987_654);
    records[1].response = response.to_string();
    records[2].params = json!([987_654]);
    assert_eq!(records[1].params[0].as_array().unwrap().len(), 12);
    let serialized = serde_json::to_vec(&records).unwrap();
    let mut rpc = TranscriptRpc::new(records);
    let snapshots =
        capture_pools_with_chain_time(&mut rpc, &registries, 1_700_000_005_000).unwrap();
    rpc.finish().unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].context, snapshots[1].context);
    for (snapshot, registry) in snapshots.iter().zip(&registries) {
        assert_eq!(snapshot.pool, registry.pool);
        assert_eq!(snapshot.block_time_seconds, Some(1_700_000_000));
        assert!(!snapshot.quality.coherent);
        assert!(!snapshot.quality.complete_for_quote);
        assert!(!snapshot.quality.quote_implementation_qualified);
    }
    // Replay the original full request/response transcript, including the time
    // response; no per-pool response is manufactured to obtain the same bytes.
    let mut replay = TranscriptRpc::new(serde_json::from_slice(&serialized).unwrap());
    let replayed =
        capture_pools_with_chain_time(&mut replay, &registries, 1_700_000_005_000).unwrap();
    replay.finish().unwrap();
    assert_eq!(
        serde_json::to_vec(&snapshots).unwrap(),
        serde_json::to_vec(&replayed).unwrap()
    );
}

#[test]
fn null_time_remains_unknown_with_unchanged_snapshot_bytes() {
    let (registries, records) = fixture(Value::Null);
    let mut legacy_rpc = TranscriptRpc::new(records[..2].to_vec());
    let legacy = capture_pools(&mut legacy_rpc, &registries, 100).unwrap();
    legacy_rpc.finish().unwrap();
    let mut rpc = TranscriptRpc::new(records);
    let snapshots = capture_pools_with_chain_time(&mut rpc, &registries, 100).unwrap();
    rpc.finish().unwrap();
    assert!(snapshots.iter().all(|s| s.block_time_seconds.is_none()));
    assert_eq!(
        serde_json::to_vec(&legacy).unwrap(),
        serde_json::to_vec(&snapshots).unwrap()
    );
}

#[test]
fn negative_noninteger_and_unsupported_utc_times_fail_acquisition() {
    for value in [
        json!(-1),
        json!("1700000000"),
        json!(true),
        json!(1.5),
        json!(1.0),
        json!([]),
        json!({"time": 100}),
        json!(253_402_300_800_u64),
        json!(u64::MAX),
        serde_json::from_str("18446744073709551616").unwrap(),
    ] {
        let (registries, records) = fixture(value);
        assert_eq!(
            capture_pools_with_chain_time(&mut TranscriptRpc::new(records), &registries, 100)
                .unwrap_err()
                .0,
            "invalid Solana block time"
        );
    }
}

#[test]
fn future_and_boundary_times_are_preserved_for_frozen_engine_policy() {
    for seconds in [0, 1_700_000_010, 253_402_300_799] {
        let (registries, records) = fixture(json!(seconds));
        let mut rpc = TranscriptRpc::new(records);
        let snapshots = capture_pools_with_chain_time(&mut rpc, &registries, 100).unwrap();
        rpc.finish().unwrap();
        assert!(
            snapshots
                .iter()
                .all(|s| s.block_time_seconds == Some(seconds))
        );
    }
}

#[test]
fn mismatched_slot_or_rpc_identity_is_not_accepted_as_time_evidence() {
    for changed in ["slot", "identity"] {
        let (registries, mut records) = fixture(json!(1_700_000_000));
        if changed == "slot" {
            records[2].params = json!([101]);
        } else {
            records[2].response =
                json!({"jsonrpc":"2.0","id":3,"result":1_700_000_000}).to_string();
        }
        assert!(
            capture_pools_with_chain_time(&mut TranscriptRpc::new(records), &registries, 100)
                .is_err()
        );
    }
}

#[test]
fn unavailable_or_error_responses_are_not_fabricated_as_unknown_time() {
    let (registries, records) = fixture(Value::Null);
    assert!(
        capture_pools_with_chain_time(
            &mut TranscriptRpc::new(records[..2].to_vec()),
            &registries,
            100
        )
        .is_err()
    );
    let mut records = records;
    records[2].response = json!({"jsonrpc":"2.0","id":2,"error":{"code":-32004}}).to_string();
    assert!(
        capture_pools_with_chain_time(&mut TranscriptRpc::new(records), &registries, 100).is_err()
    );

    struct FailedTimeRpc(TranscriptRpc);
    impl ReadRpc for FailedTimeRpc {
        fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value> {
            if method == ReadMethod::GetBlockTime {
                return Err(AdapterError("synthetic transport failure"));
            }
            self.0.call(method, params)
        }
    }
    let (_, records) = fixture(Value::Null);
    let mut rpc = FailedTimeRpc(TranscriptRpc::new(records[..2].to_vec()));
    assert_eq!(
        capture_pools_with_chain_time(&mut rpc, &registries, 100)
            .unwrap_err()
            .0,
        "synthetic transport failure"
    );
}

#[test]
fn failed_account_batch_never_queries_time_or_returns_partial_snapshots() {
    let (registries, mut records) = fixture(json!(100));
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    response["result"]["value"][11] = Value::Null;
    records[1].response = response.to_string();
    // No third record is provided: a lookup here would report a transcript error
    // instead of preserving the actual account-validation failure.
    let mut rpc = TranscriptRpc::new(records[..2].to_vec());
    assert_eq!(
        capture_pools_with_chain_time(&mut rpc, &registries, 100)
            .unwrap_err()
            .0,
        "missing required Solana account"
    );
    rpc.finish().unwrap();
}

#[test]
fn legacy_single_pool_json_and_digest_are_unchanged() {
    let registry: PoolRegistry =
        serde_json::from_str(include_str!("fixtures/registry.json")).unwrap();
    let records = serde_json::from_str(include_str!("fixtures/rpc.json")).unwrap();
    let mut rpc = TranscriptRpc::new(records);
    let snapshot = capture_pool(&mut rpc, &registry, 100).unwrap();
    rpc.finish().unwrap();
    let expected = include_str!("fixtures/legacy-snapshot.json").trim_end();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    assert_eq!(bytes, expected.as_bytes());
    assert_eq!(
        hex::encode(Sha256::digest(&bytes)),
        "7b8c11fd8e547e0a2ca302d7324b6b9559af7b2bca6bcb42b6a77d41067295a8"
    );
    let restored: PoolSnapshot = serde_json::from_str(expected).unwrap();
    assert!(restored.block_time_seconds.is_none());
    assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
}
