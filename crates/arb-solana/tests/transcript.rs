use arb_adapter_api::{RpcRecord, TranscriptRpc};
use arb_solana::{PoolRegistry, capture_pool};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
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
// Add a second, independently valid fixed tick array to the single-pool fixture
// by appending its account to both the request address list and the shared
// response, exactly as a real second-array registry entry would. Only the
// start offset changes; the array's own layout, discriminator and parent-pool
// bytes are untouched, so a decode failure here isolates the pool-level
// contiguity/duplicate checks in `decode_pool_accounts` rather than the
// per-array decoder covered by the `arb_solana::tests` unit tests.
fn with_second_tick_array(second_start: i32) -> (PoolRegistry, Vec<RpcRecord>) {
    let (mut registry, mut records) = fixture();
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    let mut second = response["result"]["value"][7].clone();
    let mut bytes = STANDARD
        .decode(second["data"][0].as_str().unwrap())
        .unwrap();
    bytes[8..12].copy_from_slice(&second_start.to_le_bytes());
    second["data"][0] = json!(STANDARD.encode(&bytes));
    let second_address = bs58::encode([42_u8; 32]).into_string();
    registry.tick_arrays.push(second_address.clone());
    let mut addresses = records[1].params[0].as_array().unwrap().clone();
    addresses.push(json!(second_address));
    records[1].params[0] = json!(addresses);
    response["result"]["value"]
        .as_array_mut()
        .unwrap()
        .push(second);
    records[1].response = response.to_string();
    (registry, records)
}
#[test]
fn gap_between_tick_arrays_is_rejected_as_incomplete_quote_state() {
    // 64 is the fixture pool's tick spacing, so the fixed-array span is 5632.
    // Twice that span is individually a valid array start but skips the array
    // in between: an inconsistent, not merely absent, account context.
    let (registry, records) = with_second_tick_array(64 * 88 * 2);
    assert_eq!(
        capture_pool(&mut TranscriptRpc::new(records), &registry, 100)
            .unwrap_err()
            .0,
        "missing intermediate tick array"
    );
}
#[test]
fn duplicate_tick_array_start_is_rejected_before_the_gap_check() {
    // Same start index as the first array (0): two accounts claiming the same
    // tick range cannot be assembled into one complete, valid quote state.
    let (registry, records) = with_second_tick_array(0);
    assert_eq!(
        capture_pool(&mut TranscriptRpc::new(records), &registry, 100)
            .unwrap_err()
            .0,
        "duplicate tick array range"
    );
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
