//! Actual PostgreSQL continuity/audit tests with synthetic inputs only.
use arb_storage::{IngestionBinding, IngestionCursor, IngestionHalt, IngestionHead, Store};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const GATE: &str = "SELECT require_current_ingestion_snapshot($1,$2,$3,$4)";
const HALT: &str = "UPDATE ingestion_streams SET state='HALTED',halt_reason='CONTINUITY_LOST',revision=revision+1,updated_at=clock_timestamp() WHERE operator_id=$1 AND stream_id=$2";

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
        operator: format!("validity-{}", Uuid::new_v4()),
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

async fn digest(f: &Fixture, stream: &str, revision: i64) -> String {
    sqlx::query_scalar("SELECT payload_digest FROM ingestion_batches WHERE operator_id=$1 AND stream_id=$2 AND revision=$3")
        .bind(&f.operator).bind(stream).bind(revision).fetch_one(&f.pool).await.unwrap()
}

async fn status(f: &Fixture, stream: &str, revision: i64) -> String {
    sqlx::query_scalar("SELECT continuity_status FROM ingestion_snapshot_validity WHERE operator_id=$1 AND stream_id=$2 AND snapshot_revision=$3")
        .bind(&f.operator).bind(stream).bind(revision).fetch_one(&f.pool).await.unwrap()
}

async fn audit_count(f: &Fixture, stream: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM ingestion_invalidations WHERE operator_id=$1 AND stream_id=$2",
    )
    .bind(&f.operator)
    .bind(stream)
    .fetch_one(&f.pool)
    .await
    .unwrap()
}

async fn gate(f: &Fixture, stream: &str, revision: i64, hash: &str) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(GATE)
        .bind(&f.operator)
        .bind(stream)
        .bind(revision)
        .bind(hash)
        .fetch_one(&f.pool)
        .await
}

fn has_code(error: sqlx::Error, expected: &str) {
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some(expected)
    );
}

#[tokio::test]
async fn every_terminal_reason_invalidates_prior_snapshots_without_rewriting_payloads() {
    let f = fixture().await;
    for (reason, name) in [
        (IngestionHalt::ContinuityLost, "CONTINUITY_LOST"),
        (IngestionHalt::ProviderFailure, "PROVIDER_FAILURE"),
        (IngestionHalt::ResourceLimit, "RESOURCE_LIMIT"),
        (IngestionHalt::InvalidInput, "INVALID_INPUT"),
    ] {
        let first = seed(&f, name).await;
        let current = f
            .store
            .commit_ingestion(&f.operator, name, &first, batch(&first))
            .await
            .unwrap();
        let before = f
            .store
            .ingestion_batches(&f.operator, name, 0, 16)
            .await
            .unwrap();
        let hash = digest(&f, name, 1).await;
        assert_eq!(status(&f, name, 1).await, "NO_KNOWN_INVALIDATION");
        assert_eq!(gate(&f, name, 1, &hash).await.unwrap(), before[0]);
        let halted = f
            .store
            .halt_ingestion(&f.operator, name, &current, reason)
            .await
            .unwrap();
        assert_eq!(halted.checkpoint, current.checkpoint);
        assert_eq!(halted.revision, current.revision + 1);
        assert_eq!(status(&f, name, 1).await, "INVALIDATED");
        assert_eq!(status(&f, name, 2).await, "INVALIDATED");
        has_code(gate(&f, name, 1, &hash).await.unwrap_err(), "55000");
        let after = f
            .store
            .ingestion_batches(&f.operator, name, 0, 16)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&before).unwrap(),
            serde_json::to_vec(&after).unwrap()
        );
        assert_eq!(digest(&f, name, 1).await, hash);
        assert_eq!(audit_count(&f, name).await, 1);
        let audit = sqlx::query(
            "SELECT * FROM ingestion_invalidations WHERE operator_id=$1 AND stream_id=$2",
        )
        .bind(&f.operator)
        .bind(name)
        .fetch_one(&f.pool)
        .await
        .unwrap();
        assert_eq!(audit.get::<String, _>("reason"), name);
        assert_eq!(
            audit.get::<String, _>("policy_version"),
            "base-finalized-stream-v1"
        );
        assert_eq!(audit.get::<String, _>("source"), "STREAM_TRANSITION");
        assert_eq!(
            audit.get::<Value, _>("checkpoint"),
            json!(current.checkpoint)
        );
        assert_eq!(audit.get::<Value, _>("binding"), json!(current.binding));
    }
}

