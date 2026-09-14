use arb_adapter_api::{RpcRecord, TranscriptRpc};
use arb_evm::{PoolRegistry, capture_pool};
fn fixture() -> (PoolRegistry, Vec<RpcRecord>) {
    (
        serde_json::from_str(include_str!("fixtures/registry.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/rpc.json")).unwrap(),
    )
}
#[test]
fn offline_capture_decodes_exact_state_and_never_claims_quote_readiness() {
    let (registry, records) = fixture();
    let mut rpc = TranscriptRpc::new(records);
    let snapshot = capture_pool(&mut rpc, &registry, 100).unwrap();
    rpc.finish().unwrap();
    assert_eq!(snapshot.tick, -2);
    assert_eq!(snapshot.liquidity, "1000");
    assert_eq!(snapshot.initialized_ticks[&0].liquidity_net, "-1000");
    assert_eq!(snapshot.bitmap_words.len(), 2);
    assert!(!snapshot.quality.complete_for_quote);
    assert!(!snapshot.quality.quote_implementation_qualified);
}
#[test]
fn changed_runtime_and_missing_tick_fail_closed() {
    let (mut registry, records) = fixture();
    registry.pool_runtime_sha256 = format!("sha256:{}", "0".repeat(64));
    assert!(capture_pool(&mut TranscriptRpc::new(records), &registry, 100).is_err());
    let (registry, mut records) = fixture();
    records.remove(14);
    assert!(capture_pool(&mut TranscriptRpc::new(records), &registry, 100).is_err());
}
#[test]
fn reorg_rejects_previously_read_snapshot() {
    let (registry, mut records) = fixture();
    let last = records.last_mut().unwrap();
    let mut response: serde_json::Value = serde_json::from_str(&last.response).unwrap();
    response["result"]["hash"] = serde_json::json!(format!("0x{}", "c".repeat(64)));
    last.response = response.to_string();
    assert_eq!(
        capture_pool(&mut TranscriptRpc::new(records), &registry, 100)
            .unwrap_err()
            .0,
        "Base block invalidated during acquisition"
    );
}

#[test]
fn matching_hash_does_not_hide_contradictory_or_missing_canonical_context() {
    for (field, value) in [
        ("number", serde_json::json!("0x65")),
        (
            "parentHash",
            serde_json::json!(format!("0x{}", "c".repeat(64))),
        ),
        ("timestamp", serde_json::json!("0x2")),
        ("number", serde_json::json!("0x+64")),
        ("timestamp", serde_json::json!("0x+1")),
        ("number", serde_json::Value::Null),
        ("parentHash", serde_json::Value::Null),
        ("timestamp", serde_json::Value::Null),
        ("hash", serde_json::Value::Null),
        ("hash", serde_json::json!(format!("0X{}", "a".repeat(64)))),
        (
            "parentHash",
            serde_json::json!(format!("0X{}", "b".repeat(64))),
        ),
    ] {
        let (registry, mut records) = fixture();
        let last = records.last_mut().unwrap();
        let mut response: serde_json::Value = serde_json::from_str(&last.response).unwrap();
        response["result"][field] = value;
        last.response = response.to_string();
        let mut rpc = TranscriptRpc::new(records);
        assert!(
            capture_pool(&mut rpc, &registry, 100).is_err(),
            "a matching hash must not admit inconsistent {field}"
        );
        rpc.finish().unwrap();
    }
}

#[test]
fn signed_quantities_fail_before_capture_can_claim_chain_identity() {
    let (registry, mut records) = fixture();
    let mut response: serde_json::Value = serde_json::from_str(&records[0].response).unwrap();
    response["result"] = serde_json::json!("0x+2105");
    records[0].response = response.to_string();
    assert!(capture_pool(&mut TranscriptRpc::new(records), &registry, 100).is_err());
}

#[test]
fn canonical_hash_comparison_uses_bytes_without_changing_pinned_requests() {
    let (registry, mut records) = fixture();
    let last = records.last_mut().unwrap();
    let mut response: serde_json::Value = serde_json::from_str(&last.response).unwrap();
    for field in ["hash", "parentHash"] {
        let hash = response["result"][field].as_str().unwrap();
        response["result"][field] = serde_json::json!(format!(
            "0x{}",
            hash.strip_prefix("0x").unwrap().to_uppercase()
        ));
    }
    last.response = response.to_string();
    let mut rpc = TranscriptRpc::new(records);
    let snapshot = capture_pool(&mut rpc, &registry, 100).unwrap();
    assert!(snapshot.quality.coherent);
    assert!(!snapshot.quality.complete_for_quote);
    rpc.finish().unwrap();
}
