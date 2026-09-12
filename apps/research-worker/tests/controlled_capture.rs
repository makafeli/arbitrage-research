//! Real process + PostgreSQL + loopback HTTP test. All RPC content is manually
//! constructed transcript data, never evidence of market or strategy performance.
use arb_adapter_api::RpcRecord;
use arb_capture::digest;
use arb_config::ValidatedConfig;
use arb_storage::{NewCommand, NewSession, Store};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
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
    let pool = sqlx::PgPool::connect(&database).await.unwrap();
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
