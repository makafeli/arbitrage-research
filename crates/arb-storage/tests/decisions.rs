use arb_domain::{DecisionTrace, Evidence, NetworkId, SourceKind};
use arb_storage::{NewCommand, NewSession, OpportunityFilter, Store, StoreError, WorkerClaim};
use serde_json::{Value, json};
use uuid::Uuid;
fn hash(c: &str) -> String {
    format!("sha256:{}", c.repeat(64))
}
async fn setup() -> (Store, String, String, WorkerClaim, u64) {
    setup_mode("OBSERVE").await
}
async fn setup_mode(mode: &str) -> (Store, String, String, WorkerClaim, u64) {
    setup_mode_policy(mode, None).await
}
async fn setup_mode_policy(
    mode: &str,
    policy: Option<Value>,
) -> (Store, String, String, WorkerClaim, u64) {
    let store = Store::connect(
        &std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is mandatory; decision DB tests must not skip"),
    )
    .await
    .unwrap();
    store.migrate().await.unwrap();
    let operator = format!("decision-{}", Uuid::new_v4());
    store
        .save_configuration(
            &operator,
            &hash("a"),
            json!({"mode":mode,"networks":{"base":{"chain_freshness":policy}}}),
        )
        .await
        .unwrap();
    let session = store
        .create_session(
            &operator,
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: mode.into(),
                configuration_digest: hash("a"),
                experiment_id: "decision-fixture".into(),
                strategy_ids: vec!["fixture-strategy".into()],
            },
        )
        .await
        .unwrap();
    let claim = store
        .claim_worker(&operator, &session.session_id, "base-mainnet", "worker", 60)
        .await
        .unwrap();
    store.complete_worker_recovery(&claim).await.unwrap();
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
    let generation = store
        .apply_pending(&claim, true, |_| Ok(()))
        .await
        .unwrap()
        .generation;
    for (id, digest) in [("capture-one", hash("1")), ("capture-two", hash("2"))] {
        store
            .record_capture_admission(&claim, generation, id, &digest, &format!("/synthetic/{id}"))
            .await
            .unwrap();
    }
    (store, operator, session.session_id, claim, generation)
}
fn trace(session: &str, generation: u64, time: u64, status: &str) -> DecisionTrace {
    let mut value = json!({
     "schema_version":"1.0.0","observation_id":"","session_id":session,"experiment_id":"decision-fixture","generation":generation.to_string(),"configuration_digest":hash("a"),"calculation_version":"fixture-math-v1","strategy_id":"fixture-strategy","network_id":"base-mainnet","mode":"OBSERVE","source_kind":"SYNTHETIC_FIXTURE","dataset_origin":"MANUALLY_CONSTRUCTED","observed_at_unix_ms":time,"input_age_ms":0,
     "capture_refs":[{"capture_id":"capture-one","manifest_digest":hash("1"),"snapshot_id":hash("1")},{"capture_id":"capture-two","manifest_digest":hash("2"),"snapshot_id":hash("2")}],
     "route":[{"pool_id":"base-mainnet:0x0000000000000000000000000000000000000003","asset_in":"base-mainnet:0x0000000000000000000000000000000000000001","asset_out":"base-mainnet:0x0000000000000000000000000000000000000002","venue_family":"uniswap-v3"},{"pool_id":"base-mainnet:0x0000000000000000000000000000000000000004","asset_in":"base-mainnet:0x0000000000000000000000000000000000000002","asset_out":"base-mainnet:0x0000000000000000000000000000000000000001","venue_family":"uniswap-v3"}],
     "amount_in_minor":"100","result":{"status":"QUOTED","quoted_output_minor":"110","gross_delta_minor":"10","included_pool_fees":["1","1"]},
     "grouping":{"version":"","key":"","window_ms":1000,"window_start_ms":0},"diagnostics":[]
    });
    match status {
        "REJECTED" => {
            value["result"] = json!({"status":"REJECTED","reason_codes":["MATH_INPUT_REJECTED"]})
        }
        "NO_ROUTE" => {
            value["result"] =
                json!({"status":"NO_ROUTE","reason_codes":["NO_ELIGIBLE_POOL_PAIRS"]});
            value["route"] = json!([]);
            value["amount_in_minor"] = Value::Null;
            value["input_age_ms"] = Value::Null;
            value["capture_refs"].as_array_mut().unwrap().pop();
        }
        "DATA_UNAVAILABLE" => {
            value["result"] =
                json!({"status":"DATA_UNAVAILABLE","reason_codes":["NO_CAPTURE_INPUTS"]});
            value["route"] = json!([]);
            value["capture_refs"] = json!([]);
            value["amount_in_minor"] = Value::Null;
            value["input_age_ms"] = Value::Null;
        }
        "NEGATIVE" => {
            value["result"] = json!({"status":"QUOTED","quoted_output_minor":"90","gross_delta_minor":"-10","included_pool_fees":["1","1"]})
        }
        _ => {}
    }
    serde_json::from_value::<DecisionTrace>(value)
        .unwrap()
        .seal()
        .unwrap()
}
#[tokio::test]
async fn grouping_preserves_denominators_and_unknown_coverage() {
    let (store, operator, id, claim, generation) = setup().await;
    let traces = vec![
        trace(&id, generation, 1001, "QUOTED"),
        trace(&id, generation, 1002, "NEGATIVE"),
        trace(&id, generation, 1003, "REJECTED"),
        trace(&id, generation, 1004, "NO_ROUTE"),
        trace(&id, generation, 1005, "DATA_UNAVAILABLE"),
    ];
    let first = store
        .append_decision_traces(&claim, generation, &traces)
        .await
        .unwrap();
    let repeat = store
        .append_decision_traces(&claim, generation, &traces)
        .await
        .unwrap();
    assert_eq!(first[0].trace_id, repeat[0].trace_id);
    let coverage = store.decision_coverage(&operator, &id).await.unwrap();
    assert_eq!(coverage.raw_observations, "5");
    assert_eq!(coverage.quoted_candidates, "2");
    assert_eq!(coverage.rejected, "1");
    assert_eq!(coverage.no_route, "1");
    assert_eq!(coverage.data_unavailable, "1");
    assert_eq!(coverage.unique_opportunity_groups, "1");
    assert_eq!(coverage.collection_completeness, "UNKNOWN");
    assert!(coverage.eligible_attempts.is_none());
    assert!(!coverage.execution_accounting_available);
    let groups = store
        .list_decision_groups(&operator, &id, None, 100)
        .await
        .unwrap();
    assert_eq!(groups.items.len(), 3);
    assert!(
        groups
            .items
            .iter()
            .any(|g| g.raw_observations == "3" && g.quoted_candidates == "2" && g.rejected == "1")
    );
    let next_window = trace(&id, generation, 2001, "QUOTED");
    store
        .append_decision_traces(&claim, generation, &[next_window])
        .await
        .unwrap();
    assert_eq!(
        store
            .decision_coverage(&operator, &id)
            .await
            .unwrap()
            .unique_opportunity_groups,
        "2"
    );
}
#[tokio::test]
async fn opportunity_filtering_happens_before_pagination_and_quotes_never_become_simulations() {
    let (store, operator, id, claim, generation) = setup().await;
    let traces = vec![
        trace(&id, generation, 1001, "REJECTED"),
        trace(&id, generation, 1002, "QUOTED"),
        trace(&id, generation, 1003, "NO_ROUTE"),
        trace(&id, generation, 1004, "NEGATIVE"),
    ];
    store
        .append_decision_traces(&claim, generation, &traces)
        .await
        .unwrap();
    let filter = OpportunityFilter {
        session_id: Some(id.clone()),
        source_kind: Some(SourceKind::SyntheticFixture),
        ..Default::default()
    };
    let first = store
        .list_opportunities(&operator, filter.clone(), None, 1)
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].evidence_label, Evidence::Candidate);
    assert!(first.items[0].net_after_explicit_costs_minor.is_none());
    assert!(first.next_cursor.is_some());
    let next = store
        .list_opportunities(&operator, filter, first.next_cursor.as_deref(), 1)
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert!(next.next_cursor.is_none());
    assert_ne!(first.items[0].opportunity_id, next.items[0].opportunity_id);
    assert!(
        store
            .list_opportunities(
                &operator,
                OpportunityFilter {
                    session_id: Some(id.clone()),
                    source_kind: Some(SourceKind::CapturedMarketData),
                    ..Default::default()
                },
                None,
                10
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .list_opportunities(
                &operator,
                OpportunityFilter {
                    session_id: Some(id),
                    evidence_label: Some(Evidence::Simulated),
                    ..Default::default()
                },
                None,
                10
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
}
#[tokio::test]
async fn changed_identity_missing_capture_and_failed_batch_are_not_partially_saved() {
    let (store, operator, id, claim, generation) = setup().await;
    let first = trace(&id, generation, 1001, "QUOTED");
    store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&first))
        .await
        .unwrap();
    let mut changed = first.clone();
    changed.observed_at_unix_ms += 1;
    assert!(matches!(
        store
            .append_decision_traces(&claim, generation, &[changed])
            .await,
        Err(StoreError::Conflict(_))
    ));
    let fresh = trace(&id, generation, 1003, "QUOTED");
    let mut missing = trace(&id, generation, 1004, "QUOTED");
    missing.capture_refs[0].capture_id = "missing".into();
    missing = missing.seal().unwrap();
    assert!(matches!(
        store
            .append_decision_traces(&claim, generation, &[fresh, missing])
            .await,
        Err(StoreError::Conflict(_))
    ));
    assert_eq!(
        store
            .decision_coverage(&operator, &id)
            .await
            .unwrap()
            .raw_observations,
        "1"
    );
    assert!(matches!(
        store
            .get_decision_trace("another", &first.observation_id)
            .await,
        Err(StoreError::NotFound)
    ));
    assert!(
        store
            .append_decision_traces(&claim, generation, &vec![first; 65])
            .await
            .is_err()
    );
}
#[tokio::test]
async fn late_generation_and_mutating_raw_history_are_rejected() {
    let (store, operator, id, claim, generation) = setup().await;
    let first = trace(&id, generation, 1001, "QUOTED");
    let saved = store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&first))
        .await
        .unwrap();
    store
        .issue_command(
            &operator,
            &id,
            "stop",
            NewCommand {
                action: "STOP".into(),
                expected_revision: "1".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    store
        .apply_pending(&claim, false, |_| Ok(()))
        .await
        .unwrap();
    assert!(matches!(
        store
            .append_decision_traces(
                &claim,
                generation,
                &[trace(&id, generation, 1002, "QUOTED")]
            )
            .await,
        Err(StoreError::Conflict(_))
    ));
    let reopened = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert_eq!(
        reopened
            .get_decision_trace(&operator, &first.observation_id)
            .await
            .unwrap()
            .trace_id,
        saved[0].trace_id
    );
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM decision_traces WHERE session_id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn paper_research_capture_and_decision_admission_do_not_authorize_execution() {
    let (store, operator, id, claim, generation) = setup_mode("PAPER").await;
    let mut observation = trace(&id, generation, 1001, "QUOTED");
    observation.mode = arb_domain::Mode::Paper;
    observation = observation.seal().unwrap();
    store
        .append_decision_traces(&claim, generation, &[observation])
        .await
        .unwrap();
    let coverage = store.decision_coverage(&operator, &id).await.unwrap();
    assert_eq!(coverage.quoted_candidates, "1");
    assert!(!coverage.execution_accounting_available);
    let items = store
        .list_opportunities(
            &operator,
            OpportunityFilter {
                session_id: Some(id),
                ..Default::default()
            },
            None,
            10,
        )
        .await
        .unwrap()
        .items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].evidence_label, Evidence::Candidate);
    assert!(items[0].net_after_explicit_costs_minor.is_none());
}

