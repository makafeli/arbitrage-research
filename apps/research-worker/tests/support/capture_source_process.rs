//! Actual research-worker process, PostgreSQL and the existing synthetic HTTP provider.
use super::*;
use arb_storage::{IngestionBinding, IngestionHead, IngestionHalt};
use sqlx::PgPool;

#[tokio::test]
async fn configured_worker_binds_actual_capture_batch_and_stops_publication_after_source_halt() {
    source_process(true).await;
}

#[tokio::test]
async fn same_height_with_another_hash_never_gets_automatic_capture_associations() {
    source_process(false).await;
}

async fn source_process(matching: bool) {
    let database=std::env::var("TEST_DATABASE_URL").expect("PostgreSQL process test must run");
    let pool=PgPool::connect(&database).await.unwrap();
    let store=Store::from_pool(pool.clone());
    store.migrate().await.unwrap();
    let root=std::env::temp_dir().join(format!("arb-source-process-{}",Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let capture_root=root.join("captures");
    fs::create_dir(&capture_root).unwrap();
    let (registry,transcripts)=two_pool_fixture();
    let config=two_pool_config(capture_root.to_str().unwrap(),&registry);
    let validated=ValidatedConfig::from_toml(&config).unwrap();
    let document=arb_registry::RegistryDocument::from_bytes(&registry,arb_domain::NetworkId::BaseMainnet).unwrap();
    let pools:Vec<_>=document.pools().iter().map(|p|match p {arb_registry::PoolRegistry::Base(p)=>p.clone(),_=>panic!("Base fixture required")}).collect();
    let mut transcript=TranscriptRpc::new(transcripts[0].clone());
    let snapshot=arb_evm::capture_pool(&mut transcript,&pools[0],100).unwrap();
    transcript.finish().unwrap();
    let arb_adapter_api::StateContext::Evm {block_number,block_hash,parent_hash,block_timestamp_seconds,..}=snapshot.context else {panic!("Base context required")};
    let checkpoint=IngestionHead {
        number:block_number,
        hash:if matching {block_hash.to_lowercase()} else {format!("0x{}","dd".repeat(32))},
        parent_hash:parent_hash.to_lowercase(),timestamp_seconds:block_timestamp_seconds,
    };
    let seed=IngestionHead {number:block_number-1,hash:checkpoint.parent_hash.clone(),parent_hash:format!("0x{}","ee".repeat(32)),timestamp_seconds:block_timestamp_seconds.saturating_sub(1)};
    let mut addresses:Vec<_>=pools.iter().map(|p|p.pool.to_lowercase()).collect();
    addresses.sort();
    let binding=IngestionBinding {schema_version:1,network_id:"base-mainnet".into(),registry_digest:digest(&serde_json::to_vec(&pools).unwrap()),abi_source_commit:arb_evm::SOURCE_COMMIT.into(),dataset_origin:"MANUALLY_CONSTRUCTED".into(),pool_addresses:addresses};
    let operator=format!("source-process-{}",Uuid::new_v4());
    let initial=store.create_ingestion(&operator,"source",&binding,&seed).await.unwrap();
    let cursor=store.commit_ingestion(&operator,"source",&initial,json!({
        "schema_version":1,"network_id":binding.network_id,"registry_digest":binding.registry_digest,
        "abi_source_commit":binding.abi_source_commit,"pool_addresses":binding.pool_addresses,
        "from_checkpoint":seed,"through":checkpoint,"blocks":[{"header":checkpoint,"logs":[]}],"full_snapshot_required":true
    })).await.unwrap();
    store.save_configuration(&operator,validated.digest(),serde_json::from_str(validated.effective_json()).unwrap()).await.unwrap();
    let id=store.create_session(&operator,"source-process",NewSession {network_id:"base-mainnet".into(),mode:"OBSERVE".into(),configuration_digest:validated.digest().into(),experiment_id:"synthetic-source-process".into(),strategy_ids:validated.strategy_ids().to_vec()}).await.unwrap().session_id;
    fs::write(root.join("config.toml"),&config).unwrap();
    fs::write(root.join("registry.json"),registry).unwrap();
    let mut provider=PairRpcGuard::start(transcripts);
    let output=fs::File::create(root.join("worker.log")).unwrap();
    let mut child=ChildGuard(Command::new(env!("CARGO_BIN_EXE_research-worker"))
        .env("ARB_WORKER_CONFIG",root.join("config.toml"))
        .env("ARB_POOL_REGISTRY",root.join("registry.json"))
        .env("ARB_OPERATOR_ID",&operator).env("ARB_SESSION_ID",&id)
        .env("TEST_WORKER_RPC",&provider.endpoint).env("TEST_DATABASE_URL",&database)
        .env("ARB_BASE_INGESTION_STREAM","source")
        .stdout(Stdio::from(output.try_clone().unwrap())).stderr(Stdio::from(output))
        .spawn().unwrap());
    wait_until(async || {
        sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM collection_attempts WHERE operator_id=$1 AND session_id=$2 AND outcome='READINESS_COMPLETED')")
            .bind(&operator).bind(&id).fetch_one(&pool).await.unwrap()
    },30).await;
    store.issue_command(&operator,&id,"start",NewCommand {action:"START".into(),expected_revision:"0".into(),reason:None}).await.unwrap();
    if matching {
        wait_until(async || store.decision_coverage(&operator,&id).await.unwrap().quoted_candidates.parse::<u64>().unwrap()>=2,30).await;
        let tracked:i64=sqlx::query_scalar("SELECT count(*) FROM decision_ingestion_validity WHERE operator_id=$1 AND session_id=$2 AND continuity_status='NO_KNOWN_INVALIDATION'")
            .bind(&operator).bind(&id).fetch_one(&pool).await.unwrap();
        assert!(tracked>=2);
        store.halt_ingestion(&operator,"source",&cursor,IngestionHalt::ContinuityLost).await.unwrap();
    }
    let deadline=Instant::now()+Duration::from_secs(30);
    let status=loop {
        if let Some(status)=child.0.try_wait().unwrap() {break status;}
        assert!(Instant::now()<deadline,"worker did not stop after invalid source: {}",fs::read_to_string(root.join("worker.log")).unwrap());
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(!status.success());
    let failures:i64=sqlx::query_scalar("SELECT count(*) FROM collection_attempts WHERE operator_id=$1 AND session_id=$2 AND outcome='ACQUISITION_FAILED' AND reason='INPUT_VALIDATION_FAILED'")
        .bind(&operator).bind(&id).fetch_one(&pool).await.unwrap();
    assert!(failures>=1,"missing typed failure: {}",fs::read_to_string(root.join("worker.log")).unwrap());
    if matching {
        let invalidated:i64=sqlx::query_scalar("SELECT count(*) FROM decision_ingestion_validity WHERE operator_id=$1 AND session_id=$2 AND continuity_status='INVALIDATED'")
            .bind(&operator).bind(&id).fetch_one(&pool).await.unwrap();
        assert!(invalidated>=2);
        let late:i64=sqlx::query_scalar("SELECT count(*) FROM decision_traces d JOIN ingestion_invalidations i ON i.operator_id=d.operator_id WHERE d.operator_id=$1 AND d.session_id=$2 AND d.result_status='QUOTED' AND d.recorded_at>i.detected_at")
            .bind(&operator).bind(&id).fetch_one(&pool).await.unwrap();
        assert_eq!(late,0);
    } else {
        assert_eq!(store.decision_coverage(&operator,&id).await.unwrap().raw_observations,"0");
        let links:i64=sqlx::query_scalar("SELECT count(*) FROM capture_ingestion_dependencies WHERE operator_id=$1 AND session_id=$2")
            .bind(&operator).bind(&id).fetch_one(&pool).await.unwrap();
        assert_eq!(links,0);
    }
    provider.finish();
    drop(child);
    fs::remove_dir_all(root).unwrap();
}
