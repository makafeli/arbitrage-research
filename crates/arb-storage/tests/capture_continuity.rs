//! PostgreSQL publication-boundary tests. All inputs are synthetic database
//! fixtures, not market captures or full engine/quote qualification.
use arb_storage::{
    IngestionBinding, IngestionCursor, IngestionHalt, IngestionHead, NewSession, Store,
};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const LINK: &str = "INSERT INTO capture_ingestion_dependencies(operator_id,session_id,capture_id,manifest_digest,stream_id,snapshot_revision,snapshot_digest,checkpoint) VALUES($1,$2,$3,$4,'source',1,$5,$6)";
const PUBLISH: &str = "INSERT INTO decision_traces(trace_id,operator_id,session_id,observation_id,payload_digest,configuration_digest,generation,observed_at_unix_ms,result_status,grouping_version,grouping_key,window_start_ms,payload) VALUES($1,$2,$3,$4,'synthetic:payload','synthetic:configuration',0,1,$5,'synthetic:v1','synthetic:group',0,$6)";
const HALT: &str = "UPDATE ingestion_streams SET state='HALTED',halt_reason='CONTINUITY_LOST',revision=revision+1,updated_at=clock_timestamp() WHERE operator_id=$1 AND stream_id='source'";
const CHECK: &str = "SELECT require_current_decision_ingestion($1,$2,$3)";

struct Fixture {
    pool: PgPool,
    store: Store,
    operator: String,
    session: String,
    cursor: IngestionCursor,
    digest: String,
}

fn manifest() -> String {
    format!("sha256:{}", "a".repeat(64))
}

fn head(number: u64) -> IngestionHead {
    IngestionHead {
        number,
        hash: format!("0x{number:064x}"),
        parent_hash: format!("0x{:064x}", number - 1),
        timestamp_seconds: number * 10,
    }
}

async fn fixture() -> Fixture {
    let database = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let pool = PgPool::connect(&database).await.unwrap();
    let store = Store::from_pool(pool.clone());
    store.migrate().await.unwrap();
    let operator = format!("capture-link-{}", Uuid::new_v4());
    store
        .save_configuration(
            &operator,
            "synthetic:configuration",
            json!({"synthetic":true}),
        )
        .await
        .unwrap();
    let session = store
        .create_session(
            &operator,
            "fixture",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: "synthetic:configuration".into(),
                experiment_id: "synthetic:continuity".into(),
                strategy_ids: vec!["synthetic".into()],
            },
        )
        .await
        .unwrap()
        .session_id;
    let binding = IngestionBinding {
        schema_version: 1,
        network_id: "base-mainnet".into(),
        registry_digest: manifest(),
        abi_source_commit: "b".repeat(40),
        dataset_origin: "MANUALLY_CONSTRUCTED".into(),
        pool_addresses: vec![format!("0x{}", "c".repeat(40))],
    };
    let initial = store
        .create_ingestion(&operator, "source", &binding, &head(100))
        .await
        .unwrap();
    let payload = json!({
        "schema_version":1, "network_id":binding.network_id,
        "registry_digest":binding.registry_digest, "abi_source_commit":binding.abi_source_commit,
        "pool_addresses":binding.pool_addresses, "from_checkpoint":initial.checkpoint,
        "through":head(101), "blocks":[{"header":head(101),"logs":[]}],
        "full_snapshot_required":true
    });
    let cursor = store
        .commit_ingestion(&operator, "source", &initial, payload)
        .await
        .unwrap();
    let digest = sqlx::query_scalar(
        "SELECT payload_digest FROM ingestion_batches WHERE operator_id=$1 AND stream_id='source' AND revision=1",
    )
    .bind(&operator)
    .fetch_one(&pool)
    .await
    .unwrap();
    Fixture {
        pool,
        store,
        operator,
        session,
        cursor,
        digest,
    }
}

async fn capture(f: &Fixture, id: &str) {
    let attempt = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO research_attempts(attempt_id,session_id,generation,admitted_worker_epoch,status) VALUES($1,$2,0,1,'OUTSTANDING')")
        .bind(&attempt).bind(&f.session).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO capture_admissions(session_id,capture_id,attempt_id,manifest_digest,artifact_path,generation) VALUES($1,$2,$3,$4,'synthetic-only',0)")
        .bind(&f.session).bind(id).bind(attempt).bind(manifest()).execute(&f.pool).await.unwrap();
}

