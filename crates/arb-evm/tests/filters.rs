//! Synthetic HTTP filter protocol tests; no provider or market qualification.
use arb_adapter_api::{AdapterError, ReadMethod as M, ReadRpc};
use arb_evm::{
    PoolRegistry,
    events::INITIALIZE,
    filters::{FilterError as E, PoolFilters},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Rpc {
    replies: VecDeque<arb_adapter_api::Result<Value>>,
    calls: Vec<(M, Value)>,
    cancel_after: Option<(usize, Arc<AtomicBool>)>,
}
impl Rpc {
    fn new(values: Vec<Value>) -> Self {
        Self {
            replies: values.into_iter().map(Ok).collect(),
            calls: vec![],
            cancel_after: None,
        }
    }
}
impl ReadRpc for Rpc {
    fn call(&mut self, method: M, params: Value) -> arb_adapter_api::Result<Value> {
        self.calls.push((method, params));
        if let Some((n, flag)) = &self.cancel_after
            && self.calls.len() == *n
        {
            flag.store(true, Ordering::SeqCst);
        }
        self.replies
            .pop_front()
            .expect("unexpected network request")
    }
}
fn pools() -> Vec<PoolRegistry> {
    serde_json::from_str(include_str!("fixtures/batch-registries.json")).unwrap()
}
fn opening() -> Vec<Value> {
    vec![json!("0x2105"), json!("0x1"), json!("0x2")]
}
fn log() -> Value {
    json!({"address":pools()[0].pool,"blockNumber":"0x99","blockHash":format!("0x{}","1".repeat(64)),
        "transactionHash":format!("0x{}","2".repeat(64)),"transactionIndex":"0x0","logIndex":"0x0", "removed":true,
        "topics":[INITIALIZE],"data":format!("0x{:064x}{:064x}",1_u128<<96,0)})
}

#[test]
fn opening_binds_chain_and_exact_pool_set_then_cleanup_is_explicit() {
    let cancel = AtomicBool::new(false);
    let mut values = opening();
    values.extend([json!(true), json!(false)]);
    let mut rpc = Rpc::new(values);
    let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
    let addresses: Vec<_> = pools().iter().map(|p| p.pool.to_lowercase()).collect();
    assert_eq!(
        rpc.calls,
        vec![
            (M::EthChainId, json!([])),
            (M::EthNewBlockFilter, json!([])),
            (
                M::EthNewFilter,
                json!([{"fromBlock":"latest","toBlock":"latest","address":addresses}])
            )
        ]
    );
    filters.close(&mut rpc).unwrap();
    assert_eq!(
        rpc.calls[3..],
        [
            (M::EthUninstallFilter, json!(["0x1"])),
            (M::EthUninstallFilter, json!(["0x2"]))
        ]
    );
    filters.close(&mut rpc).unwrap();
    assert_eq!(rpc.calls.len(), 5);
    assert!(matches!(filters.poll(&mut rpc, &cancel), Err(E::Closed)));
}

#[test]
fn bad_scope_and_preexisting_cancellation_make_no_request() {
    let cancel = AtomicBool::new(false);
    for scope in [
        vec![],
        vec![pools()[0].clone(); 2],
        vec![pools()[0].clone(); 9],
    ] {
        let mut rpc = Rpc::new(vec![]);
        assert!(matches!(
            PoolFilters::open(&mut rpc, &scope, &cancel),
            Err(E::InvalidScope)
        ));
        assert!(rpc.calls.is_empty());
    }
    let mut rpc = Rpc::new(vec![]);
    cancel.store(true, Ordering::SeqCst);
    assert!(matches!(
        PoolFilters::open(&mut rpc, &pools(), &cancel),
        Err(E::Cancelled)
    ));
    assert!(rpc.calls.is_empty());
}

#[test]
fn wrong_chain_cannot_allocate_filters() {
    let mut rpc = Rpc::new(vec![json!("0x1")]);
    assert!(matches!(
        PoolFilters::open(&mut rpc, &pools(), &AtomicBool::new(false)),
        Err(E::InvalidScope)
    ));
    assert_eq!(rpc.calls.len(), 1);
}

#[test]
fn invalid_and_duplicate_filter_ids_are_not_used_or_leaked_into_requests() {
    for id in [
        json!(null),
        json!("private-url"),
        json!("0x"),
        json!("0x00"),
        json!(format!("0x{}", "f".repeat(65))),
        json!("0x1"),
    ] {
        let mut rpc = Rpc::new(vec![json!("0x2105"), json!("0x1"), id, json!(true)]);
        assert!(matches!(
            PoolFilters::open(&mut rpc, &pools(), &AtomicBool::new(false)),
            Err(E::InvalidResponse)
        ));
        assert_eq!(
            rpc.calls.last(),
            Some(&(M::EthUninstallFilter, json!(["0x1"])))
        );
        assert_eq!(rpc.calls.len(), 4);
        assert!(
            !serde_json::to_string(&rpc.calls)
                .unwrap()
                .contains("private-url")
        );
    }
}

#[test]
fn partial_allocation_failure_closes_known_id_without_retrying_creation() {
    let mut rpc = Rpc::new(vec![json!("0x2105"), json!("0x1")]);
    rpc.replies
        .push_back(Err(AdapterError("synthetic provider refusal")));
    rpc.replies.push_back(Ok(json!(true)));
    assert!(matches!(
        PoolFilters::open(&mut rpc, &pools(), &AtomicBool::new(false)),
        Err(E::Provider)
    ));
    assert_eq!(rpc.calls.len(), 4);
    assert_eq!(rpc.calls[3], (M::EthUninstallFilter, json!(["0x1"])));
}

#[test]
fn cancelled_open_after_known_id_cleans_up_without_allocating_second_filter() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut rpc = Rpc::new(vec![json!("0x2105"), json!("0x1"), json!(true)]);
    rpc.cancel_after = Some((2, cancel.clone()));
    assert!(matches!(
        PoolFilters::open(&mut rpc, &pools(), &cancel),
        Err(E::Cancelled)
    ));
    assert_eq!(rpc.calls[2].0, M::EthUninstallFilter);
}