#[tokio::test]
async fn invalidation_is_scoped_to_exact_operator_and_stream() {
    let f = fixture().await;
    let other = Fixture {
        pool: f.pool.clone(),
        store: f.store.clone(),
        operator: format!("validity-{}", Uuid::new_v4()),
    };
    let cursor = seed(&f, "same").await;
    seed(&f, "unrelated").await;
    seed(&other, "same").await;
    f.store
        .halt_ingestion(&f.operator, "same", &cursor, IngestionHalt::ContinuityLost)
        .await
        .unwrap();
    assert_eq!(status(&f, "same", 1).await, "INVALIDATED");
    assert_eq!(status(&f, "unrelated", 1).await, "NO_KNOWN_INVALIDATION");
    assert_eq!(status(&other, "same", 1).await, "NO_KNOWN_INVALIDATION");
    assert_eq!(audit_count(&f, "unrelated").await, 0);
    assert_eq!(audit_count(&other, "same").await, 0);
}

#[tokio::test]
async fn invalidation_survives_connection_restart_and_cannot_rearm() {
    let f = fixture().await;
    let cursor = seed(&f, "restart").await;
    let hash = digest(&f, "restart", 1).await;
    let halted = f
        .store
        .halt_ingestion(
            &f.operator,
            "restart",
            &cursor,
            IngestionHalt::ProviderFailure,
        )
        .await
        .unwrap();
    let reopened = Store::connect(&database()).await.unwrap();
    assert_eq!(
        reopened
            .ingestion_cursor(&f.operator, "restart")
            .await
            .unwrap(),
        halted
    );
    assert!(
        reopened
            .commit_ingestion(&f.operator, "restart", &cursor, batch(&cursor))
            .await
            .is_err()
    );
    let pool = PgPool::connect(&database()).await.unwrap();
    let error = sqlx::query_scalar::<_, Value>(GATE)
        .bind(&f.operator)
        .bind("restart")
        .bind(1_i64)
        .bind(&hash)
        .fetch_one(&pool)
        .await
        .unwrap_err();
    has_code(error, "55000");
    assert_eq!(audit_count(&f, "restart").await, 1);
    pool.close().await;
}

#[tokio::test]
async fn rejected_and_rolled_back_halts_leave_no_false_invalidation() {
    let f = fixture().await;
    let cursor = seed(&f, "rollback").await;
    let mut stale = cursor.clone();
    stale.revision = 0;
    assert!(
        f.store
            .halt_ingestion(&f.operator, "rollback", &stale, IngestionHalt::InvalidInput)
            .await
            .is_err()
    );
    assert_eq!(audit_count(&f, "rollback").await, 0);
    let mut tx = f.pool.begin().await.unwrap();
    sqlx::query(HALT)
        .bind(&f.operator)
        .bind("rollback")
        .execute(&mut *tx)
        .await
        .unwrap();
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM ingestion_invalidations WHERE operator_id=$1 AND stream_id='rollback'")
        .bind(&f.operator).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(pending, 1);
    assert_eq!(audit_count(&f, "rollback").await, 0);
    tx.rollback().await.unwrap();
    assert_eq!(audit_count(&f, "rollback").await, 0);
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "rollback")
            .await
            .unwrap(),
        cursor
    );
    assert_eq!(status(&f, "rollback", 1).await, "NO_KNOWN_INVALIDATION");
}

