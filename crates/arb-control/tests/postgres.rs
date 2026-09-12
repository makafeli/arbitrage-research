use arb_control::ControlWorker;
use arb_storage::{NewCommand, NewSession, Store, StoreError};
use serde_json::json;
use uuid::Uuid;

async fn fixture() -> (Store, String, String) {
    let store = Store::connect(
        &std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is required: control tests must not silently skip"),
    )
    .await
    .unwrap();
    store.migrate().await.unwrap();
    let operator = format!("control-{}", Uuid::new_v4());
    store
        .save_configuration(&operator, "test", json!({"mode":"PAPER"}))
        .await
        .unwrap();
    let session = store
        .create_session(
            &operator,
            "new",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "PAPER".into(),
                configuration_digest: "test".into(),
                experiment_id: "control-test".into(),
                strategy_ids: vec!["test".into()],
            },
        )
        .await
        .unwrap();
    (store, operator, session.session_id)
}
async fn issue(
    store: &Store,
    operator: &str,
    id: &str,
    key: &str,
    action: &str,
    revision: &str,
) -> arb_storage::CommandReceipt {
    store
        .issue_command(
            operator,
            id,
            key,
            NewCommand {
                action: action.into(),
                expected_revision: revision.into(),
                reason: None,
            },
        )
        .await
        .unwrap()
}
async fn worker(store: &Store, operator: &str, id: &str) -> ControlWorker {
    let worker = ControlWorker::claim(
        store.clone(),
        operator,
        id,
        "base-mainnet",
        &Uuid::new_v4().to_string(),
        60,
    )
    .await
    .unwrap();
    let update = worker.complete_recovery().await.unwrap();
    assert_eq!(update.session.observed_state, "STOPPED");
    worker
}

