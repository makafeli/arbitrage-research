//! Real executable, loopback JSON-RPC and PostgreSQL; never a market provider.
use arb_storage::Store;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn hash(n: u64) -> String {
    format!("0x{n:064x}")
}
fn block(n: u64) -> Value {
    json!({"number":format!("0x{n:x}"),"hash":hash(n),"parentHash":hash(n-1),"timestamp":"0x64"})
}
struct Fixture {
    endpoint: String,
    root: PathBuf,
    operator: String,
    tip: Arc<AtomicU64>,
    calls: Arc<AtomicUsize>,
    observed: Arc<Mutex<Vec<Value>>>,
    filter_logs: Arc<Mutex<Value>>,
    reorg: Arc<AtomicBool>,
    stalled: Arc<AtomicBool>,
    recovery_stalled: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("arb-ingestion-{unique}"));
        std::fs::create_dir(&root).unwrap();
        let mut pools: Value = serde_json::from_str(include_str!(
            "../../../crates/arb-evm/tests/fixtures/batch-registries.json"
        ))
        .unwrap();
        let code = format!("sha256:{}", hex::encode(Sha256::digest([0x60, 0x00])));
        for pool in pools.as_array_mut().unwrap() {
            pool["pool_runtime_sha256"] = json!(code);
            pool["factory_runtime_sha256"] = json!(code);
        }
        std::fs::write(root.join("registries.json"), pools.to_string()).unwrap();
        let pool = pools[0]["pool"].as_str().unwrap().to_owned();
        let tip = Arc::new(AtomicU64::new(100));
        let calls = Arc::new(AtomicUsize::new(0));
        let reorg = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let stalled = Arc::new(AtomicBool::new(false));
        let stall = stalled.clone();
        let recovery_stalled = Arc::new(AtomicBool::new(false));
        let recovery_stall = recovery_stalled.clone();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let filter_logs = Arc::new(Mutex::new(json!([])));
        let (o, notifications) = (observed.clone(), filter_logs.clone());
        let (t, c, r, d) = (tip.clone(), calls.clone(), reorg.clone(), done.clone());
        let handle = thread::spawn(move || {
            while !d.load(Ordering::SeqCst) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut piece = [0; 4096];
                let split = loop {
                    let n = socket.read(&mut piece).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&piece[..n]);
                    assert!(bytes.len() < 65536);
                    if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break index + 4;
                    }
                };
                let header = String::from_utf8_lossy(&bytes[..split]).to_lowercase();
                let length: usize = header
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                assert!(length < 65536);
                while bytes.len() < split + length {
                    let n = socket.read(&mut piece).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&piece[..n]);
                }
                let req: Value = serde_json::from_slice(&bytes[split..split + length]).unwrap();
                c.fetch_add(1, Ordering::SeqCst);
                o.lock().unwrap().push(req.clone());
                let limit = std::time::Instant::now() + Duration::from_secs(6);
                while (stall.load(Ordering::SeqCst)
                    || (req["method"] == "eth_getLogs" && recovery_stall.load(Ordering::SeqCst)))
                    && !d.load(Ordering::SeqCst)
                    && std::time::Instant::now() < limit
                {
                    thread::sleep(Duration::from_millis(2));
                }
                let result = match req["method"].as_str().unwrap() {
                    "eth_chainId" => json!("0x2105"),
                    "eth_newBlockFilter" => json!("0x1"),
                    "eth_newFilter" => json!("0x2"),
                    "eth_getFilterChanges" => {
                        if req["params"][0] == "0x1" {
                            json!([])
                        } else {
                            notifications.lock().unwrap().clone()
                        }
                    }
                    "eth_uninstallFilter" => json!(true),
                    "eth_getCode" => json!("0x6000"),
                    "eth_getBlockByNumber" => {
                        let selector = req["params"][0].as_str().unwrap();
                        let n = if selector == "finalized" {
                            t.load(Ordering::SeqCst)
                        } else {
                            u64::from_str_radix(selector.trim_start_matches("0x"), 16).unwrap()
                        };
                        let mut h = block(n);
                        if r.load(Ordering::SeqCst) {
                            h["hash"] = json!(hash(9999));
                        }
                        h
                    }
                    "eth_getLogs" => {
                        let h = req["params"][0]["blockHash"].as_str().unwrap();
                        let n = u64::from_str_radix(h.trim_start_matches("0x"), 16).unwrap();
                        json!([{"address":pool,"blockNumber":format!("0x{n:x}"),"blockHash":h,
                            "transactionHash":hash(900+n),"transactionIndex":"0x0","logIndex":"0x0","removed":false,
                            "topics":[arb_evm::events::INITIALIZE],"data":format!("0x{:064x}{:064x}",1_u128<<96,0)}])
                    }
                    other => panic!("unexpected method: {other}"),
                };
                let body = json!({"jsonrpc":"2.0","id":req["id"],"result":result}).to_string();
                // Process-stop tests may deliberately close the peer before
                // its response is written. The client still verifies success.
                if let Err(error) = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                ) {
                    assert!(matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                    ));
                }
            }
        });
        Self {
            endpoint,
            root,
            operator: format!("executable-{unique}"),
            tip,
            calls,
            observed,
            filter_logs,
            reorg,
            stalled,
            recovery_stalled,
            done,
            thread: Some(handle),
        }
    }
    fn command(&self, action: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_base-ingest"));
        command
            .arg(action)
            .env("ARB_INGEST_OPERATOR_ID", &self.operator)
            .env("ARB_INGEST_STREAM_ID", "stream")
            .env(
                "ARB_INGEST_DATABASE_URL",
                std::env::var("TEST_DATABASE_URL").expect("required disposable PostgreSQL"),
            )
            .env(
                "ARB_INGEST_REGISTRY_FILE",
                self.root.join("registries.json"),
            )
            .env("ARB_BASE_RPC_URL", &self.endpoint)
            .env("ARB_INGEST_MAX_POLLS", "1")
            .env("ARB_INGEST_POLL_MS", "1000")
            .env("ARB_RPC_MIN_INTERVAL_MS", "0");
        command
    }
    fn run(&self, action: &str) -> Output {
        self.command(action).output().unwrap()
    }
    fn success(&self, action: &str) -> Value {
        let output = self.run(action);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread.take() {
            h.join().unwrap();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
fn default_and_invalid_arguments_never_start_services_or_expose_credentials() {
    let out = Command::new(env!("CARGO_BIN_EXE_base-ingest"))
        .env("ARB_BASE_RPC_URL", "SECRET_MUST_NOT_APPEAR")
        .env("ARB_INGEST_DATABASE_URL", "SECRET_MUST_NOT_APPEAR")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["status"],
        "NOT_STARTED"
    );
    let out = Command::new(env!("CARGO_BIN_EXE_base-ingest"))
        .args(["--run", "SECRET_MUST_NOT_APPEAR"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("SECRET_MUST_NOT_APPEAR"));
}
#[tokio::test]
async fn real_process_restart_resumes_saved_events_then_halts_on_changed_history() {
    let fixture = Fixture::new();
    fixture.success("--migrate");
    assert_eq!(fixture.success("--initialize")["checkpoint"]["number"], 100);
    let count = fixture.calls.load(Ordering::SeqCst);
    assert!(!fixture.run("--initialize").status.success());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), count);
    fixture.tip.store(101, Ordering::SeqCst);
    let first = fixture.success("--run");
    assert_eq!(first["status"], "BATCH_COMMITTED");
    assert_eq!(first["revision"], "1");
    assert_eq!(first["dataset_origin"], "MANUALLY_CONSTRUCTED");
    assert_eq!(first["execution_authorized"], false);
    // A distinct executable process starts from PostgreSQL, not an in-memory cursor.
    fixture.tip.store(102, Ordering::SeqCst);
    let second = fixture.success("--run");
    assert_eq!(second["checkpoint"]["number"], 102);
    assert_eq!(second["revision"], "2");
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let batches = store
        .ingestion_batches(&fixture.operator, "stream", 0, 16)
        .await
        .unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0]["blocks"][0]["logs"].as_array().unwrap().len(), 1);
    assert_eq!(batches[1]["from_checkpoint"]["number"], 101);
    let count = fixture.calls.load(Ordering::SeqCst);
    fixture.success("--status");
    assert_eq!(fixture.calls.load(Ordering::SeqCst), count);
    fixture.reorg.store(true, Ordering::SeqCst);
    assert!(!fixture.run("--run").status.success());
    let halted = fixture.success("--status");
    assert_eq!(halted["state"], "HALTED");
    assert_eq!(halted["checkpoint"]["number"], 102);
    let count = fixture.calls.load(Ordering::SeqCst);
    assert!(!fixture.run("--run").status.success());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), count);
}
#[tokio::test]
async fn no_new_blocks_repeated_polling_and_changed_registry_are_explicit() {
    let fixture = Fixture::new();
    fixture.success("--migrate");
    fixture.success("--initialize");
    let out = fixture
        .command("--run")
        .env("ARB_INGEST_MAX_POLLS", "2")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|r| r["status"] == "NO_NEW_FINALIZED_BLOCKS" && r["revision"] == "0")
    );
    let path = fixture.root.join("registries.json");
    let mut pools: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    pools[0]["qualification_reference"] = json!("changed-qualified-scope");
    std::fs::write(path, pools.to_string()).unwrap();
    let count = fixture.calls.load(Ordering::SeqCst);
    let out = fixture.run("--run");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("REGISTRY_OR_ORIGIN_CHANGED"));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), count);
}

#[cfg(unix)]
#[tokio::test]
async fn interrupt_during_an_actual_rpc_prevents_checkpoint_publication() {
    use std::process::Stdio;
    let fixture = Fixture::new();
    fixture.success("--migrate");
    fixture.success("--initialize");
    fixture.tip.store(101, Ordering::SeqCst);
    fixture.stalled.store(true, Ordering::SeqCst);
    let before = fixture.calls.load(Ordering::SeqCst);
    let mut child = fixture
        .command("--run")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while fixture.calls.load(Ordering::SeqCst) == before {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            panic!("worker never entered RPC");
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    thread::sleep(Duration::from_millis(100));
    fixture.stalled.store(false, Ordering::SeqCst);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            panic!("worker did not cancel");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CANCELLED"));
    let status = fixture.success("--status");
    assert_eq!(status["revision"], "0");
    assert_eq!(status["checkpoint"]["number"], 100);
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert!(
        store
            .ingestion_batches(&fixture.operator, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
}

#[cfg(unix)]
#[path = "ingestion/shutdown.rs"]
mod shutdown;

#[path = "ingestion/filters.rs"]
mod filter_tests;