#[tokio::test]
async fn audit_and_batch_history_reject_update_delete_and_false_insertion() {
    let f = fixture().await;
    let cursor = seed(&f, "immutable").await;
    let fabricated = sqlx::query("INSERT INTO ingestion_invalidations(operator_id,stream_id,invalidation_revision,reason,binding,checkpoint,source,detected_at) SELECT operator_id,stream_id,revision,'PROVIDER_FAILURE',binding,checkpoint,'STREAM_TRANSITION',updated_at FROM ingestion_streams WHERE operator_id=$1 AND stream_id='immutable'")
        .bind(&f.operator).execute(&f.pool).await;
    assert!(fabricated.is_err());
    f.store
        .halt_ingestion(
            &f.operator,
            "immutable",
            &cursor,
            IngestionHalt::ContinuityLost,
        )
        .await
        .unwrap();
    for statement in [
        "UPDATE ingestion_invalidations SET reason='PROVIDER_FAILURE' WHERE operator_id=$1",
        "DELETE FROM ingestion_invalidations WHERE operator_id=$1",
        "UPDATE ingestion_batches SET payload='{}' WHERE operator_id=$1",
        "DELETE FROM ingestion_batches WHERE operator_id=$1",
    ] {
        assert!(
            sqlx::query(statement)
                .bind(&f.operator)
                .execute(&f.pool)
                .await
                .is_err()
        );
    }
    assert_eq!(audit_count(&f, "immutable").await, 1);
    assert_eq!(status(&f, "immutable", 1).await, "INVALIDATED");
}

#[tokio::test]
async fn exact_reference_and_missing_state_fail_with_fixed_errors() {
    let f = fixture().await;
    seed(&f, "references").await;
    let hash = digest(&f, "references", 1).await;
    for revision in [-1_i64, 0, 4097] {
        has_code(
            gate(&f, "references", revision, &hash).await.unwrap_err(),
            "22023",
        );
    }
    has_code(gate(&f, "references", 2, &hash).await.unwrap_err(), "P0002");
    has_code(gate(&f, "missing", 1, &hash).await.unwrap_err(), "P0002");
    has_code(
        gate(&f, "references", 1, &format!("sha256:{}", "f".repeat(64)))
            .await
            .unwrap_err(),
        "22023",
    );
    let error = gate(&f, "references", 1, "PRIVATE_SENTINEL")
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().message(),
        "invalid snapshot reference"
    );
    let rejected = sqlx::query_scalar::<_, Value>(GATE)
        .bind(Option::<String>::None)
        .bind("references")
        .bind(1_i64)
        .bind(&hash)
        .fetch_one(&f.pool)
        .await
        .unwrap_err();
    has_code(rejected, "22023");
    assert_eq!(audit_count(&f, "references").await, 0);
}

#[tokio::test]
async fn gate_retains_shared_lock_until_caller_transaction_commits() {
    let f = fixture().await;
    let cursor = seed(&f, "gate-first").await;
    let hash = digest(&f, "gate-first", 1).await;
    let mut tx = f.pool.begin().await.unwrap();
    let payload: Value = sqlx::query_scalar(GATE)
        .bind(&f.operator)
        .bind("gate-first")
        .bind(1_i64)
        .bind(&hash)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(payload["through"], json!(cursor.checkpoint));
    let blocked = sqlx::query_scalar::<_, i32>("SELECT 1 FROM ingestion_streams WHERE operator_id=$1 AND stream_id='gate-first' FOR UPDATE NOWAIT")
        .bind(&f.operator).fetch_one(&f.pool).await.unwrap_err();
    has_code(blocked, "55P03");
    tx.commit().await.unwrap();
    f.store
        .halt_ingestion(
            &f.operator,
            "gate-first",
            &cursor,
            IngestionHalt::ContinuityLost,
        )
        .await
        .unwrap();
    has_code(gate(&f, "gate-first", 1, &hash).await.unwrap_err(), "55000");
}

#[tokio::test]
async fn held_halt_excludes_gate_and_rollback_restores_eligibility() {
    let f = fixture().await;
    seed(&f, "halt-first").await;
    let hash = digest(&f, "halt-first", 1).await;
    let mut tx = f.pool.begin().await.unwrap();
    sqlx::query(HALT)
        .bind(&f.operator)
        .bind("halt-first")
        .execute(&mut *tx)
        .await
        .unwrap();
    // The actual gate's bounded lock wait must fail, not return the older MVCC row.
    has_code(gate(&f, "halt-first", 1, &hash).await.unwrap_err(), "55P03");
    tx.rollback().await.unwrap();
    assert!(gate(&f, "halt-first", 1, &hash).await.is_ok());
    assert_eq!(audit_count(&f, "halt-first").await, 0);
}