#[tokio::test]
async fn pending_until_worker_ack_and_old_generations_stay_fenced_after_resume() {
    let (store, operator, id) = fixture().await;
    let worker = worker(&store, &operator, &id).await;
    let start = issue(&store, &operator, &id, "start", "START", "0").await;
    assert_eq!(start.status, "PENDING");
    assert!(worker.generation().await.is_err());
    let waiting = worker.tick(false).await.unwrap();
    assert_eq!(waiting.command.unwrap().status, "PENDING");
    let started = worker.tick(true).await.unwrap();
    assert_eq!(started.session.observed_state, "RUNNING");
    assert_eq!(
        store
            .get_command(&operator, &start.command_id)
            .await
            .unwrap()
            .status,
        "APPLIED"
    );
    let stale = worker.generation().await.unwrap();
    issue(&store, &operator, &id, "pause", "PAUSE", "1").await;
    let paused = worker.tick(true).await.unwrap();
    assert_eq!(paused.session.observed_state, "PAUSED");
    assert!(paused.command.unwrap().fence_effective);
    assert!(worker.admit_research_result(stale).await.is_err());
    issue(&store, &operator, &id, "resume", "RESUME", "2").await;
    worker.tick(true).await.unwrap();
    assert!(worker.admit_research_result(stale).await.is_err());
    let fresh = worker.generation().await.unwrap();
    assert_ne!(fresh, stale);
}
#[tokio::test]
async fn stop_is_draining_until_positive_resolution_while_reconciliation_stays_available() {
    let (store, operator, id) = fixture().await;
    let worker = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    worker.tick(true).await.unwrap();
    let attempt = worker
        .admit_research_result(worker.generation().await.unwrap())
        .await
        .unwrap()
        .attempt_id
        .unwrap();
    let stop = issue(&store, &operator, &id, "stop", "STOP", "1").await;
    let draining = worker.tick(true).await.unwrap();
    assert_eq!(draining.session.observed_state, "DRAINING");
    assert_eq!(draining.session.outstanding_attempts, 1);
    let receipt = store
        .get_command(&operator, &stop.command_id)
        .await
        .unwrap();
    assert_eq!(receipt.status, "APPLIED");
    assert!(receipt.fence_effective);
    assert_eq!(receipt.outstanding_attempts, 1);
    assert!(worker.resolve_research_attempt(&attempt, "").await.is_err());
    let stopped = worker
        .resolve_research_attempt(&attempt, "synthetic fixture explicitly resolved")
        .await
        .unwrap();
    assert_eq!(stopped.session.observed_state, "STOPPED");
    assert!(worker.generation().await.is_err());
}
#[tokio::test]
async fn restart_rejects_pending_never_auto_starts_and_fences_previous_owner() {
    let (store, operator, id) = fixture().await;
    let old = worker(&store, &operator, &id).await;
    let pending = issue(&store, &operator, &id, "start", "START", "0").await;
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::query("UPDATE research_sessions SET lease_until=clock_timestamp()-interval '1 second' WHERE session_id=$1").bind(&id).execute(&pool).await.unwrap();
    let new = ControlWorker::claim(
        store.clone(),
        &operator,
        &id,
        "base-mainnet",
        "replacement",
        60,
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .observed_state,
        "RECOVERING"
    );
    assert_eq!(
        store
            .get_command(&operator, &pending.command_id)
            .await
            .unwrap()
            .status,
        "REJECTED"
    );
    assert!(old.tick(true).await.is_err());
    assert!(old.generation().await.is_err());
    new.complete_recovery().await.unwrap();
    new.tick(true).await.unwrap();
    assert!(new.generation().await.is_err());
    assert_eq!(
        store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .observed_state,
        "STOPPED"
    );
    issue(&store, &operator, &id, "fresh-start", "START", "1").await;
    new.tick(true).await.unwrap();
    assert!(new.generation().await.is_ok());
}
#[tokio::test]
async fn chain_and_active_lease_ownership_are_isolated() {
    let (store, operator, id) = fixture().await;
    assert!(matches!(
        ControlWorker::claim(store.clone(), &operator, &id, "solana-mainnet", "wrong", 60).await,
        Err(StoreError::Conflict(_))
    ));
    let _first = worker(&store, &operator, &id).await;
    assert!(matches!(
        ControlWorker::claim(store.clone(), &operator, &id, "base-mainnet", "second", 60).await,
        Err(StoreError::Conflict(_))
    ));
}
#[tokio::test]
async fn stop_all_is_independent_per_session_and_disconnected_session_remains_pending() {
    let (store, a, id_a) = fixture().await;
    let (_, b, id_b) = fixture().await;
    let worker = worker(&store, &a, &id_a).await;
    let stop_a = issue(&store, &a, &id_a, "stop-all", "STOP", "0").await;
    let stop_b = issue(&store, &b, &id_b, "stop-all", "STOP", "0").await;
    worker.tick(true).await.unwrap();
    assert_eq!(
        store
            .get_command(&a, &stop_a.command_id)
            .await
            .unwrap()
            .status,
        "APPLIED"
    );
    assert_eq!(
        store
            .get_command(&b, &stop_b.command_id)
            .await
            .unwrap()
            .status,
        "PENDING"
    );
}
#[tokio::test]
async fn local_shutdown_fence_cannot_be_reopened_by_polling() {
    let (store, operator, id) = fixture().await;
    let worker = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    worker.tick(true).await.unwrap();
    let generation = worker.generation().await.unwrap();
    worker.fence_local().await;
    assert!(worker.tick(true).await.is_err());
    assert!(worker.admit_research_result(generation).await.is_err());
}
#[tokio::test]
async fn duplicate_resolution_never_clears_another_attempt_and_is_session_scoped() {
    let (store, operator, id) = fixture().await;
    let worker = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    worker.tick(true).await.unwrap();
    let generation = worker.generation().await.unwrap();
    let first = worker
        .admit_research_result(generation)
        .await
        .unwrap()
        .attempt_id
        .unwrap();
    let second = worker
        .admit_research_result(generation)
        .await
        .unwrap()
        .attempt_id
        .unwrap();
    issue(&store, &operator, &id, "stop", "STOP", "1").await;
    worker.tick(true).await.unwrap();
    assert_eq!(
        worker
            .resolve_research_attempt(&first, "fixture positive")
            .await
            .unwrap()
            .session
            .outstanding_attempts,
        1
    );
    assert_eq!(
        worker
            .resolve_research_attempt(&first, "fixture positive")
            .await
            .unwrap()
            .session
            .outstanding_attempts,
        1
    );
    assert!(
        worker
            .resolve_research_attempt(&first, "different evidence")
            .await
            .is_err()
    );
    assert!(matches!(
        worker
            .resolve_research_attempt("unknown", "fixture positive")
            .await,
        Err(StoreError::NotFound)
    ));
    let (_, other_operator, other_id) = fixture().await;
    let other = ControlWorker::claim(
        store.clone(),
        &other_operator,
        &other_id,
        "base-mainnet",
        "other",
        60,
    )
    .await
    .unwrap();
    other.complete_recovery().await.unwrap();
    assert!(matches!(
        other
            .resolve_research_attempt(&second, "fixture positive")
            .await,
        Err(StoreError::NotFound)
    ));
    assert_eq!(
        worker
            .resolve_research_attempt(&second, "fixture positive")
            .await
            .unwrap()
            .session
            .observed_state,
        "STOPPED"
    );
}
#[tokio::test]
async fn cancelled_database_operation_leaves_local_gate_closed_until_restart() {
    let (store, operator, id) = fixture().await;
    let worker = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    worker.tick(true).await.unwrap();
    let generation = worker.generation().await.unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT session_id FROM research_sessions WHERE session_id=$1 FOR UPDATE")
        .bind(&id)
        .execute(&mut *lock)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), worker.tick(true))
            .await
            .is_err()
    );
    lock.rollback().await.unwrap();
    assert!(worker.generation().await.is_err());
    assert!(worker.admit_research_result(generation).await.is_err());
    assert!(worker.tick(true).await.is_err());
}
#[tokio::test]
async fn restart_preserves_unresolved_attempt_identity_and_requires_resolution_before_stopped() {
    let (store, operator, id) = fixture().await;
    let old = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    old.tick(true).await.unwrap();
    let attempt = old
        .admit_research_result(old.generation().await.unwrap())
        .await
        .unwrap()
        .attempt_id
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::query("UPDATE research_sessions SET lease_until=clock_timestamp()-interval '1 second' WHERE session_id=$1").bind(&id).execute(&pool).await.unwrap();
    let replacement = ControlWorker::claim(
        store.clone(),
        &operator,
        &id,
        "base-mainnet",
        "replacement",
        60,
    )
    .await
    .unwrap();
    assert!(replacement.complete_recovery().await.is_err());
    assert_eq!(
        replacement
            .outstanding_research_attempt_ids(100)
            .await
            .unwrap(),
        vec![attempt.clone()]
    );
    assert_eq!(
        store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .observed_state,
        "RECOVERING"
    );
    replacement
        .resolve_research_attempt(&attempt, "persisted synthetic outcome verified")
        .await
        .unwrap();
    assert_eq!(
        replacement
            .complete_recovery()
            .await
            .unwrap()
            .session
            .observed_state,
        "STOPPED"
    );
    assert!(replacement.generation().await.is_err());
}
#[tokio::test]
async fn loss_of_running_readiness_faults_and_never_reopens_on_next_poll() {
    let (store, operator, id) = fixture().await;
    let worker = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    worker.tick(true).await.unwrap();
    let old = worker.generation().await.unwrap();
    assert_eq!(
        worker.tick(false).await.unwrap().session.observed_state,
        "FAULTED"
    );
    assert!(worker.generation().await.is_err());
    assert!(worker.admit_research_result(old).await.is_err());
    worker.tick(true).await.unwrap();
    assert!(worker.generation().await.is_err());
}

