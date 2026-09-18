//! Real process/PostgreSQL with synthetic HTTP state. No provider credentials.
use super::*;
use arb_adapter_api::ReadMethod;
use arb_storage::{IngestionBinding, IngestionHead};
use sqlx::PgPool;

fn header(offset: i64) -> Value {
    let hash = match offset {
        -1 => format!("0x{}", "bb".repeat(32)),
        0 => format!("0x{}", "aa".repeat(32)),
        n => format!("0x{:064x}", 4000 + n),
    };
    let parent = if offset == -1 {
        format!("0x{}", "ee".repeat(32))
    } else {
        header(offset - 1)["hash"].as_str().unwrap().to_string()
    };
    json!({"number":format!("0x{:x}",100+offset),"hash":hash,
        "parentHash":parent,"timestamp":format!("0x{:x}",(1+offset*2).max(0))})
}
fn stored(offset: i64) -> IngestionHead {
    let h = arb_evm::events::BlockHeader::from_rpc(&header(offset)).unwrap();
    IngestionHead {
        number: h.number,
        hash: h.hash,
        parent_hash: h.parent_hash,
        timestamp_seconds: h.timestamp_seconds,
    }
}
struct Provider {
    endpoint: String,
    tip: Arc<AtomicUsize>,
    hold: Arc<AtomicBool>,
    held: Arc<AtomicBool>,
    malformed: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Provider {
    fn start(transcripts: Vec<Vec<RpcRecord>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let tip = Arc::new(AtomicUsize::new(0));
        let hold = Arc::new(AtomicBool::new(false));
        let held = Arc::new(AtomicBool::new(false));
        let malformed = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let (tip_s, hold_s, held_s, malformed_s, calls_s, done_s) = (
            tip.clone(),
            hold.clone(),
            held.clone(),
            malformed.clone(),
            calls.clone(),
            done.clone(),
        );
        let thread = std::thread::spawn(move || {
            let records: Vec<_> = transcripts.into_iter().flatten().collect();
            while !done_s.load(Ordering::SeqCst) {
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
                let length = length.unwrap();
                assert!(length <= 64 * 1024);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request: Value = serde_json::from_slice(&body).unwrap();
                calls_s.fetch_add(1, Ordering::SeqCst);
                let method = request["method"].as_str().unwrap();
                let params = &request["params"];
                let result = if method == "eth_getBlockByNumber" {
                    let selector = params[0].as_str().unwrap();
                    assert_eq!(params[1], false);
                    let offset = if selector == "finalized" {
                        tip_s.load(Ordering::SeqCst) as i64
                    } else {
                        i64::from_str_radix(selector.strip_prefix("0x").unwrap(), 16).unwrap() - 100
                    };
                    assert!((-1..=40).contains(&offset));
                    header(offset)
                } else if method == "eth_getLogs" {
                    held_s.store(true, Ordering::SeqCst);
                    let until = Instant::now() + Duration::from_secs(4);
                    while hold_s.load(Ordering::SeqCst)
                        && !done_s.load(Ordering::SeqCst)
                        && Instant::now() < until
                    {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    assert!(params[0]["address"].as_array().unwrap().len() == 2);
                    let tip_now = tip_s.load(Ordering::SeqCst) as i64;
                    assert!((0..=tip_now).any(|k| header(k)["hash"] == params[0]["blockHash"]));
                    if malformed_s.load(Ordering::SeqCst) {
                        json!([{"private":"provider diagnostic must not escape"}])
                    } else {
                        json!([])
                    }
                } else {
                    let mut comparable = params.clone();
                    if method == "eth_call" || method == "eth_getCode" {
                        assert_eq!(params[1]["requireCanonical"], true);
                        let tip_now = tip_s.load(Ordering::SeqCst) as i64;
                        assert!((0..=tip_now).any(|k| header(k)["hash"] == params[1]["blockHash"]));
                        comparable[1]["blockHash"] = header(0)["hash"].clone();
                    }
                    let record = records
                        .iter()
                        .find(|r| r.method.wire_name() == method && r.params == comparable)
                        .unwrap_or_else(|| {
                            panic!("unexpected read-only fixture request: {method} {comparable}")
                        });
                    serde_json::from_str::<Value>(&record.response).unwrap()["result"].clone()
                };
                let body = json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            }
        });
        Self {
            endpoint,
            tip,
            hold,
            held,
            malformed,
            calls,
            done,
            thread: Some(thread),
        }
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.hold.store(false, Ordering::SeqCst);
        self.done.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
struct Fixture {
    pool: PgPool,
    store: Store,
    operator: String,
    session: String,
    root: std::path::PathBuf,
    database: String,
    provider: Provider,
}
impl Fixture {
    async fn new(seed_at_target: bool) -> Self {
        let database = std::env::var("TEST_DATABASE_URL").expect("real PostgreSQL required");
        let pool = PgPool::connect(&database).await.unwrap();
        let store = Store::from_pool(pool.clone());
        store.migrate().await.unwrap();
        let root = std::env::temp_dir().join(format!("arb-managed-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("captures")).unwrap();
        let (registry, transcripts) = two_pool_fixture();
        let config = two_pool_config(root.join("captures").to_str().unwrap(), &registry);
        let validated = ValidatedConfig::from_toml(&config).unwrap();
        let doc = arb_registry::RegistryDocument::from_bytes(
            &registry,
            arb_domain::NetworkId::BaseMainnet,
        )
        .unwrap();
        let pools: Vec<_> = doc
            .pools()
            .iter()
            .map(|p| match p {
                arb_registry::PoolRegistry::Base(p) => p.clone(),
                _ => panic!("Base only"),
            })
            .collect();
        let mut addresses: Vec<_> = pools.iter().map(|p| p.pool.to_lowercase()).collect();
        addresses.sort();
        let binding = IngestionBinding {
            schema_version: 1,
            network_id: "base-mainnet".into(),
            registry_digest: digest(&serde_json::to_vec(&pools).unwrap()),
            abi_source_commit: arb_evm::SOURCE_COMMIT.into(),
            dataset_origin: "MANUALLY_CONSTRUCTED".into(),
            pool_addresses: addresses,
        };
        let operator = format!("managed-{}", Uuid::new_v4());
        store
            .create_ingestion(
                &operator,
                "source",
                &binding,
                &stored(if seed_at_target { 0 } else { -1 }),
            )
            .await
            .unwrap();
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
                "managed",
                NewSession {
                    network_id: "base-mainnet".into(),
                    mode: "OBSERVE".into(),
                    configuration_digest: validated.digest().into(),
                    experiment_id: "synthetic-managed-source".into(),
                    strategy_ids: validated.strategy_ids().to_vec(),
                },
            )
            .await
            .unwrap()
            .session_id;
        fs::write(root.join("config.toml"), config).unwrap();
        fs::write(root.join("registry.json"), registry).unwrap();
        Self {
            pool,
            store,
            operator,
            session,
            root,
            database,
            provider: Provider::start(transcripts),
        }
    }
    fn start(&self) -> ChildGuard {
        let output = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("worker.log"))
            .unwrap();
        ChildGuard(
            Command::new(env!("CARGO_BIN_EXE_research-worker"))
                .env("ARB_WORKER_CONFIG", self.root.join("config.toml"))
                .env("ARB_POOL_REGISTRY", self.root.join("registry.json"))
                .env("ARB_OPERATOR_ID", &self.operator)
                .env("ARB_SESSION_ID", &self.session)
                .env("TEST_WORKER_RPC", &self.provider.endpoint)
                .env("TEST_DATABASE_URL", &self.database)
                .env("ARB_BASE_INGESTION_STREAM", "source")
                .env("ARB_BASE_MANAGED_INGESTION", "true")
                .stdout(Stdio::from(output.try_clone().unwrap()))
                .stderr(Stdio::from(output))
                .spawn()
                .unwrap(),
        )
    }
    async fn ready(&self) {
        wait_until(async||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM collection_attempts WHERE session_id=$1 AND outcome='READINESS_COMPLETED')")
            .bind(&self.session).fetch_one(&self.pool).await.unwrap(),30).await;
        // The next control tick installs readiness after the completed batch.
        tokio::time::sleep(Duration::from_millis(350)).await;
    }
    async fn start_research(&self) {
        self.store
            .issue_command(
                &self.operator,
                &self.session,
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
                self.store
                    .decision_coverage(&self.operator, &self.session)
                    .await
                    .unwrap()
                    .quoted_candidates
                    .parse::<u64>()
                    .unwrap()
                    >= 2
            },
            30,
        )
        .await;
    }
    async fn quoted(&self) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM decision_traces WHERE session_id=$1 AND result_status='QUOTED'",
        )
        .bind(&self.session)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
    async fn exited(&self, child: &mut ChildGuard) -> std::process::ExitStatus {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < until,
                "worker did not exit: {}",
                fs::read_to_string(self.root.join("worker.log")).unwrap()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn managed_source_persists_before_quotes_and_preserves_original_quote_replay() {
    let f = Fixture::new(false).await;
    let child = f.start();
    f.ready().await;
    let cursor = f
        .store
        .ingestion_cursor(&f.operator, "source")
        .await
        .unwrap();
    assert_eq!(cursor.revision, 1);
    assert_eq!(cursor.checkpoint, stored(0));
    f.start_research().await;
    let tracked:i64=sqlx::query_scalar("SELECT count(*) FROM decision_ingestion_validity WHERE session_id=$1 AND continuity_status='NO_KNOWN_INVALIDATION'")
        .bind(&f.session).fetch_one(&f.pool).await.unwrap();
    assert!(tracked >= 2);
    let paths: Vec<String> =
        sqlx::query_scalar("SELECT artifact_path FROM capture_admissions WHERE session_id=$1")
            .bind(&f.session)
            .fetch_all(&f.pool)
            .await
            .unwrap();
    let (registry, _) = two_pool_fixture();
    let doc =
        arb_registry::RegistryDocument::from_bytes(&registry, arb_domain::NetworkId::BaseMainnet)
            .unwrap();
    let pools: Vec<_> = doc
        .pools()
        .iter()
        .map(|p| match p {
            arb_registry::PoolRegistry::Base(p) => p.clone(),
            _ => panic!(),
        })
        .collect();
    for path in paths {
        let path = std::path::Path::new(&path);
        let records: Vec<RpcRecord> =
            serde_json::from_slice(&fs::read(path.join("rpc.json")).unwrap()).unwrap();
        assert!(!records.iter().any(|r| r.method == ReadMethod::EthGetLogs));
        let manifest: Value =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
        let snapshot: Value =
            serde_json::from_slice(&fs::read(path.join("snapshot.json")).unwrap()).unwrap();
        let mut replay = TranscriptRpc::new(records);
        let reproduced = arb_evm::capture_pools(
            &mut replay,
            &pools,
            manifest["created_at_ms"].as_u64().unwrap(),
        )
        .unwrap();
        replay.finish().unwrap();
        assert!(
            reproduced
                .iter()
                .any(|p| serde_json::to_value(p).unwrap() == snapshot)
        );
    }
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .revision,
        1
    );
    drop(child);
}

#[tokio::test]
async fn managed_stop_is_acknowledged_during_log_recovery_and_late_quotes_are_fenced() {
    let f = Fixture::new(false).await;
    let child = f.start();
    f.ready().await;
    f.start_research().await;
    let quoted = f.quoted().await;
    f.provider.held.store(false, Ordering::SeqCst);
    f.provider.hold.store(true, Ordering::SeqCst);
    f.provider.tip.store(1, Ordering::SeqCst);
    wait_until(async || f.provider.held.load(Ordering::SeqCst), 15).await;
    let stop = f
        .store
        .issue_command(
            &f.operator,
            &f.session,
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
            f.store
                .get_command(&f.operator, &stop.command_id)
                .await
                .unwrap()
                .status
                == "APPLIED"
        },
        2,
    )
    .await;
    assert!(f.provider.hold.load(Ordering::SeqCst));
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .checkpoint,
        stored(0)
    );
    f.provider.hold.store(false, Ordering::SeqCst);
    wait_until(
        async || {
            f.store
                .ingestion_cursor(&f.operator, "source")
                .await
                .unwrap()
                .checkpoint
                == stored(1)
        },
        10,
    )
    .await;
    assert_eq!(f.quoted().await, quoted);
    assert_eq!(
        f.store
            .get_session(&f.operator, &f.session)
            .await
            .unwrap()
            .observed_state,
        "STOPPED"
    );
    drop(child);
}

