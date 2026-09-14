//! Kill a real test subprocess after committing intent and before worker ACK.
//! This exercises production Store/ControlWorker methods with isolated synthetic
//! sessions in a required disposable PostgreSQL database, never a provider.
use arb_control::ControlWorker;
use arb_storage::{NewCommand, NewSession, Store};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

const CHILD: &str = "ARB_TEST_PENDING_COMMIT_CHILD";

fn database() -> String {
    std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL is required for the process-loss acceptance test")
}

fn start_intent() -> NewCommand {
    NewCommand { action: "START".into(), expected_revision: "0".into(), reason: None }
}

/// Test-executable entry point; only the explicitly spawned process runs the
/// crash fixture. The ordinary test-runner invocation starts no child itself.
#[tokio::test]
async fn pending_commit_child_entry() {
    if std::env::var(CHILD).as_deref() != Ok("1") { return; }
    let operator = std::env::var("ARB_TEST_OPERATOR").unwrap();
    let session = std::env::var("ARB_TEST_SESSION").unwrap();
    let network = std::env::var("ARB_TEST_NETWORK").unwrap();
    let marker = std::env::var("ARB_TEST_MARKER").unwrap();
    let store = Store::connect(&database()).await.unwrap();
    let worker = ControlWorker::claim(store.clone(), &operator, &session, &network, "crash-child", 60)
        .await.unwrap();
    worker.complete_recovery().await.unwrap();
    let pending = store.issue_command(&operator, &session, "same-intent", start_intent())
        .await.unwrap();
    assert_eq!(pending.status, "PENDING");
    assert!(worker.generation().await.is_err());
    let mut file = OpenOptions::new().write(true).create_new(true).open(marker).unwrap();
    file.write_all(pending.command_id.as_bytes()).unwrap();
    file.sync_all().unwrap();
    // The parent terminates this OS process while no worker tick can ACK intent.
    tokio::time::sleep(Duration::from_secs(30)).await;
    panic!("parent failed to terminate the pending-command child");
}

struct ChildGuard { process: Child, root: PathBuf }
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn killed_pending_process_preserves_intent_and_recovers_stopped_on_both_chains() {
    for network in ["base-mainnet", "solana-mainnet"] {
        for mode in ["OBSERVE", "PAPER"] {
            let store = Store::connect(&database()).await.unwrap();
            store.migrate().await.unwrap();
            let operator = format!("process-closeout-{}", Uuid::new_v4());
            store.save_configuration(&operator, "synthetic:config", json!({"mode":mode}))
                .await.unwrap();
            let session = store.create_session(&operator, "new", NewSession {
                network_id: network.into(), mode: mode.into(),
                configuration_digest: "synthetic:config".into(),
                experiment_id: "synthetic:process-loss".into(), strategy_ids: vec!["test".into()],
            }).await.unwrap();
            let root = std::env::temp_dir().join(format!("arb-closeout-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let marker = root.join("committed-command");
            let child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "pending_commit_child_entry", "--nocapture"])
                .env(CHILD, "1").env("ARB_TEST_OPERATOR", &operator)
                .env("ARB_TEST_SESSION", &session.session_id)
                .env("ARB_TEST_NETWORK", network).env("ARB_TEST_MARKER", &marker)
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                .spawn().unwrap();
            let mut guard = ChildGuard { process: child, root };
            let deadline = Instant::now() + Duration::from_secs(15);
            let command = loop {
                assert!(guard.process.try_wait().unwrap().is_none(), "crash child exited before marker");
                if let Ok(value) = fs::read_to_string(&marker) {
                    if Uuid::parse_str(&value).is_ok() { break value; }
                }
                assert!(Instant::now() < deadline, "pending-commit marker timed out");
                tokio::time::sleep(Duration::from_millis(20)).await;
            };
            guard.process.kill().unwrap();
            assert!(!guard.process.wait().unwrap().success());
            let reopened = Store::connect(&database()).await.unwrap();
            let receipt = reopened.get_command(&operator, &command).await.unwrap();
            assert_eq!(receipt.status, "PENDING");
            assert!(!receipt.fence_effective);
            assert_eq!(reopened.get_session(&operator, &session.session_id).await.unwrap().applied_revision, "0");
            let repeated = reopened.issue_command(&operator, &session.session_id, "same-intent", start_intent())
                .await.unwrap();
            assert_eq!(repeated.command_id, command);
            assert_eq!(repeated.status, "PENDING");
            // Advance only the disposable test lease instead of waiting sixty
            // wall-clock seconds. All recovery and epoch checks run unchanged.
            let admin = sqlx::PgPool::connect(&database()).await.unwrap();
            sqlx::query("UPDATE research_sessions SET lease_until=clock_timestamp()-interval '1 second' WHERE session_id=$1")
                .bind(&session.session_id).execute(&admin).await.unwrap();
            let replacement = ControlWorker::claim(reopened.clone(), &operator, &session.session_id,
                network, "replacement-after-process-loss", 60).await.unwrap();
            assert_eq!(reopened.get_command(&operator, &command).await.unwrap().status, "REJECTED");
            let restored = replacement.complete_recovery().await.unwrap().session;
            assert_eq!(restored.observed_state, "STOPPED");
            assert_eq!(restored.mode, mode);
            assert_eq!(restored.network_id, network);
            assert_eq!(restored.configuration_digest, "synthetic:config");
            assert!(!restored.execution_authorized);
            assert!(replacement.generation().await.is_err());
            assert_eq!(replacement.tick(true).await.unwrap().session.observed_state, "STOPPED");
            let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM control_commands WHERE session_id=$1")
                .bind(&session.session_id).fetch_one(&admin).await.unwrap();
            assert_eq!(recorded, 1);
        }
    }
}
