use arb_domain::{DecisionTrace, NetworkId};
use arb_paper::{
    AccountingAsset, InitialBalance, PaperCommand, PaperOutcome, PaperRun, ReservationRequest,
};
use arb_storage::{
    CollectionFinish, CollectionOutcome, CollectionPurpose, NewCommand, NewPaperRun, NewSession,
    Store, StoreError, WorkerClaim, export_content_digest,
};
use serde_json::json;
use uuid::Uuid;

const TOKEN: &str = "base-mainnet:0x0000000000000000000000000000000000000001";
const SECRET: &str = "provider-private-token-never-export";

async fn setup() -> (Store, sqlx::PgPool, String, String, WorkerClaim) {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL is mandatory; export DB tests must not skip");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let store = Store::from_pool(pool.clone());
    store.migrate().await.unwrap();
    let operator = format!("export-{}", Uuid::new_v4());
    let digest = format!("sha256:{}", "a".repeat(64));
    store.save_configuration(&operator, &digest, json!({
        "deployment":{"mode":"PAPER"}, "provider_url":format!("https://rpc.invalid?api_key={SECRET}"),
        "networks":{"base":{"enabled":true,"verified_asset_ids":[TOKEN]}}
    })).await.unwrap();
    let session = store
        .create_session(
            &operator,
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "PAPER".into(),
                configuration_digest: digest,
                experiment_id: "export-test".into(),
                strategy_ids: vec!["test-strategy".into()],
            },
        )
        .await
        .unwrap();
    let claim = store
        .claim_worker(
            &operator,
            &session.session_id,
            "base-mainnet",
            "export-worker",
            60,
        )
        .await
        .unwrap();
    store.complete_worker_recovery(&claim).await.unwrap();
    (store, pool, operator, session.session_id, claim)
}
fn balances() -> Vec<InitialBalance> {
    vec![
        InitialBalance {
            asset: AccountingAsset::Token(TOKEN.parse().unwrap()),
            amount: "100000000000000000000001".parse().unwrap(),
        },
        InitialBalance {
            asset: AccountingAsset::Native(NetworkId::BaseMainnet),
            amount: 100_u64.into(),
        },
    ]
}

