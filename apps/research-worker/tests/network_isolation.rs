//! Two actual research processes and independent bounded local RPC services.
//! Deliberately constructed inputs prove control isolation, not market coverage.
use arb_adapter_api::RpcRecord;
use arb_capture::digest;
use arb_config::ValidatedConfig;
use arb_storage::{NewCommand, NewSession, Store};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::{Child, Command, Stdio},
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant},
};
use uuid::Uuid;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn wait_until(mut check: impl AsyncFnMut() -> bool, seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if check().await { return; }
        assert!(Instant::now() < deadline, "dual-worker condition timed out");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
use arb_domain::NetworkId;

struct IsolatedRpc {
    endpoint: String,
    release: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    server: Option<std::thread::JoinHandle<()>>,
}
impl IsolatedRpc {
    fn start(network: NetworkId, blocked: bool) -> Self {
        let records: Vec<RpcRecord> = serde_json::from_str(match network {
            NetworkId::BaseMainnet => include_str!("../../../crates/arb-evm/tests/fixtures/rpc.json"),
            NetworkId::SolanaMainnet => include_str!("../../../crates/arb-solana/tests/fixtures/rpc.json"),
        }).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let release = Arc::new(AtomicBool::new(!blocked));
        let entered = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let (allow, waiting, finished) = (release.clone(), entered.clone(), done.clone());
        let server = std::thread::spawn(move || {
            let mut cursor = 0;
            let mut batches = 0;
            while !finished.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert!(line.starts_with("POST "));
                let mut length = None;
                let mut header_bytes = line.len();
                loop {
                    line.clear();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    header_bytes += line.len();
                    assert!(header_bytes <= 16_384);
                    if line == "\r\n" { break; }
                    if let Some((key, value)) = line.split_once(':')
                        && key.eq_ignore_ascii_case("content-length") {
                        assert!(length.is_none());
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let length = length.unwrap();
                assert!(length <= 65_536);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: Value = serde_json::from_slice(&body).unwrap();
                let record = &records[cursor];
                assert_eq!(request["method"], record.method.wire_name());
                assert_eq!(request["params"], record.params);
                assert_eq!(request["id"], record.sequence);
                // Let readiness finish. Only the subsequent research capture stalls.
                if cursor == 0 && batches == 1 && blocked {
                    waiting.store(true, Ordering::SeqCst);
                    let deadline = Instant::now() + Duration::from_secs(20);
                    while !allow.load(Ordering::SeqCst) && !finished.load(Ordering::SeqCst) {
                        assert!(Instant::now() < deadline, "isolation hold exceeded test deadline");
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
                if finished.load(Ordering::SeqCst) { break; }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    record.response.len(), record.response,
                );
                if stream.write_all(response.as_bytes()).is_err() {
                    assert!(finished.load(Ordering::SeqCst), "unexpected provider disconnect");
                    break;
                }
                cursor = (cursor + 1) % records.len();
                if cursor == 0 { batches += 1; }
            }
        });
        Self { endpoint, release, entered, done, server: Some(server) }
    }
}
impl Drop for IsolatedRpc {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        self.release.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            // Do not hide a test-server panic on the successful test path.
            if std::thread::panicking() { let _ = server.join(); } else { server.join().unwrap(); }
        }
    }
}

fn configuration(network: NetworkId, root: &std::path::Path, registry: &[u8]) -> String {
    let value: Value = serde_json::from_slice(registry).unwrap();
    let mut input = include_str!("../../../config/research.example.toml").replace("mode = \"PAPER\"", "mode = \"OBSERVE\"");
    let (key, end_marker, venue, asset_a, asset_b, identity) = match network {
        NetworkId::BaseMainnet => ("base", "[networks.solana]", "uniswap-v3", "token0", "token1", "expected_evm_chain_id = 8453".to_string()),
        NetworkId::SolanaMainnet => ("solana", "[resources]", "orca-whirlpools", "mint_a", "mint_b", format!("expected_genesis_identity = {}", value["expected_genesis_hash"])),
    };
    let start = input.find(&format!("[networks.{key}]")).unwrap();
    let end = input.find(end_marker).unwrap();
    let section = format!(
        "[networks.{key}]\nregistry_id = \"{id}\"\nenabled = true\ncandidate_venue = \"{venue}\"\n{identity}\nverified_pool_ids = [\"{id}:{pool}\"]\nverified_asset_ids = [\"{id}:{a}\",\"{id}:{b}\"]\nrpc_secret_reference = \"env:TEST_WORKER_RPC\"\nregistry_qualification_digest = \"{digest}\"\n\n",
        id = network.as_str(), pool = value["pool"].as_str().unwrap(),
        a = value[asset_a].as_str().unwrap(), b = value[asset_b].as_str().unwrap(), digest = digest(registry),
    );
    input.replace_range(start..end, &section);
    input.replace("database_secret_reference = \"UNCONFIGURED\"", "database_secret_reference = \"env:TEST_DATABASE_URL\"")
        .replace("capture_directory = \"./data/captures\"", &format!("capture_directory = \"{}\"", root.display()))
}

#[test]
fn both_isolation_inputs_are_valid_but_explicitly_not_market_qualification() {
    for (network, registry) in [
        (NetworkId::BaseMainnet, include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json").as_slice()),
        (NetworkId::SolanaMainnet, include_bytes!("../../../crates/arb-solana/tests/fixtures/registry.json").as_slice()),
    ] {
        let config = ValidatedConfig::from_toml(&configuration(network, std::path::Path::new("./synthetic-captures"), registry)).unwrap();
        arb_registry::RegistryDocument::from_bytes(registry, network).unwrap().authorize(&config).unwrap();
        assert_eq!(config.mode(), arb_domain::Mode::Observe);
    }
}

#[tokio::test]
async fn either_blocked_provider_leaves_the_other_worker_evaluating_and_both_controls_responsive() {
    let database = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL required for dual-process isolation");
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let observer = sqlx::postgres::PgPoolOptions::new().max_connections(2).connect(&database).await.unwrap();
    let mut evidence = Vec::new();
    for blocked in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
        let root = std::env::temp_dir().join(format!("arb-dual-isolation-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let operator = format!("dual-isolation-{}", Uuid::new_v4());
        let mut providers = Vec::new();
        let mut workers = Vec::new();
        let mut sessions = Vec::new();
        for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
            let data = match network {
                NetworkId::BaseMainnet => include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json").as_slice(),
                NetworkId::SolanaMainnet => include_bytes!("../../../crates/arb-solana/tests/fixtures/registry.json").as_slice(),
            };
            let directory = root.join(network.as_str());
            fs::create_dir(&directory).unwrap();
            let captures = directory.join("captures");
            fs::create_dir(&captures).unwrap();
            let text = configuration(network, &captures, data);
            let validated = ValidatedConfig::from_toml(&text).unwrap();
            let config = directory.join("config.toml");
            let registry = directory.join("registry.json");
            fs::write(&config, &text).unwrap();
            fs::write(&registry, data).unwrap();
            store.save_configuration(&operator, validated.digest(), serde_json::from_str(validated.effective_json()).unwrap()).await.unwrap();
            let session = store.create_session(&operator, network.as_str(), NewSession {
                network_id: network.as_str().into(), mode: "OBSERVE".into(),
                configuration_digest: validated.digest().into(), experiment_id: "synthetic-dual-isolation".into(),
                strategy_ids: validated.strategy_ids().to_vec(),
            }).await.unwrap();
            let provider = IsolatedRpc::start(network, network == blocked);
            let log = fs::File::create(directory.join("worker.log")).unwrap();
            let worker = ChildGuard(Command::new(env!("CARGO_BIN_EXE_research-worker"))
                .env("ARB_WORKER_CONFIG", config).env("ARB_POOL_REGISTRY", registry)
                .env("ARB_OPERATOR_ID", &operator).env("ARB_SESSION_ID", &session.session_id)
                .env("TEST_DATABASE_URL", &database).env("TEST_WORKER_RPC", &provider.endpoint)
                .env("ARB_STAGE_METRICS_STDERR", "1")
                .stdout(Stdio::from(log.try_clone().unwrap())).stderr(Stdio::from(log))
                .spawn().unwrap());
            providers.push(provider); workers.push(worker); sessions.push(session);
        }
        for session in &sessions {
            wait_until(async || {
                let state = store.get_session(&operator, &session.session_id).await.unwrap();
                state.observed_state == "STOPPED" && state.health == "HEALTHY"
            }, 15).await;
            let command = store.issue_command(&operator, &session.session_id, "start", NewCommand {
                action: "START".into(), expected_revision: "0".into(), reason: None,
            }).await.unwrap();
            assert_eq!(command.status, "PENDING");
            wait_until(async || store.get_command(&operator, &command.command_id).await.unwrap().status == "APPLIED", 5).await;
        }
        let blocked_index = usize::from(blocked == NetworkId::SolanaMainnet);
        let active_index = 1 - blocked_index;
        wait_until(async || providers[blocked_index].entered.load(Ordering::SeqCst), 10).await;
        let active = &sessions[active_index];
        wait_until(async || {
            !store.list_decision_traces(&operator, &active.session_id, None, 10).await.unwrap().items.is_empty()
        }, 8).await;
        assert!(!providers[blocked_index].release.load(Ordering::SeqCst));
        let absent: i64 = sqlx::query_scalar("SELECT count(*) FROM decision_traces WHERE session_id=$1")
            .bind(&sessions[blocked_index].session_id).fetch_one(&observer).await.unwrap();
        assert_eq!(absent, 0, "blocked input cannot fabricate a decision");
        let mut receipts = Vec::new();
        for index in [active_index, blocked_index] {
            let session = &sessions[index];
            let started = Instant::now();
            let stop = store.issue_command(&operator, &session.session_id, "stop", NewCommand {
                action: "STOP".into(), expected_revision: "1".into(), reason: None,
            }).await.unwrap();
            assert_eq!(stop.status, "PENDING");
            assert!(!stop.fence_effective);
            wait_until(async || store.get_command(&operator, &stop.command_id).await.unwrap().status == "APPLIED", 3).await;
            let elapsed = started.elapsed();
            let state = store.get_session(&operator, &session.session_id).await.unwrap();
            assert_eq!(state.observed_state, "STOPPED");
            assert!(!state.execution_authorized);
            assert!(providers[blocked_index].entered.load(Ordering::SeqCst));
            assert!(!providers[blocked_index].release.load(Ordering::SeqCst));
            receipts.push(json!({"network":session.network_id,"command_id":stop.command_id,"status":"APPLIED","stop_elapsed_ns":elapsed.as_nanos().to_string()}));
        }
        assert_ne!(receipts[0]["command_id"], receipts[1]["command_id"]);
        providers[blocked_index].release.store(true, Ordering::SeqCst);
        wait_until(async || {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='SUPPRESSED'")
                .bind(&sessions[blocked_index].session_id).fetch_one(&observer).await.unwrap() > 0
        }, 8).await;
        let decisions = store.list_decision_traces(&operator, &active.session_id, None, 10).await.unwrap();
        assert!(decisions.items.iter().all(|item| item.trace.dataset_origin == arb_domain::DatasetOrigin::ManuallyConstructed));
        evidence.push(json!({"blocked_network":blocked.as_str(),"other_network_decisions":decisions.items.len(),"blocked_decisions":0,"stop_receipts":receipts,"late_capture_suppressed":true}));
        drop(workers); drop(providers); fs::remove_dir_all(&root).unwrap();
    }
    if let Ok(directory) = std::env::var("ARB_TEST_STAGE_EVIDENCE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("dual-worker-isolation.json"), serde_json::to_vec_pretty(&json!({
            "origin":"MANUALLY_CONSTRUCTED_LOOPBACK_INPUTS","real_processes":true,
            "deployment_capacity_qualified":false,"cases":evidence,
        })).unwrap()).unwrap();
    }
}
