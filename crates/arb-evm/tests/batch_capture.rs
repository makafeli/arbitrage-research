use arb_adapter_api::{AdapterError, ReadMethod, ReadRpc, Result, RpcRecord, TranscriptRpc};
use arb_evm::{PoolRegistry, capture_pools};
use serde_json::{Value, json};

fn fixture() -> (Vec<PoolRegistry>, Vec<RpcRecord>) {
    (
        serde_json::from_str(include_str!("fixtures/batch-registries.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/batch-rpc.json")).unwrap(),
    )
}

struct AdvancingTipRpc {
    transcript: TranscriptRpc,
    finalized_reads: usize,
    tip_advanced: bool,
    pinned_reads_after_advance: usize,
}
impl ReadRpc for AdvancingTipRpc {
    fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value> {
        if method == ReadMethod::EthGetBlockByNumber && params[0] == "finalized" {
            self.finalized_reads += 1;
            // A second acquisition of the finalized tip would use a different block.
            if self.tip_advanced {
                return Ok(
                    json!({"number":"0x65","hash":format!("0x{}", "c".repeat(64)),
                    "parentHash":format!("0x{}", "a".repeat(64)),"timestamp":"0x3e9"}),
                );
            }
        }
        if matches!(method, ReadMethod::EthCall | ReadMethod::EthGetCode) {
            assert_eq!(
                params[1],
                json!({"blockHash":format!("0x{}", "a".repeat(64)),"requireCanonical":true})
            );
            if self.tip_advanced {
                self.pinned_reads_after_advance += 1;
            }
        }
        let response = self.transcript.call(method, params.clone())?;
        if method == ReadMethod::EthCall && params[0]["data"] == "0x1a686502" {
            self.tip_advanced = true;
        }
        Ok(response)
    }
}

#[test]
fn advancing_tip_does_not_split_the_batch_anchor() {
    let (registries, records) = fixture();
    let mut rpc = AdvancingTipRpc {
        transcript: TranscriptRpc::new(records),
        finalized_reads: 0,
        tip_advanced: false,
        pinned_reads_after_advance: 0,
    };
    let snapshots = capture_pools(&mut rpc, &registries, 777).unwrap();
    rpc.transcript.finish().unwrap();
    assert_eq!(rpc.finalized_reads, 1);
    assert!(rpc.pinned_reads_after_advance > 10);
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].pool, registries[0].pool);
    assert_eq!(snapshots[1].pool, registries[1].pool);
    assert_eq!(snapshots[0].context, snapshots[1].context);
    for snapshot in snapshots {
        assert_eq!(snapshot.quality.observed_at_ms, 777);
        assert!(snapshot.quality.coherent);
        assert!(!snapshot.quality.complete_for_quote);
        assert!(!snapshot.quality.quote_implementation_qualified);
    }
}

#[test]
fn final_reorg_invalidates_both_completed_pool_reads() {
    let (registries, mut records) = fixture();
    let record = records.last_mut().unwrap();
    let mut response: Value = serde_json::from_str(&record.response).unwrap();
    response["result"]["hash"] = json!(format!("0x{}", "c".repeat(64)));
    record.response = response.to_string();
    let mut rpc = TranscriptRpc::new(records);
    assert_eq!(
        capture_pools(&mut rpc, &registries, 100).unwrap_err().0,
        "Base block invalidated during acquisition"
    );
    rpc.finish().unwrap();
}

#[test]
fn contradictory_canonical_timestamp_invalidates_both_completed_pool_reads() {
    let (registries, mut records) = fixture();
    let record = records.last_mut().unwrap();
    let mut response: Value = serde_json::from_str(&record.response).unwrap();
    response["result"]["timestamp"] = json!("0x3e9");
    record.response = response.to_string();
    let mut rpc = TranscriptRpc::new(records);
    assert_eq!(
        capture_pools(&mut rpc, &registries, 100).unwrap_err().0,
        "Base block invalidated during acquisition"
    );
    rpc.finish().unwrap();
}

struct NoIo;
impl ReadRpc for NoIo {
    fn call(&mut self, _: ReadMethod, _: Value) -> Result<Value> {
        panic!("invalid batch must fail before RPC")
    }
}

#[test]
fn bounds_duplicates_and_late_invalid_registry_fail_before_io() {
    let (registries, _) = fixture();
    assert_eq!(
        capture_pools(&mut NoIo, &[], 100).unwrap_err().0,
        "Base capture batch requires 1..=8 pools"
    );
    assert!(capture_pools(&mut NoIo, &vec![registries[0].clone(); 9], 100).is_err());
    let mut lower = registries[0].clone();
    lower.pool = format!("0x{}", "ab".repeat(20));
    let mut upper = lower.clone();
    upper.pool = format!("0x{}", "AB".repeat(20));
    assert_eq!(
        capture_pools(&mut NoIo, &[lower, upper], 100)
            .unwrap_err()
            .0,
        "duplicate Base capture pool"
    );
    let mut invalid = registries.clone();
    invalid[1].qualification_reference.clear();
    assert!(capture_pools(&mut NoIo, &invalid, 100).is_err());
}

#[test]
fn wrong_chain_and_second_pool_identity_fail_the_entire_batch() {
    let (registries, mut records) = fixture();
    let mut response: Value = serde_json::from_str(&records[0].response).unwrap();
    response["result"] = json!("0x1");
    records[0].response = response.to_string();
    assert_eq!(
        capture_pools(&mut TranscriptRpc::new(records), &registries, 100)
            .unwrap_err()
            .0,
        "wrong EVM chain"
    );
    let (registries, mut records) = fixture();
    let record = records
        .iter_mut()
        .find(|r| {
            r.method == ReadMethod::EthCall
                && r.params[0]["to"] == registries[1].pool
                && r.params[0]["data"] == "0x0dfe1681"
        })
        .unwrap();
    let mut response: Value = serde_json::from_str(&record.response).unwrap();
    response["result"] = json!(format!("0x{}{}", "0".repeat(24), "09".repeat(20)));
    record.response = response.to_string();
    assert_eq!(
        capture_pools(&mut TranscriptRpc::new(records), &registries, 100)
            .unwrap_err()
            .0,
        "pool token identity mismatch"
    );
}

struct RejectHashPin {
    transcript: TranscriptRpc,
    requests: Vec<(ReadMethod, Value)>,
}
impl ReadRpc for RejectHashPin {
    fn call(&mut self, method: ReadMethod, params: Value) -> Result<Value> {
        self.requests.push((method, params.clone()));
        if method == ReadMethod::EthGetCode {
            return Err(AdapterError("provider rejected EIP-1898"));
        }
        self.transcript.call(method, params)
    }
}

#[test]
fn unsupported_hash_pinning_does_not_retry_against_latest_or_number() {
    let (registries, records) = fixture();
    let mut rpc = RejectHashPin {
        transcript: TranscriptRpc::new(records),
        requests: Vec::new(),
    };
    assert_eq!(
        capture_pools(&mut rpc, &registries, 100).unwrap_err().0,
        "provider rejected EIP-1898"
    );
    assert_eq!(rpc.requests.len(), 3);
    assert_eq!(rpc.requests[2].1[1]["requireCanonical"], true);
}
