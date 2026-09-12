use arb_domain::{AssetId,NetworkId};
use arb_paper::{AccountingAsset,InitialBalance,PaperCommand,PaperOutcome,PaperRun,ReservationRequest,ReservationState};
use arb_storage::{NewCommand,NewPaperRun,NewSession,Store,StoreError,WorkerClaim};
use serde_json::json;
use uuid::Uuid;
fn asset()->AssetId{"base-mainnet:0x0000000000000000000000000000000000000001".parse().unwrap()}
fn initial()->NewPaperRun{NewPaperRun{initial_balances:vec![InitialBalance{asset:AccountingAsset::Token(asset()),amount:100.into()},InitialBalance{asset:AccountingAsset::Native(NetworkId::BaseMainnet),amount:10.into()}]}}
fn reserve(attempt:&str,principal:u64,fee:u64)->PaperCommand{PaperCommand::Reserve{request:ReservationRequest{attempt_id:attempt.into(),principal_asset:asset(),principal:principal.into(),native_fee_budget:fee.into()}}}
async fn setup()->(Store,String,String,WorkerClaim){
 let database=std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required; PostgreSQL tests must never skip");
 let store=Store::connect(&database).await.unwrap();store.migrate().await.unwrap();let operator=format!("paper-{}",Uuid::new_v4());
 let config=format!("sha256:{}","a".repeat(64));
 store.save_configuration(&operator,&config,json!({"deployment":{"mode":"PAPER"},"networks":{"base":{"enabled":true,"verified_asset_ids":[asset().to_string()]}}})).await.unwrap();
 let session=store.create_session(&operator,"session",NewSession{network_id:"base-mainnet".into(),mode:"PAPER".into(),configuration_digest:config,experiment_id:"paper-fixture".into(),strategy_ids:vec!["fixture-strategy".into()]}).await.unwrap();
 let claim=store.claim_worker(&operator,&session.session_id,"base-mainnet","worker",60).await.unwrap();store.complete_worker_recovery(&claim).await.unwrap();
 (store,operator,session.session_id,claim)
}
async fn start(store:&Store,operator:&str,id:&str,claim:&WorkerClaim)->u64{
 store.issue_command(operator,id,"start",NewCommand{action:"START".into(),expected_revision:"0".into(),reason:None}).await.unwrap();
 store.apply_pending(claim,true,|_|Ok(())).await.unwrap().generation
}
#[tokio::test]
async fn concurrent_run_creation_retry_and_reset_preserve_initial_history(){
 let(store,operator,id,_)=setup().await;
 let(a,b)=tokio::join!(store.create_paper_run(&operator,&id,"same",initial()),store.create_paper_run(&operator,&id,"same",initial()));
 let run=a.unwrap();assert_eq!(run.run_id,b.unwrap().run_id);assert_eq!(run.revision,"0");
 let mut changed=initial();changed.initial_balances[0].amount=101.into();
 assert!(matches!(store.create_paper_run(&operator,&id,"same",changed).await,Err(StoreError::Conflict(_))));
 let reset=store.create_paper_run(&operator,&id,"new",initial()).await.unwrap();assert_ne!(run.run_id,reset.run_id);
 assert_eq!(store.list_paper_runs(&operator,&id,None,10).await.unwrap().items.len(),2);
 assert_eq!(store.replay_paper_run_creation(&operator,&id,"same",&initial()).await.unwrap().unwrap().run_id,run.run_id);
 assert!(matches!(store.get_paper_run("another",&run.run_id).await,Err(StoreError::NotFound)));
}
#[tokio::test]
async fn competing_reservations_cannot_reuse_principal_or_native_fee(){
 let(store,operator,id,claim)=setup().await;let run=store.create_paper_run(&operator,&id,"run",initial()).await.unwrap();let generation=start(&store,&operator,&id,&claim).await;
 let(a,b)=tokio::join!(store.apply_paper_command(&claim,generation,&run.run_id,"first",reserve("first",80,8)),store.apply_paper_command(&claim,generation,&run.run_id,"second",reserve("second",80,8)));
 assert_ne!(a.is_ok(),b.is_ok());
 let after=store.get_paper_run(&operator,&run.run_id).await.unwrap();assert_eq!(after.revision,"1");assert_eq!(after.outstanding_reservations,1);
 let principal=after.balances.iter().find(|b|b.asset==AccountingAsset::Token(asset())).unwrap();assert_eq!(principal.free.to_string(),"20");assert_eq!(principal.reserved.to_string(),"80");assert_eq!(principal.total.to_string(),"100");
 let native=after.balances.iter().find(|b|b.asset==AccountingAsset::Native(NetworkId::BaseMainnet)).unwrap();assert_eq!(native.free.to_string(),"2");assert_eq!(native.reserved.to_string(),"8");
}
#[tokio::test]
async fn fee_inventory_alone_does_not_satisfy_principal_and_failed_command_is_atomic(){
 let(store,operator,id,claim)=setup().await;let run=store.create_paper_run(&operator,&id,"run",NewPaperRun{initial_balances:vec![InitialBalance{asset:AccountingAsset::Native(NetworkId::BaseMainnet),amount:100.into()}]}).await.unwrap();let generation=start(&store,&operator,&id,&claim).await;
 assert!(store.apply_paper_command(&claim,generation,&run.run_id,"reserve",reserve("attempt",1,1)).await.is_err());
 let after=store.get_paper_run(&operator,&run.run_id).await.unwrap();assert_eq!(after.revision,"0");assert_eq!(after.outstanding_reservations,0);assert_eq!(after.balances[0].free.to_string(),"100");
}
#[tokio::test]
async fn unknown_survives_reconnect_and_idempotent_resolution_replays_exact_balances(){
 let(store,operator,id,claim)=setup().await;let run=store.create_paper_run(&operator,&id,"run",initial()).await.unwrap();let generation=start(&store,&operator,&id,&claim).await;
 let reserved=store.apply_paper_command(&claim,generation,&run.run_id,"reserve",reserve("attempt",90,8)).await.unwrap();
 let duplicate=store.apply_paper_command(&claim,generation,&run.run_id,"reserve",reserve("attempt",90,8)).await.unwrap();assert_eq!(reserved.event_id,duplicate.event_id);
 assert!(matches!(store.apply_paper_command(&claim,generation,&run.run_id,"reserve",reserve("attempt",91,8)).await,Err(StoreError::Conflict(_))));
 store.apply_paper_command(&claim,generation,&run.run_id,"unknown",PaperCommand::MarkUnknown{attempt_id:"attempt".into(),reason:"EXPLICIT_SCENARIO_UNKNOWN".into()}).await.unwrap();
 let reopened=Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap()).await.unwrap();
 assert_eq!(reopened.list_paper_reservations(&operator,&run.run_id,None,10).await.unwrap().items[0].state,ReservationState::Unknown);
 assert_eq!(reopened.get_paper_run(&operator,&run.run_id).await.unwrap().outstanding_reservations,1);
 let outcome=PaperCommand::Resolve{attempt_id:"attempt".into(),outcome:PaperOutcome::Succeeded{amount_out:95.into(),actual_native_fee:3.into()}};
 let resolved=reopened.apply_paper_command(&claim,generation,&run.run_id,"resolve",outcome.clone()).await.unwrap();
 assert_eq!(reopened.apply_paper_command(&claim,generation,&run.run_id,"resolve",outcome).await.unwrap().event_id,resolved.event_id);
 let after=reopened.get_paper_run(&operator,&run.run_id).await.unwrap();assert_eq!(after.outstanding_reservations,0);
 let token=after.balances.iter().find(|b|b.asset==AccountingAsset::Token(asset())).unwrap();assert_eq!(token.free.to_string(),"105");assert_eq!(token.reserved.to_string(),"0");
 let native=after.balances.iter().find(|b|b.asset==AccountingAsset::Native(NetworkId::BaseMainnet)).unwrap();assert_eq!(native.free.to_string(),"7");
 let mut events=Vec::new();let mut cursor=None;loop{let page=reopened.list_paper_journal(&operator,&run.run_id,cursor.as_deref(),2).await.unwrap();events.extend(page.items.into_iter().map(|r|r.event));cursor=page.next_cursor;if cursor.is_none(){break;}}
 assert_eq!(PaperRun::replay(&events).unwrap().balances().unwrap(),after.balances);
 assert_eq!(serde_json::to_value(&events[0]).unwrap()["sequence"],"0");
}
#[tokio::test]
async fn stopped_generation_blocks_reservation_but_positive_reconciliation_releases_budget(){
 let(store,operator,id,claim)=setup().await;let run=store.create_paper_run(&operator,&id,"run",initial()).await.unwrap();let generation=start(&store,&operator,&id,&claim).await;
 store.apply_paper_command(&claim,generation,&run.run_id,"reserve",reserve("attempt",80,8)).await.unwrap();
 store.issue_command(&operator,&id,"stop",NewCommand{action:"STOP".into(),expected_revision:"1".into(),reason:None}).await.unwrap();store.apply_pending(&claim,false,|_|Ok(())).await.unwrap();
 assert!(matches!(store.apply_paper_command(&claim,generation,&run.run_id,"late",reserve("late",1,1)).await,Err(StoreError::Conflict(_))));
 store.apply_paper_command(&claim,generation,&run.run_id,"release",PaperCommand::Resolve{attempt_id:"attempt".into(),outcome:PaperOutcome::NotIncluded{reason:"EXPLICIT_NO_INCLUSION_SCENARIO".into()}}).await.unwrap();
 let after=store.get_paper_run(&operator,&run.run_id).await.unwrap();assert_eq!(after.outstanding_reservations,0);assert_eq!(after.balances,run.balances);
}
#[tokio::test]
async fn sql_history_and_config_allowlists_are_enforced(){
 let(store,operator,id,_)=setup().await;
 let mut bad=initial();bad.initial_balances[0].asset=AccountingAsset::Token("base-mainnet:0x0000000000000000000000000000000000000002".parse().unwrap());
 assert!(matches!(store.create_paper_run(&operator,&id,"bad",bad).await,Err(StoreError::InvalidInput(_))));
 let run=store.create_paper_run(&operator,&id,"run",initial()).await.unwrap();let pool=sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap()).await.unwrap();
 assert!(sqlx::query("UPDATE paper_runs SET initial_balances='[]'::jsonb WHERE run_id=$1").bind(&run.run_id).execute(&pool).await.is_err());
 assert!(sqlx::query("DELETE FROM paper_journal WHERE run_id=$1").bind(&run.run_id).execute(&pool).await.is_err());
 assert!(store.list_paper_runs(&operator,&id,None,101).await.is_err());
}
#[tokio::test]
async fn included_failure_spends_only_fee_and_preserves_principal(){
 let(store,operator,id,claim)=setup().await;let run=store.create_paper_run(&operator,&id,"run",initial()).await.unwrap();let generation=start(&store,&operator,&id,&claim).await;
 store.apply_paper_command(&claim,generation,&run.run_id,"reserve",reserve("attempt",70,9)).await.unwrap();
 store.apply_paper_command(&claim,generation,&run.run_id,"failure",PaperCommand::Resolve{attempt_id:"attempt".into(),outcome:PaperOutcome::FailedIncluded{actual_native_fee:4.into()}}).await.unwrap();
 let after=store.get_paper_run(&operator,&run.run_id).await.unwrap();
 assert_eq!(after.balances.iter().find(|b|b.asset==AccountingAsset::Token(asset())).unwrap().free.to_string(),"100");
 assert_eq!(after.balances.iter().find(|b|b.asset==AccountingAsset::Native(NetworkId::BaseMainnet)).unwrap().free.to_string(),"6");
}
#[tokio::test]
async fn rejected_revision_gap_does_not_prevent_new_frozen_run_after_restart(){
 let(store,operator,id,_)=setup().await;
 store.issue_command(&operator,&id,"pending-start",NewCommand{action:"START".into(),expected_revision:"0".into(),reason:None}).await.unwrap();
 let pool=sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap()).await.unwrap();
 sqlx::query("UPDATE research_sessions SET lease_until=clock_timestamp()-interval '1 second' WHERE session_id=$1").bind(&id).execute(&pool).await.unwrap();
 let replacement=store.claim_worker(&operator,&id,"base-mainnet","replacement",60).await.unwrap();store.complete_worker_recovery(&replacement).await.unwrap();
 let session=store.get_session(&operator,&id).await.unwrap();assert_eq!(session.desired_revision,"1");assert_eq!(session.applied_revision,"0");
 assert!(store.create_paper_run(&operator,&id,"run",initial()).await.is_ok());
}