#[tokio::test]
async fn capture_admission_is_observe_only_idempotent_and_fenced_after_stop() {
    let (store, operator, _) = fixture().await;
    let session = store
        .create_session(
            &operator,
            "observe",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: "test".into(),
                experiment_id: "capture".into(),
                strategy_ids: vec!["test".into()],
            },
        )
        .await
        .unwrap();
    let id = session.session_id;
    let worker = worker(&store, &operator, &id).await;
    issue(&store, &operator, &id, "start", "START", "0").await;
    worker.tick(true).await.unwrap();
    let generation = worker.generation().await.unwrap();
    let hash = format!("sha256:{}", "a".repeat(64));
    let first = worker
        .admit_capture_manifest(generation, "capture-1", &hash, "/test/capture-1")
        .await
        .unwrap();
    let duplicate = worker
        .admit_capture_manifest(generation, "capture-1", &hash, "/test/capture-1")
        .await
        .unwrap();
    assert_eq!(first.attempt_id, duplicate.attempt_id);
    assert_eq!(first.session.outstanding_attempts, 0);
    issue(&store, &operator, &id, "stop", "STOP", "1").await;
    worker.tick(false).await.unwrap();
    assert!(
        worker
            .admit_capture_manifest(generation, "late-capture", &hash, "/test/late-capture")
            .await
            .is_err()
    );
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM capture_admissions WHERE session_id=$1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn recovery_rejects_counter_that_disagrees_with_attempt_history() {
    let (store, operator, id) = fixture().await;
    let claim = store
        .claim_worker(&operator, &id, "base-mainnet", "worker", 60)
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::query("INSERT INTO research_attempts(attempt_id,session_id,generation,admitted_worker_epoch,status) VALUES($1,$2,0,1,'OUTSTANDING')")
        .bind(Uuid::new_v4().to_string()).bind(&id).execute(&pool).await.unwrap();
    assert!(matches!(
        store.complete_worker_recovery(&claim).await,
        Err(StoreError::CorruptState)
    ));
    assert_eq!(
        store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .observed_state,
        "RECOVERING"
    );
}
