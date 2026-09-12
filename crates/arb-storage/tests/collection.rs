use arb_domain::DecisionTrace;
use arb_storage::{
    CollectionFinish, CollectionOutcome as O, CollectionPurpose as P, CollectionReason as R,
    NewCommand, NewSession, Store, StoreError, WorkerClaim,
};
use serde_json::json;
use uuid::Uuid;

fn hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}
async fn setup(start: bool) -> (Store, String, String, WorkerClaim, u64) {
    let store = Store::connect(
        &std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL required; collection DB tests never skip"),
    )
    .await
    .unwrap();
    store.migrate().await.unwrap();
    let operator = format!("collection-{}", Uuid::new_v4());
    store
        .save_configuration(&operator, &hash(), json!({"mode":"OBSERVE"}))
        .await
        .unwrap();
    let session = store
        .create_session(
            &operator,
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: hash(),
                experiment_id: "collection-test".into(),
                strategy_ids: vec!["strategy".into()],
            },
        )
        .await
        .unwrap();
    let claim = store
        .claim_worker(
            &operator,
            &session.session_id,
            "base-mainnet",
            "original-worker",
            60,
        )
        .await
        .unwrap();
    let mut generation = store
        .complete_worker_recovery(&claim)
        .await
        .unwrap()
        .generation;
    if start {
        store
            .issue_command(
                &operator,
                &session.session_id,
                "start",
                NewCommand {
                    action: "START".into(),
                    expected_revision: "0".into(),
                    reason: None,
                },
            )
            .await
            .unwrap();
        generation = store
            .apply_pending(&claim, true, |_| Ok(()))
            .await
            .unwrap()
            .generation;
    }
    (store, operator, session.session_id, claim, generation)
}
fn finish(outcome: O, reason: Option<R>) -> CollectionFinish {
    CollectionFinish {
        outcome,
        reason,
        captured_pools: 0,
        elapsed_ms: 25,
    }
}
fn trace(session: &str, generation: u64, time: u64) -> DecisionTrace {
    serde_json::from_value::<DecisionTrace>(json!({
        "schema_version":"1.0.0","observation_id":"","session_id":session,"experiment_id":"collection-test","generation":generation.to_string(),"configuration_digest":hash(),"calculation_version":"fixture-math-v1","strategy_id":"strategy","network_id":"base-mainnet","mode":"OBSERVE","source_kind":"SYNTHETIC_FIXTURE","dataset_origin":"MANUALLY_CONSTRUCTED","observed_at_unix_ms":time,"input_age_ms":null,
        "capture_refs":[],"route":[],"amount_in_minor":null,"result":{"status":"DATA_UNAVAILABLE","reason_codes":["NO_CAPTURE_INPUTS"]},"grouping":{"version":"","key":"","window_ms":1000,"window_start_ms":0},"diagnostics":[]
    })).unwrap().seal().unwrap()
}
async fn pool() -> sqlx::PgPool {
    sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn committed_start_is_visible_idempotent_and_unknown_until_terminal_evidence() {
    let (store, operator, session, claim, generation) = setup(false).await;
    let id = Uuid::now_v7().to_string();
    let first = store
        .begin_collection_attempt(&claim, &id, generation, P::Readiness)
        .await
        .unwrap();
    assert_eq!(first.outcome, O::InProgress);
    assert!(first.finished_at.is_none() && first.elapsed_ms.is_none());
    assert_eq!(first.decision_rows, "0");
    let retry = store
        .begin_collection_attempt(&claim, &id, generation, P::Readiness)
        .await
        .unwrap();
    assert_eq!(first.started_at, retry.started_at);
    assert!(matches!(
        store
            .begin_collection_attempt(&claim, &id, generation, P::Research)
            .await,
        Err(StoreError::Conflict(_))
    ));
    let coverage = store
        .collection_coverage(&operator, &session)
        .await
        .unwrap();
    assert_eq!(coverage.attempts_started, "1");
    assert_eq!(coverage.in_progress, "1");
    assert_eq!(coverage.acquisition_failed, "0");
    assert_eq!(coverage.collection_completeness, "UNKNOWN");
    assert_eq!(coverage.denominator, "RECORDED_COLLECTION_ATTEMPTS");
    let terminal = finish(O::AcquisitionFailed, Some(R::ProviderUnavailable));
    let done = store
        .finish_collection_attempt(&claim, &id, terminal.clone())
        .await
        .unwrap();
    let repeated = store
        .finish_collection_attempt(&claim, &id, terminal)
        .await
        .unwrap();
    assert_eq!(done.finished_at, repeated.finished_at);
    assert!(matches!(
        store
            .finish_collection_attempt(&claim, &id, finish(O::ReadinessCompleted, None))
            .await,
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store
            .collection_coverage("foreign-operator", &session)
            .await,
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        store
            .list_collection_attempts(&operator, &session, None, 101)
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    assert!(matches!(
        store
            .list_collection_attempts(&operator, &session, Some("bad-cursor"), 1)
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    assert_eq!(
        store
            .get_session(&operator, &session)
            .await
            .unwrap()
            .outstanding_attempts,
        0
    );
}

#[tokio::test]
async fn decisions_and_terminal_receipt_are_atomic_unique_and_scope_bound() {
    let (store, operator, session, claim, generation) = setup(true).await;
    let id = Uuid::now_v7().to_string();
    store
        .begin_collection_attempt(&claim, &id, generation, P::Research)
        .await
        .unwrap();
    let a = trace(&session, generation, 1001);
    let mut invalid = trace(&session, generation, 1002);
    invalid.experiment_id = "wrong-experiment".into();
    let done = finish(O::DecisionsRecorded, None);
    assert!(
        store
            .append_collection_decision_traces(
                &claim,
                &id,
                generation,
                &[a.clone(), invalid],
                done.clone()
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .decision_coverage(&operator, &session)
            .await
            .unwrap()
            .raw_observations,
        "0"
    );
    assert_eq!(
        store
            .collection_coverage(&operator, &session)
            .await
            .unwrap()
            .in_progress,
        "1"
    );
    assert!(matches!(
        store
            .append_collection_decision_traces(
                &claim,
                &id,
                generation,
                &[a.clone(), a.clone()],
                done.clone()
            )
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let first = store
        .append_collection_decision_traces(
            &claim,
            &id,
            generation,
            std::slice::from_ref(&a),
            done.clone(),
        )
        .await
        .unwrap();
    let retry = store
        .append_collection_decision_traces(
            &claim,
            &id,
            generation,
            std::slice::from_ref(&a),
            done.clone(),
        )
        .await
        .unwrap();
    assert_eq!(first[0].trace_id, retry[0].trace_id);
    let second = Uuid::now_v7().to_string();
    store
        .begin_collection_attempt(&claim, &second, generation, P::Research)
        .await
        .unwrap();
    assert!(matches!(
        store
            .append_collection_decision_traces(&claim, &second, generation, &[a], done)
            .await,
        Err(StoreError::Conflict(_))
    ));
    let coverage = store
        .collection_coverage(&operator, &session)
        .await
        .unwrap();
    assert_eq!(coverage.attempts_started, "2");
    assert_eq!(coverage.in_progress, "1");
    assert_eq!(coverage.decisions_recorded, "1");
    assert_eq!(coverage.decision_rows_recorded, "1");
    let page = store
        .list_collection_attempts(&operator, &session, None, 1)
        .await
        .unwrap();
    assert_eq!(
        page.items[0].decision_observation_ids,
        vec![first[0].trace.observation_id.clone()]
    );
    assert!(page.next_cursor.is_some());
    let page2 = store
        .list_collection_attempts(&operator, &session, page.next_cursor.as_deref(), 1)
        .await
        .unwrap();
    assert_eq!(page2.items[0].outcome, O::InProgress);
    assert!(page2.next_cursor.is_none());
}

#[tokio::test]
async fn stop_and_worker_replacement_allow_original_failure_receipt_but_no_late_decisions() {
    let (store, operator, session, claim, generation) = setup(true).await;
    let id = Uuid::now_v7().to_string();
    let abandoned = Uuid::now_v7().to_string();
    store
        .begin_collection_attempt(&claim, &id, generation, P::Research)
        .await
        .unwrap();
    store
        .begin_collection_attempt(&claim, &abandoned, generation, P::Research)
        .await
        .unwrap();
    store
        .issue_command(
            &operator,
            &session,
            "stop",
            NewCommand {
                action: "STOP".into(),
                expected_revision: "1".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let stopped = store.apply_pending(&claim, true, |_| Ok(())).await.unwrap();
    assert!(!stopped.gate_open);
    assert!(matches!(
        store
            .append_collection_decision_traces(
                &claim,
                &id,
                generation,
                &[trace(&session, generation, 1001)],
                finish(O::DecisionsRecorded, None)
            )
            .await,
        Err(StoreError::Conflict(_))
    ));
    let db = pool().await;
    sqlx::query("UPDATE research_sessions SET lease_until=clock_timestamp()-interval '1 second' WHERE session_id=$1").bind(&session).execute(&db).await.unwrap();
    let replacement = store
        .claim_worker(
            &operator,
            &session,
            "base-mainnet",
            "replacement-worker",
            60,
        )
        .await
        .unwrap();
    store.complete_worker_recovery(&replacement).await.unwrap();
    assert!(matches!(
        store
            .finish_collection_attempt(
                &replacement,
                &id,
                finish(O::Suppressed, Some(R::GenerationFenced))
            )
            .await,
        Err(StoreError::NotFound)
    ));
    store
        .finish_collection_attempt(
            &claim,
            &id,
            finish(O::Suppressed, Some(R::GenerationFenced)),
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .begin_collection_attempt(&claim, &Uuid::now_v7().to_string(), generation, P::Research)
            .await,
        Err(StoreError::Conflict(_))
    ));
    let coverage = store
        .collection_coverage(&operator, &session)
        .await
        .unwrap();
    assert_eq!(coverage.suppressed, "1");
    assert_eq!(coverage.in_progress, "1");
    assert_eq!(coverage.acquisition_failed, "0");
    assert_eq!(coverage.decision_rows_recorded, "0");
    assert_eq!(coverage.collection_completeness, "UNKNOWN");
    let current = store.get_session(&operator, &session).await.unwrap();
    assert_eq!(current.observed_state, "STOPPED");
    assert_eq!(current.outstanding_attempts, 0);
}

#[tokio::test]
async fn finish_rejects_invalid_pairs_success_forgery_and_concurrent_conflicting_receipts() {
    let (store, operator, session, claim, generation) = setup(false).await;
    let id = Uuid::now_v7().to_string();
    store
        .begin_collection_attempt(&claim, &id, generation, P::Readiness)
        .await
        .unwrap();
    for terminal in [
        finish(O::DecisionsRecorded, None),
        finish(O::AcquisitionFailed, None),
        finish(O::Suppressed, Some(R::ProviderUnavailable)),
        finish(O::InProgress, None),
    ] {
        assert!(matches!(
            store.finish_collection_attempt(&claim, &id, terminal).await,
            Err(StoreError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        store
            .finish_collection_attempt(
                &claim,
                &id,
                finish(O::EvaluationFailed, Some(R::EvaluationRejected))
            )
            .await,
        Err(StoreError::Conflict(_))
    ));
    let mut oversized = finish(O::ReadinessCompleted, None);
    oversized.elapsed_ms = 86_400_001;
    assert!(matches!(
        store
            .finish_collection_attempt(&claim, &id, oversized)
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let (a, b) = tokio::join!(
        store.finish_collection_attempt(&claim, &id, finish(O::ReadinessCompleted, None)),
        store.finish_collection_attempt(
            &claim,
            &id,
            finish(O::AcquisitionFailed, Some(R::ProviderUnavailable))
        )
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(StoreError::Conflict(_))
    ));
    assert_eq!(
        store
            .collection_coverage(&operator, &session)
            .await
            .unwrap()
            .in_progress,
        "0"
    );
}

#[tokio::test]
async fn sql_guards_preserve_origin_terminal_history_and_strict_status_reasons() {
    let (store, operator, session, claim, generation) = setup(false).await;
    let id = Uuid::now_v7().to_string();
    store
        .begin_collection_attempt(&claim, &id, generation, P::Readiness)
        .await
        .unwrap();
    let db = pool().await;
    assert!(
        sqlx::query("UPDATE collection_attempts SET generation=generation+1 WHERE attempt_id=$1")
            .bind(&id)
            .execute(&db)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE collection_attempts SET outcome='ACQUISITION_FAILED',reason=NULL,elapsed_ms=1,finish_digest='bad',finished_at=clock_timestamp() WHERE attempt_id=$1").bind(&id).execute(&db).await.is_err());
    store
        .finish_collection_attempt(
            &claim,
            &id,
            finish(O::DeadlineExceeded, Some(R::AcquisitionDeadline)),
        )
        .await
        .unwrap();
    assert!(
        sqlx::query(
            "UPDATE collection_attempts SET reason='PROVIDER_UNAVAILABLE' WHERE attempt_id=$1"
        )
        .bind(&id)
        .execute(&db)
        .await
        .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM collection_attempts WHERE attempt_id=$1")
            .bind(&id)
            .execute(&db)
            .await
            .is_err()
    );
    let coverage = store
        .collection_coverage(&operator, &session)
        .await
        .unwrap();
    assert_eq!(coverage.deadline_exceeded, "1");
    assert_eq!(coverage.acquisition_failed, "0");
}