#[tokio::test]
async fn concurrent_terminal_reports_leave_exactly_one_matching_audit_record() {
    let f = fixture().await;
    let cursor = seed(&f, "race").await;
    let (first, second) = tokio::join!(
        f.store
            .halt_ingestion(&f.operator, "race", &cursor, IngestionHalt::ContinuityLost),
        f.store
            .halt_ingestion(&f.operator, "race", &cursor, IngestionHalt::ProviderFailure)
    );
    assert_ne!(first.is_ok(), second.is_ok());
    assert_eq!(audit_count(&f, "race").await, 1);
    assert_eq!(status(&f, "race", 1).await, "INVALIDATED");
    let matching: bool = sqlx::query_scalar("SELECT i.reason=s.halt_reason AND i.invalidation_revision=s.revision AND i.checkpoint=s.checkpoint FROM ingestion_invalidations i JOIN ingestion_streams s USING(operator_id,stream_id) WHERE operator_id=$1 AND stream_id='race'")
        .bind(&f.operator).fetch_one(&f.pool).await.unwrap();
    assert!(matching);
}

#[tokio::test]
async fn upgrade_records_old_halt_without_rewriting_its_detection_or_payload() {
    let f = fixture().await;
    let mut tx = f.pool.begin().await.unwrap();
    // Generated identifier only. The schema and its objects vanish on rollback.
    let schema = format!("validity_upgrade_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!(
        "CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema}, pg_catalog"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    for migration in [
        include_str!("../../../migrations/0001_control.sql"),
        include_str!("../../../migrations/0002_decisions_and_paper.sql"),
        include_str!("../../../migrations/0003_collection_attempts.sql"),
        include_str!("../../../migrations/0004_cost_assessments.sql"),
        include_str!("../../../migrations/0005_ingestion_checkpoints.sql"),
        include_str!("../../../migrations/0006_operator_account.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO ingestion_streams(operator_id,stream_id,binding,initial_checkpoint,checkpoint) VALUES('upgrade','old',$1,$2,$2)")
        .bind(json!(binding())).bind(json!(head(100))).execute(&mut *tx).await.unwrap();
    let hash = format!("sha256:{}", "d".repeat(64));
    sqlx::query("INSERT INTO ingestion_batches(operator_id,stream_id,revision,payload_digest,payload,payload_bytes,checkpoint) VALUES('upgrade','old',1,$1,'{}',2,$2)")
        .bind(&hash).bind(json!(head(101))).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE ingestion_streams SET checkpoint=$1,revision=1,retained_bytes=2 WHERE operator_id='upgrade' AND stream_id='old'")
        .bind(json!(head(101))).execute(&mut *tx).await.unwrap();
    sqlx::query(HALT)
        .bind("upgrade")
        .bind("old")
        .execute(&mut *tx)
        .await
        .unwrap();
    let detected: String = sqlx::query_scalar("SELECT updated_at::text FROM ingestion_streams WHERE operator_id='upgrade' AND stream_id='old'")
        .fetch_one(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/0007_ingestion_invalidations.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let audit = sqlx::query("SELECT source,detected_at::text,recorded_at>=detected_at AS ordered FROM ingestion_invalidations WHERE operator_id='upgrade' AND stream_id='old'")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(audit.get::<String, _>("source"), "MIGRATED_HALT");
    assert_eq!(audit.get::<String, _>("detected_at"), detected);
    assert!(audit.get::<bool, _>("ordered"));
    let view = sqlx::query("SELECT continuity_status,snapshot_digest,snapshot_checkpoint FROM ingestion_snapshot_validity WHERE operator_id='upgrade' AND stream_id='old'")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(view.get::<String, _>("continuity_status"), "INVALIDATED");
    assert_eq!(view.get::<String, _>("snapshot_digest"), hash);
    assert_eq!(
        view.get::<Value, _>("snapshot_checkpoint"),
        json!(head(101))
    );
    let retained: Value = sqlx::query_scalar(
        "SELECT payload FROM ingestion_batches WHERE operator_id='upgrade' AND stream_id='old'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(retained, json!({}));
    tx.rollback().await.unwrap();
    let remains: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_namespace WHERE nspname=$1")
        .bind(schema)
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(remains, 0);
}
