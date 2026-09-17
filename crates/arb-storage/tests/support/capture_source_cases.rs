//! Real PostgreSQL producer-boundary regressions, using synthetic inputs only.
use super::*;
use arb_domain::DecisionCaptureRef;
use arb_storage::{CaptureSourceBinding, IngestionBinding, IngestionCursor, IngestionHead, IngestionHalt};
use sqlx::PgPool;

fn refs(count: usize) -> Vec<DecisionCaptureRef> {
    (0..count).map(|n| {
        let digest = format!("sha256:{:064x}", n + 10);
        DecisionCaptureRef { capture_id: format!("publication-{n}"), manifest_digest: digest.clone(), snapshot_id: digest }
    }).collect()
}

fn nonroute(session: &str, generation: u64, time: u64, count: usize) -> DecisionTrace {
    let mut value = trace(session, generation, time, "NO_ROUTE");
    value.capture_refs = refs(count);
    value.seal().unwrap()
}

#[test]
fn new_publication_limit_keeps_legacy_validation_and_empty_diagnostics() {
    let eight = nonroute("synthetic-session", 1, 1000, 8);
    eight.validate_for_publication().unwrap();
    let nine = nonroute("synthetic-session", 1, 1001, 9);
    nine.validate().unwrap();
    assert!(nine.validate_for_publication().is_err());
    let sixty_four = nonroute("synthetic-session", 1, 1002, 64);
    sixty_four.validate().unwrap();
    assert!(sixty_four.validate_for_publication().is_err());
    trace("synthetic-session", 1, 1003, "DATA_UNAVAILABLE").validate_for_publication().unwrap();
    assert_eq!(arb_domain::MAX_PUBLICATION_CAPTURE_REFS, 8);
}

#[tokio::test]
async fn publication_rejects_ninth_reference_before_partial_batch_persistence() {
    let (store, operator, id, claim, generation) = setup().await;
    for capture in refs(9) {
        store.record_capture_admission(&claim, generation, &capture.capture_id, &capture.manifest_digest, "/synthetic/publication").await.unwrap();
    }
    let eight = nonroute(&id, generation, 1000, 8);
    let nine = nonroute(&id, generation, 1001, 9);
    assert!(matches!(store.append_decision_traces(&claim, generation, &[eight.clone(), nine]).await,
        Err(StoreError::InvalidInput("invalid sealed decision trace"))));
    assert_eq!(store.decision_coverage(&operator, &id).await.unwrap().raw_observations, "0");
    let saved = store.append_decision_traces(&claim, generation, std::slice::from_ref(&eight)).await.unwrap();
    let repeated = store.append_decision_traces(&claim, generation, std::slice::from_ref(&eight)).await.unwrap();
    assert_eq!(saved[0].trace_id, repeated[0].trace_id);
    assert_eq!(store.decision_coverage(&operator, &id).await.unwrap().raw_observations, "1");
}

struct SourceFixture {
    store: Store,
    pool: PgPool,
    operator: String,
    id: String,
    claim: WorkerClaim,
    generation: u64,
    source: CaptureSourceBinding,
    cursor: IngestionCursor,
}
fn head(number: u64) -> IngestionHead {
    IngestionHead { number, hash: format!("0x{number:064x}"), parent_hash: format!("0x{:064x}", number - 1), timestamp_seconds: number }
}

