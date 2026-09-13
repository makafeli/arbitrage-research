//! Real process + PostgreSQL + loopback HTTP test. All RPC content is manually
//! constructed transcript data, never evidence of market or strategy performance.
use arb_adapter_api::{RpcRecord, TranscriptRpc};
use arb_capture::digest;
use arb_config::ValidatedConfig;
use arb_storage::{NewCommand, NewSession, Store};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
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

fn test_config(root: &str, registry: &[u8], mode: &str) -> String {
    let mut input = include_str!("../../../config/research.example.toml")
        .replace("mode = \"PAPER\"", &format!("mode = \"{mode}\""));
    let begin = input.find("[networks.base]").unwrap();
    let end = input.find("[networks.solana]").unwrap();
    let mut base = input[begin..end].replace("enabled = false", "enabled = true");
    base=base.replace("verified_pool_ids = []","verified_pool_ids = [\"base-mainnet:0x0303030303030303030303030303030303030303\", \"base-mainnet:0x0404040404040404040404040404040404040404\"]");
    base=base.replace("verified_asset_ids = []","verified_asset_ids = [\"base-mainnet:0x0101010101010101010101010101010101010101\", \"base-mainnet:0x0202020202020202020202020202020202020202\"]");
    base=base.replace("rpc_secret_reference = \"UNCONFIGURED\"",&format!("rpc_secret_reference = \"env:TEST_WORKER_RPC\"\nregistry_qualification_digest = \"{}\"",digest(registry)));
    if mode == "PAPER" {
        let fixture: Value = serde_json::from_slice(registry).unwrap();
        let token0 = fixture["token0"].as_str().unwrap();
        base.push_str(&format!("starting_asset_id = \"base-mainnet:{token0}\"\n"));
    }
    input.replace_range(begin..end, &base);
    if mode == "PAPER" {
        // Exact fixture-token base units, used only to exercise research capture.
        // A single manually constructed pool still cannot form an arbitrage route.
        input = input.replace("trade_sizes_minor = []", "trade_sizes_minor = [\"1000\"]");
    }
    input = input.replace(
        "database_secret_reference = \"UNCONFIGURED\"",
        "database_secret_reference = \"env:TEST_DATABASE_URL\"",
    );
    input.replace(
        "capture_directory = \"./data/captures\"",
        &format!("capture_directory = \"{root}\""),
    )
}

#[test]
fn process_fixtures_pass_production_config_validation_without_postgres() {
    let registry = include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json");
    let fixture: Value = serde_json::from_slice(registry).unwrap();
    for mode in ["OBSERVE", "PAPER"] {
        let config = test_config("./synthetic-test-captures", registry, mode);
        let validated = ValidatedConfig::from_toml(&config).unwrap();
        assert_eq!(serde_json::to_value(validated.mode()).unwrap(), mode);
        arb_registry::RegistryDocument::from_bytes(registry, arb_domain::NetworkId::BaseMainnet)
            .unwrap()
            .authorize(&validated)
            .unwrap();
        if mode == "PAPER" {
            assert_eq!(
                validated
                    .starting_asset(arb_domain::NetworkId::BaseMainnet)
                    .unwrap()
                    .to_string(),
                format!("base-mainnet:{}", fixture["token0"].as_str().unwrap())
            );
            assert_eq!(validated.trade_sizes()[0].to_string(), "1000");
        }
    }
}

