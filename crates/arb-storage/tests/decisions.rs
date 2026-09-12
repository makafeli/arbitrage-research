use arb_domain::{DecisionTrace,Evidence,SourceKind};
use arb_storage::{NewCommand,NewSession,OpportunityFilter,Store,StoreError,WorkerClaim};
use serde_json::{Value,json};
use uuid::Uuid;
fn hash(c:&str)->String{format!("sha256:{}",c.repeat(64))}
async fn setup()->(Store,String,String,WorkerClaim,u64){
 let store=Store::connect(&std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is mandatory; decision DB tests must not skip")).await.unwrap();store.migrate().await.unwrap();
 let operator=format!("decision-{}",Uuid::new_v4());store.save_configuration(&operator,&hash("a"),json!({"mode":"OBSERVE"})).await.unwrap();
 let session=store.create_session(&operator,"session",NewSession{network_id:"base-mainnet".into(),mode:"OBSERVE".into(),configuration_digest:hash("a"),experiment_id:"decision-fixture".into(),strategy_ids:vec!["fixture-strategy".into()]}).await.unwrap();
 let claim=store.claim_worker(&operator,&session.session_id,"base-mainnet","worker",60).await.unwrap();store.complete_worker_recovery(&claim).await.unwrap();
 store.issue_command(&operator,&session.session_id,"start",NewCommand{action:"START".into(),expected_revision:"0".into(),reason:None}).await.unwrap();let generation=store.apply_pending(&claim,true,|_|Ok(())).await.unwrap().generation;
 for (id,digest) in [("capture-one",hash("1")),("capture-two",hash("2"))]{store.record_capture_admission(&claim,generation,id,&digest,&format!("/synthetic/{id}")).await.unwrap();}
 (store,operator,session.session_id,claim,generation)
}
fn trace(session:&str,generation:u64,time:u64,status:&str)->DecisionTrace{
 let mut value=json!({
  "schema_version":"1.0.0","observation_id":"","session_id":session,"experiment_id":"decision-fixture","generation":generation.to_string(),"configuration_digest":hash("a"),"calculation_version":"fixture-math-v1","strategy_id":"fixture-strategy","network_id":"base-mainnet","mode":"OBSERVE","source_kind":"SYNTHETIC_FIXTURE","dataset_origin":"MANUALLY_CONSTRUCTED","observed_at_unix_ms":time,"input_age_ms":0,
  "capture_refs":[{"capture_id":"capture-one","manifest_digest":hash("1"),"snapshot_id":hash("1")},{"capture_id":"capture-two","manifest_digest":hash("2"),"snapshot_id":hash("2")}],
  "route":[{"pool_id":"base-mainnet:0x0000000000000000000000000000000000000003","asset_in":"base-mainnet:0x0000000000000000000000000000000000000001","asset_out":"base-mainnet:0x0000000000000000000000000000000000000002","venue_family":"uniswap-v3"},{"pool_id":"base-mainnet:0x0000000000000000000000000000000000000004","asset_in":"base-mainnet:0x0000000000000000000000000000000000000002","asset_out":"base-mainnet:0x0000000000000000000000000000000000000001","venue_family":"uniswap-v3"}],
  "amount_in_minor":"100","result":{"status":"QUOTED","quoted_output_minor":"110","gross_delta_minor":"10","included_pool_fees":["1","1"]},
  "grouping":{"version":"","key":"","window_ms":1000,"window_start_ms":0},"diagnostics":[]
 });
 match status{
  "REJECTED"=>value["result"]=json!({"status":"REJECTED","reason_codes":["MATH_INPUT_REJECTED"]}),
  "NO_ROUTE"=>{value["result"]=json!({"status":"NO_ROUTE","reason_codes":["NO_ELIGIBLE_POOL_PAIRS"]});value["route"]=json!([]);value["amount_in_minor"]=Value::Null;value["input_age_ms"]=Value::Null;value["capture_refs"].as_array_mut().unwrap().pop();},
  "DATA_UNAVAILABLE"=>{value["result"]=json!({"status":"DATA_UNAVAILABLE","reason_codes":["NO_CAPTURE_INPUTS"]});value["route"]=json!([]);value["capture_refs"]=json!([]);value["amount_in_minor"]=Value::Null;value["input_age_ms"]=Value::Null;},
  "NEGATIVE"=>value["result"]=json!({"status":"QUOTED","quoted_output_minor":"90","gross_delta_minor":"-10","included_pool_fees":["1","1"]}),
  _=>{}
 }
 serde_json::from_value::<DecisionTrace>(value).unwrap().seal().unwrap()
}
#[tokio::test]
async fn grouping_preserves_denominators_and_unknown_coverage(){
 let(store,operator,id,claim,generation)=setup().await;
 let traces=vec![trace(&id,generation,1001,"QUOTED"),trace(&id,generation,1002,"NEGATIVE"),trace(&id,generation,1003,"REJECTED"),trace(&id,generation,1004,"NO_ROUTE"),trace(&id,generation,1005,"DATA_UNAVAILABLE")];
 let first=store.append_decision_traces(&claim,generation,&traces).await.unwrap();
 let repeat=store.append_decision_traces(&claim,generation,&traces).await.unwrap();assert_eq!(first[0].trace_id,repeat[0].trace_id);
 let coverage=store.decision_coverage(&operator,&id).await.unwrap();assert_eq!(coverage.raw_observations,"5");assert_eq!(coverage.quoted_candidates,"2");assert_eq!(coverage.rejected,"1");assert_eq!(coverage.no_route,"1");assert_eq!(coverage.data_unavailable,"1");assert_eq!(coverage.unique_opportunity_groups,"1");assert_eq!(coverage.collection_completeness,"UNKNOWN");assert!(coverage.eligible_attempts.is_none());assert!(!coverage.execution_accounting_available);
 let groups=store.list_decision_groups(&operator,&id,None,100).await.unwrap();assert_eq!(groups.items.len(),3);assert!(groups.items.iter().any(|g|g.raw_observations=="3"&&g.quoted_candidates=="2"&&g.rejected=="1"));
 let next_window=trace(&id,generation,2001,"QUOTED");store.append_decision_traces(&claim,generation,&[next_window]).await.unwrap();assert_eq!(store.decision_coverage(&operator,&id).await.unwrap().unique_opportunity_groups,"2");
}
#[tokio::test]
async fn opportunity_filtering_happens_before_pagination_and_quotes_never_become_simulations(){
 let(store,operator,id,claim,generation)=setup().await;
 let traces=vec![trace(&id,generation,1001,"REJECTED"),trace(&id,generation,1002,"QUOTED"),trace(&id,generation,1003,"NO_ROUTE"),trace(&id,generation,1004,"NEGATIVE")];store.append_decision_traces(&claim,generation,&traces).await.unwrap();
 let filter=OpportunityFilter{session_id:Some(id.clone()),source_kind:Some(SourceKind::SyntheticFixture),..Default::default()};
 let first=store.list_opportunities(&operator,filter.clone(),None,1).await.unwrap();assert_eq!(first.items.len(),1);assert_eq!(first.items[0].evidence_label,Evidence::Candidate);assert!(first.items[0].net_after_explicit_costs_minor.is_none());assert!(first.next_cursor.is_some());
 let next=store.list_opportunities(&operator,filter,first.next_cursor.as_deref(),1).await.unwrap();assert_eq!(next.items.len(),1);assert!(next.next_cursor.is_none());assert_ne!(first.items[0].opportunity_id,next.items[0].opportunity_id);
 assert!(store.list_opportunities(&operator,OpportunityFilter{session_id:Some(id.clone()),source_kind:Some(SourceKind::CapturedMarketData),..Default::default()},None,10).await.unwrap().items.is_empty());
 assert!(store.list_opportunities(&operator,OpportunityFilter{session_id:Some(id),evidence_label:Some(Evidence::Simulated),..Default::default()},None,10).await.unwrap().items.is_empty());
}
#[tokio::test]
async fn changed_identity_missing_capture_and_failed_batch_are_not_partially_saved(){
 let(store,operator,id,claim,generation)=setup().await;let first=trace(&id,generation,1001,"QUOTED");store.append_decision_traces(&claim,generation,std::slice::from_ref(&first)).await.unwrap();
 let mut changed=first.clone();changed.observed_at_unix_ms+=1;assert!(matches!(store.append_decision_traces(&claim,generation,&[changed]).await,Err(StoreError::Conflict(_))));
 let fresh=trace(&id,generation,1003,"QUOTED");let mut missing=trace(&id,generation,1004,"QUOTED");missing.capture_refs[0].capture_id="missing".into();missing=missing.seal().unwrap();
 assert!(matches!(store.append_decision_traces(&claim,generation,&[fresh,missing]).await,Err(StoreError::Conflict(_))));
 assert_eq!(store.decision_coverage(&operator,&id).await.unwrap().raw_observations,"1");
 assert!(matches!(store.get_decision_trace("another",&first.observation_id).await,Err(StoreError::NotFound)));
 assert!(store.append_decision_traces(&claim,generation,&vec![first;65]).await.is_err());
}
#[tokio::test]
async fn late_generation_and_mutating_raw_history_are_rejected(){
 let(store,operator,id,claim,generation)=setup().await;let first=trace(&id,generation,1001,"QUOTED");let saved=store.append_decision_traces(&claim,generation,std::slice::from_ref(&first)).await.unwrap();
 store.issue_command(&operator,&id,"stop",NewCommand{action:"STOP".into(),expected_revision:"1".into(),reason:None}).await.unwrap();store.apply_pending(&claim,false,|_|Ok(())).await.unwrap();
 assert!(matches!(store.append_decision_traces(&claim,generation,&[trace(&id,generation,1002,"QUOTED")]).await,Err(StoreError::Conflict(_))));
 let reopened=Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap()).await.unwrap();assert_eq!(reopened.get_decision_trace(&operator,&first.observation_id).await.unwrap().trace_id,saved[0].trace_id);
 let pool=sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap()).await.unwrap();assert!(sqlx::query("DELETE FROM decision_traces WHERE session_id=$1").bind(&id).execute(&pool).await.is_err());
}
