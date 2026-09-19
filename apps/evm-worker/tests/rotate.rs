//! `--rotate` executable tests: database-only, no RPC endpoint, no provider contact.
use arb_storage::{
    IngestionBinding, IngestionCursor, IngestionHalt, IngestionHead, NewSession, Store, StoreError,
};
use serde_json::{Value, json};
use std::{
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Disambiguates operator ids created in the same wall-clock instant, matching
/// the pattern already used by the sibling `ingestion.rs` fixture.
static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn head(number: u64) -> IngestionHead {
    IngestionHead {
        number,
        hash: format!("0x{number:064x}"),
        parent_hash: format!("0x{:064x}", number - 1),
        timestamp_seconds: number * 10,
    }
}

fn binding() -> IngestionBinding {
    IngestionBinding {
        schema_version: 1,
        network_id: "base-mainnet".into(),
        registry_digest: format!("sha256:{}", "a".repeat(64)),
        abi_source_commit: "b".repeat(40),
        dataset_origin: "MANUALLY_CONSTRUCTED".into(),
        pool_addresses: vec![format!("0x{}", "c".repeat(40))],
    }
}

fn batch(cursor: &IngestionCursor) -> Value {
    let through = head(cursor.checkpoint.number + 1);
    json!({
        "schema_version": 1,
        "network_id": cursor.binding.network_id,
        "registry_digest": cursor.binding.registry_digest,
        "abi_source_commit": cursor.binding.abi_source_commit,
        "pool_addresses": cursor.binding.pool_addresses,
        "from_checkpoint": cursor.checkpoint,
        "through": through,
        "blocks": [{"header": through, "logs": []}],
        "full_snapshot_required": true
    })
}

async fn seed(store: &Store, operator: &str, stream: &str) -> IngestionCursor {
    store
        .create_ingestion(operator, stream, &binding(), &head(100))
        .await
        .unwrap()
}

struct Fixture {
    store: Store,
    database_url: String,
    operator: String,
}

impl Fixture {
    async fn new() -> Self {
        let database_url =
            std::env::var("TEST_DATABASE_URL").expect("required disposable PostgreSQL");
        let store = Store::connect(&database_url).await.unwrap();
        store.migrate().await.unwrap();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        Fixture {
            store,
            database_url,
            operator: format!("rotate-{}-{unique}-{sequence}", std::process::id()),
        }
    }
    fn command(&self, to: &str, from: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_base-ingest"));
        command
            .arg("--rotate")
            .env("ARB_INGEST_OPERATOR_ID", &self.operator)
            .env("ARB_INGEST_STREAM_ID", to)
            .env("ARB_INGEST_ROTATE_FROM", from)
            .env("ARB_INGEST_DATABASE_URL", &self.database_url)
            // The happy path never contacts a provider; a real endpoint here
            // would prove this test wrong rather than right.
            .env_remove("ARB_BASE_RPC_URL");
        command
    }
    fn run(&self, to: &str, from: &str) -> Output {
        self.command(to, from).output().unwrap()
    }
    fn success(&self, to: &str, from: &str) -> Value {
        let output = self.run(to, from);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

#[tokio::test]
async fn rotate_happy_path_anchors_the_new_generation_and_leaves_the_source_untouched() {
    let f = Fixture::new().await;
    let source = seed(&f.store, &f.operator, "stream.g3").await;

    // A live lease on a session of a different network must never block a
    // Base rotation: this pins the `network_id == "base-mainnet"` filter,
    // not merely the absence of any live lease at all.
    let digest = format!("sha256:{}", "d".repeat(64));
    f.store
        .save_configuration(&f.operator, &digest, json!({"mode": "OBSERVE"}))
        .await
        .unwrap();
    let solana_session = f
        .store
        .create_session(
            &f.operator,
            "solana-session",
            NewSession {
                network_id: "solana-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: digest,
                experiment_id: "rotate-test-solana".into(),
                strategy_ids: vec!["strategy".into()],
            },
        )
        .await
        .unwrap();
    f.store
        .claim_worker(
            &f.operator,
            &solana_session.session_id,
            "solana-mainnet",
            "worker",
            60,
        )
        .await
        .unwrap();

    let output = f.success("stream.g4", "stream.g3");
    assert_eq!(output["status"], "ROTATED");
    assert_eq!(output["state"], "ACTIVE");
    assert_eq!(output["revision"], "0");
    assert_eq!(output["checkpoint"]["number"], source.checkpoint.number);
    assert_eq!(output["execution_authorized"], false);

    let rotated = f
        .store
        .ingestion_cursor(&f.operator, "stream.g4")
        .await
        .unwrap();
    assert_eq!(rotated.checkpoint, source.checkpoint);
    assert_eq!(rotated.state, "ACTIVE");
    assert_eq!(rotated.revision, 0);

    let untouched = f
        .store
        .ingestion_cursor(&f.operator, "stream.g3")
        .await
        .unwrap();
    assert_eq!(untouched, source);
}

#[tokio::test]
async fn rotate_rejects_a_target_that_is_not_the_immediate_next_generation() {
    let f = Fixture::new().await;
    seed(&f.store, &f.operator, "stream").await;

    let output = f.run("stream.g3", "stream");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ROTATION_TARGET_NOT_NEXT_GENERATION")
    );
    assert!(matches!(
        f.store.ingestion_cursor(&f.operator, "stream.g3").await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn rotate_refuses_while_a_base_mainnet_session_holds_a_live_lease() {
    let f = Fixture::new().await;
    seed(&f.store, &f.operator, "stream").await;

    let digest = format!("sha256:{}", "a".repeat(64));
    f.store
        .save_configuration(&f.operator, &digest, json!({"mode": "OBSERVE"}))
        .await
        .unwrap();
    let session = f
        .store
        .create_session(
            &f.operator,
            "session",
            NewSession {
                network_id: "base-mainnet".into(),
                mode: "OBSERVE".into(),
                configuration_digest: digest,
                experiment_id: "rotate-test".into(),
                strategy_ids: vec!["strategy".into()],
            },
        )
        .await
        .unwrap();
    // A 5-second lease gives the spawned `--rotate` binary a wide margin over
    // the 1-second `claim_worker` minimum, so the sleep below cannot race the
    // process spawn/exec/connect overhead and see a lease that already expired.
    let claim = f
        .store
        .claim_worker(
            &f.operator,
            &session.session_id,
            "base-mainnet",
            "worker",
            5,
        )
        .await
        .unwrap();
    // A crashed-between-commits worker would leave this row behind with no live
    // lease; `complete_worker_recovery` instead leaves it STOPPED with a fresh
    // lease, exactly the "container still committing readiness batches" case
    // the rotation check must refuse.
    f.store.complete_worker_recovery(&claim).await.unwrap();

    let output = f.run("stream.g2", "stream");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ROTATION_BLOCKED_BY_LIVE_LEASE"));
    assert!(matches!(
        f.store.ingestion_cursor(&f.operator, "stream.g2").await,
        Err(StoreError::NotFound)
    ));

    tokio::time::sleep(Duration::from_millis(5_100)).await;

    let rotated = f.success("stream.g2", "stream");
    assert_eq!(rotated["status"], "ROTATED");
}

#[tokio::test]
async fn rotate_refuses_when_the_source_is_halted() {
    let f = Fixture::new().await;
    let source = seed(&f.store, &f.operator, "stream").await;
    f.store
        .halt_ingestion(&f.operator, "stream", &source, IngestionHalt::ResourceLimit)
        .await
        .unwrap();

    let output = f.run("stream.g2", "stream");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ROTATION_SOURCE_HALTED"));
    assert!(matches!(
        f.store.ingestion_cursor(&f.operator, "stream.g2").await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn rotate_refuses_when_the_source_is_missing() {
    let f = Fixture::new().await;

    let output = f.run("absent.g2", "absent");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ROTATION_SOURCE_MISSING"));
}

#[tokio::test]
async fn rotate_retry_is_idempotent() {
    let f = Fixture::new().await;
    let source = seed(&f.store, &f.operator, "stream").await;

    let first = f.success("stream.g2", "stream");
    let second = f.success("stream.g2", "stream");
    assert_eq!(first, second);
    assert_eq!(second["checkpoint"]["number"], source.checkpoint.number);
}

#[tokio::test]
async fn rotate_retry_after_the_source_advanced_is_rejected_as_stale() {
    let f = Fixture::new().await;
    let source = seed(&f.store, &f.operator, "stream").await;
    f.success("stream.g2", "stream");

    f.store
        .commit_ingestion(&f.operator, "stream", &source, batch(&source))
        .await
        .unwrap();

    let output = f.run("stream.g2", "stream");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ROTATION_SOURCE_ADVANCED"));
}

#[tokio::test]
async fn rotate_never_reads_the_provider_endpoint_setting() {
    let f = Fixture::new().await;
    seed(&f.store, &f.operator, "stream").await;

    let mut command = f.command("stream.g2", "stream");
    // A value that would fail every RPC-endpoint validation base-ingest has,
    // to prove --rotate never reaches that code path.
    command.env("ARB_BASE_RPC_URL", "ftp://unreachable.invalid:0/never-read");
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["status"], "ROTATED");
}