async fn wait_until(mut check: impl AsyncFnMut() -> bool, seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if check().await {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "condition timed out after {seconds}s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn stop_acknowledges_while_rpc_is_blocked_and_late_capture_stays_unadmitted() {
    controlled_process("OBSERVE").await;
}
#[tokio::test]
async fn paper_worker_recovers_and_fences_read_only_research_without_settlement() {
    controlled_process("PAPER").await;
}
async fn controlled_process(mode: &str) {
    let database = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL required; process integration must not silently skip");
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let root = std::env::temp_dir().join(format!("arb-controlled-capture-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let capture_root = root.join("captures");
    fs::create_dir(&capture_root).unwrap();
    let registry = include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json");
    let config = test_config(capture_root.to_str().unwrap(), registry, mode);
    let validated = ValidatedConfig::from_toml(&config).unwrap();
    let config_path = root.join("config.toml");
    let registry_path = root.join("registry.json");
    fs::write(&config_path, &config).unwrap();
    fs::write(&registry_path, registry).unwrap();
    let operator = format!("process-{}", Uuid::new_v4());
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
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: mode.into(),
                configuration_digest: validated.digest().into(),
                experiment_id: "synthetic-loopback-control".into(),
                strategy_ids: validated.strategy_ids().to_vec(),
            },
        )
        .await
        .unwrap();
    let id = session.session_id;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let hold = Arc::new(AtomicBool::new(true));
    let rounds = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let (server_hold, server_rounds, server_done) = (hold.clone(), rounds.clone(), done.clone());
    let server = std::thread::spawn(move || {
        let records: Vec<RpcRecord> = serde_json::from_str(include_str!(
            "../../../crates/arb-evm/tests/fixtures/rpc.json"
        ))
        .unwrap();
        let mut cursor = 0;
        while !server_done.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            assert!(first.starts_with("POST "));
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            assert!(length < 64 * 1024);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();
            let record = &records[cursor];
            assert_eq!(request["method"], record.method.wire_name());
            assert_eq!(request["params"], record.params);
            if cursor == 0 {
                let round = server_rounds.fetch_add(1, Ordering::SeqCst) + 1;
                if round == 2 {
                    let deadline = Instant::now() + Duration::from_secs(4);
                    while server_hold.load(Ordering::SeqCst)
                        && Instant::now() < deadline
                        && !server_done.load(Ordering::SeqCst)
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                record.response.len(),
                record.response
            );
            stream.write_all(response.as_bytes()).unwrap();
            cursor = (cursor + 1) % records.len();
        }
    });
    let log = fs::File::create(root.join("worker.log")).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_research-worker"))
        .env("ARB_WORKER_CONFIG", &config_path)
        .env("ARB_POOL_REGISTRY", &registry_path)
        .env("ARB_OPERATOR_ID", &operator)
        .env("ARB_SESSION_ID", &id)
        .env("TEST_DATABASE_URL", &database)
        .env("TEST_WORKER_RPC", &endpoint)
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let _child = ChildGuard(child);
    wait_until(
        async || {
            store
                .get_session(&operator, &id)
                .await
                .unwrap()
                .observed_state
                == "STOPPED"
        },
        10,
    )
    .await;
    store
        .issue_command(
            &operator,
            &id,
            "start",
            NewCommand {
                action: "START".into(),
                expected_revision: "0".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    wait_until(
        async || {
            store
                .get_session(&operator, &id)
                .await
                .unwrap()
                .observed_state
                == "RUNNING"
        },
        10,
    )
    .await;
    wait_until(async || rounds.load(Ordering::SeqCst) >= 2, 10).await;
    let pool = sqlx::PgPool::connect(&database).await.unwrap();
    let collection_id: String = sqlx::query_scalar(
        "SELECT attempt_id FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='IN_PROGRESS'",
    ).bind(&id).fetch_one(&pool).await.unwrap();
    let readiness: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='READINESS' AND outcome='READINESS_COMPLETED' AND decision_rows=0",
    ).bind(&id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        readiness, 1,
        "readiness is durable and separate from the research denominator"
    );
    let before = Instant::now();
    let stop = store
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
    assert_eq!(stop.status, "PENDING");
    wait_until(
        async || {
            store
                .get_command(&operator, &stop.command_id)
                .await
                .unwrap()
                .status
                == "APPLIED"
        },
        2,
    )
    .await;
    assert!(before.elapsed() < Duration::from_secs(2));
    assert!(hold.load(Ordering::SeqCst));
    assert_eq!(
        store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .observed_state,
        "STOPPED"
    );
    hold.store(false, Ordering::SeqCst);
    wait_until(
        async || fs::read_dir(&capture_root).unwrap().count() >= 2,
        5,
    )
    .await;
    wait_until(
        async || {
            sqlx::query_scalar::<_, String>(
                "SELECT outcome FROM collection_attempts WHERE attempt_id=$1",
            )
            .bind(&collection_id)
            .fetch_one(&pool)
            .await
            .unwrap()
                == "SUPPRESSED"
        },
        5,
    )
    .await;
    let terminal: (String, i32, i64) = sqlx::query_as(
        "SELECT reason,captured_pools,decision_rows FROM collection_attempts WHERE attempt_id=$1",
    )
    .bind(&collection_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(terminal, ("GENERATION_FENCED".into(), 1, 0));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM capture_admissions WHERE session_id=$1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "fenced capture must remain unadmitted");
    let decisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM decision_traces WHERE session_id=$1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        decisions, 0,
        "late capture must not create a research decision"
    );
    store
        .issue_command(
            &operator,
            &id,
            "start-again",
            NewCommand {
                action: "START".into(),
                expected_revision: "2".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    wait_until(
        async || {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM capture_admissions WHERE session_id=$1",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap()
                == 1
        },
        12,
    )
    .await;
    assert_eq!(
        store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .outstanding_attempts,
        0
    );
    wait_until(
        async || {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM decision_traces WHERE session_id=$1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap()
                > 0
        },
        5,
    )
    .await;
    let invalid:i64=sqlx::query_scalar("SELECT count(*) FROM decision_traces WHERE session_id=$1 AND (result_status='QUOTED' OR payload->>'dataset_origin'!='MANUALLY_CONSTRUCTED')")
        .bind(&id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        invalid, 0,
        "single-pool loopback data cannot become a market opportunity"
    );
    let log = fs::read_to_string(root.join("worker.log")).unwrap();
    assert!(log.contains("UNADMITTED_RAW_CAPTURE"));
    assert!(log.contains("ADMITTED_RAW_CAPTURE"));
    assert!(!log.contains(&endpoint));
    assert!(!log.contains(&database));
    done.store(true, Ordering::SeqCst);
    hold.store(false, Ordering::SeqCst);
    server.join().unwrap();
    drop(_child);
    fs::remove_dir_all(root).unwrap();
}

const FIRST_POOL: &str = "0x0303030303030303030303030303030303030303";
const SECOND_POOL: &str = "0x0404040404040404040404040404040404040404";
const START_TOKEN: &str = "0x0101010101010101010101010101010101010101";

/// Exact manually constructed ABI inputs: two fee tiers at price 1, tick 0,
/// liquidity 10^12 and empty captured bitmap words. These are not mainnet pools.
fn two_pool_fixture() -> (Vec<u8>, Vec<Vec<RpcRecord>>) {
    let original: Value = serde_json::from_str(include_str!(
        "../../../crates/arb-evm/tests/fixtures/registry.json"
    ))
    .unwrap();
    let template: Vec<RpcRecord> = serde_json::from_str(include_str!(
        "../../../crates/arb-evm/tests/fixtures/rpc.json"
    ))
    .unwrap();
    let mut registries = Vec::new();
    let mut transcripts = Vec::new();
    for (pool, fee, spacing) in [(FIRST_POOL, 500_u32, 10_u32), (SECOND_POOL, 3000, 60)] {
        let mut registry = original.clone();
        registry["pool"] = json!(pool);
        registry["fee"] = json!(fee);
        registry["tick_spacing"] = json!(spacing);
        registries.push(registry);
        let mut records = Vec::new();
        for source in &template {
            let mut record: RpcRecord = serde_json::from_str(
                &serde_json::to_string(source)
                    .unwrap()
                    .replace(&FIRST_POOL[2..], &pool[2..]),
            )
            .unwrap();
            let selector = record.params[0]["data"].as_str().unwrap_or("").to_owned();
            if selector.starts_with("0xf30dba93") {
                continue; // No initialized ticks are advertised in this fixture.
            }
            let mut response: Value = serde_json::from_str(&record.response).unwrap();
            let result = match selector.as_str() {
                "0xddca3f43" => Some(format!("0x{fee:064x}")),
                "0xd0c93a7c" => Some(format!("0x{spacing:064x}")),
                "0x3850c7bd" => Some(format!(
                    "0x{}",
                    [1_u128 << 96, 0, 0, 1, 1, 0, 1]
                        .into_iter()
                        .map(|word| format!("{word:064x}"))
                        .collect::<String>()
                )),
                "0x1a686502" => Some(format!("0x{:064x}", 1_000_000_000_000_u128)),
                _ if selector.starts_with("0x5339c296") => Some(format!("0x{:064x}", 0)),
                _ => None,
            };
            if let Some(result) = result {
                response["result"] = json!(result);
            }
            if selector.starts_with("0x1698ee82") {
                // Different fees give different factory getPool keys at the same block.
                record.params[0]["data"] =
                    json!(format!("{}{fee:064x}", &selector[..selector.len() - 64]));
            }
            record.sequence = records.len() as u64;
            response["id"] = json!(record.sequence);
            record.response = response.to_string();
            records.push(record);
        }
        transcripts.push(records);
    }
    (
        serde_json::to_vec(&json!({
            "schema_version":1,"network_id":"base-mainnet","pools":registries
        }))
        .unwrap(),
        transcripts,
    )
}

fn two_pool_config(root: &str, registry: &[u8]) -> String {
    let mut input = test_config(root, registry, "OBSERVE")
        .replace("trade_sizes_minor = []", "trade_sizes_minor = [\"10000\"]");
    let end = input.find("[networks.solana]").unwrap();
    input.insert_str(
        end,
        &format!("starting_asset_id = \"base-mainnet:{START_TOKEN}\"\n\n"),
    );
    // The fixture budget includes CI scheduling and file synchronization. This is
    // not a freshness qualification for the deliberately historical block below.
    input.push_str("\n[simulation]\ndelay_scenarios_ms = [0]\nfee_buffer_bps = 1000\nmaximum_state_age_ms = 30000\n");
    input
}

#[test]
fn two_pool_process_fixture_has_exact_negative_round_trip_without_market_claims() {
    let (registry, records) = two_pool_fixture();
    let config = ValidatedConfig::from_toml(&two_pool_config("./manual-pair", &registry)).unwrap();
    let document =
        arb_registry::RegistryDocument::from_bytes(&registry, arb_domain::NetworkId::BaseMainnet)
            .unwrap();
    document.authorize(&config).unwrap();
    let mut snapshots = Vec::new();
    for (entry, transcript) in document.pools().iter().zip(records) {
        let arb_registry::PoolRegistry::Base(pool) = entry else {
            panic!("expected Base fixture")
        };
        let mut rpc = TranscriptRpc::new(transcript);
        let snapshot = arb_evm::capture_pool(&mut rpc, pool, 100).unwrap();
        rpc.finish().unwrap();
        assert!(!snapshot.quality.quote_implementation_qualified);
        snapshots.push((snapshot, pool));
    }
    assert_eq!(snapshots[0].0.context, snapshots[1].0.context);
    for (first, second) in [(0, 1), (1, 0)] {
        let out = arb_evm::math::quote_exact_input_math(
            &snapshots[first].0,
            snapshots[first].1,
            arb_domain::AtomicAmount::from(10000),
            true,
        )
        .unwrap();
        let back = arb_evm::math::quote_exact_input_math(
            &snapshots[second].0,
            snapshots[second].1,
            out.amount_out,
            false,
        )
        .unwrap();
        // Integer fees plus downward output rounding: 10,000 -> 9,963.
        assert_eq!(back.amount_out.as_str(), "9963");
        assert_eq!(back.evidence, "CANDIDATE");
    }
}

struct PairRpcGuard {
    endpoint: String,
    done: Arc<AtomicBool>,
    tip_advances: Arc<AtomicUsize>,
    server: Option<std::thread::JoinHandle<()>>,
}
impl PairRpcGuard {
    fn start(transcripts: Vec<Vec<RpcRecord>>) -> Self {
        Self::with_canonical_failure_after(transcripts, None)
    }
    fn with_canonical_failure_after(
        transcripts: Vec<Vec<RpcRecord>>,
        fail_after: Option<usize>,
    ) -> Self {
        // Exact v2 acquisition: one chain/anchor, both pools' pinned reads,
        // then one canonical recheck. Reindex the actual fixture RPC responses.
        let second_pool_start = transcripts[0].len() - 1;
        let mut records = transcripts[0][..2].to_vec();
        for transcript in &transcripts {
            records.extend_from_slice(&transcript[2..transcript.len() - 1]);
        }
        records.push(transcripts.last().unwrap().last().unwrap().clone());
        for (sequence, record) in records.iter_mut().enumerate() {
            record.sequence = sequence as u64;
            let mut response: Value = serde_json::from_str(&record.response).unwrap();
            response["id"] = json!(sequence);
            record.response = response.to_string();
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let finished = Arc::clone(&done);
        let tip_advances = Arc::new(AtomicUsize::new(0));
        let server_tip_advances = Arc::clone(&tip_advances);
        let server = std::thread::spawn(move || {
            let original_tip: Value = serde_json::from_str(&records[1].response).unwrap();
            let mut finalized_tip = original_tip.clone();
            let mut cursor = 0;
            let mut completed_batches = 0;
            while !finished.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
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
                    assert!(header_bytes <= 16384);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        assert!(length.is_none());
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                }
                let length = length.expect("bounded JSON body required");
                assert!(length <= 65536);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: Value = serde_json::from_slice(&body).unwrap();
                let record = &records[cursor];
                if cursor == second_pool_start {
                    // The provider's finalized tip advances between the two
                    // pools. Hash-pinned reads and the old block's canonical
                    // lookup still return the original anchored state.
                    finalized_tip["result"]["hash"] = json!(format!("0x{}", "cd".repeat(32)));
                    let old = original_tip["result"]["number"].as_str().unwrap();
                    let next = u64::from_str_radix(old.trim_start_matches("0x"), 16).unwrap() + 1;
                    finalized_tip["result"]["number"] = json!(format!("0x{next:x}"));
                    server_tip_advances.fetch_add(1, Ordering::SeqCst);
                }
                assert_eq!(request["jsonrpc"], "2.0");
                assert_eq!(request["id"], record.sequence);
                assert_eq!(request["method"], record.method.wire_name());
                assert_eq!(request["params"], record.params);
                let response_body = if record.method
                    == arb_adapter_api::ReadMethod::EthGetBlockByNumber
                    && request["params"][0] == "finalized"
                {
                    finalized_tip["id"] = request["id"].clone();
                    finalized_tip.to_string()
                } else if cursor == records.len() - 1
                    && fail_after.is_some_and(|limit| completed_batches >= limit)
                {
                    let mut response: Value = serde_json::from_str(&record.response).unwrap();
                    response["result"]["hash"] = json!(format!("0x{}", "ab".repeat(32)));
                    response.to_string()
                } else {
                    record.response.clone()
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                if stream.write_all(response.as_bytes()).is_err() {
                    assert!(
                        finished.load(Ordering::SeqCst),
                        "worker closed an active fixture RPC"
                    );
                    break;
                }
                cursor = (cursor + 1) % records.len();
                if cursor == 0 {
                    completed_batches += 1;
                    finalized_tip = original_tip.clone();
                }
            }
        });
        Self {
            endpoint,
            done,
            tip_advances,
            server: Some(server),
        }
    }
    fn finish(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        self.server.take().unwrap().join().unwrap();
    }
}
impl Drop for PairRpcGuard {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

struct LocalHttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Value,
}
fn local_http(
    address: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&Value>,
) -> std::io::Result<LocalHttpResponse> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let body = body.map(Value::to_string).unwrap_or_default();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    )?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n{body}")?;
    let mut received = Vec::new();
    stream.take(1024 * 1024 + 1).read_to_end(&mut received)?;
    assert!(
        received.len() <= 1024 * 1024,
        "HTTP fixture response bound exceeded"
    );
    let response = std::str::from_utf8(&received).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(':').unwrap();
            (name.to_ascii_lowercase(), value.trim().to_owned())
        })
        .collect::<Vec<_>>();
    assert!(
        !headers.iter().any(|(name, _)| name == "transfer-encoding"),
        "JSON service responses must have a bounded content length"
    );
    Ok(LocalHttpResponse {
        status,
        headers,
        body: if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(body).unwrap()
        },
    })
}

