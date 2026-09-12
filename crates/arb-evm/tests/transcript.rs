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
