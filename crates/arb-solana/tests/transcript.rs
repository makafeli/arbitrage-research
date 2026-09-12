use arb_adapter_api::{RpcRecord, TranscriptRpc};
use arb_solana::{PoolRegistry, capture_pool};
fn fixture() -> (PoolRegistry, Vec<RpcRecord>) {
    (
        serde_json::from_str(include_str!("fixtures/registry.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/rpc.json")).unwrap(),
    )
}
#[test]
fn offline_capture_checks_program_tokens_and_array_context() {
    let (registry, records) = fixture();
    let mut rpc = TranscriptRpc::new(records);
    let snapshot = capture_pool(&mut rpc, &registry, 100).unwrap();
    rpc.finish().unwrap();
    assert_eq!(snapshot.state.liquidity, "1000");
    assert_eq!(snapshot.state.sqrt_price_x64, "18446744073709551616");
    assert_eq!(
        snapshot.tick_arrays[0].initialized_ticks[0].liquidity_net,
        "1000"
    );
    assert!(!snapshot.quality.coherent);
    assert!(!snapshot.quality.complete_for_quote);
}
#[test]
fn account_owner_and_missing_array_fail_closed() {
    for field in ["owner", "missing"] {
        let (registry, mut records) = fixture();
        let mut response: serde_json::Value = serde_json::from_str(&records[1].response).unwrap();
        if field == "owner" {
            response["result"]["value"][0]["owner"] = serde_json::json!("attacker");
        } else {
            response["result"]["value"][7] = serde_json::Value::Null;
        }
        records[1].response = response.to_string();
        assert!(capture_pool(&mut TranscriptRpc::new(records), &registry, 100).is_err());
    }
}
#[test]
fn unexpected_genesis_and_upgrade_are_rejected() {
    let (mut registry, records) = fixture();
    registry.program_data_sha256 = format!("sha256:{}", "0".repeat(64));
    assert!(capture_pool(&mut TranscriptRpc::new(records), &registry, 100).is_err());
    let (registry, mut records) = fixture();
    let mut response: serde_json::Value = serde_json::from_str(&records[0].response).unwrap();
    response["result"] = serde_json::json!("wrong-genesis");
    records[0].response = response.to_string();
    assert_eq!(
        capture_pool(&mut TranscriptRpc::new(records), &registry, 100)
            .unwrap_err()
            .0,
        "Solana genesis identity mismatch"
    );
}
