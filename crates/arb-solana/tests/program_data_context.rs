use arb_adapter_api::{RpcRecord, StateContext, TranscriptRpc};
use arb_solana::{PoolRegistry, PoolSnapshot, capture_pool, capture_pools};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn fixture(batch: bool) -> (Vec<PoolRegistry>, Vec<RpcRecord>) {
    if batch {
        (
            serde_json::from_str(include_str!("fixtures/batch-registries.json")).unwrap(),
            serde_json::from_str(include_str!("fixtures/batch-rpc.json")).unwrap(),
        )
    } else {
        (
            vec![serde_json::from_str(include_str!("fixtures/registry.json")).unwrap()],
            serde_json::from_str(include_str!("fixtures/rpc.json")).unwrap(),
        )
    }
}

// Alter only the synthetic loader metadata and repin its exact bytes. A matching
// registry digest must not turn malformed metadata or future state into valid
// captured input. All pools in the batch reference this same ProgramData account.
fn program_data_metadata(
    registries: &mut [PoolRegistry],
    records: &mut [RpcRecord],
    deployment_slot: u64,
    authority_tag: u8,
    context_slot: u64,
) {
    let mut response: Value = serde_json::from_str(&records[1].response).unwrap();
    let value = &mut response["result"]["value"][6]["data"][0];
    let mut bytes = STANDARD.decode(value.as_str().unwrap()).unwrap();
    bytes[4..12].copy_from_slice(&deployment_slot.to_le_bytes());
    bytes[12] = authority_tag;
    // None can leave unused metadata bytes behind after authority revocation.
    // Both None with such bytes and Some with a 32-byte key remain supported.
    bytes[13..45].fill(17);
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    for registry in registries {
        registry.program_data_sha256.clone_from(&digest);
    }
    *value = json!(STANDARD.encode(bytes));
    response["result"]["context"]["slot"] = json!(context_slot);
    records[1].response = response.to_string();
}

fn capture(
    batch: bool,
    registries: &[PoolRegistry],
    records: Vec<RpcRecord>,
) -> arb_adapter_api::Result<Vec<PoolSnapshot>> {
    let mut rpc = TranscriptRpc::new(records);
    let result = if batch {
        capture_pools(&mut rpc, registries, 100)
    } else {
        capture_pool(&mut rpc, &registries[0], 100).map(|snapshot| vec![snapshot])
    };
    rpc.finish().unwrap();
    result
}

#[test]
fn future_program_data_is_rejected_even_when_its_digest_matches() {
    for batch in [false, true] {
        for deployment_slot in [101, u64::MAX] {
            let (mut registries, mut records) = fixture(batch);
            program_data_metadata(&mut registries, &mut records, deployment_slot, 1, 100);
            assert_eq!(
                capture(batch, &registries, records).unwrap_err().0,
                "Whirlpool program data slot exceeds account context",
                "batch={batch}, deployment_slot={deployment_slot}"
            );
        }
    }
}

#[test]
fn malformed_authority_option_is_rejected_even_when_its_digest_matches() {
    for batch in [false, true] {
        for authority_tag in [2, u8::MAX] {
            let (mut registries, mut records) = fixture(batch);
            program_data_metadata(&mut registries, &mut records, 99, authority_tag, 100);
            assert_eq!(
                capture(batch, &registries, records).unwrap_err().0,
                "unsupported Whirlpool program data layout",
                "batch={batch}, authority_tag={authority_tag}"
            );
        }
    }
}

#[test]
fn valid_program_data_metadata_preserves_unqualified_capture_and_exact_slot() {
    for batch in [false, true] {
        for authority_tag in [0, 1] {
            for (deployment_slot, context_slot) in [
                (0, 100),
                (99, 100),
                (100, 100),
                (9_007_199_254_740_993, 9_007_199_254_740_993),
            ] {
                let (mut registries, mut records) = fixture(batch);
                program_data_metadata(
                    &mut registries,
                    &mut records,
                    deployment_slot,
                    authority_tag,
                    context_slot,
                );
                let snapshots = capture(batch, &registries, records).unwrap();
                assert_eq!(snapshots.len(), registries.len());
                for snapshot in snapshots {
                    let StateContext::Solana { slot, .. } = snapshot.context else {
                        panic!("Solana capture must retain Solana context");
                    };
                    assert_eq!(slot, context_slot);
                    assert!(!snapshot.quality.coherent);
                    assert!(!snapshot.quality.complete_for_quote);
                    assert!(!snapshot.quality.quote_implementation_qualified);
                }
            }
        }
    }
}