async fn link_in_tx(
    f: &Fixture,
    id: &str,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(LINK)
        .bind(&f.operator)
        .bind(&f.session)
        .bind(id)
        .bind(manifest())
        .bind(&f.digest)
        .bind(json!(f.cursor.checkpoint))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn link(f: &Fixture, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = f.pool.begin().await?;
    link_in_tx(f, id, &mut tx).await?;
    tx.commit().await
}

fn payload(f: &Fixture, ids: &[&str]) -> Value {
    json!({
        "network_id":"base-mainnet", "session_id":f.session,
        "generation":"0", "dataset_origin":"SYNTHETIC",
        "result":{"status":"QUOTED"},
        "capture_refs":ids.iter().map(|id| json!({
            "capture_id":id, "manifest_digest":manifest(), "snapshot_id":manifest()
        })).collect::<Vec<_>>()
    })
}

async fn publish_in_tx(
    f: &Fixture,
    id: &str,
    value: &Value,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(PUBLISH)
        .bind(format!("{}:{id}", f.session))
        .bind(&f.operator)
        .bind(&f.session)
        .bind(id)
        .bind(value["result"]["status"].as_str().unwrap())
        .bind(value)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn publish(f: &Fixture, id: &str, value: &Value) -> Result<(), sqlx::Error> {
    let mut tx = f.pool.begin().await?;
    publish_in_tx(f, id, value, &mut tx).await?;
    tx.commit().await
}

async fn status(f: &Fixture, id: &str) -> String {
    sqlx::query_scalar("SELECT continuity_status FROM decision_ingestion_validity WHERE operator_id=$1 AND session_id=$2 AND observation_id=$3")
        .bind(&f.operator).bind(&f.session).bind(id).fetch_one(&f.pool).await.unwrap()
}

async fn check(f: &Fixture, id: &str) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(CHECK)
        .bind(&f.operator)
        .bind(&f.session)
        .bind(id)
        .fetch_one(&f.pool)
        .await
}

fn code(error: sqlx::Error, expected: &str) {
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some(expected)
    );
}

