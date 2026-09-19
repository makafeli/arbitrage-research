//! Actual PostgreSQL rotation tests with synthetic inputs only.
use arb_storage::{IngestionBinding, IngestionCursor, IngestionHalt, IngestionHead, Store};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

struct Fixture {
    pool: PgPool,
    store: Store,
    operator: String,
}

fn database() -> String {
    std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required")
}

async fn fixture() -> Fixture {
    let pool = PgPool::connect(&database()).await.unwrap();
    let store = Store::from_pool(pool.clone());
    store.migrate().await.unwrap();
    Fixture {
        pool,
        store,
        operator: format!("rotation-{}", Uuid::new_v4()),
    }
}

fn head(number: u64) -> IngestionHead {
    IngestionHead {
        number,
        hash: format!("0x{number:064x}"),
        parent_hash: format!("0x{:064x}", number - 1),
        timestamp_seconds: number * 10,
    }
}

fn binding() -> IngestionBinding {
    IngestionBinding {
        schema_version: 1,
        network_id: "base-mainnet".into(),
        registry_digest: format!("sha256:{}", "a".repeat(64)),
        abi_source_commit: "b".repeat(40),
        dataset_origin: "MANUALLY_CONSTRUCTED".into(),
        pool_addresses: vec![format!("0x{}", "c".repeat(40))],
    }
}

fn batch(cursor: &IngestionCursor) -> Value {
    let through = head(cursor.checkpoint.number + 1);
    json!({
        "schema_version": 1,
        "network_id": cursor.binding.network_id,
        "registry_digest": cursor.binding.registry_digest,
        "abi_source_commit": cursor.binding.abi_source_commit,
        "pool_addresses": cursor.binding.pool_addresses,
        "from_checkpoint": cursor.checkpoint,
        "through": through,
        "blocks": [{"header": through, "logs": []}],
        "full_snapshot_required": true
    })
}

async fn seed(f: &Fixture, stream: &str) -> IngestionCursor {
    let initial = f
        .store
        .create_ingestion(&f.operator, stream, &binding(), &head(100))
        .await
        .unwrap();
    f.store
        .commit_ingestion(&f.operator, stream, &initial, batch(&initial))
        .await
        .unwrap()
}

async fn status(f: &Fixture, stream: &str, revision: i64) -> String {
    sqlx::query_scalar("SELECT continuity_status FROM ingestion_snapshot_validity WHERE operator_id=$1 AND stream_id=$2 AND snapshot_revision=$3")
        .bind(&f.operator).bind(stream).bind(revision).fetch_one(&f.pool).await.unwrap()
}

/// Snapshot of every column that must be byte-for-byte unchanged by a rotation.
async fn row_snapshot(f: &Fixture, stream: &str) -> Value {
    let row = sqlx::query(
        "SELECT state,revision,checkpoint,retained_bytes,updated_at::text FROM ingestion_streams WHERE operator_id=$1 AND stream_id=$2",
    )
    .bind(&f.operator)
    .bind(stream)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    json!({
        "state": row.get::<String, _>("state"),
        "revision": row.get::<i64, _>("revision"),
        "checkpoint": row.get::<Value, _>("checkpoint"),
        "retained_bytes": row.get::<i64, _>("retained_bytes"),
        "updated_at": row.get::<String, _>("updated_at"),
    })
}

#[tokio::test]
async fn rotation_anchors_a_new_generation_at_the_checkpoint_and_leaves_the_old_stream_untouched() {
    let f = fixture().await;
    let old_cursor = seed(&f, "old").await;
    let before = row_snapshot(&f, "old").await;

    let rotated = f
        .store
        .rotate_ingestion(&f.operator, "old", "old.g2")
        .await
        .unwrap();

    assert_eq!(rotated.revision, 0);
    assert_eq!(rotated.state, "ACTIVE");
    assert_eq!(rotated.halt_reason, None);
    assert_eq!(rotated.binding, old_cursor.binding);
    assert_eq!(rotated.checkpoint, old_cursor.checkpoint);
    let new_row = sqlx::query("SELECT initial_checkpoint FROM ingestion_streams WHERE operator_id=$1 AND stream_id='old.g2'")
        .bind(&f.operator).fetch_one(&f.pool).await.unwrap();
    assert_eq!(
        new_row.get::<Value, _>("initial_checkpoint"),
        json!(old_cursor.checkpoint)
    );

    // The old stream row is byte-for-byte unchanged.
    let after = row_snapshot(&f, "old").await;
    assert_eq!(before, after);
    // Its batches keep reading as valid, untouched by the rotation.
    assert_eq!(status(&f, "old", 1).await, "NO_KNOWN_INVALIDATION");
}