#[tokio::test]
async fn complete_export_preserves_exact_economics_redacts_freeform_and_declares_unknown_artifacts()
{
    let (store, pool, operator, session, claim) = setup().await;
    let run = store
        .create_paper_run(
            &operator,
            &session,
            "run",
            NewPaperRun {
                initial_balances: balances(),
            },
        )
        .await
        .unwrap();
    store
        .issue_command(
            &operator,
            &session,
            "start",
            NewCommand {
                action: "START".into(),
                expected_revision: "0".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let generation = store
        .apply_pending(&claim, true, |_| Ok(()))
        .await
        .unwrap()
        .generation;
    store
        .record_capture_admission(
            &claim,
            generation,
            "capture-export",
            &format!("sha256:{}", "b".repeat(64)),
            &format!("/secret/{SECRET}/manifest.json"),
        )
        .await
        .unwrap();
    for (key, command) in [
        (
            format!("key-{SECRET}-1"),
            PaperCommand::Reserve {
                request: ReservationRequest {
                    attempt_id: format!("attempt-{SECRET}"),
                    principal_asset: TOKEN.parse().unwrap(),
                    principal: 99_u64.into(),
                    native_fee_budget: 10_u64.into(),
                },
            },
        ),
        (
            format!("key-{SECRET}-2"),
            PaperCommand::MarkUnknown {
                attempt_id: format!("attempt-{SECRET}"),
                reason: format!("Authorization Bearer {SECRET}\nprovider timeout"),
            },
        ),
        (
            format!("key-{SECRET}-3"),
            PaperCommand::Resolve {
                attempt_id: format!("attempt-{SECRET}"),
                outcome: PaperOutcome::NotIncluded {
                    reason: format!("api_key={SECRET}"),
                },
            },
        ),
    ] {
        store
            .apply_paper_command(&claim, generation, &run.run_id, &key, command)
            .await
            .unwrap();
    }
    let export = store.export_session(&operator, &session).await.unwrap();
    assert_eq!(export.snapshot.source_counts.paper_runs, "1");
    assert_eq!(export.snapshot.source_counts.paper_journal_events, "4");
    assert_eq!(export.snapshot.source_counts.capture_catalog_entries, "1");
    assert_eq!(export.snapshot.collection_completeness, "UNKNOWN");
    assert_eq!(
        export.content_sha256,
        export_content_digest(&export).unwrap()
    );
    let item = &export.data.paper_runs[0];
    let events = item
        .journal
        .iter()
        .map(|v| v.event.clone())
        .collect::<Vec<_>>();
    let replay = PaperRun::replay(&events).unwrap();
    assert_eq!(replay.balances().unwrap(), item.run.balances);
    assert_eq!(replay.reservations(), item.reservations);
    assert_eq!(
        item.run
            .balances
            .iter()
            .find(|b| matches!(b.asset, AccountingAsset::Token(_)))
            .unwrap()
            .total
            .to_string(),
        "100000000000000000000001"
    );
    let source = store
        .list_paper_journal(&operator, &run.run_id, None, 100)
        .await
        .unwrap();
    assert_ne!(
        source.items[1].event.command_id(),
        item.journal[1].event.command_id()
    );
    assert!(
        item.journal
            .iter()
            .all(|event| event.source_payload_sha256.starts_with("sha256:"))
    );
    let text = serde_json::to_string(&export).unwrap();
    assert!(!text.contains(SECRET));
    assert!(!text.contains("artifact_path"));
    assert!(!text.contains("provider_url"));
    assert!(text.contains("REDACTED_FREEFORM_TEXT"));
    let capture = &export.data.capture_dependencies[0];
    assert_eq!(capture.catalog_status, "PRESENT");
    assert_eq!(capture.raw_artifact_status, "NOT_VERIFIED");
    assert_eq!(capture.expiration_status, "UNKNOWN");
    assert!(matches!(
        store.export_session("another-operator", &session).await,
        Err(StoreError::NotFound)
    ));
    let mut changed = export.clone();
    changed.export_id = Uuid::new_v4().to_string();
    changed.exported_at = "2020-01-01T00:00:00Z".into();
    assert_eq!(
        export.content_sha256,
        export_content_digest(&changed).unwrap()
    );
    changed.snapshot.source_counts.paper_journal_events = "3".into();
    assert_ne!(
        export.content_sha256,
        export_content_digest(&changed).unwrap()
    );

    let collection = Uuid::new_v4().to_string();
    store
        .begin_collection_attempt(&claim, &collection, generation, CollectionPurpose::Research)
        .await
        .unwrap();
    let trace = unavailable_trace(&session, generation, 1);
    store
        .append_collection_decision_traces(
            &claim,
            &collection,
            generation,
            std::slice::from_ref(&trace),
            CollectionFinish {
                outcome: CollectionOutcome::DecisionsRecorded,
                reason: None,
                captured_pools: 0,
                elapsed_ms: 1,
            },
        )
        .await
        .unwrap();
    let associated = store.export_session(&operator, &session).await.unwrap();
    assert_eq!(
        associated.data.collection_attempts[0].decision_observation_ids,
        vec![trace.observation_id]
    );
    // Legacy observations can remain unassociated and still appear completely.
    let legacy = store
        .append_decision_traces(
            &claim,
            generation,
            &[unavailable_trace(&session, generation, 2)],
        )
        .await
        .unwrap();
    let both = store.export_session(&operator, &session).await.unwrap();
    assert_eq!(both.snapshot.source_counts.decisions, "2");
    assert_eq!(both.data.collection_attempts[0].decision_rows, "1");
    // Simulate a bad direct association write without weakening any audit
    // trigger. Its FK is valid but it contradicts the sealed terminal IDs.
    sqlx::query("INSERT INTO collection_decision_links(attempt_id,trace_id) VALUES($1,$2)")
        .bind(&collection)
        .bind(&legacy[0].trace_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        store.export_session(&operator, &session).await,
        Err(StoreError::CorruptState)
    ));
}

fn unavailable_trace(session: &str, generation: u64, time: u64) -> DecisionTrace {
    serde_json::from_value::<DecisionTrace>(json!({
        "schema_version":"1.0.0","observation_id":"","session_id":session,"experiment_id":"export-test",
        "generation":generation.to_string(),"configuration_digest":format!("sha256:{}","a".repeat(64)),
        "calculation_version":"test-math","strategy_id":"test-strategy","network_id":"base-mainnet","mode":"PAPER",
        "source_kind":"SYNTHETIC_FIXTURE","dataset_origin":"MANUALLY_CONSTRUCTED","observed_at_unix_ms":time,"input_age_ms":null,
        "capture_refs":[],"route":[],"amount_in_minor":null,"result":{"status":"DATA_UNAVAILABLE","reason_codes":["NO_CAPTURE_INPUTS"]},
        "grouping":{"version":"","key":"","window_ms":1000,"window_start_ms":0},"diagnostics":[]
    })).unwrap().seal().unwrap()
}

#[tokio::test]
async fn oversized_source_is_refused_before_decoding_without_partial_or_cross_operator_rows() {
    let (store, pool, operator, session, _claim) = setup().await;
    // One bulk fixture makes the source too large. It is never decoded; no valid
    // collection evidence or provider observation is claimed by this count test.
    sqlx::query("INSERT INTO collection_attempts(attempt_id,operator_id,session_id,network_id,configuration_digest,experiment_id,generation,worker_epoch,worker_id,purpose,start_digest) SELECT $3||'-'||n::text,$1,$2,'base-mainnet',$4,'export-test',0,1,'fixture','READINESS','fixture' FROM generate_series(1,10001) n")
        .bind(&operator).bind(&session).bind(Uuid::new_v4().to_string()).bind(format!("sha256:{}","a".repeat(64))).execute(&pool).await.unwrap();
    assert!(matches!(
        store.export_session(&operator, &session).await,
        Err(StoreError::ExportLimitExceeded)
    ));
    assert!(matches!(
        store.export_session("other", &session).await,
        Err(StoreError::NotFound)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM collection_attempts WHERE session_id=$1"
        )
        .bind(&session)
        .fetch_one(&pool)
        .await
        .unwrap(),
        10001
    );
}

#[tokio::test]
async fn oversized_payload_is_refused_before_loading_or_decoding_even_with_one_source_row() {
    let (store, pool, operator, session, _claim) = setup().await;
    // Deliberately invalid sealed-domain fixture: rejection must occur at the
    // aggregate byte bound before its raw payload is fetched or deserialized.
    sqlx::query("INSERT INTO decision_traces(trace_id,operator_id,session_id,observation_id,payload_digest,configuration_digest,generation,observed_at_unix_ms,result_status,grouping_version,grouping_key,window_start_ms,payload) VALUES($1,$2,$3,'oversized-fixture','fixture',$4,0,1,'DATA_UNAVAILABLE','fixture','fixture',0,$5)")
        .bind(Uuid::new_v4().to_string()).bind(&operator).bind(&session)
        .bind(format!("sha256:{}", "a".repeat(64)))
        .bind(json!({"oversized_fixture":"x".repeat(arb_storage::MAX_EXPORT_BYTES + 1)}))
        .execute(&pool).await.unwrap();
    assert!(matches!(
        store.export_session(&operator, &session).await,
        Err(StoreError::ExportLimitExceeded)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM decision_traces WHERE operator_id=$1 AND session_id=$2"
        )
        .bind(&operator)
        .bind(&session)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}
