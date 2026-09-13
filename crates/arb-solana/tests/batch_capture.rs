use arb_adapter_api::{ReadMethod, ReadRpc, Result, RpcRecord, TranscriptRpc};
use arb_solana::{PoolRegistry, capture_pools};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn fixture() -> (Vec<PoolRegistry>, Vec<RpcRecord>) {
    (
        serde_json::from_str(include_str!("fixtures/batch-registries.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/batch-rpc.json")).unwrap(),
    )
}

#[test]
fn shared_accounts_are_read_once_in_one_finalized_response() {
    let (registries, records) = fixture();
    let addresses = records[1].params[0].as_array().unwrap();
    assert_eq!(addresses.len(), 12);
    assert_eq!(
        addresses
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        12
    );
    assert!(records[1].params[1].get("minContextSlot").is_none());
    let mut rpc = TranscriptRpc::new(records);
    let snapshots = capture_pools(&mut rpc, &registries, 777).unwrap();
    rpc.finish().unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].pool, registries[0].pool);
    assert_eq!(snapshots[1].pool, registries[1].pool);
    assert_eq!(snapshots[0].state.liquidity, "1000");
    assert_eq!(snapshots[1].state.liquidity, "2000");
    assert_eq!(snapshots[0].context, snapshots[1].context);
    for snapshot in snapshots {
        assert_eq!(snapshot.quality.observed_at_ms, 777);
        assert!(!snapshot.quality.coherent);
        assert!(!snapshot.quality.complete_for_quote);
        assert!(!snapshot.quality.quote_implementation_qualified);
        assert!(
            snapshot
                .quality
                .reasons
                .iter()
                .any(|s| s == "provider-bank-context-unqualified")
        );
    }
}

#[test]
fn reversed_registry_order_maps_the_shared_accounts_to_the_right_pool() {
    let (mut registries, mut records) = fixture();
    registries.reverse();
    let old_addresses = records[1].params[0].as_array().unwrap();
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    let old_accounts = response["result"]["value"].as_array().unwrap();
    let mut addresses = Vec::new();
    let mut selected = Vec::new();
    for registry in &registries {
        for address in registry.addresses() {
            if !addresses.contains(&address) {
                let index = old_addresses.iter().position(|a| a == &address).unwrap();
                addresses.push(address);
                selected.push(old_accounts[index].clone());
            }
        }
    }
    records[1].params[0] = json!(addresses);
    response["result"]["value"] = json!(selected);
    records[1].response = response.to_string();
    let mut rpc = TranscriptRpc::new(records);
    let snapshots = capture_pools(&mut rpc, &registries, 100).unwrap();
    rpc.finish().unwrap();
    assert_eq!(snapshots[0].pool, registries[0].pool);
    assert_eq!(snapshots[0].state.liquidity, "2000");
    assert_eq!(snapshots[1].state.liquidity, "1000");
}

#[test]
fn missing_shared_or_later_pool_accounts_fail_the_entire_batch() {
    for index in [3, 8, 11] {
        let (registries, mut records) = fixture();
        let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
        response["result"]["value"][index] = Value::Null;
        records[1].response = response.to_string();
        assert_eq!(
            capture_pools(&mut TranscriptRpc::new(records), &registries, 100)
                .unwrap_err()
                .0,
            "missing required Solana account"
        );
    }
    let (registries, mut records) = fixture();
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    response["result"]["value"].as_array_mut().unwrap().pop();
    records[1].response = response.to_string();
    assert!(capture_pools(&mut TranscriptRpc::new(records), &registries, 100).is_err());
}

#[test]
fn every_pool_is_independently_checked_for_identity_and_program_hash() {
    for change in ["owner", "vault", "program-hash"] {
        let (mut registries, mut records) = fixture();
        let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
        let expected = match change {
            "owner" => {
                response["result"]["value"][8]["owner"] = json!("attacker");
                "Solana account owner or executable mismatch"
            }
            "vault" => {
                let value = &mut response["result"]["value"][9]["data"][0];
                let mut bytes = STANDARD.decode(value.as_str().unwrap()).unwrap();
                bytes[32..64].fill(1); // First pool's authority is invalid for the second vault.
                *value = json!(STANDARD.encode(bytes));
                "unsupported, frozen or mismatched token vault"
            }
            _ => {
                registries[1].program_data_sha256 = format!("sha256:{}", "0".repeat(64));
                "Whirlpool program data hash mismatch"
            }
        };
        records[1].response = response.to_string();
        assert_eq!(
            capture_pools(&mut TranscriptRpc::new(records), &registries, 100)
                .unwrap_err()
                .0,
            expected
        );
    }
}

struct NoIo;
impl ReadRpc for NoIo {
    fn call(&mut self, _: ReadMethod, _: Value) -> Result<Value> {
        panic!("invalid batch must fail before RPC")
    }
}
fn address(n: u16) -> String {
    let mut bytes = [55; 32];
    bytes[..2].copy_from_slice(&n.to_le_bytes());
    bs58::encode(bytes).into_string()
}

#[test]
fn oversize_account_union_is_rejected_without_chunking_or_rpc() {
    let (registries, _) = fixture();
    let expanded = (0..6_u16)
        .map(|n| {
            let mut registry = registries[0].clone();
            registry.pool = address(n * 20);
            registry.vault_a = address(n * 20 + 1);
            registry.vault_b = address(n * 20 + 2);
            registry.tick_arrays = (3..19).map(|a| address(n * 20 + a)).collect();
            registry.validate().unwrap();
            registry
        })
        .collect::<Vec<_>>();
    assert_eq!(
        capture_pools(&mut NoIo, &expanded, 100).unwrap_err().0,
        "Solana capture batch exceeds 100 unique accounts"
    );
}

#[test]
fn pool_bounds_duplicates_and_mixed_genesis_fail_before_io() {
    let (registries, _) = fixture();
    assert!(capture_pools(&mut NoIo, &[], 100).is_err());
    assert!(capture_pools(&mut NoIo, &vec![registries[0].clone(); 9], 100).is_err());
    assert_eq!(
        capture_pools(
            &mut NoIo,
            &[registries[0].clone(), registries[0].clone()],
            100
        )
        .unwrap_err()
        .0,
        "duplicate Solana capture pool"
    );
    let mut mixed = registries.clone();
    mixed[1].expected_genesis_hash = address(1000);
    assert_eq!(
        capture_pools(&mut NoIo, &mixed, 100).unwrap_err().0,
        "mixed Solana genesis identities in capture batch"
    );
    let mut invalid = registries;
    invalid[1].qualification_reference.clear();
    assert!(capture_pools(&mut NoIo, &invalid, 100).is_err());
}

#[test]
fn wrong_genesis_and_missing_context_are_not_accepted() {
    let (registries, mut records) = fixture();
    let mut response: Value = serde_json::from_str(&records[0].response).unwrap();
    response["result"] = json!(address(1000));
    records[0].response = response.to_string();
    assert_eq!(
        capture_pools(&mut TranscriptRpc::new(records), &registries, 100)
            .unwrap_err()
            .0,
        "Solana genesis identity mismatch"
    );
    let (registries, mut records) = fixture();
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    response["result"]["context"]["slot"] = Value::Null;
    records[1].response = response.to_string();
    assert_eq!(
        capture_pools(&mut TranscriptRpc::new(records), &registries, 100)
            .unwrap_err()
            .0,
        "missing Solana context slot"
    );
}
