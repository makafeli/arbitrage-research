//! Real disposable PostgreSQL tests. Fixtures have no mainnet provenance.
use arb_storage::{
    IngestionBinding, IngestionCursor, IngestionHalt, IngestionHead, Store, StoreError,
};
use serde_json::{Value, json};
use uuid::Uuid;

fn url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL is required; no skipped PostgreSQL checks")
}
fn hash(n: u64) -> String {
    format!("0x{n:064x}")
}
fn head(n: u64) -> IngestionHead {
    IngestionHead {
        number: n,
        hash: hash(n),
        parent_hash: hash(n - 1),
        timestamp_seconds: 100,
    }
}
fn binding() -> IngestionBinding {
    IngestionBinding {
        schema_version: 1,
        network_id: "base-mainnet".into(),
        registry_digest: format!("sha256:{}", "a".repeat(64)),
        abi_source_commit: "b".repeat(40),
        dataset_origin: "MANUALLY_CONSTRUCTED".into(),
        pool_addresses: vec![format!("0x{:040x}", 1)],
    }
}
async fn fixture() -> (Store, String, IngestionCursor) {
    let store = Store::connect(&url()).await.unwrap();
    store.migrate().await.unwrap();
    let op = format!("ingest-{}", Uuid::new_v4());
    let cursor = store
        .create_ingestion(&op, "stream", &binding(), &head(100))
        .await
        .unwrap();
    (store, op, cursor)
}
fn batch(cursor: &IngestionCursor, through: u64) -> Value {
    json!({"schema_version":1,"network_id":cursor.binding.network_id,"registry_digest":cursor.binding.registry_digest,
        "abi_source_commit":cursor.binding.abi_source_commit,"pool_addresses":cursor.binding.pool_addresses,
        "from_checkpoint":cursor.checkpoint,"through":head(through),"full_snapshot_required":true,
        "blocks":(cursor.checkpoint.number+1..=through).map(|n| json!({"header":head(n),"logs":[]})).collect::<Vec<_>>()})
}
#[tokio::test]
async fn restart_reads_exact_atomically_saved_batch_and_duplicate_commit_is_idempotent() {
    let (store, op, c) = fixture().await;
    let payload = batch(&c, 102);
    let first = store
        .commit_ingestion(&op, "stream", &c, payload.clone())
        .await
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.checkpoint.number, 102);
    drop(store);
    let reopened = Store::connect(&url()).await.unwrap();
    assert_eq!(
        reopened.ingestion_cursor(&op, "stream").await.unwrap(),
        first
    );
    assert_eq!(
        reopened
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap(),
        vec![payload.clone()]
    );
    assert_eq!(
        reopened
            .commit_ingestion(&op, "stream", &c, payload)
            .await
            .unwrap(),
        first
    );
    assert_eq!(
        reopened
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn concurrent_same_batch_commits_once_and_changed_stale_payload_conflicts() {
    let (store, op, c) = fixture().await;
    let p = batch(&c, 101);
    let (a, b) = tokio::join!(
        store.commit_ingestion(&op, "stream", &c, p.clone()),
        store.commit_ingestion(&op, "stream", &c, p)
    );
    assert_eq!(a.unwrap(), b.unwrap());
    assert!(matches!(
        store
            .commit_ingestion(&op, "stream", &c, batch(&c, 102))
            .await,
        Err(StoreError::Conflict(_))
    ));
    assert_eq!(
        store
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn initializing_again_never_overwrites_the_seed_or_registry_or_origin() {
    let (store, op, c) = fixture().await;
    assert_eq!(
        store
            .create_ingestion(&op, "stream", &binding(), &head(100))
            .await
            .unwrap(),
        c
    );
    assert!(
        store
            .create_ingestion(&op, "stream", &binding(), &head(101))
            .await
            .is_err()
    );
    let mut changed = binding();
    changed.dataset_origin = "RECORDED_LIVE".into();
    assert!(
        store
            .create_ingestion(&op, "stream", &changed, &head(100))
            .await
            .is_err()
    );
    changed = binding();
    changed.registry_digest = format!("sha256:{}", "c".repeat(64));
    assert!(
        store
            .create_ingestion(&op, "stream", &changed, &head(100))
            .await
            .is_err()
    );
    assert_eq!(store.ingestion_cursor(&op, "stream").await.unwrap(), c);
}
#[tokio::test]
async fn halt_is_durable_keeps_checkpoint_and_cannot_be_implicitly_rearmed() {
    let (store, op, c) = fixture().await;
    let halted = store
        .halt_ingestion(&op, "stream", &c, IngestionHalt::ContinuityLost)
        .await
        .unwrap();
    assert_eq!(halted.checkpoint, c.checkpoint);
    assert_eq!(halted.state, "HALTED");
    assert_eq!(halted.halt_reason.as_deref(), Some("CONTINUITY_LOST"));
    assert!(
        store
            .commit_ingestion(&op, "stream", &halted, batch(&halted, 101))
            .await
            .is_err()
    );
    assert!(
        store
            .commit_ingestion(&op, "stream", &c, batch(&c, 101))
            .await
            .is_err()
    );
    assert_eq!(
        Store::connect(&url())
            .await
            .unwrap()
            .ingestion_cursor(&op, "stream")
            .await
            .unwrap(),
        halted
    );
    assert!(
        store
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn stale_worker_cannot_halt_a_newer_committed_cursor() {
    let (store, op, c) = fixture().await;
    let advanced = store
        .commit_ingestion(&op, "stream", &c, batch(&c, 101))
        .await
        .unwrap();
    assert!(
        store
            .halt_ingestion(&op, "stream", &c, IngestionHalt::ProviderFailure)
            .await
            .is_err()
    );
    assert_eq!(
        store.ingestion_cursor(&op, "stream").await.unwrap(),
        advanced
    );
}
#[tokio::test]
async fn empty_same_head_has_no_new_receipt_or_fake_coverage() {
    let (store, op, c) = fixture().await;
    assert_eq!(
        store
            .commit_ingestion(&op, "stream", &c, batch(&c, 100))
            .await
            .unwrap(),
        c
    );
    assert!(
        store
            .ingestion_batches(&op, "stream", 0, 1)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn malformed_or_nonconsecutive_or_over_bound_batches_do_not_write_a_prefix() {
    let (store, op, c) = fixture().await;
    for mutate in 0..9 {
        let mut p = batch(&c, 102);
        match mutate {
            0 => p["network_id"] = json!("solana-mainnet"),
            1 => p["registry_digest"] = json!(format!("sha256:{}", "c".repeat(64))),
            2 => p["blocks"][1]["header"]["parent_hash"] = json!(hash(999)),
            3 => p["blocks"][0]["header"]["number"] = json!(103),
            4 => {
                p["blocks"].as_array_mut().unwrap().pop();
            }
            5 => p["through"]["hash"] = json!(hash(999)),
            6 => p["full_snapshot_required"] = json!(false),
            7 => p["extra"] = json!("unexpected"),
            _ => p = batch(&c, 133),
        }
        assert!(
            store.commit_ingestion(&op, "stream", &c, p).await.is_err(),
            "case {mutate}"
        );
    }
    assert_eq!(store.ingestion_cursor(&op, "stream").await.unwrap(), c);
    assert!(
        store
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn operator_scope_and_binding_cannot_cross_stream_boundaries() {
    let (store, op, c) = fixture().await;
    assert!(matches!(
        store.ingestion_cursor("someone-else", "stream").await,
        Err(StoreError::NotFound)
    ));
    assert!(
        store
            .commit_ingestion("someone-else", "stream", &c, batch(&c, 101))
            .await
            .is_err()
    );
    let mut changed = c.clone();
    changed.binding.abi_source_commit = "f".repeat(40);
    assert!(
        store
            .commit_ingestion(&op, "stream", &changed, batch(&changed, 101))
            .await
            .is_err()
    );
    assert_eq!(store.ingestion_cursor(&op, "stream").await.unwrap(), c);
}
#[tokio::test]
async fn post_insert_database_failure_rolls_back_both_batch_and_cursor() {
    let (store, op, c) = fixture().await;
    let admin = sqlx::PgPool::connect(&url()).await.unwrap();
    let name = format!("fail_{}", Uuid::new_v4().simple());
    // A scoped failure after the Store inserted the batch and updated its cursor.
    sqlx::raw_sql(&format!("CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.operator_id='{op}' THEN RAISE EXCEPTION 'injected atomicity test'; END IF; RETURN NEW; END $$; CREATE TRIGGER {name} AFTER UPDATE ON ingestion_streams FOR EACH ROW EXECUTE FUNCTION {name}();")).execute(&admin).await.unwrap();
    assert!(
        store
            .commit_ingestion(&op, "stream", &c, batch(&c, 101))
            .await
            .is_err()
    );
    assert!(
        store
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.ingestion_cursor(&op, "stream").await.unwrap(), c);
    sqlx::raw_sql(&format!(
        "DROP TRIGGER {name} ON ingestion_streams; DROP FUNCTION {name}();"
    ))
    .execute(&admin)
    .await
    .unwrap();
    assert_eq!(
        store
            .commit_ingestion(&op, "stream", &c, batch(&c, 101))
            .await
            .unwrap()
            .revision,
        1
    );
}
#[tokio::test]
async fn database_constraints_protect_history_and_unbacked_cursor_advance() {
    let (store, op, c) = fixture().await;
    let admin = sqlx::PgPool::connect(&url()).await.unwrap();
    assert!(
        sqlx::query(
            "UPDATE ingestion_streams SET revision=revision+1,checkpoint=$2 WHERE operator_id=$1"
        )
        .bind(&op)
        .bind(json!(head(101)))
        .execute(&admin)
        .await
        .is_err()
    );
    store
        .commit_ingestion(&op, "stream", &c, batch(&c, 101))
        .await
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM ingestion_batches WHERE operator_id=$1")
            .bind(&op)
            .execute(&admin)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE ingestion_batches SET payload='{}' WHERE operator_id=$1")
            .bind(&op)
            .execute(&admin)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM ingestion_streams WHERE operator_id=$1")
            .bind(&op)
            .execute(&admin)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn cancelling_a_waiting_writer_cannot_advance_a_checkpoint() {
    let (store, op, c) = fixture().await;
    let admin = sqlx::PgPool::connect(&url()).await.unwrap();
    let mut lock = admin.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM ingestion_streams WHERE operator_id=$1 FOR UPDATE")
        .bind(&op)
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    let writer_store = store.clone();
    let writer_op = op.clone();
    let old = c.clone();
    let writing = tokio::spawn(async move {
        writer_store
            .commit_ingestion(&writer_op, "stream", &old, batch(&old, 101))
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    writing.abort();
    assert!(writing.await.unwrap_err().is_cancelled());
    lock.rollback().await.unwrap();
    assert_eq!(store.ingestion_cursor(&op, "stream").await.unwrap(), c);
    assert!(
        store
            .ingestion_batches(&op, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn large_heights_remain_exact_and_page_bounds_are_enforced() {
    let (store, op, _) = fixture().await;
    let large = 9_007_199_254_740_993;
    let c = store
        .create_ingestion(&op, "large", &binding(), &head(large))
        .await
        .unwrap();
    let saved = store
        .commit_ingestion(&op, "large", &c, batch(&c, large + 1))
        .await
        .unwrap();
    assert_eq!(saved.checkpoint.number, large + 1);
    for bound in [0, 17, u32::MAX] {
        assert!(
            store
                .ingestion_batches(&op, "large", 0, bound)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn old_idempotent_commit_cannot_mask_a_later_halt() {
    let (store, op, c) = fixture().await;
    let payload = batch(&c, 101);
    let advanced = store
        .commit_ingestion(&op, "stream", &c, payload.clone())
        .await
        .unwrap();
    let halted = store
        .halt_ingestion(&op, "stream", &advanced, IngestionHalt::ProviderFailure)
        .await
        .unwrap();
    assert!(
        store
            .commit_ingestion(&op, "stream", &c, payload)
            .await
            .is_err()
    );
    assert_eq!(store.ingestion_cursor(&op, "stream").await.unwrap(), halted);
}