#[tokio::test]
async fn malformed_recovery_halts_source_invalidates_history_and_restart_never_rearms() {
    let f = Fixture::new(false).await;
    let mut child = f.start();
    f.ready().await;
    f.start_research().await;
    let quoted = f.quoted().await;
    let artifacts = fs::read_dir(f.root.join("captures")).unwrap().count();
    f.provider.malformed.store(true, Ordering::SeqCst);
    f.provider.tip.store(1, Ordering::SeqCst);
    assert!(!f.exited(&mut child).await.success());
    let cursor = f
        .store
        .ingestion_cursor(&f.operator, "source")
        .await
        .unwrap();
    assert_eq!(cursor.state, "HALTED");
    assert_eq!(cursor.halt_reason.as_deref(), Some("CONTINUITY_LOST"));
    assert_eq!(cursor.checkpoint, stored(0));
    assert_eq!(
        f.store
            .get_session(&f.operator, &f.session)
            .await
            .unwrap()
            .observed_state,
        "FAULTED"
    );
    assert_eq!(f.quoted().await, quoted);
    assert_eq!(
        fs::read_dir(f.root.join("captures")).unwrap().count(),
        artifacts
    );
    let invalidated:i64=sqlx::query_scalar("SELECT count(*) FROM decision_ingestion_validity WHERE session_id=$1 AND continuity_status='INVALIDATED'")
        .bind(&f.session).fetch_one(&f.pool).await.unwrap();
    assert_eq!(invalidated, quoted);
    let before = f.provider.calls.load(Ordering::SeqCst);
    let mut restarted = f.start();
    assert!(!f.exited(&mut restarted).await.success());
    assert_eq!(f.provider.calls.load(Ordering::SeqCst), before);
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap(),
        cursor
    );
    assert!(
        !fs::read_to_string(f.root.join("worker.log"))
            .unwrap()
            .contains("provider diagnostic must not escape")
    );
}