fn manual_scenario() -> arb_paper::CostScenario {
    serde_json::from_value(json!({
        "schema_version":"1.0.0","scenario_id":"manual-baseline","version":"v1",
        "origin":"MANUALLY_CONSTRUCTED","provenance_reference":"operator-assumption",
        "valuation_max_age_ms":1000,"fee_composition":"BASE_EXECUTION_INCLUDES_PRIORITY",
        "expenses":[],"funding":{"status":"OWN_VIRTUAL_CAPITAL"},"overhead":{"status":"NOT_ALLOCATED"}
    })).unwrap()
}
#[tokio::test]
async fn cost_assessment_concurrent_retry_restart_and_negative_unknowns_preserve_source() {
    let (store, operator, id, claim, generation) = setup().await;
    let source = trace(&id, generation, 2001, "NEGATIVE");
    let saved = store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&source))
        .await
        .unwrap();
    let before = store.export_session(&operator, &id).await.unwrap();
    let request = arb_storage::NewCostAssessment {
        observation_id: source.observation_id.clone(),
        scenario: manual_scenario(),
    };
    let (first, retry) = tokio::join!(
        store.create_cost_assessment(&operator, &id, "concurrent-cost-key", request.clone()),
        store.create_cost_assessment(&operator, &id, "concurrent-cost-key", request.clone())
    );
    let first = first.unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(retry.unwrap()).unwrap()
    );
    assert_eq!(
        first
            .assessment
            .report
            .gross_after_quote_included_costs
            .to_string(),
        "-10"
    );
    assert!(first.assessment.report.transaction_net.is_none());
    assert!(first.assessment.report.fully_allocated_net.is_none());
    first.assessment.replay(&source).unwrap();
    let restarted = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let after_restart = restarted
        .get_cost_assessment(&operator, &id, &first.record_id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(after_restart).unwrap()
    );
    assert_eq!(
        restarted
            .create_cost_assessment(&operator, &id, "concurrent-cost-key", request.clone())
            .await
            .unwrap()
            .record_id,
        first.record_id
    );
    let after = restarted.export_session(&operator, &id).await.unwrap();
    assert_eq!(after.schema_version, "1.1.0");
    assert_eq!(after.snapshot.source_counts.cost_assessments, "1");
    assert_eq!(after.data.cost_assessments[0].record_id, first.record_id);
    assert_eq!(
        serde_json::to_value(&after.data.decisions).unwrap(),
        serde_json::to_value(&before.data.decisions).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&after.data.paper_runs).unwrap(),
        serde_json::to_value(&before.data.paper_runs).unwrap()
    );
    assert_eq!(after.data.decisions[0].trace_id, saved[0].trace_id);
    assert_eq!(
        after.content_sha256,
        arb_storage::export_content_digest(&after).unwrap()
    );
    assert_eq!(
        after.content_sha256,
        restarted
            .export_session(&operator, &id)
            .await
            .unwrap()
            .content_sha256
    );
    assert_ne!(after.content_sha256, before.content_sha256);
    assert!(
        after.data.decisions[0]
            .trace
            .to_opportunity()
            .unwrap()
            .unwrap()
            .net_after_explicit_costs_minor
            .is_none()
    );
    let mut complete = request.clone();
    complete.scenario.expenses = ["NETWORK_EXECUTION","BASE_L1_DATA","RELAY_TIP","FUNDING","ACCOUNT_SETUP","OTHER"].into_iter().map(|kind| {
        let native=matches!(kind,"NETWORK_EXECUTION"|"BASE_L1_DATA"|"RELAY_TIP");
        serde_json::from_value(json!({"kind":kind,"amount":{"status":"KNOWN",
            "asset":if native {json!({"kind":"NATIVE","identity":"base-mainnet"})} else {json!({"kind":"TOKEN","identity":source.route[0].asset_in})},
            "amount":"1","valuation":if native {json!({"kind":"RATIO","numerator":"1","denominator":"1","reference":"manual-ratio","valued_at_unix_ms":2001})} else {json!({"kind":"SAME_ASSET","reference":"manual-same-asset","valued_at_unix_ms":2001})}
        }})).unwrap()
    }).collect();
    complete.scenario.overhead=serde_json::from_value(json!({"status":"ALLOCATED","amount_in_start_asset":"2","method":"per-observation","version":"v1","reference":"manual-overhead"})).unwrap();
    let known = store
        .create_cost_assessment(&operator, &id, "known-negative", complete)
        .await
        .unwrap();
    assert_eq!(
        known.assessment.report.transaction_net.unwrap().to_string(),
        "-16"
    );
    assert_eq!(
        known
            .assessment
            .report
            .fully_allocated_net
            .unwrap()
            .to_string(),
        "-18"
    );
    let restored = store
        .get_cost_assessment(&operator, &id, &known.record_id)
        .await
        .unwrap();
    restored.assessment.replay(&source).unwrap();
    let mut changed = request;
    changed.scenario.version = "v2".into();
    assert!(matches!(
        store
            .create_cost_assessment(&operator, &id, "concurrent-cost-key", changed)
            .await,
        Err(StoreError::Conflict(_))
    ));
}
#[tokio::test]
async fn cost_assessments_reject_scope_nonquotes_and_paginate_without_partial_records() {
    let (store, operator, id, claim, generation) = setup().await;
    let sources = [
        trace(&id, generation, 3001, "QUOTED"),
        trace(&id, generation, 3002, "REJECTED"),
    ];
    store
        .append_decision_traces(&claim, generation, &sources)
        .await
        .unwrap();
    let request = arb_storage::NewCostAssessment {
        observation_id: sources[0].observation_id.clone(),
        scenario: manual_scenario(),
    };
    assert!(matches!(
        store
            .create_cost_assessment("other", &id, "wrong-operator", request.clone())
            .await,
        Err(StoreError::NotFound)
    ));
    let other = store
        .create_session(
            &operator,
            "other-session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: hash("a"),
                experiment_id: "decision-fixture".into(),
                strategy_ids: vec!["fixture-strategy".into()],
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .create_cost_assessment(
                &operator,
                &other.session_id,
                "wrong-session",
                request.clone()
            )
            .await,
        Err(StoreError::NotFound)
    ));
    let rejected = arb_storage::NewCostAssessment {
        observation_id: sources[1].observation_id.clone(),
        scenario: manual_scenario(),
    };
    assert!(matches!(
        store
            .create_cost_assessment(&operator, &id, "rejected", rejected)
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let first = store
        .create_cost_assessment(&operator, &id, "first", request.clone())
        .await
        .unwrap();
    let second = store
        .create_cost_assessment(&operator, &id, "second", request)
        .await
        .unwrap();
    assert!(matches!(
        store
            .get_cost_assessment("other", &id, &first.record_id)
            .await,
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        store
            .get_cost_assessment(&operator, &other.session_id, &first.record_id)
            .await,
        Err(StoreError::NotFound)
    ));
    let page = store
        .list_cost_assessments(&operator, &id, None, 1)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let last = store
        .list_cost_assessments(&operator, &id, page.next_cursor.as_deref(), 1)
        .await
        .unwrap();
    assert_eq!(last.items.len(), 1);
    assert!(last.next_cursor.is_none());
    let mut ids = vec![
        page.items[0].record_id.clone(),
        last.items[0].record_id.clone(),
    ];
    ids.sort();
    let mut expected = vec![first.record_id, second.record_id];
    expected.sort();
    assert_eq!(ids, expected);
    assert!(matches!(
        store
            .list_cost_assessments(&operator, &id, Some("bad"), 1)
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    assert!(matches!(
        store.list_cost_assessments(&operator, &id, None, 101).await,
        Err(StoreError::InvalidInput(_))
    ));
}
#[tokio::test]
async fn cost_assessment_history_is_append_only_and_rejects_self_consistent_tampered_reports() {
    use sha2::{Digest, Sha256};
    let (store, operator, id, claim, generation) = setup().await;
    let source = trace(&id, generation, 4001, "QUOTED");
    store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&source))
        .await
        .unwrap();
    let record = store
        .create_cost_assessment(
            &operator,
            &id,
            "original",
            arb_storage::NewCostAssessment {
                observation_id: source.observation_id,
                scenario: manual_scenario(),
            },
        )
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE cost_assessments SET network_id='solana-mainnet' WHERE record_id=$1")
            .bind(&record.record_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM cost_assessments WHERE record_id=$1")
            .bind(&record.record_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let mut bad = record.assessment.clone();
    bad.report.transaction_net = Some("999".parse().unwrap());
    let digest = hex::encode(Sha256::digest(serde_json::to_vec(&bad).unwrap()));
    let bad_id = Uuid::now_v7().to_string();
    sqlx::query("INSERT INTO cost_assessments(record_id,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,idempotency_key,request_digest,payload_digest,payload) SELECT $2,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,'corrupt-copy',request_digest,$3,$4 FROM cost_assessments WHERE record_id=$1")
        .bind(&record.record_id).bind(&bad_id).bind(digest).bind(serde_json::to_value(bad).unwrap()).execute(&pool).await.unwrap();
    assert!(matches!(
        store.get_cost_assessment(&operator, &id, &bad_id).await,
        Err(StoreError::CorruptState)
    ));
    assert!(matches!(
        store.export_session(&operator, &id).await,
        Err(StoreError::CorruptState)
    ));
}
#[tokio::test]
async fn export_bound_includes_cost_assessment_rows_before_loading_them() {
    let (store, operator, id, claim, generation) = setup().await;
    let source = trace(&id, generation, 5001, "QUOTED");
    store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&source))
        .await
        .unwrap();
    let record = store
        .create_cost_assessment(
            &operator,
            &id,
            "original",
            arb_storage::NewCostAssessment {
                observation_id: source.observation_id,
                scenario: manual_scenario(),
            },
        )
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    // Deliberately tiny invalid payloads keep this below the byte bound. The
    // sixth dataset's source-row bound must reject before any payload decoding.
    sqlx::query("INSERT INTO cost_assessments(record_id,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,idempotency_key,request_digest,payload_digest,payload) SELECT $1||'-'||n::text,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,'bound-'||n::text,request_digest,payload_digest,'{}'::jsonb FROM cost_assessments CROSS JOIN generate_series(1,10000) n WHERE record_id=$1")
        .bind(record.record_id).execute(&pool).await.unwrap();
    assert!(matches!(
        store.export_session(&operator, &id).await,
        Err(StoreError::ExportLimitExceeded)
    ));
}

#[tokio::test]
async fn export_cost_payload_byte_bound_precedes_typed_decoding() {
    let (store, operator, id, claim, generation) = setup().await;
    let source = trace(&id, generation, 6001, "QUOTED");
    store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&source))
        .await
        .unwrap();
    let record = store
        .create_cost_assessment(
            &operator,
            &id,
            "original",
            arb_storage::NewCostAssessment {
                observation_id: source.observation_id,
                scenario: manual_scenario(),
            },
        )
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    // An isolated corrupt source fixture has only two rows, so the byte limit
    // must reject before deserializing its deliberately invalid report shape.
    sqlx::query("INSERT INTO cost_assessments(record_id,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,idempotency_key,request_digest,payload_digest,payload) SELECT $2,operator_id,session_id,source_trace_id,observation_id,configuration_digest,experiment_id,network_id,'oversized-copy',request_digest,payload_digest,jsonb_build_object('oversized_fixture',repeat('x',8388609)) FROM cost_assessments WHERE record_id=$1")
        .bind(record.record_id).bind(Uuid::now_v7().to_string()).execute(&pool).await.unwrap();
    assert!(matches!(
        store.export_session(&operator, &id).await,
        Err(StoreError::ExportLimitExceeded)
    ));
}