async fn source_fixture(start: bool) -> SourceFixture {
    let database = std::env::var("TEST_DATABASE_URL").expect("real PostgreSQL is required");
    let pool = PgPool::connect(&database).await.unwrap();
    let store = Store::from_pool(pool.clone());
    store.migrate().await.unwrap();
    let operator = format!("source-{}", Uuid::new_v4());
    store.save_configuration(&operator, &hash("a"), json!({"mode":"OBSERVE"})).await.unwrap();
    let id = store.create_session(&operator, "source-session", NewSession {
        network_id: "base-mainnet".into(), mode: "OBSERVE".into(), configuration_digest: hash("a"),
        experiment_id: "decision-fixture".into(), strategy_ids: vec!["fixture-strategy".into()],
    }).await.unwrap().session_id;
    let claim = store.claim_worker(&operator, &id, "base-mainnet", "source-worker", 60).await.unwrap();
    store.complete_worker_recovery(&claim).await.unwrap();
    let source = CaptureSourceBinding {
        stream_id: "source".into(),
        binding: IngestionBinding {
            schema_version: 1, network_id: "base-mainnet".into(), registry_digest: hash("b"),
            abi_source_commit: "c".repeat(40), dataset_origin: "MANUALLY_CONSTRUCTED".into(),
            pool_addresses: vec![format!("0x{:040x}", 3), format!("0x{:040x}", 4)],
        },
    };
    let initial = store.create_ingestion(&operator, "source", &source.binding, &head(100)).await.unwrap();
    let value = json!({
        "schema_version":1, "network_id":source.binding.network_id, "registry_digest":source.binding.registry_digest,
        "abi_source_commit":source.binding.abi_source_commit, "pool_addresses":source.binding.pool_addresses,
        "from_checkpoint":initial.checkpoint, "through":head(101),
        "blocks":[{"header":head(101),"logs":[]}], "full_snapshot_required":true
    });
    let cursor = store.commit_ingestion(&operator, "source", &initial, value).await.unwrap();
    store.configure_capture_ingestion_source(&claim, Some(&source)).await.unwrap();
    let generation = if start {
        store.issue_command(&operator, &id, "start", NewCommand { action:"START".into(),expected_revision:"0".into(),reason:None }).await.unwrap();
        let generation = store.apply_pending(&claim, true, |_| Ok(())).await.unwrap().generation;
        for (id, digest) in [("capture-one", hash("1")), ("capture-two", hash("2"))] {
            store.record_capture_admission(&claim, generation, id, &digest, "/synthetic/source").await.unwrap();
        }
        generation
    } else { 0 };
    SourceFixture { store, pool, operator, id, claim, generation, source, cursor }
}

async fn link_count(f: &SourceFixture) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM capture_ingestion_dependencies WHERE operator_id=$1 AND session_id=$2")
        .bind(&f.operator).bind(&f.id).fetch_one(&f.pool).await.unwrap()
}

#[tokio::test]
async fn source_requirement_is_persisted_idempotent_and_cannot_be_removed_or_changed() {
    let f = source_fixture(false).await;
    f.store.configure_capture_ingestion_source(&f.claim, Some(&f.source)).await.unwrap();
    assert!(matches!(f.store.configure_capture_ingestion_source(&f.claim, None).await,
        Err(StoreError::Conflict("configured capture source is required on restart"))));
    let reopened = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap()).await.unwrap();
    assert!(reopened.configure_capture_ingestion_source(&f.claim, None).await.is_err());
    reopened.configure_capture_ingestion_source(&f.claim, Some(&f.source)).await.unwrap();
    let mut wrong = f.source.clone();
    wrong.stream_id = "replacement".into();
    assert!(f.store.configure_capture_ingestion_source(&f.claim, Some(&wrong)).await.is_err());
    for sql in ["DELETE FROM session_ingestion_sources WHERE operator_id=$1", "UPDATE session_ingestion_sources SET stream_id='replacement' WHERE operator_id=$1"] {
        assert!(sqlx::query(sql).bind(&f.operator).execute(&f.pool).await.is_err());
    }
    let count:i64 = sqlx::query_scalar("SELECT count(*) FROM control_audit_events WHERE operator_id=$1 AND event_kind='CAPTURE_SOURCE_CONFIGURED'")
        .bind(&f.operator).fetch_one(&f.pool).await.unwrap();
    assert_eq!(count,1);
}

