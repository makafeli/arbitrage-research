//! Two actual research processes and independent bounded local RPC services.
//! Deliberately constructed inputs prove control isolation, not market coverage.
#[path = "support/isolation_provider.rs"]
mod isolation_provider;
use arb_capture::digest;
use arb_config::ValidatedConfig;
use arb_storage::{NewCommand, NewSession, Store};
use isolation_provider::{ASSERTION_WINDOW, ChildGuard, Handshake, IsolatedRpc};
use serde_json::{Value, json};
use std::{
    fs,
    process::{Command, Stdio},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use uuid::Uuid;

async fn wait_until(mut check: impl AsyncFnMut() -> bool, seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if check().await {
            return;
        }
        assert!(Instant::now() < deadline, "dual-worker condition timed out");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
use arb_domain::NetworkId;

/// Reuse the adapters' exact two-pool fixtures and common-anchor transcripts.
/// These remain explicitly synthetic; duplicating one pool would not be a route.
fn registry_bytes(network: NetworkId) -> Vec<u8> {
    let pools: Value = serde_json::from_str(match network {
        NetworkId::BaseMainnet => {
            include_str!("../../../crates/arb-evm/tests/fixtures/batch-registries.json")
        }
        NetworkId::SolanaMainnet => {
            include_str!("../../../crates/arb-solana/tests/fixtures/batch-registries.json")
        }
    })
    .unwrap();
    serde_json::to_vec(&json!({
        "schema_version": 1, "network_id": network.as_str(), "pools": pools,
    }))
    .unwrap()
}

fn configuration(network: NetworkId, root: &std::path::Path, registry: &[u8]) -> String {
    let document: Value = serde_json::from_slice(registry).unwrap();
    let pools = document["pools"].as_array().unwrap();
    assert_eq!(pools.len(), 2);
    let first = &pools[0];
    let mut input = include_str!("../../../config/research.example.toml")
        .replace("mode = \"PAPER\"", "mode = \"OBSERVE\"")
        .replace("trade_sizes_minor = []", "trade_sizes_minor = [\"10000\"]");
    let (key, end_marker, venue, asset_a, asset_b, identity) = match network {
        NetworkId::BaseMainnet => (
            "base",
            "[networks.solana]",
            "uniswap-v3",
            "token0",
            "token1",
            "expected_evm_chain_id = 8453".to_string(),
        ),
        NetworkId::SolanaMainnet => (
            "solana",
            "[resources]",
            "orca-whirlpools",
            "mint_a",
            "mint_b",
            format!(
                "expected_genesis_identity = {}",
                first["expected_genesis_hash"]
            ),
        ),
    };
    let start = input.find(&format!("[networks.{key}]")).unwrap();
    let end = input.find(end_marker).unwrap();
    let ids: Vec<_> = pools
        .iter()
        .map(|pool| {
            assert_eq!(pool[asset_a], first[asset_a]);
            assert_eq!(pool[asset_b], first[asset_b]);
            format!("{}:{}", network.as_str(), pool["pool"].as_str().unwrap())
        })
        .collect();
    assert_ne!(ids[0], ids[1]);
    let section = format!(
        "[networks.{key}]\nregistry_id = \"{id}\"\nenabled = true\ncandidate_venue = \"{venue}\"\n{identity}\nverified_pool_ids = {pools}\nverified_asset_ids = [\"{id}:{a}\",\"{id}:{b}\"]\nstarting_asset_id = \"{id}:{a}\"\nrpc_secret_reference = \"env:TEST_WORKER_RPC\"\nregistry_qualification_digest = \"{digest}\"\n\n",
        id = network.as_str(),
        pools = serde_json::to_string(&ids).unwrap(),
        a = first[asset_a].as_str().unwrap(),
        b = first[asset_b].as_str().unwrap(),
        digest = digest(registry),
    );
    input.replace_range(start..end, &section);
    input
        .replace(
            "database_secret_reference = \"UNCONFIGURED\"",
            "database_secret_reference = \"env:TEST_DATABASE_URL\"",
        )
        .replace(
            "capture_directory = \"./data/captures\"",
            &format!("capture_directory = \"{}\"", root.display()),
        )
}

#[test]
fn both_isolation_inputs_are_valid_but_explicitly_not_market_qualification() {
    for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
        let registry = registry_bytes(network);
        let config = ValidatedConfig::from_toml(&configuration(
            network,
            std::path::Path::new("./synthetic-captures"),
            &registry,
        ))
        .unwrap();
        let document = arb_registry::RegistryDocument::from_bytes(&registry, network).unwrap();
        document.authorize(&config).unwrap();
        assert_eq!(config.mode(), arb_domain::Mode::Observe);
        assert_eq!(document.pools().len(), 2);
        assert_eq!(config.verified_pools(network).len(), 2);
        assert_eq!(config.trade_sizes()[0].to_string(), "10000");
        let encoded: Value = serde_json::from_slice(&registry).unwrap();
        assert!(
            encoded["pools"].as_array().unwrap().iter().all(|p| {
                p["qualification_reference"] == "MANUALLY-CONSTRUCTED-NOT-A-DEPLOYMENT"
            })
        );
    }
}

#[test]
fn a_single_pool_allowlist_remains_invalid() {
    for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
        let registry = registry_bytes(network);
        let valid = configuration(
            network,
            std::path::Path::new("./synthetic-captures"),
            &registry,
        );
        let row = valid
            .lines()
            .find(|line| line.starts_with("verified_pool_ids = [\""))
            .unwrap();
        let pool_ids: Vec<String> = serde_json::from_str(row.split_once(" = ").unwrap().1).unwrap();
        let invalid = valid.replace(
            row,
            &format!("verified_pool_ids = {}", json!([pool_ids[0]])),
        );
        let error = ValidatedConfig::from_toml(&invalid).unwrap_err();
        assert!(
            error
                .reason
                .contains("at least two distinct qualified pools")
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn either_blocked_provider_leaves_the_other_worker_evaluating_and_both_controls_responsive() {
    let database = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL required for dual-process isolation");
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let observer = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database)
        .await
        .unwrap();
    let mut evidence = Vec::new();
    for blocked in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
        let root = std::env::temp_dir().join(format!("arb-dual-isolation-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let operator = format!("dual-isolation-{}", Uuid::new_v4());
        let handshake = Arc::new(Handshake::default());
        let mut providers = Vec::new();
        let mut workers = Vec::new();
        let mut sessions = Vec::new();
        for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
            let data = registry_bytes(network);
            let directory = root.join(network.as_str());
            fs::create_dir(&directory).unwrap();
            let captures = directory.join("captures");
            fs::create_dir(&captures).unwrap();
            let text = configuration(network, &captures, &data);
            let validated = ValidatedConfig::from_toml(&text).unwrap();
            let config = directory.join("config.toml");
            let registry = directory.join("registry.json");
            fs::write(&config, &text).unwrap();
            fs::write(&registry, &data).unwrap();
            store
                .save_configuration(
                    &operator,
                    validated.digest(),
                    serde_json::from_str(validated.effective_json()).unwrap(),
                )
                .await
                .unwrap();
            let session = store
                .create_session(
                    &operator,
                    network.as_str(),
                    NewSession {
                        network_id: network.as_str().into(),
                        mode: "OBSERVE".into(),
                        configuration_digest: validated.digest().into(),
                        experiment_id: "synthetic-dual-isolation".into(),
                        strategy_ids: validated.strategy_ids().to_vec(),
                    },
                )
                .await
                .unwrap();
            let provider = IsolatedRpc::start(
                network,
                network == blocked,
                Arc::clone(&handshake),
                observer.clone(),
                session.session_id.clone(),
            );
            let log = fs::File::create(directory.join("worker.log")).unwrap();
            let worker = ChildGuard {
                process: Command::new(env!("CARGO_BIN_EXE_research-worker"))
                    .env("ARB_WORKER_CONFIG", config)
                    .env("ARB_POOL_REGISTRY", registry)
                    .env("ARB_OPERATOR_ID", &operator)
                    .env("ARB_SESSION_ID", &session.session_id)
                    .env("TEST_DATABASE_URL", &database)
                    .env("TEST_WORKER_RPC", &provider.endpoint)
                    .env("ARB_STAGE_METRICS_STDERR", "1")
                    .stdout(Stdio::from(log.try_clone().unwrap()))
                    .stderr(Stdio::from(log))
                    .spawn()
                    .unwrap(),
                shutdown: Arc::clone(&provider.shutdown),
            };
            providers.push(provider);
            workers.push(worker);
            sessions.push(session);
        }
        for session in &sessions {
            wait_until(
                async || {
                    let state = store
                        .get_session(&operator, &session.session_id)
                        .await
                        .unwrap();
                    // Generic session health is deliberately DEGRADED for a research
                    // worker; actual acquisition readiness is durable evidence.
                    let ready: i64 = sqlx::query_scalar(
                        "SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='READINESS' AND outcome='READINESS_COMPLETED'",
                    )
                    .bind(&session.session_id)
                    .fetch_one(&observer)
                    .await
                    .unwrap();
                    state.observed_state == "STOPPED" && ready > 0
                },
                15,
            )
            .await;
        }
        for session in &sessions {
            let command = store
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
            assert_eq!(command.status, "PENDING");
            wait_until(
                async || {
                    store
                        .get_command(&operator, &command.command_id)
                        .await
                        .unwrap()
                        .status
                        == "APPLIED"
                },
                5,
            )
            .await;
        }
        let blocked_index = usize::from(blocked == NetworkId::SolanaMainnet);
        let active_index = 1 - blocked_index;
        wait_until(async || handshake.entered.load(Ordering::SeqCst), 10).await;
        let (held_attempt, held_since) = handshake
            .held
            .lock()
            .unwrap()
            .clone()
            .expect("handshake has exact attempt");
        let receipts = tokio::time::timeout_at(
            tokio::time::Instant::from_std(held_since + ASSERTION_WINDOW), async {
        let active = &sessions[active_index];
        wait_until(
            async || {
                !store
                    .list_decision_traces(&operator, &active.session_id, None, 10)
                    .await
                    .unwrap()
                    .items
                    .is_empty()
            },
            8,
        )
        .await;
        assert!(!handshake.release.load(Ordering::SeqCst));
        let pending_capture: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM collection_attempts WHERE attempt_id=$1 AND session_id=$2 AND purpose='RESEARCH' AND outcome='IN_PROGRESS'",
        )
        .bind(&held_attempt)
        .bind(&sessions[blocked_index].session_id)
        .fetch_one(&observer)
        .await
        .unwrap();
        assert_eq!(
            pending_capture, 1,
            "the held request must belong to research, not readiness"
        );
        let absent: i64 =
            sqlx::query_scalar("SELECT count(*) FROM decision_traces WHERE session_id=$1")
                .bind(&sessions[blocked_index].session_id)
                .fetch_one(&observer)
                .await
                .unwrap();
        assert_eq!(absent, 0, "blocked input cannot fabricate a decision");
        let mut receipts = Vec::new();
        for index in [active_index, blocked_index] {
            let session = &sessions[index];
            let started = Instant::now();
            let stop = store
                .issue_command(
                    &operator,
                    &session.session_id,
                    "stop",
                    NewCommand {
                        action: "STOP".into(),
                        expected_revision: "1".into(),
                        reason: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(stop.status, "PENDING");
            assert!(!stop.fence_effective);
            wait_until(
                async || {
                    store
                        .get_command(&operator, &stop.command_id)
                        .await
                        .unwrap()
                        .status
                        == "APPLIED"
                },
                3,
            )
            .await;
            let elapsed = started.elapsed();
            let state = store
                .get_session(&operator, &session.session_id)
                .await
                .unwrap();
            assert_eq!(state.observed_state, "STOPPED");
            assert!(!state.execution_authorized);
            assert!(handshake.entered.load(Ordering::SeqCst));
            assert!(!handshake.release.load(Ordering::SeqCst));
            receipts.push(json!({"network":session.network_id,"command_id":stop.command_id,"status":"APPLIED","stop_elapsed_ns":elapsed.as_nanos().to_string()}));
        }
        assert_ne!(receipts[0]["command_id"], receipts[1]["command_id"]);
        let unchanged: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_attempts WHERE attempt_id=$1 AND outcome='IN_PROGRESS'")
            .bind(&held_attempt).fetch_one(&observer).await.unwrap();
        assert_eq!(unchanged, 1, "both ACKs must precede completion of the exact held attempt");
        receipts
        }).await.expect("isolation and both STOP acknowledgements must fit below the real RPC timeout");
        handshake.release.store(true, Ordering::SeqCst);
        wait_until(async || {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_attempts WHERE attempt_id=$1 AND purpose='RESEARCH' AND outcome='SUPPRESSED' AND captured_pools=2")
                .bind(&held_attempt).fetch_one(&observer).await.unwrap() == 1
        }, 8).await;
        // A timed-out earlier attempt or a later retry cannot satisfy this check:
        // the socket's exact held attempt must finish a complete two-pool capture.
        let active_attempt = handshake
            .active
            .lock()
            .unwrap()
            .clone()
            .expect("active research handshake");
        let active_completed: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_attempts WHERE attempt_id=$1 AND session_id=$2 AND outcome='DECISIONS_RECORDED'")
            .bind(&active_attempt).bind(&sessions[active_index].session_id).fetch_one(&observer).await.unwrap();
        assert_eq!(active_completed, 1);
        let active = &sessions[active_index];
        let decisions = store
            .list_decision_traces(&operator, &active.session_id, None, 10)
            .await
            .unwrap();
        assert!(decisions.items.iter().all(
            |item| item.trace.dataset_origin == arb_domain::DatasetOrigin::ManuallyConstructed
        ));
        evidence.push(json!({"blocked_network":blocked.as_str(),"other_network_decisions":decisions.items.len(),"blocked_decisions":0,"stop_receipts":receipts,"late_capture_suppressed":true,"held_attempt_id":held_attempt,"active_attempt_id":active_attempt,"held_rpc_completed_two_pools":true,"assertion_window_ms":ASSERTION_WINDOW.as_millis().to_string()}));
        drop(workers);
        drop(providers);
        fs::remove_dir_all(&root).unwrap();
    }
    if let Ok(directory) = std::env::var("ARB_TEST_STAGE_EVIDENCE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("dual-worker-isolation.json"),
            serde_json::to_vec_pretty(&json!({
                "origin":"MANUALLY_CONSTRUCTED_LOOPBACK_INPUTS","real_processes":true,
                "deployment_capacity_qualified":false,"cases":evidence,
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