#[tokio::test]
async fn two_pool_worker_quotes_are_durable_and_visible_through_authenticated_http() {
    two_pool_worker_http(false).await;
}

#[tokio::test]
async fn recently_acquired_old_base_state_is_durable_stale_data_through_http() {
    two_pool_worker_http(true).await;
}

async fn two_pool_worker_http(chain_stale: bool) {
    let api_binary = std::env::var("ARB_TEST_CONTROL_API_BIN")
        .expect("build control-api first and set ARB_TEST_CONTROL_API_BIN to its absolute path");
    assert!(std::path::Path::new(&api_binary).is_absolute());
    assert!(std::path::Path::new(&api_binary).is_file());
    let database = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL required; full quote-path integration must not silently skip");
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let root = std::env::temp_dir().join(format!("arb-two-pool-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let capture_root = root.join("captures");
    fs::create_dir(&capture_root).unwrap();
    let (registry, transcripts) = two_pool_fixture();
    let mut config = two_pool_config(capture_root.to_str().unwrap(), &registry);
    if chain_stale {
        config.push_str("\n[networks.base.chain_freshness]\nversion = \"finalized-chain-time-v1\"\nmax_chain_age_ms = 30000\n");
    }
    let expected_decisions = if chain_stale { 1 } else { 2 };
    let validated = ValidatedConfig::from_toml(&config).unwrap();
    let config_path = root.join("config.toml");
    let registry_path = root.join("registry.json");
    fs::write(&config_path, &config).unwrap();
    fs::write(&registry_path, &registry).unwrap();
    let experiment = format!("manual-two-pool-{}", Uuid::new_v4());
    // The real HTTP service is intentionally single-operator; the unique session,
    // experiment, digest and creation key isolate this case from other PG tests.
    let operator = "operator";
    store
        .save_configuration(
            operator,
            validated.digest(),
            serde_json::from_str(validated.effective_json()).unwrap(),
        )
        .await
        .unwrap();
    let session = store
        .create_session(
            operator,
            &experiment,
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: validated.digest().into(),
                experiment_id: experiment.clone(),
                strategy_ids: validated.strategy_ids().to_vec(),
            },
        )
        .await
        .unwrap();
    let id = session.session_id;
    let mut rpc = PairRpcGuard::start(transcripts);
    let log = fs::File::create(root.join("worker.log")).unwrap();
    let worker = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_research-worker"))
            .env("ARB_WORKER_CONFIG", &config_path)
            .env("ARB_POOL_REGISTRY", &registry_path)
            .env("ARB_OPERATOR_ID", operator)
            .env("ARB_SESSION_ID", &id)
            .env("TEST_DATABASE_URL", &database)
            .env("TEST_WORKER_RPC", &rpc.endpoint)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    wait_until(
        async || {
            store
                .get_session(operator, &id)
                .await
                .unwrap()
                .observed_state
                == "STOPPED"
        },
        10,
    )
    .await;
    let start = store
        .issue_command(
            operator,
            &id,
            "pair-start",
            NewCommand {
                action: "START".into(),
                expected_revision: "0".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    wait_until(
        async || {
            store
                .get_command(operator, &start.command_id)
                .await
                .unwrap()
                .status
                == "APPLIED"
        },
        10,
    )
    .await;
    wait_until(
        async || {
            store
                .list_decision_traces(operator, &id, None, 100)
                .await
                .unwrap()
                .items
                .len()
                >= expected_decisions
        },
        20,
    )
    .await;
    let observed = store.get_session(operator, &id).await.unwrap();
    let stop = store
        .issue_command(
            operator,
            &id,
            "pair-stop",
            NewCommand {
                action: "STOP".into(),
                expected_revision: observed.desired_revision,
                reason: Some("freeze manually constructed quote evidence".into()),
            },
        )
        .await
        .unwrap();
    wait_until(
        async || {
            store
                .get_command(operator, &stop.command_id)
                .await
                .unwrap()
                .status
                == "APPLIED"
        },
        5,
    )
    .await;
    let page = store
        .list_decision_traces(operator, &id, None, 100)
        .await
        .unwrap();
    assert!(page.next_cursor.is_none());
    assert_eq!(
        page.items.len(),
        expected_decisions,
        "one admitted batch records the complete expected result"
    );
    if !chain_stale {
        let route_directions = page
            .items
            .iter()
            .map(|stored| {
                (
                    stored.trace.route[0].pool_id.to_string(),
                    stored.trace.route[1].pool_id.to_string(),
                )
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            route_directions,
            std::collections::BTreeSet::from([
                (
                    format!("base-mainnet:{FIRST_POOL}"),
                    format!("base-mainnet:{SECOND_POOL}")
                ),
                (
                    format!("base-mainnet:{SECOND_POOL}"),
                    format!("base-mainnet:{FIRST_POOL}")
                ),
            ])
        );
    }
    let generation = page.items[0].trace.generation;
    for stored in &page.items {
        let trace = &stored.trace;
        trace.validate().unwrap();
        assert_eq!(trace.session_id, id);
        assert_eq!(trace.experiment_id, experiment);
        assert_eq!(trace.configuration_digest, validated.digest());
        assert_eq!(trace.generation, generation);
        assert_eq!(trace.mode, arb_domain::Mode::Observe);
        assert_eq!(
            trace.dataset_origin,
            arb_domain::DatasetOrigin::ManuallyConstructed
        );
        assert_eq!(trace.source_kind, arb_domain::SourceKind::SyntheticFixture);
        if chain_stale {
            assert!(trace.amount_in_minor.is_none());
            assert!(trace.route.is_empty());
            assert_eq!(trace.capture_refs.len(), 2);
            assert!(
                trace.input_age_ms.unwrap() <= 30000,
                "acquisition is recent"
            );
            let raw = serde_json::to_value(trace).unwrap();
            assert_eq!(raw["result"]["status"], "DATA_UNAVAILABLE");
            assert_eq!(raw["result"]["reason_codes"], json!(["CHAIN_TIME_STALE"]));
            assert_eq!(raw["chain_freshness"]["status"], "STALE");
            assert_eq!(
                raw["chain_freshness"]["evaluation_elapsed_ms"],
                raw["input_age_ms"]
            );
            assert!(
                trace
                    .calculation_version
                    .contains("finalized-chain-time-v1")
            );
            assert!(raw["result"].get("quoted_output_minor").is_none());
        } else {
            assert_eq!(trace.amount_in_minor.as_ref().unwrap().as_str(), "10000");
            assert!(trace.input_age_ms.unwrap() <= 30000);
            assert_eq!(trace.route.len(), 2);
            assert_eq!(trace.capture_refs.len(), 2);
            assert_ne!(trace.route[0].pool_id, trace.route[1].pool_id);
            assert_ne!(
                trace.capture_refs[0].capture_id,
                trace.capture_refs[1].capture_id
            );
            let result = serde_json::to_value(&trace.result).unwrap();
            assert_eq!(result["status"], "QUOTED");
            assert_eq!(result["quoted_output_minor"], "9963");
            assert_eq!(result["gross_delta_minor"], "-37");
            assert!(
                serde_json::to_value(trace)
                    .unwrap()
                    .get("chain_freshness")
                    .is_none()
            );
        }
        for (index, capture) in trace.capture_refs.iter().enumerate() {
            let bundle = arb_capture::load_bundle(
                &capture_root.join(&capture.capture_id),
                Some(&capture.manifest_digest),
                trace.observed_at_unix_ms + trace.input_age_ms.unwrap(),
            )
            .unwrap();
            assert_eq!(
                bundle.manifest.origin,
                arb_capture::Origin::ManuallyConstructed
            );
            assert_eq!(bundle.manifest.config_digest, validated.digest());
            assert_eq!(bundle.manifest.adapter_version, "arb_evm-pool-set-v2");
            let records: Vec<RpcRecord> = serde_json::from_slice(
                &bundle
                    .objects
                    .iter()
                    .find(|(name, _)| name == "rpc.json")
                    .unwrap()
                    .1,
            )
            .unwrap();
            assert_eq!(
                records
                    .iter()
                    .filter(|record| record.method == arb_adapter_api::ReadMethod::EthChainId)
                    .count(),
                1
            );
            assert_eq!(
                records
                    .iter()
                    .filter(|record| record.method
                        == arb_adapter_api::ReadMethod::EthGetBlockByNumber
                        && record.params[0] == "finalized")
                    .count(),
                1
            );
            for member in [FIRST_POOL, SECOND_POOL] {
                assert!(
                    records
                        .iter()
                        .any(|record| record.params[0]["to"] == member)
                );
            }
            assert!(!bundle.manifest.complete_for_quote);
            let raw: Value = serde_json::from_slice(
                &bundle
                    .objects
                    .iter()
                    .find(|(name, _)| name == "snapshot.json")
                    .unwrap()
                    .1,
            )
            .unwrap();
            if !chain_stale {
                assert_eq!(
                    format!("base-mainnet:{}", raw["pool"].as_str().unwrap()),
                    trace.route[index].pool_id.to_string()
                );
            }
            if chain_stale {
                let historical_seconds = raw["context"]["block_timestamp_seconds"]
                    .as_u64()
                    .expect("retained old block timestamp");
                assert!(trace.observed_at_unix_ms - historical_seconds * 1000 > 30000);
            }
        }
    }
    let query_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database)
        .await
        .unwrap();
    let collections: (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*),COALESCE(sum(decision_rows),0)::bigint,COALESCE(sum(captured_pools),0)::bigint FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='DECISIONS_RECORDED'",
    ).bind(&id).fetch_one(&query_pool).await.unwrap();
    assert_eq!(
        collections,
        (1, expected_decisions as i64, 2),
        "received stale data is admitted decision evidence, not an acquisition or route failure"
    );
    assert!(
        rpc.tip_advances.load(Ordering::SeqCst) >= 2,
        "readiness and research both captured their pair while finalized tip advanced"
    );
    let stopped_generation: i64 = sqlx::query_scalar(
        "SELECT generation FROM research_sessions WHERE operator_id=$1 AND session_id=$2",
    )
    .bind(operator)
    .bind(&id)
    .fetch_one(&query_pool)
    .await
    .unwrap();
    assert!(stopped_generation as u64 > generation);
    let paper_runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM paper_runs WHERE operator_id=$1 AND session_id=$2",
    )
    .bind(operator)
    .bind(&id)
    .fetch_one(&query_pool)
    .await
    .unwrap();
    assert_eq!(
        paper_runs, 0,
        "research quotes must not create a virtual account or settlement"
    );
    let admitted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_admissions WHERE session_id=$1 AND generation=$2",
    )
    .bind(&id)
    .bind(generation as i64)
    .fetch_one(&query_pool)
    .await
    .unwrap();
    assert_eq!(
        admitted, 2,
        "both inputs were durably admitted in the quote generation"
    );
    assert_eq!(
        store
            .get_session(operator, &id)
            .await
            .unwrap()
            .outstanding_attempts,
        0
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let origin = format!("http://{address}");
    let secret = format!("isolated-manual-fixture-{}", Uuid::new_v4());
    let log = fs::File::create(root.join("api.log")).unwrap();
    let api = ChildGuard(
        Command::new(api_binary)
            .env("ARB_OPERATOR_SECRET", &secret)
            .env("ARB_PUBLIC_ORIGIN", &origin)
            .env("ARB_ALLOW_INSECURE_LOOPBACK", "true")
            .env("ARB_API_BIND_IP", "127.0.0.1")
            .env("ARB_API_PORT", address.port().to_string())
            .env("ARB_CONFIG_FILES", &config_path)
            .env("ARB_DATABASE_URL", &database)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    wait_until(
        async || local_http(address, "GET", "/healthz", &[], None).is_ok_and(|r| r.status == 200),
        10,
    )
    .await;
    let resource = if chain_stale {
        "decisions"
    } else {
        "opportunities"
    };
    let path = format!("/v1/{resource}?session_id={id}&limit=1");
    assert_eq!(
        local_http(address, "GET", &path, &[], None).unwrap().status,
        401
    );
    let login = local_http(
        address,
        "POST",
        "/v1/auth/login",
        &[("Origin", &origin)],
        Some(&json!({"operator_secret":secret})),
    )
    .unwrap();
    assert_eq!(login.status, 200);
    let set_cookie = &login
        .headers
        .iter()
        .find(|(name, _)| name == "set-cookie")
        .unwrap()
        .1;
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    let cookie = set_cookie.split(';').next().unwrap();
    let auth = [("Cookie", cookie)];
    let first = local_http(address, "GET", &path, &auth, None).unwrap();
    assert_eq!(first.status, 200);
    assert_eq!(first.body["items"].as_array().unwrap().len(), 1);
    let mut responses = vec![first];
    if !chain_stale {
        let cursor = responses[0].body["next_cursor"]
            .as_str()
            .expect("second quote requires a cursor");
        let next = local_http(
            address,
            "GET",
            &format!("{path}&cursor={cursor}"),
            &auth,
            None,
        )
        .unwrap();
        assert_eq!(next.status, 200);
        assert!(next.body["next_cursor"].is_null());
        assert_eq!(next.body["items"].as_array().unwrap().len(), 1);
        responses.push(next);
    } else {
        assert!(responses[0].body["next_cursor"].is_null());
    }
    let expected = page
        .items
        .iter()
        .map(|stored| stored.trace.observation_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut received = std::collections::BTreeSet::new();
    for response in &responses {
        let raw = &response.body["items"][0];
        if chain_stale {
            let stored = &page.items[0];
            assert_eq!(*raw, serde_json::to_value(stored).unwrap());
            assert_eq!(raw["trace"]["result"]["status"], "DATA_UNAVAILABLE");
            assert_eq!(
                raw["trace"]["result"]["reason_codes"],
                json!(["CHAIN_TIME_STALE"])
            );
            assert_eq!(raw["trace"]["chain_freshness"]["status"], "STALE");
            let detail = local_http(
                address,
                "GET",
                &format!("/v1/decisions/{}", stored.trace.observation_id),
                &auth,
                None,
            )
            .unwrap();
            assert_eq!(detail.status, 200);
            assert_eq!(detail.body, *raw);
            received.insert(stored.trace.observation_id.clone());
            continue;
        }
        let quote: arb_domain::OpportunityRecord = serde_json::from_value(raw.clone()).unwrap();
        quote.validate_research().unwrap();
        assert_eq!(quote.session_id, id);
        assert_eq!(quote.experiment_id, experiment);
        assert_eq!(raw["schema_version"], "1.1.0");
        assert_eq!(raw["source_kind"], "SYNTHETIC_FIXTURE");
        assert_eq!(raw["dataset_origin"], "MANUALLY_CONSTRUCTED");
        assert_eq!(raw["evidence_label"], "CANDIDATE");
        assert_eq!(raw["quoted_output_minor"], "9963");
        assert!(raw["net_after_explicit_costs_minor"].is_null());
        assert_eq!(raw["simulation_status"], "NOT_RUN");
        assert!(raw["transaction_id"].is_null());
        assert!(raw["execution_plan_digest"].is_null());
        assert!(
            raw["eligibility_checks"]
                .as_object()
                .unwrap()
                .values()
                .all(|value| value == false)
        );
        assert!(
            quote
                .route
                .iter()
                .all(|leg| leg.pool_id.starts_with("fixture:pool-base:"))
        );
        received.insert(quote.opportunity_id.clone());
        let detail = local_http(
            address,
            "GET",
            &format!("/v1/decisions/{}", quote.opportunity_id),
            &auth,
            None,
        )
        .unwrap();
        assert_eq!(detail.status, 200);
        let durable = page
            .items
            .iter()
            .find(|stored| stored.trace.observation_id == quote.opportunity_id)
            .unwrap();
        assert_eq!(detail.body, serde_json::to_value(durable).unwrap());
    }
    assert_eq!(received, expected);
    let captured = local_http(
        address,
        "GET",
        &format!("/v1/opportunities?session_id={id}&source_kind=CAPTURED_MARKET_DATA"),
        &auth,
        None,
    )
    .unwrap();
    assert_eq!(captured.status, 200);
    assert!(captured.body["items"].as_array().unwrap().is_empty());
    let coverage = local_http(
        address,
        "GET",
        &format!("/v1/decision-coverage?session_id={id}"),
        &auth,
        None,
    )
    .unwrap();
    assert_eq!(coverage.status, 200);
    assert_eq!(
        coverage.body["raw_observations"],
        expected_decisions.to_string()
    );
    assert_eq!(
        coverage.body["quoted_candidates"],
        if chain_stale { "0" } else { "2" }
    );
    assert!(coverage.body["eligible_attempts"].is_null());
    assert!(coverage.body["reconciled_transactions"].is_null());
    if chain_stale {
        assert_eq!(coverage.body["data_unavailable"], "1");
        assert_eq!(coverage.body["no_route"], "0");
        let opportunities = local_http(
            address,
            "GET",
            &format!("/v1/opportunities?session_id={id}"),
            &auth,
            None,
        )
        .unwrap();
        assert_eq!(opportunities.status, 200);
        assert!(opportunities.body["items"].as_array().unwrap().is_empty());
    }
    assert_eq!(coverage.body["execution_accounting_available"], false);
    assert_eq!(coverage.body["collection_completeness"], "UNKNOWN");
    drop(api);
    rpc.finish();
    drop(worker);
    let log = fs::read_to_string(root.join("worker.log")).unwrap();
    assert!(!log.contains(&rpc.endpoint));
    assert!(!log.contains(&database));
    assert!(
        !fs::read_to_string(root.join("api.log"))
            .unwrap()
            .contains(&secret)
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn provider_failure_is_redacted_and_killed_collection_stays_unresolved() {
    let database = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL required; collection process evidence must not silently skip");
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let pool = sqlx::PgPool::connect(&database).await.unwrap();
    let root = std::env::temp_dir().join(format!("arb-failed-collection-{}", Uuid::new_v4()));
    let capture_root = root.join("captures");
    fs::create_dir_all(&capture_root).unwrap();
    let registry = include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json");
    let config = test_config(capture_root.to_str().unwrap(), registry, "OBSERVE");
    let validated = ValidatedConfig::from_toml(&config).unwrap();
    let config_path = root.join("config.toml");
    let registry_path = root.join("registry.json");
    fs::write(&config_path, &config).unwrap();
    fs::write(&registry_path, registry).unwrap();
    let operator = format!("failed-collection-{}", Uuid::new_v4());
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
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: validated.digest().into(),
                experiment_id: "synthetic-provider-outage".into(),
                strategy_ids: validated.strategy_ids().to_vec(),
            },
        )
        .await
        .unwrap();
    let id = session.session_id;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!(
        "http://{}/provider-secret-sentinel",
        listener.local_addr().unwrap()
    );
    let requests = Arc::new(AtomicUsize::new(0));
    let release_error = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let (server_requests, server_release, server_done) = (
        Arc::clone(&requests),
        Arc::clone(&release_error),
        Arc::clone(&done),
    );
    let server = std::thread::spawn(move || {
        while !server_done.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /provider-secret-sentinel "));
            let mut length = 0;
            let mut total = line.len();
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                total += line.len();
                assert!(total <= 16384);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            assert!(length > 0 && length <= 65536);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(request["method"], "eth_chainId");
            let round = server_requests.fetch_add(1, Ordering::SeqCst) + 1;
            let deadline = Instant::now() + Duration::from_secs(10);
            while !server_done.load(Ordering::SeqCst)
                && (round != 1 || !server_release.load(Ordering::SeqCst))
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            if round == 1 && !server_done.load(Ordering::SeqCst) {
                let body = "provider-response-secret-sentinel";
                write!(stream,"HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        }
    });
    let log = fs::File::create(root.join("worker.log")).unwrap();
    let mut worker = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_research-worker"))
            .env("ARB_WORKER_CONFIG", &config_path)
            .env("ARB_POOL_REGISTRY", &registry_path)
            .env("ARB_OPERATOR_ID", &operator)
            .env("ARB_SESSION_ID", &id)
            .env("TEST_DATABASE_URL", &database)
            .env("TEST_WORKER_RPC", &endpoint)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    wait_until(async || requests.load(Ordering::SeqCst) == 1, 10).await;
    let first: String = sqlx::query_scalar("SELECT attempt_id FROM collection_attempts WHERE session_id=$1 AND purpose='READINESS' AND outcome='IN_PROGRESS'")
        .bind(&id).fetch_one(&pool).await.unwrap();
    release_error.store(true, Ordering::SeqCst);
    wait_until(
        async || {
            sqlx::query_scalar::<_, String>(
                "SELECT outcome FROM collection_attempts WHERE attempt_id=$1",
            )
            .bind(&first)
            .fetch_one(&pool)
            .await
            .unwrap()
                == "ACQUISITION_FAILED"
        },
        5,
    )
    .await;
    let failed: (String,i32,i64,bool) = sqlx::query_as("SELECT reason,captured_pools,decision_rows,finished_at IS NOT NULL FROM collection_attempts WHERE attempt_id=$1")
        .bind(&first).fetch_one(&pool).await.unwrap();
    assert_eq!(failed, ("PROVIDER_UNAVAILABLE".into(), 0, 0, true));
    wait_until(async || requests.load(Ordering::SeqCst) == 2, 10).await;
    let unfinished: String = sqlx::query_scalar(
        "SELECT attempt_id FROM collection_attempts WHERE session_id=$1 AND outcome='IN_PROGRESS'",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(unfinished, first);
    worker.0.kill().unwrap();
    worker.0.wait().unwrap();
    done.store(true, Ordering::SeqCst);
    server.join().unwrap();
    let unresolved: (String,bool,bool,i64) = sqlx::query_as("SELECT outcome,finished_at IS NULL,reason IS NULL,decision_rows FROM collection_attempts WHERE attempt_id=$1")
        .bind(&unfinished).fetch_one(&pool).await.unwrap();
    assert_eq!(
        unresolved,
        ("IN_PROGRESS".into(), true, true, 0),
        "killed collection is unresolved evidence, not an inferred provider failure"
    );
    let persisted: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(c)::text FROM collection_attempts c WHERE session_id=$1",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let log = fs::read_to_string(root.join("worker.log")).unwrap();
    for text in persisted
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(log.as_str()))
    {
        for secret in [
            &endpoint,
            &database,
            "provider-secret-sentinel",
            "provider-response-secret-sentinel",
        ] {
            assert!(
                !text.contains(secret),
                "collection evidence must not copy provider secrets"
            );
        }
    }
    let admitted: i64 =
        sqlx::query_scalar("SELECT count(*) FROM decision_traces WHERE session_id=$1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(admitted, 0);
    assert_eq!(fs::read_dir(&capture_root).unwrap().count(), 0);
    drop(worker);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn canonical_batch_failure_admits_no_partial_captures_or_decisions() {
    let database = std::env::var("TEST_DATABASE_URL").expect(
        "TEST_DATABASE_URL required; worker batch rejection evidence must not silently skip",
    );
    let store = Store::connect(&database).await.unwrap();
    store.migrate().await.unwrap();
    let pool = sqlx::PgPool::connect(&database).await.unwrap();
    let root = std::env::temp_dir().join(format!("arb-context-failure-{}", Uuid::new_v4()));
    let capture_root = root.join("captures");
    fs::create_dir_all(&capture_root).unwrap();
    let (registry, transcripts) = two_pool_fixture();
    // Readiness succeeds. Subsequent research batches decode both pools but fail
    // the single final canonical check, so no partial batch may be persisted.
    let config = two_pool_config(capture_root.to_str().unwrap(), &registry);
    let validated = ValidatedConfig::from_toml(&config).unwrap();
    let config_path = root.join("config.toml");
    let registry_path = root.join("registry.json");
    fs::write(&config_path, &config).unwrap();
    fs::write(&registry_path, &registry).unwrap();
    let operator = format!("context-failure-{}", Uuid::new_v4());
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
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: validated.digest().into(),
                experiment_id: "manual-canonical-batch-failure".into(),
                strategy_ids: validated.strategy_ids().to_vec(),
            },
        )
        .await
        .unwrap();
    let id = session.session_id;
    let mut rpc = PairRpcGuard::with_canonical_failure_after(transcripts, Some(1));
    let log = fs::File::create(root.join("worker.log")).unwrap();
    let mut worker = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_research-worker"))
            .env("ARB_WORKER_CONFIG", &config_path)
            .env("ARB_POOL_REGISTRY", &registry_path)
            .env("ARB_OPERATOR_ID", &operator)
            .env("ARB_SESSION_ID", &id)
            .env("TEST_DATABASE_URL", &database)
            .env("TEST_WORKER_RPC", &rpc.endpoint)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let diagnostics = async || {
        let session = store.get_session(&operator, &id).await.unwrap();
        let outcomes: Vec<(String, Option<String>, i64)> = sqlx::query_as("SELECT outcome,reason,decision_rows FROM collection_attempts WHERE session_id=$1 ORDER BY attempt_id LIMIT 20")
            .bind(&id).fetch_all(&pool).await.unwrap();
        let mut log = String::new();
        fs::File::open(root.join("worker.log"))
            .unwrap()
            .take(16384)
            .read_to_string(&mut log)
            .unwrap();
        format!(
            "state={}, desired_revision={}, applied_revision={}, outstanding={}; collection outcomes (at most 20): {outcomes:?}; worker log (at most 16 KiB): {log}",
            session.observed_state,
            session.desired_revision,
            session.applied_revision,
            session.outstanding_attempts
        )
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if store
            .get_session(&operator, &id)
            .await
            .unwrap()
            .observed_state
            == "STOPPED"
        {
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "canonical batch stage=recovery expected STOPPED within 10s; {}",
                diagnostics().await
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    store
        .issue_command(
            &operator,
            &id,
            "start",
            NewCommand {
                action: "START".into(),
                expected_revision: "0".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let completed: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='ACQUISITION_FAILED' AND reason='INPUT_VALIDATION_FAILED' AND captured_pools=0 AND decision_rows=0")
            .bind(&id).fetch_one(&pool).await.unwrap();
        if completed > 0 {
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "canonical batch stage=acquisition expected rejected batch with no captures or decisions within 20s; {}",
                diagnostics().await
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Readiness loss deliberately faults and terminates the worker. A STOP
    // issued after that exit cannot be acknowledged by the departed process.
    // Require the existing durable fault/generation fence and bounded exit.
    let failed_generation: i64 = sqlx::query_scalar("SELECT generation FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='ACQUISITION_FAILED' ORDER BY attempt_id LIMIT 1")
        .bind(&id).fetch_one(&pool).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state: (String, bool, i64) = sqlx::query_as("SELECT observed_state,local_fence,generation FROM research_sessions WHERE session_id=$1")
            .bind(&id).fetch_one(&pool).await.unwrap();
        let exit = worker.0.try_wait().unwrap();
        if state.0 == "FAULTED" && state.1 && state.2 > failed_generation && exit.is_some() {
            assert_eq!(
                exit.unwrap().code(),
                Some(2),
                "readiness loss must report failure"
            );
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "canonical batch stage=fault-exit expected durable FAULTED, closed fence, newer generation and process exit within 5s; actual={state:?}, failed_generation={failed_generation}, exit={exit:?}; {}",
                diagnostics().await
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let session = store.get_session(&operator, &id).await.unwrap();
    assert_eq!(session.outstanding_attempts, 0);
    assert!(!session.execution_authorized);
    let stop = store
        .issue_command(
            &operator,
            &id,
            "stop-after-fault",
            NewCommand {
                action: "STOP".into(),
                expected_revision: session.desired_revision,
                reason: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        stop.status, "PENDING",
        "an exited worker cannot acknowledge STOP"
    );
    assert!(stop.applied_at.is_none());
    let persisted_stop = store
        .get_command(&operator, &stop.command_id)
        .await
        .unwrap();
    assert_eq!(persisted_stop.status, "PENDING");
    assert!(persisted_stop.applied_at.is_none());
    let captures: i64 =
        sqlx::query_scalar("SELECT count(*) FROM capture_admissions WHERE session_id=$1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(captures, 0, "no pool from a failed batch may be admitted");
    let decisions = store
        .list_decision_traces(&operator, &id, None, 100)
        .await
        .unwrap();
    assert!(decisions.next_cursor.is_none());
    assert!(decisions.items.is_empty());
    let completed: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='DECISIONS_RECORDED'")
        .bind(&id).fetch_one(&pool).await.unwrap();
    assert_eq!(completed, 0);
    // Exactly the original two readiness bundles remain. A failed batch must
    // not write even its successfully decoded first pool's raw artifact.
    assert_eq!(fs::read_dir(&capture_root).unwrap().count(), 2);
    rpc.finish();
    drop(worker);
    fs::remove_dir_all(root).unwrap();
}