#[tokio::test]
async fn a_seed_without_any_recovered_batch_does_not_open_research_admission() {
    let f = Fixture::new(true).await;
    let child = f.start();
    f.ready().await;
    let start = f
        .store
        .issue_command(
            &f.operator,
            &f.session,
            "start",
            NewCommand {
                action: "START".into(),
                expected_revision: "0".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    // The established control contract leaves START pending until readiness;
    // it must not fabricate APPLIED, reject the command, or publish a quote.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        f.store
            .get_command(&f.operator, &start.command_id)
            .await
            .unwrap()
            .status,
        "PENDING"
    );
    assert_eq!(
        f.store
            .get_session(&f.operator, &f.session)
            .await
            .unwrap()
            .observed_state,
        "STOPPED"
    );
    assert_eq!(f.quoted().await, 0);
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .revision,
        0
    );
    drop(child);
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_during_recovery_preserves_the_cursor_without_fabricating_a_source_fault() {
    let f = Fixture::new(false).await;
    let mut child = f.start();
    f.ready().await;
    f.provider.held.store(false, Ordering::SeqCst);
    f.provider.hold.store(true, Ordering::SeqCst);
    f.provider.tip.store(1, Ordering::SeqCst);
    wait_until(async || f.provider.held.load(Ordering::SeqCst), 15).await;
    let cursor = f
        .store
        .ingestion_cursor(&f.operator, "source")
        .await
        .unwrap();
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    wait_until(
        async || {
            fs::read_to_string(f.root.join("worker.log"))
                .unwrap()
                .contains("shutdown-fenced")
        },
        2,
    )
    .await;
    f.provider.hold.store(false, Ordering::SeqCst);
    assert!(f.exited(&mut child).await.success());
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap(),
        cursor
    );
    assert_eq!(f.quoted().await, 0);
}