#[tokio::test]
async fn a_batch_continues_across_the_rotation_boundary_into_the_new_stream() {
    let f = fixture().await;
    let old_cursor = seed(&f, "continuity-old").await;
    let rotated = f
        .store
        .rotate_ingestion(&f.operator, "continuity-old", "continuity-old.g2")
        .await
        .unwrap();
    assert_eq!(rotated.checkpoint, old_cursor.checkpoint);

    let advanced = f
        .store
        .commit_ingestion(&f.operator, "continuity-old.g2", &rotated, batch(&rotated))
        .await
        .unwrap();

    assert_eq!(advanced.revision, 1);
    assert_eq!(advanced.checkpoint.number, old_cursor.checkpoint.number + 1);
    assert_eq!(
        status(&f, "continuity-old.g2", 1).await,
        "NO_KNOWN_INVALIDATION"
    );
    // The old stream is still exactly where the rotation left it.
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "continuity-old")
            .await
            .unwrap(),
        old_cursor
    );
}

#[tokio::test]
async fn rotating_the_same_pair_again_is_idempotent_even_after_the_new_stream_advanced() {
    let f = fixture().await;
    seed(&f, "retry-old").await;
    let first = f
        .store
        .rotate_ingestion(&f.operator, "retry-old", "retry-old.g2")
        .await
        .unwrap();
    let repeat = f
        .store
        .rotate_ingestion(&f.operator, "retry-old", "retry-old.g2")
        .await
        .unwrap();
    assert_eq!(first, repeat);

    let advanced = f
        .store
        .commit_ingestion(&f.operator, "retry-old.g2", &first, batch(&first))
        .await
        .unwrap();
    let repeat_after_advance = f
        .store
        .rotate_ingestion(&f.operator, "retry-old", "retry-old.g2")
        .await
        .unwrap();
    assert_eq!(repeat_after_advance, advanced);
}

#[tokio::test]
async fn rotating_a_halted_stream_is_rejected_and_creates_nothing() {
    let f = fixture().await;
    let cursor = seed(&f, "halted-old").await;
    f.store
        .halt_ingestion(
            &f.operator,
            "halted-old",
            &cursor,
            IngestionHalt::ContinuityLost,
        )
        .await
        .unwrap();

    assert!(
        f.store
            .rotate_ingestion(&f.operator, "halted-old", "halted-old.g2")
            .await
            .is_err()
    );
    assert!(
        f.store
            .ingestion_cursor(&f.operator, "halted-old.g2")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rotating_into_a_pre_existing_stream_with_a_different_binding_or_checkpoint_is_rejected() {
    let f = fixture().await;
    seed(&f, "clash-old").await;

    // Different binding: a pre-existing, unrelated stream must never be adopted.
    let mut other_binding = binding();
    other_binding.dataset_origin = "RECORDED_LIVE".into();
    let unrelated = f
        .store
        .create_ingestion(&f.operator, "clash-target", &other_binding, &head(100))
        .await
        .unwrap();
    assert!(
        f.store
            .rotate_ingestion(&f.operator, "clash-old", "clash-target")
            .await
            .is_err()
    );
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "clash-target")
            .await
            .unwrap(),
        unrelated
    );

    // Same binding, different initial checkpoint: still rejected.
    let different_checkpoint = f
        .store
        .create_ingestion(&f.operator, "clash-target-2", &binding(), &head(200))
        .await
        .unwrap();
    assert!(
        f.store
            .rotate_ingestion(&f.operator, "clash-old", "clash-target-2")
            .await
            .is_err()
    );
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "clash-target-2")
            .await
            .unwrap(),
        different_checkpoint
    );
}

#[tokio::test]
async fn rotating_a_stream_into_itself_is_rejected() {
    let f = fixture().await;
    seed(&f, "self-old").await;
    assert!(
        f.store
            .rotate_ingestion(&f.operator, "self-old", "self-old")
            .await
            .is_err()
    );
}