#[tokio::test]
async fn source_requirement_guards_first_quote_then_exact_binding_publishes_and_halt_invalidates() {
    let f = source_fixture(true).await;
    let first = trace(&f.id, f.generation, 1000, "QUOTED");
    assert!(matches!(f.store.append_decision_traces(&f.claim, f.generation, std::slice::from_ref(&first)).await,
        Err(StoreError::Conflict("capture source continuity unavailable"))));
    assert_eq!(f.store.decision_coverage(&f.operator, &f.id).await.unwrap().raw_observations,"0");
    f.store.bind_captures_to_ingestion(&f.claim, f.generation, &f.source, &head(101), &first.capture_refs).await.unwrap();
    f.store.bind_captures_to_ingestion(&f.claim, f.generation, &f.source, &head(101), &first.capture_refs).await.unwrap();
    assert_eq!(link_count(&f).await,2);
    f.store.append_decision_traces(&f.claim, f.generation, std::slice::from_ref(&first)).await.unwrap();
    let before = serde_json::to_value(f.store.get_decision_trace(&f.operator,&first.observation_id).await.unwrap().trace).unwrap();
    f.store.halt_ingestion(&f.operator,"source",&f.cursor,IngestionHalt::ContinuityLost).await.unwrap();
    let status:String=sqlx::query_scalar("SELECT continuity_status FROM decision_ingestion_validity WHERE operator_id=$1 AND observation_id=$2")
        .bind(&f.operator).bind(&first.observation_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(status,"INVALIDATED");
    assert!(f.store.bind_captures_to_ingestion(&f.claim,f.generation,&f.source,&head(101),&first.capture_refs).await.is_err());
    assert!(matches!(f.store.append_decision_traces(&f.claim,f.generation,&[trace(&f.id,f.generation,1001,"QUOTED")]).await,
        Err(StoreError::Conflict("capture source continuity unavailable"))));
    let after=serde_json::to_value(f.store.get_decision_trace(&f.operator,&first.observation_id).await.unwrap().trace).unwrap();
    assert_eq!(before,after);
    f.store.append_decision_traces(&f.claim,f.generation,&[trace(&f.id,f.generation,1002,"DATA_UNAVAILABLE")]).await.unwrap();
}

#[tokio::test]
async fn incomplete_mismatched_and_stale_bindings_never_persist_a_prefix() {
    let f=source_fixture(true).await;
    let first=trace(&f.id,f.generation,1000,"QUOTED");
    let mut missing=first.capture_refs.clone();
    missing[1].capture_id="not-admitted".into();
    assert!(f.store.bind_captures_to_ingestion(&f.claim,f.generation,&f.source,&head(101),&missing).await.is_err());
    assert_eq!(link_count(&f).await,0);
    assert!(f.store.bind_captures_to_ingestion(&f.claim,f.generation+1,&f.source,&head(101),&first.capture_refs).await.is_err());
    let mut wrong_head=head(101);
    wrong_head.hash=format!("0x{}","d".repeat(64));
    assert!(f.store.bind_captures_to_ingestion(&f.claim,f.generation,&f.source,&wrong_head,&first.capture_refs).await.is_err());
    for changed_origin in [false,true] {
        let mut source=f.source.clone();
        if changed_origin { source.binding.dataset_origin="RECORDED_LIVE".into(); }
        else { source.binding.registry_digest=hash("d"); }
        assert!(f.store.bind_captures_to_ingestion(&f.claim,f.generation,&source,&head(101),&first.capture_refs).await.is_err());
    }
    assert_eq!(link_count(&f).await,0);
    f.store.bind_captures_to_ingestion(&f.claim,f.generation,&f.source,&head(101),&first.capture_refs).await.unwrap();
    assert_eq!(link_count(&f).await,2);
}

#[tokio::test]
async fn stopped_generation_cannot_add_source_dependencies() {
    let f=source_fixture(true).await;
    f.store.issue_command(&f.operator,&f.id,"stop",NewCommand {action:"STOP".into(),expected_revision:"1".into(),reason:None}).await.unwrap();
    f.store.apply_pending(&f.claim,true,|_|Ok(())).await.unwrap();
    let value=trace(&f.id,f.generation,1000,"QUOTED");
    assert!(f.store.bind_captures_to_ingestion(&f.claim,f.generation,&f.source,&head(101),&value.capture_refs).await.is_err());
    assert_eq!(link_count(&f).await,0);
}