/// A finalized step of 20 blocks exceeds `BackfillLimits::default().max_blocks`
/// (16), so the bounded walk must span two consecutive capture attempts instead
/// of halting the source. Neither attempt skips a block or raises the limit.
#[tokio::test]
async fn a_finalized_step_beyond_the_bounded_range_is_walked_over_consecutive_captures() {
    let f = Fixture::new(true).await;
    let mut child = f.start();
    f.ready().await;
    f.store
        .issue_command(
            &f.operator,
            &f.session,
            "start",
            NewCommand {
                action: "START".into(),
                expected_revision: "0".into(),
                reason: None,
            },
        )
        .await
        .unwrap();
    f.provider.tip.store(20, Ordering::SeqCst);
    wait_until(
        async || {
            f.store
                .ingestion_cursor(&f.operator, "source")
                .await
                .unwrap()
                .checkpoint
                == stored(16)
        },
        30,
    )
    .await;
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .state,
        "ACTIVE"
    );
    wait_until(
        async || {
            f.store
                .ingestion_cursor(&f.operator, "source")
                .await
                .unwrap()
                .checkpoint
                == stored(20)
        },
        30,
    )
    .await;
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .state,
        "ACTIVE"
    );
    wait_until(
        async || {
            f.store
                .get_session(&f.operator, &f.session)
                .await
                .unwrap()
                .observed_state
                == "RUNNING"
        },
        10,
    )
    .await;
    let batches = f
        .store
        .ingestion_batches(&f.operator, "source", 0, 16)
        .await
        .unwrap();
    assert_eq!(batches.len(), 2, "batches: {batches:?}");
    assert_eq!(batches[0]["through"]["number"].as_u64(), Some(116));
    assert_eq!(batches[0]["blocks"].as_array().unwrap().len(), 16);
    assert_eq!(batches[1]["through"]["number"].as_u64(), Some(120));
    assert_eq!(batches[1]["blocks"].as_array().unwrap().len(), 4);
    let log = fs::read_to_string(f.root.join("worker.log")).unwrap();
    let events: Vec<Value> = log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["event"] == "capture-written")
        .collect();
    assert!(
        events
            .iter()
            .any(|e| e["admission"] == "UNADMITTED_RAW_CAPTURE"
                && e["source_caught_up"] == false
                && e["source_lag_blocks"] == 4),
        "events: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e["admission"] == "ADMITTED_RAW_CAPTURE"
                && e["source_caught_up"] == true
                && e["source_lag_blocks"] == 0),
        "events: {events:?}"
    );

    // A second finalized step while RUNNING must be walked the same way: bounded,
    // never skipped, and no research admission until the source is fully caught
    // up again. This pins the production admission gate (`admit = source_ready
    // && batch.source_caught_up`) with a real running session, not just the
    // pre-RUNNING walk above.
    f.provider.tip.store(40, Ordering::SeqCst);
    wait_until(
        async || {
            f.store
                .ingestion_cursor(&f.operator, "source")
                .await
                .unwrap()
                .checkpoint
                == stored(36)
        },
        30,
    )
    .await;
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .state,
        "ACTIVE"
    );
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "worker exited while still walking the second bounded step"
    );
    wait_until(
        async || {
            f.store
                .ingestion_cursor(&f.operator, "source")
                .await
                .unwrap()
                .checkpoint
                == stored(40)
        },
        30,
    )
    .await;
    assert_eq!(
        f.store
            .ingestion_cursor(&f.operator, "source")
            .await
            .unwrap()
            .state,
        "ACTIVE"
    );
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "worker exited after the second bounded step reached its anchor"
    );

    // The finished RESEARCH-purpose collections created while the second step
    // was still short of its anchor must be ACQUISITION_FAILED /
    // ACQUISITION_UNAVAILABLE, read from the store, not only from the log.
    let unavailable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='ACQUISITION_FAILED' AND reason='ACQUISITION_UNAVAILABLE'",
    )
    .bind(&f.session)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert!(
        unavailable >= 1,
        "expected at least one ACQUISITION_FAILED/ACQUISITION_UNAVAILABLE research collection while the second step was still being walked"
    );
    let mismatched: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_attempts WHERE session_id=$1 AND purpose='RESEARCH' AND outcome='ACQUISITION_FAILED' AND reason IS DISTINCT FROM 'ACQUISITION_UNAVAILABLE'",
    )
    .bind(&f.session)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(
        mismatched, 0,
        "every ACQUISITION_FAILED research collection in this run must be ACQUISITION_UNAVAILABLE"
    );

    let log = fs::read_to_string(f.root.join("worker.log")).unwrap();
    let events: Vec<Value> = log
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["event"] == "capture-written")
        .collect();
    assert!(
        events
            .iter()
            .any(|e| e["admission"] == "UNADMITTED_RAW_CAPTURE"
                && e["source_caught_up"] == false
                && e["source_lag_blocks"] == 4),
        "events: {events:?}"
    );
    let admitted_caught_up = events
        .iter()
        .filter(|e| {
            e["admission"] == "ADMITTED_RAW_CAPTURE"
                && e["source_caught_up"] == true
                && e["source_lag_blocks"] == 0
        })
        .count();
    assert!(
        admitted_caught_up >= 2,
        "expected a new RESEARCH collection to admit after each finalized step reached its anchor: {events:?}"
    );
    drop(child);
}
