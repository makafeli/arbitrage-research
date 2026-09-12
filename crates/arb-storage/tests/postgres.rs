use arb_storage::{NewCommand, NewSession, Store, StoreError};
use serde_json::json;
use uuid::Uuid;

async fn fixture() -> (Store, String, NewSession) {
    let url = std::env::var("TEST_DATABASE_URL").expect(
        "TEST_DATABASE_URL is required: PostgreSQL integration tests must not silently skip",
    );
    let store = Store::connect(&url).await.expect("connect test PostgreSQL");
    store.migrate().await.expect("migrate test PostgreSQL");
    let operator = format!("test-{}", Uuid::new_v4());
    store
        .save_configuration(
            &operator,
            "sha256:test",
            json!({"mode":"PAPER","live":false}),
        )
        .await
        .unwrap();
    let input = NewSession {
        network_id: "base-mainnet".into(),
        mode: "PAPER".into(),
        configuration_digest: "sha256:test".into(),
        experiment_id: "research".into(),
        strategy_ids: vec!["cycle".into()],
    };
    (store, operator, input)
}
fn command(action: &str, revision: &str) -> NewCommand {
    NewCommand {
        action: action.into(),
        expected_revision: revision.into(),
        reason: None,
    }
}

#[tokio::test]
async fn concurrent_creation_is_idempotent_and_payload_conflicts() {
    let (store, operator, input) = fixture().await;
    let (a, b) = tokio::join!(
        store.create_session(&operator, "same", input.clone()),
        store.create_session(&operator, "same", input.clone())
    );
    let a = a.unwrap();
    assert_eq!(a.session_id, b.unwrap().session_id);
    assert_eq!(a.observed_state, "RECOVERING");
    let mut changed = input;
    changed.experiment_id = "different".into();
    assert!(matches!(
        store.create_session(&operator, "same", changed).await,
        Err(StoreError::Conflict(_))
    ));
    assert_eq!(
        store
            .list_sessions_page(&operator, None, 100)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}
#[tokio::test]
async fn accepted_commands_survive_connection_restart_without_fabricated_ack() {
    let (store, operator, input) = fixture().await;
    let session = store
        .create_session(&operator, "create", input)
        .await
        .unwrap();
    let stop = store
        .issue_command(&operator, &session.session_id, "stop", command("STOP", "0"))
        .await
        .unwrap();
    assert_eq!(stop.status, "PENDING");
    assert!(!stop.fence_effective);
    let reopened = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert_eq!(
        stop,
        reopened
            .get_command(&operator, &stop.command_id)
            .await
            .unwrap()
    );
    assert_eq!(
        reopened
            .get_session(&operator, &session.session_id)
            .await
            .unwrap()
            .applied_revision,
        "0"
    );
    assert!(matches!(
        reopened
            .get_command("another-operator", &stop.command_id)
            .await,
        Err(StoreError::NotFound)
    ));
}
#[tokio::test]
async fn concurrent_command_retry_serializes_and_changed_payload_or_stale_revision_conflicts() {
    let (store, operator, input) = fixture().await;
    let session = store
        .create_session(&operator, "create", input)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        store.issue_command(&operator, &session.session_id, "stop", command("STOP", "0")),
        store.issue_command(&operator, &session.session_id, "stop", command("STOP", "0"))
    );
    assert_eq!(a.unwrap().command_id, b.unwrap().command_id);
    assert!(matches!(
        store
            .issue_command(
                &operator,
                &session.session_id,
                "different",
                command("STOP", "0")
            )
            .await,
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store
            .issue_command(&operator, &session.session_id, "stop", command("STOP", "1"))
            .await,
        Err(StoreError::Conflict(_))
    ));
    assert_eq!(
        store
            .get_session(&operator, &session.session_id)
            .await
            .unwrap()
            .desired_revision,
        "1"
    );
}
#[tokio::test]
async fn stop_supersedes_without_claiming_prior_command_applied() {
    let (store, operator, input) = fixture().await;
    let session = store
        .create_session(&operator, "create", input)
        .await
        .unwrap();
    let first = store
        .issue_command(
            &operator,
            &session.session_id,
            "first",
            command("STOP", "0"),
        )
        .await
        .unwrap();
    let second = store
        .issue_command(
            &operator,
            &session.session_id,
            "second",
            command("STOP", "1"),
        )
        .await
        .unwrap();
    assert_eq!(second.status, "PENDING");
    let first = store
        .get_command(&operator, &first.command_id)
        .await
        .unwrap();
    assert_eq!(first.status, "SUPERSEDED");
    assert!(first.applied_at.is_none());
    assert!(!first.fence_effective);
}
#[tokio::test]
async fn immutable_snapshots_operator_scope_and_research_only() {
    let (store, operator, input) = fixture().await;
    assert!(matches!(
        store
            .save_configuration(&operator, "sha256:test", json!({"live":true}))
            .await,
        Err(StoreError::Conflict(_))
    ));
    let session = store
        .create_session(&operator, "create", input.clone())
        .await
        .unwrap();
    assert!(matches!(
        store.get_session("other", &session.session_id).await,
        Err(StoreError::NotFound)
    ));
    let mut live = input.clone();
    live.mode = "LIVE".into();
    assert!(matches!(
        store.create_session(&operator, "live", live).await,
        Err(StoreError::CapabilityUnavailable)
    ));
    assert!(matches!(
        store
            .issue_command(
                &operator,
                &session.session_id,
                "disarm",
                command("DISARM", "0")
            )
            .await,
        Err(StoreError::CapabilityUnavailable)
    ));
    let mut unknown = input;
    unknown.configuration_digest = "unknown".into();
    assert!(matches!(
        store.create_session(&operator, "unknown", unknown).await,
        Err(StoreError::InvalidInput(_))
    ));
}
#[tokio::test]
async fn bounded_pagination_and_noncanonical_revisions_fail_closed() {
    let (store, operator, input) = fixture().await;
    for i in 0..3 {
        store
            .create_session(&operator, &format!("create-{i}"), input.clone())
            .await
            .unwrap();
    }
    let first = store.list_sessions_page(&operator, None, 2).await.unwrap();
    assert_eq!(first.items.len(), 2);
    let next = store
        .list_sessions_page(&operator, first.next_cursor.as_deref(), 2)
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert!(next.next_cursor.is_none());
    assert!(
        store
            .list_sessions_page(&operator, None, 101)
            .await
            .is_err()
    );
    assert!(
        store
            .list_sessions_page(&operator, Some("not-a-cursor"), 1)
            .await
            .is_err()
    );
    for rev in ["01", "-1", "18446744073709551615", "1.0"] {
        assert!(matches!(
            store
                .issue_command(
                    &operator,
                    &first.items[0].session_id,
                    "bad",
                    command("STOP", rev)
                )
                .await,
            Err(StoreError::InvalidInput(_))
        ));
    }
}
#[tokio::test]
async fn sql_constraints_protect_identity_and_append_only_audit() {
    let (store, operator, input) = fixture().await;
    let session = store
        .create_session(&operator, "create", input)
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE research_sessions SET mode='OBSERVE' WHERE session_id=$1")
            .bind(&session.session_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM control_audit_events WHERE session_id=$1")
            .bind(&session.session_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE research_sessions SET applied_revision=2 WHERE session_id=$1")
            .bind(&session.session_id)
            .execute(&pool)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn replay_creation_survives_registry_changes_and_detects_payload_conflicts() {
    let (store, operator, input) = fixture().await;
    let session = store
        .create_session(&operator, "create", input.clone())
        .await
        .unwrap();
    assert_eq!(
        store
            .replay_session_creation(&operator, "create", &input)
            .await
            .unwrap()
            .unwrap()
            .session_id,
        session.session_id
    );
    assert!(
        store
            .replay_session_creation(&operator, "unknown", &input)
            .await
            .unwrap()
            .is_none()
    );
    let mut changed = input;
    changed.experiment_id = "changed".into();
    assert!(matches!(
        store
            .replay_session_creation(&operator, "create", &changed)
            .await,
        Err(StoreError::Conflict(_))
    ));
}
#[tokio::test]
async fn corrupted_denormalized_state_is_not_a_worker_authority() {
    let (store, operator, input) = fixture().await;
    let session = store
        .create_session(&operator, "create", input)
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::query("UPDATE research_sessions SET desired_revision=5 WHERE session_id=$1")
        .bind(&session.session_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        store
            .issue_command(&operator, &session.session_id, "stop", command("STOP", "5"))
            .await,
        Err(StoreError::CorruptState)
    ));
    assert!(matches!(
        store.get_session(&operator, &session.session_id).await,
        Err(StoreError::CorruptState)
    ));
    assert!(matches!(
        store.list_sessions_page(&operator, None, 10).await,
        Err(StoreError::CorruptState)
    ));
}