#[test]
fn removed_and_nonfinal_hints_are_preserved_without_advancing_any_cursor() {
    let mut values = opening();
    values.extend([json!([format!("0x{}", "A".repeat(64))]), json!([log()])]);
    let mut rpc = Rpc::new(values);
    let cancel = AtomicBool::new(false);
    let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
    let hints = filters.poll(&mut rpc, &cancel).unwrap();
    assert_eq!(hints.block_hashes[0], format!("0x{}", "a".repeat(64)));
    assert!(hints.logs[0].removed);
    assert_eq!(hints.logs[0].block_number, 153);
    assert_eq!(
        rpc.calls[3..],
        [
            (M::EthGetFilterChanges, json!(["0x1"])),
            (M::EthGetFilterChanges, json!(["0x2"]))
        ]
    );
}

#[test]
fn malformed_or_foreign_reply_permanently_fences_poll() {
    let mut foreign = log();
    foreign["address"] = json!("0x0000000000000000000000000000000000000099");
    for (heads, logs) in [
        (json!([null]), json!([])),
        (json!([]), json!([{}])),
        (json!([]), json!([foreign])),
        (json!([]), json!({})),
    ] {
        let mut values = opening();
        values.extend([heads, logs]);
        let mut rpc = Rpc::new(values);
        let cancel = AtomicBool::new(false);
        let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
        assert!(filters.poll(&mut rpc, &cancel).is_err());
        let sent = rpc.calls.len();
        assert!(matches!(filters.poll(&mut rpc, &cancel), Err(E::Closed)));
        assert_eq!(rpc.calls.len(), sent);
    }
}

#[test]
fn response_and_poll_count_limits_are_hard_bounds() {
    for too_many_heads in [true, false] {
        let mut values = opening();
        values.push(if too_many_heads {
            json!(vec![format!("0x{}", "0".repeat(64)); 513])
        } else {
            json!([])
        });
        if !too_many_heads {
            values.push(json!(vec![log(); 513]));
        }
        let mut rpc = Rpc::new(values);
        let cancel = AtomicBool::new(false);
        let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
        assert!(matches!(
            filters.poll(&mut rpc, &cancel),
            Err(E::LimitExceeded)
        ));
    }
    let mut values = opening();
    values.extend(vec![json!([]); 200]);
    let mut rpc = Rpc::new(values);
    let cancel = AtomicBool::new(false);
    let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
    for _ in 0..100 {
        let h = filters.poll(&mut rpc, &cancel).unwrap();
        assert!(h.logs.is_empty() && h.block_hashes.is_empty());
    }
    let count = rpc.calls.len();
    assert!(matches!(
        filters.poll(&mut rpc, &cancel),
        Err(E::LimitExceeded)
    ));
    assert_eq!(rpc.calls.len(), count);
}

#[test]
fn cancellation_after_head_read_discards_partial_poll_and_still_allows_cleanup() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut values = opening();
    values.extend([json!([]), json!(true), json!(true)]);
    let mut rpc = Rpc::new(values);
    rpc.cancel_after = Some((4, cancel.clone()));
    let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
    assert!(matches!(filters.poll(&mut rpc, &cancel), Err(E::Cancelled)));
    filters.close(&mut rpc).unwrap();
    assert_eq!(rpc.calls.len(), 6);
    assert_eq!(rpc.calls[4].0, M::EthUninstallFilter);
    assert_eq!(rpc.calls[5].0, M::EthUninstallFilter);
}

#[test]
fn cleanup_failure_never_skips_second_id_and_does_not_retry() {
    let mut rpc = Rpc::new(opening());
    rpc.replies
        .extend([Err(AdapterError("test error")), Ok(json!(true))]);
    let mut filters = PoolFilters::open(&mut rpc, &pools(), &AtomicBool::new(false)).unwrap();
    assert_eq!(filters.close(&mut rpc), Err(E::Provider));
    assert_eq!(rpc.calls.len(), 5);
    filters.close(&mut rpc).unwrap();
    assert_eq!(rpc.calls.len(), 5);
}

#[test]
fn provider_failure_does_not_retry_or_accept_a_partial_notification_batch() {
    let mut rpc = Rpc::new(opening());
    rpc.replies.extend([
        Ok(json!([])),
        Err(AdapterError(
            "RPC HTTP error: rate limited (details redacted)",
        )),
        Ok(json!(true)),
        Ok(json!(true)),
    ]);
    let cancel = AtomicBool::new(false);
    let mut filters = PoolFilters::open(&mut rpc, &pools(), &cancel).unwrap();
    assert!(matches!(filters.poll(&mut rpc, &cancel), Err(E::Provider)));
    assert!(matches!(filters.poll(&mut rpc, &cancel), Err(E::Closed)));
    assert_eq!(rpc.calls.len(), 5);
    filters.close(&mut rpc).unwrap();
    assert_eq!(rpc.calls.len(), 7);
}