fn freshness_trace(mut source: DecisionTrace, max_chain_age_ms: u64) -> DecisionTrace {
    let policy = arb_domain::ChainFreshnessPolicy {
        version: arb_domain::CHAIN_FRESHNESS_VERSION.into(),
        max_chain_age_ms,
    };
    let inputs = source
        .capture_refs
        .iter()
        .map(|capture| arb_domain::ChainTimeInput {
            capture_id: capture.capture_id.clone(),
            source: arb_domain::ChainTimeSource::BaseFinalizedBlockTimestamp {
                block_number: "1".into(),
                block_hash: format!("0x{}", "1".repeat(64)),
                parent_hash: format!("0x{}", "2".repeat(64)),
            },
            chain_time_seconds: Some(1),
        })
        .collect();
    source.chain_freshness = Some(
        arb_domain::ChainFreshnessReport::assess(
            policy,
            source.observed_at_unix_ms,
            source.input_age_ms.unwrap(),
            inputs,
        )
        .unwrap(),
    );
    source.schema_version = arb_domain::FRESHNESS_DECISION_SCHEMA_VERSION.into();
    source.calculation_version = arb_domain::FRESHNESS_CALCULATION_VERSION.into();
    source.seal().unwrap()
}

#[tokio::test]
async fn immutable_chain_policy_prevents_report_changes_and_legacy_downgrades() {
    let policy = json!({"version":"finalized-chain-time-v1","max_chain_age_ms":100});
    let (store, operator, id, claim, generation) = setup_mode_policy("OBSERVE", Some(policy)).await;
    let legacy = trace(&id, generation, 1001, "QUOTED");
    assert!(matches!(
        store
            .append_decision_traces(&claim, generation, std::slice::from_ref(&legacy))
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let fresh = freshness_trace(legacy.clone(), 100);
    let stored = store
        .append_decision_traces(&claim, generation, std::slice::from_ref(&fresh))
        .await
        .unwrap();
    let retrieved = store
        .get_decision_trace(&operator, &fresh.observation_id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&retrieved.trace).unwrap(),
        serde_json::to_value(&fresh).unwrap()
    );
    assert_eq!(retrieved.trace_id, stored[0].trace_id);
    assert!(matches!(
        store
            .get_decision_trace("different-operator", &fresh.observation_id)
            .await,
        Err(StoreError::NotFound)
    ));
    let changed = freshness_trace(legacy, 200);
    assert!(matches!(
        store
            .append_decision_traces(&claim, generation, std::slice::from_ref(&changed))
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let mut downgrade = fresh.clone();
    downgrade.chain_freshness = None;
    downgrade.schema_version = arb_domain::DECISION_SCHEMA_VERSION.into();
    downgrade.calculation_version = "fixture-math-v1".into();
    let downgrade = downgrade.seal().unwrap();
    assert!(matches!(
        store
            .append_decision_traces(&claim, generation, &[downgrade])
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let export = store.export_session(&operator, &id).await.unwrap();
    assert_eq!(
        serde_json::to_value(export.data.decisions[0].trace.chain_freshness.clone()).unwrap(),
        serde_json::to_value(fresh.chain_freshness).unwrap()
    );
    let retained = store
        .create_cost_assessment(
            &operator,
            &id,
            "fresh-cost",
            arb_storage::NewCostAssessment {
                observation_id: fresh.observation_id.clone(),
                scenario: manual_scenario(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        retained.assessment.binding.observation_id,
        fresh.observation_id
    );
}

#[tokio::test]
async fn self_consistent_policy_tamper_is_rejected_by_reads_costs_and_frozen_exports() {
    use sha2::{Digest, Sha256};
    let policy = json!({"version":"finalized-chain-time-v1","max_chain_age_ms":100});
    let (store, operator, id, _claim, generation) =
        setup_mode_policy("OBSERVE", Some(policy)).await;
    let changed = freshness_trace(trace(&id, generation, 1001, "QUOTED"), 200);
    changed.validate().unwrap(); // Arithmetic and observation hash alone are valid.
    let payload = serde_json::to_value(&changed).unwrap();
    let digest = hex::encode(Sha256::digest(serde_json::to_vec(&changed).unwrap()));
    let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    sqlx::query("INSERT INTO decision_traces(trace_id,operator_id,session_id,observation_id,payload_digest,configuration_digest,generation,observed_at_unix_ms,result_status,grouping_version,grouping_key,window_start_ms,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
        .bind(Uuid::new_v4().to_string()).bind(&operator).bind(&id).bind(&changed.observation_id).bind(digest).bind(&changed.configuration_digest)
        .bind(generation as i64).bind(changed.observed_at_unix_ms as i64).bind(changed.result.status())
        .bind(&changed.grouping.version).bind(&changed.grouping.key).bind(changed.grouping.window_start_ms as i64).bind(payload).execute(&pool).await.unwrap();
    assert!(matches!(
        store
            .get_decision_trace(&operator, &changed.observation_id)
            .await,
        Err(StoreError::CorruptState)
    ));
    assert!(matches!(
        store.list_decision_traces(&operator, &id, None, 100).await,
        Err(StoreError::CorruptState)
    ));
    assert!(matches!(
        store.export_session(&operator, &id).await,
        Err(StoreError::CorruptState)
    ));
    assert!(matches!(
        store
            .create_cost_assessment(
                &operator,
                &id,
                "tampered-cost",
                arb_storage::NewCostAssessment {
                    observation_id: changed.observation_id,
                    scenario: manual_scenario(),
                }
            )
            .await,
        Err(StoreError::CorruptState)
    ));
    assert!(
        store
            .list_cost_assessments(&operator, &id, None, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn malformed_present_snapshot_policy_never_becomes_an_absent_legacy_policy() {
    for policy in [
        json!({"version":"unsupported","max_chain_age_ms":100}),
        json!({"version":"finalized-chain-time-v1","max_chain_age_ms":0}),
        json!("invalid-policy"),
    ] {
        let (store, _, id, claim, generation) = setup_mode_policy("OBSERVE", Some(policy)).await;
        assert!(matches!(
            store
                .append_decision_traces(
                    &claim,
                    generation,
                    &[trace(&id, generation, 1001, "QUOTED")]
                )
                .await,
            Err(StoreError::InvalidInput(_))
        ));
    }
}

/// One operator, one shared configuration digest, two sessions on different networks
/// (base-mainnet, solana-mainnet), each with its own `networks.<id>.chain_freshness`
/// policy embedded in the single configuration snapshot.
async fn setup_two_networks(
    policy_base: Option<Value>,
    policy_solana: Option<Value>,
) -> (
    Store,
    String,
    (String, WorkerClaim, u64),
    (String, WorkerClaim, u64),
) {
    let store = Store::connect(
        &std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is mandatory; decision DB tests must not skip"),
    )
    .await
    .unwrap();
    store.migrate().await.unwrap();
    let operator = format!("decision-cross-chain-{}", Uuid::new_v4());
    store
        .save_configuration(
            &operator,
            &hash("a"),
            json!({"mode":"OBSERVE","networks":{"base":{"chain_freshness":policy_base},"solana":{"chain_freshness":policy_solana}}}),
        )
        .await
        .unwrap();
    let mut sessions = Vec::new();
    for network in ["base-mainnet", "solana-mainnet"] {
        let session = store
            .create_session(
                &operator,
                &format!("session-{network}"),
                NewSession {
                    network_id: network.into(),
                    mode: "OBSERVE".into(),
                    configuration_digest: hash("a"),
                    experiment_id: "decision-fixture".into(),
                    strategy_ids: vec!["fixture-strategy".into()],
                },
            )
            .await
            .unwrap();
        let claim = store
            .claim_worker(&operator, &session.session_id, network, "worker", 60)
            .await
            .unwrap();
        store.complete_worker_recovery(&claim).await.unwrap();
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
        let generation = store
            .apply_pending(&claim, true, |_| Ok(()))
            .await
            .unwrap()
            .generation;
        for (id, digest) in [("capture-one", hash("1")), ("capture-two", hash("2"))] {
            store
                .record_capture_admission(
                    &claim,
                    generation,
                    id,
                    &digest,
                    &format!("/synthetic/{id}"),
                )
                .await
                .unwrap();
        }
        sessions.push((session.session_id, claim, generation));
    }
    let solana = sessions.pop().unwrap();
    let base = sessions.pop().unwrap();
    (store, operator, base, solana)
}

fn solana_trace(session: &str, generation: u64, time: u64) -> DecisionTrace {
    let value = json!({
     "schema_version":"1.0.0","observation_id":"","session_id":session,"experiment_id":"decision-fixture","generation":generation.to_string(),"configuration_digest":hash("a"),"calculation_version":"fixture-math-v1","strategy_id":"fixture-strategy","network_id":"solana-mainnet","mode":"OBSERVE","source_kind":"SYNTHETIC_FIXTURE","dataset_origin":"MANUALLY_CONSTRUCTED","observed_at_unix_ms":time,"input_age_ms":0,
     "capture_refs":[{"capture_id":"capture-one","manifest_digest":hash("1"),"snapshot_id":hash("1")},{"capture_id":"capture-two","manifest_digest":hash("2"),"snapshot_id":hash("2")}],
     "route":[{"pool_id":"solana-mainnet:9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM","asset_in":"solana-mainnet:So11111111111111111111111111111111111111112","asset_out":"solana-mainnet:EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","venue_family":"orca-whirlpools"},{"pool_id":"solana-mainnet:Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB","asset_in":"solana-mainnet:EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","asset_out":"solana-mainnet:So11111111111111111111111111111111111111112","venue_family":"orca-whirlpools"}],
     "amount_in_minor":"100","result":{"status":"QUOTED","quoted_output_minor":"110","gross_delta_minor":"10","included_pool_fees":["1","1"]},
     "grouping":{"version":"","key":"","window_ms":1000,"window_start_ms":0},"diagnostics":[]
    });
    serde_json::from_value::<DecisionTrace>(value)
        .unwrap()
        .seal()
        .unwrap()
}
fn solana_freshness_trace(mut source: DecisionTrace, max_chain_age_ms: u64) -> DecisionTrace {
    let policy = arb_domain::ChainFreshnessPolicy {
        version: arb_domain::CHAIN_FRESHNESS_VERSION.into(),
        max_chain_age_ms,
    };
    let inputs = source
        .capture_refs
        .iter()
        .map(|capture| arb_domain::ChainTimeInput {
            capture_id: capture.capture_id.clone(),
            source: arb_domain::ChainTimeSource::SolanaEstimatedBlockTime {
                slot: "1".into(),
                genesis_hash: "So11111111111111111111111111111111111111112".into(),
                account_context: "fixture-context".into(),
            },
            chain_time_seconds: Some(1),
        })
        .collect();
    source.chain_freshness = Some(
        arb_domain::ChainFreshnessReport::assess(
            policy,
            source.observed_at_unix_ms,
            source.input_age_ms.unwrap(),
            inputs,
        )
        .unwrap(),
    );
    source.schema_version = arb_domain::FRESHNESS_DECISION_SCHEMA_VERSION.into();
    source.calculation_version = arb_domain::FRESHNESS_CALCULATION_VERSION.into();
    source.seal().unwrap()
}

/// ARB-018 criterion: "No cross-chain shared atomic snapshot is implied by similar
/// wall-clock times." One operator and one configuration snapshot carry independent
/// Base and Solana chain-freshness policies; two sessions append traces at the exact
/// same wall-clock millisecond. Each network must accept only its own policy and
/// reject the other network's, and each session's coverage/opportunity views must
/// stay scoped to its own trace.
#[tokio::test]
async fn base_and_solana_sessions_never_share_or_conflate_chain_freshness_policy() {
    let base_policy = json!({"version":"finalized-chain-time-v1","max_chain_age_ms":100});
    let solana_policy = json!({"version":"finalized-chain-time-v1","max_chain_age_ms":500});
    let (store, operator, (base_id, base_claim, base_gen), (solana_id, solana_claim, solana_gen)) =
        setup_two_networks(Some(base_policy), Some(solana_policy)).await;

    let same_wall_clock_ms = 1001;
    let base_ok = freshness_trace(trace(&base_id, base_gen, same_wall_clock_ms, "QUOTED"), 100);
    store
        .append_decision_traces(&base_claim, base_gen, std::slice::from_ref(&base_ok))
        .await
        .unwrap();
    let solana_ok = solana_freshness_trace(
        solana_trace(&solana_id, solana_gen, same_wall_clock_ms),
        500,
    );
    store
        .append_decision_traces(&solana_claim, solana_gen, std::slice::from_ref(&solana_ok))
        .await
        .unwrap();

    // A trace correctly self-consistent under the OTHER network's policy value must be
    // rejected: the storage layer must read each session's own `networks.<id>` key,
    // never the sibling network's, even though both share operator/configuration/time.
    let base_tagged_with_solanas_policy =
        freshness_trace(trace(&base_id, base_gen, same_wall_clock_ms, "QUOTED"), 500);
    assert!(matches!(
        store
            .append_decision_traces(
                &base_claim,
                base_gen,
                std::slice::from_ref(&base_tagged_with_solanas_policy)
            )
            .await,
        Err(StoreError::InvalidInput(_))
    ));
    let solana_tagged_with_bases_policy = solana_freshness_trace(
        solana_trace(&solana_id, solana_gen, same_wall_clock_ms),
        100,
    );
    assert!(matches!(
        store
            .append_decision_traces(
                &solana_claim,
                solana_gen,
                std::slice::from_ref(&solana_tagged_with_bases_policy)
            )
            .await,
        Err(StoreError::InvalidInput(_))
    ));

    // Read paths stay scoped per session/network too: neither coverage nor the
    // opportunity projection leaks the sibling chain's accepted trace.
    let base_coverage = store.decision_coverage(&operator, &base_id).await.unwrap();
    assert_eq!(base_coverage.raw_observations, "1");
    let solana_coverage = store
        .decision_coverage(&operator, &solana_id)
        .await
        .unwrap();
    assert_eq!(solana_coverage.raw_observations, "1");

    let base_opportunities = store
        .list_opportunities(
            &operator,
            OpportunityFilter {
                session_id: Some(base_id.clone()),
                ..Default::default()
            },
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(base_opportunities.items.len(), 1);
    assert_eq!(
        base_opportunities.items[0].network_id,
        NetworkId::BaseMainnet
    );
    let solana_opportunities = store
        .list_opportunities(
            &operator,
            OpportunityFilter {
                session_id: Some(solana_id.clone()),
                ..Default::default()
            },
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(solana_opportunities.items.len(), 1);
    assert_eq!(
        solana_opportunities.items[0].network_id,
        NetworkId::SolanaMainnet
    );
}

#[path = "support/capture_source_cases.rs"]
mod capture_source_cases;