async fn halt(f: &Fixture) {
    f.store
        .halt_ingestion(
            &f.operator,
            "source",
            &f.cursor,
            IngestionHalt::ContinuityLost,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn terminal_source_blocks_new_quotes_but_preserves_historical_evidence() {
    let f = fixture().await;
    capture(&f, "capture").await;
    link(&f, "capture").await.unwrap();
    let value = payload(&f, &["capture"]);
    publish(&f, "first", &value).await.unwrap();
    assert_eq!(status(&f, "first").await, "NO_KNOWN_INVALIDATION");
    assert_eq!(check(&f, "first").await.unwrap(), value);
    halt(&f).await;
    assert_eq!(status(&f, "first").await, "INVALIDATED");
    code(check(&f, "first").await.unwrap_err(), "55000");
    code(publish(&f, "second", &value).await.unwrap_err(), "55000");
    let original: Value = sqlx::query_scalar(
        "SELECT payload FROM decision_traces WHERE session_id=$1 AND observation_id='first'",
    )
    .bind(&f.session)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(original, value);
    let mut diagnostic = value;
    diagnostic["result"]["status"] = json!("DATA_UNAVAILABLE");
    publish(&f, "diagnostic", &diagnostic).await.unwrap();
    assert_eq!(status(&f, "diagnostic").await, "INVALIDATED");
}

#[tokio::test]
async fn untracked_history_is_not_relabelled_or_silently_promoted() {
    let f = fixture().await;
    capture(&f, "legacy").await;
    publish(&f, "legacy", &payload(&f, &["legacy"]))
        .await
        .unwrap();
    assert_eq!(status(&f, "legacy").await, "UNTRACKED");
    code(check(&f, "legacy").await.unwrap_err(), "55000");
    code(link(&f, "legacy").await.unwrap_err(), "55000");
    assert_eq!(status(&f, "legacy").await, "UNTRACKED");
}

#[tokio::test]
async fn every_capture_and_exact_manifest_must_be_bound_in_opted_in_sessions() {
    let f = fixture().await;
    capture(&f, "one").await;
    capture(&f, "two").await;
    link(&f, "one").await.unwrap();
    let both = payload(&f, &["one", "two"]);
    code(publish(&f, "partial", &both).await.unwrap_err(), "55000");
    link(&f, "two").await.unwrap();
    publish(&f, "complete", &both).await.unwrap();
    let mut wrong = both.clone();
    wrong["capture_refs"][0]["manifest_digest"] = json!(format!("sha256:{}", "d".repeat(64)));
    code(publish(&f, "wrong", &wrong).await.unwrap_err(), "55000");
    wrong = both;
    wrong["generation"] = json!("1");
    code(
        publish(&f, "generation", &wrong).await.unwrap_err(),
        "55000",
    );
    code(
        publish(&f, "duplicate", &payload(&f, &["one", "one"]))
            .await
            .unwrap_err(),
        "22023",
    );
    code(
        publish(&f, "empty", &payload(&f, &[])).await.unwrap_err(),
        "22023",
    );
}

#[tokio::test]
async fn binding_validates_admission_exact_snapshot_and_checkpoint() {
    let f = fixture().await;
    capture(&f, "capture").await;
    for (manifest_digest, snapshot_digest, checkpoint, expected) in [
        (
            "sha256:bad".to_string(),
            f.digest.clone(),
            json!(head(101)),
            "22023",
        ),
        (
            manifest(),
            format!("sha256:{}", "d".repeat(64)),
            json!(head(101)),
            "22023",
        ),
        (manifest(), f.digest.clone(), json!(head(102)), "22023"),
    ] {
        let error = sqlx::query(LINK)
            .bind(&f.operator)
            .bind(&f.session)
            .bind("capture")
            .bind(manifest_digest)
            .bind(snapshot_digest)
            .bind(checkpoint)
            .execute(&f.pool)
            .await
            .unwrap_err();
        code(error, expected);
    }
    code(link(&f, "missing").await.unwrap_err(), "22023");
    link(&f, "capture").await.unwrap();
    code(link(&f, "capture").await.unwrap_err(), "23505");
}

#[tokio::test]
async fn operator_and_session_scope_cannot_borrow_another_capture() {
    let f = fixture().await;
    let other = fixture().await;
    capture(&f, "same").await;
    capture(&other, "same").await;
    link(&f, "same").await.unwrap();
    let error = sqlx::query(LINK)
        .bind(&other.operator)
        .bind(&f.session)
        .bind("same")
        .bind(manifest())
        .bind(&f.digest)
        .bind(json!(head(101)))
        .execute(&f.pool)
        .await
        .unwrap_err();
    code(error, "P0002");
    publish(&f, "first", &payload(&f, &["same"])).await.unwrap();
    publish(&other, "first", &payload(&other, &["same"]))
        .await
        .unwrap();
    halt(&f).await;
    assert_eq!(status(&f, "first").await, "INVALIDATED");
    assert_eq!(status(&other, "first").await, "UNTRACKED");
    let missing = sqlx::query_scalar::<_, Value>(CHECK)
        .bind(&other.operator)
        .bind(&f.session)
        .bind("first")
        .fetch_one(&f.pool)
        .await
        .unwrap_err();
    code(missing, "P0002");
}

#[tokio::test]
async fn associations_are_immutable_and_rollback_leaves_no_partial_binding() {
    let f = fixture().await;
    capture(&f, "rolled-back").await;
    let mut tx = f.pool.begin().await.unwrap();
    link_in_tx(&f, "rolled-back", &mut tx).await.unwrap();
    tx.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_ingestion_dependencies WHERE session_id=$1",
    )
    .bind(&f.session)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    link(&f, "rolled-back").await.unwrap();
    for sql in [
        "UPDATE capture_ingestion_dependencies SET stream_id='replacement' WHERE session_id=$1",
        "DELETE FROM capture_ingestion_dependencies WHERE session_id=$1",
    ] {
        assert!(
            sqlx::query(sql)
                .bind(&f.session)
                .execute(&f.pool)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn publication_holds_source_lock_until_commit_then_invalidation_is_visible() {
    let f = fixture().await;
    capture(&f, "capture").await;
    link(&f, "capture").await.unwrap();
    let mut publication = f.pool.begin().await.unwrap();
    publish_in_tx(&f, "first", &payload(&f, &["capture"]), &mut publication)
        .await
        .unwrap();
    let mut competing = f.pool.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='50ms'")
        .execute(&mut *competing)
        .await
        .unwrap();
    let error = sqlx::query(HALT)
        .bind(&f.operator)
        .execute(&mut *competing)
        .await
        .unwrap_err();
    code(error, "55P03");
    competing.rollback().await.unwrap();
    publication.commit().await.unwrap();
    halt(&f).await;
    assert_eq!(status(&f, "first").await, "INVALIDATED");
}

#[tokio::test]
async fn current_decision_recheck_retains_transaction_lock_and_never_renews_validity() {
    let f = fixture().await;
    capture(&f, "capture").await;
    link(&f, "capture").await.unwrap();
    publish(&f, "first", &payload(&f, &["capture"]))
        .await
        .unwrap();
    let mut consuming = f.pool.begin().await.unwrap();
    let original: Value = sqlx::query_scalar(CHECK)
        .bind(&f.operator)
        .bind(&f.session)
        .bind("first")
        .fetch_one(&mut *consuming)
        .await
        .unwrap();
    assert_eq!(original, payload(&f, &["capture"]));
    let mut competing = f.pool.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='50ms'")
        .execute(&mut *competing)
        .await
        .unwrap();
    code(
        sqlx::query(HALT)
            .bind(&f.operator)
            .execute(&mut *competing)
            .await
            .unwrap_err(),
        "55P03",
    );
    competing.rollback().await.unwrap();
    consuming.rollback().await.unwrap();
    halt(&f).await;
    code(check(&f, "first").await.unwrap_err(), "55000");
}

#[tokio::test]
async fn diagnostic_read_tracks_halt_without_rewriting_historical_trace() {
    let f = fixture().await;
    capture(&f, "capture").await;
    link(&f, "capture").await.unwrap();
    let original = payload(&f, &["capture"]);
    publish(&f, "read-status", &original).await.unwrap();
    let before = f
        .store
        .decision_continuity(&f.operator, &f.session, "read-status")
        .await
        .unwrap();
    assert_eq!(before.continuity_status, "NO_KNOWN_INVALIDATION");
    assert_eq!(before.capture_count, "1");
    assert_eq!(before.bound_count, "1");
    assert!(!before.authorizes_execution);
    assert_eq!(before.assessment_kind, "CONTINUITY_ONLY");
    assert!(before.invalidation_reasons.is_empty());
    assert!(chrono::DateTime::parse_from_rfc3339(&before.checked_at).is_ok());
    halt(&f).await;
    let after = f
        .store
        .decision_continuity(&f.operator, &f.session, "read-status")
        .await
        .unwrap();
    assert_eq!(after.trace_id, before.trace_id);
    assert_eq!(after.continuity_status, "INVALIDATED");
    assert_eq!(after.invalidation_reasons, vec!["CONTINUITY_LOST"]);
    assert!(!after.authorizes_execution);
    let retained: Value = sqlx::query_scalar(
        "SELECT payload FROM decision_traces WHERE session_id=$1 AND observation_id='read-status'",
    )
    .bind(&f.session)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(retained, original);
}

#[tokio::test]
async fn diagnostic_read_does_not_borrow_another_session_or_operator() {
    let f = fixture().await;
    let other = fixture().await;
    capture(&f, "legacy").await;
    publish(&f, "same-observation", &payload(&f, &["legacy"]))
        .await
        .unwrap();
    let report = f
        .store
        .decision_continuity(&f.operator, &f.session, "same-observation")
        .await
        .unwrap();
    assert_eq!(report.continuity_status, "UNTRACKED");
    assert_eq!(report.bound_count, "0");
    for (operator, session, observation) in [
        (&other.operator, &f.session, "same-observation"),
        (&f.operator, &other.session, "same-observation"),
        (&f.operator, &f.session, "missing"),
    ] {
        assert!(matches!(
            f.store
                .decision_continuity(operator, session, observation)
                .await,
            Err(arb_storage::StoreError::NotFound)
        ));
    }
    assert!(
        f.store
            .decision_continuity(&f.operator, &f.session, &"x".repeat(129))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn zero_input_diagnostics_do_not_invent_a_healthy_source() {
    let f = fixture().await;
    let mut diagnostic = payload(&f, &[]);
    diagnostic["result"]["status"] = json!("DATA_UNAVAILABLE");
    publish(&f, "zero-input", &diagnostic).await.unwrap();
    let report = f
        .store
        .decision_continuity(&f.operator, &f.session, "zero-input")
        .await
        .unwrap();
    assert_eq!(report.capture_count, "0");
    assert_eq!(report.bound_count, "0");
    assert_eq!(report.continuity_status, "UNTRACKED");
    assert!(report.invalidation_reasons.is_empty());
    assert!(!report.authorizes_execution);
}
