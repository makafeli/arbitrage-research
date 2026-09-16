//! Actual CLI/HTTP/PostgreSQL filter integration with synthetic node replies.
use super::*;
fn rows(output: Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn count(fixture: &Fixture, method: &str) -> usize {
    fixture
        .observed
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["method"] == method)
        .count()
}

#[tokio::test]
async fn quiet_filters_still_recover_finalized_blocks_and_process_restart_resumes() {
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    for tip in [101, 102] {
        f.tip.store(tip, Ordering::SeqCst);
        let output = rows(f.run("--follow"));
        assert_eq!(output[0]["status"], "FILTER_HINTS");
        assert_eq!(output[0]["pool_notifications"], 0);
        assert_eq!(output[1]["checkpoint"]["number"], tip);
        assert_eq!(output[1]["status"], "BATCH_COMMITTED");
    }
    assert_eq!(count(&f, "eth_newBlockFilter"), 2);
    assert_eq!(count(&f, "eth_newFilter"), 2);
    assert_eq!(count(&f, "eth_uninstallFilter"), 4);
    let status = f.success("--status");
    assert_eq!(status["revision"], "2");
    assert_eq!(status["state"], "ACTIVE");
}

#[test]
fn repeated_follow_polls_reuse_filters_and_do_not_invent_coverage() {
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    let out = rows(
        f.command("--follow")
            .env("ARB_INGEST_MAX_POLLS", "2")
            .output()
            .unwrap(),
    );
    assert_eq!(out.len(), 4);
    assert_eq!(out[1]["status"], "NO_NEW_FINALIZED_BLOCKS");
    assert_eq!(out[3]["revision"], "0");
    assert_eq!(count(&f, "eth_newFilter"), 1);
    assert_eq!(count(&f, "eth_getFilterChanges"), 4);
    assert_eq!(count(&f, "eth_uninstallFilter"), 2);
}

#[tokio::test]
async fn malformed_notification_halts_without_checkpoint_advance_and_cleans_up() {
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    f.tip.store(101, Ordering::SeqCst);
    *f.filter_logs.lock().unwrap() = json!([{"address":"not-an-approved-pool"}]);
    let out = f.run("--follow");
    assert!(!out.status.success());
    assert_eq!(count(&f, "eth_getLogs"), 0);
    assert_eq!(count(&f, "eth_uninstallFilter"), 2);
    let status = f.success("--status");
    // HALTED is a persisted state transition, so its revision increases.
    // The chain checkpoint and accepted event batches must not advance.
    assert_eq!(status["revision"], "1");
    assert_eq!(status["checkpoint"]["number"], 100);
    assert_eq!(status["state"], "HALTED");
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert!(
        store
            .ingestion_batches(&f.operator, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
    let sent = f.calls.load(Ordering::SeqCst);
    assert!(!f.run("--follow").status.success());
    assert_eq!(f.calls.load(Ordering::SeqCst), sent);
}

#[tokio::test]
async fn removed_nonfinal_hint_is_not_written_as_a_finalized_event() {
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    f.tip.store(101, Ordering::SeqCst);
    let pools: Value =
        serde_json::from_str(&std::fs::read_to_string(f.root.join("registries.json")).unwrap())
            .unwrap();
    *f.filter_logs.lock().unwrap() = json!([{"address":pools[0]["pool"],"blockNumber":"0x3e7","blockHash":hash(999),
        "transactionHash":hash(1999),"transactionIndex":"0x0","logIndex":"0x0","removed":true,
        "topics":[arb_evm::events::INITIALIZE],"data":format!("0x{:064x}{:064x}",1_u128<<96,0)}]);
    let out = rows(f.run("--follow"));
    assert_eq!(out[0]["removed_notifications"], 1);
    assert_eq!(out[0]["authoritative"], false);
    assert_eq!(out[1]["checkpoint"]["number"], 101);
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let batches = store
        .ingestion_batches(&f.operator, "stream", 0, 16)
        .await
        .unwrap();
    let serialized = serde_json::to_string(&batches).unwrap();
    assert!(!serialized.contains(&hash(999)));
    assert!(serialized.contains(&hash(101)));
}

#[test]
fn follow_of_missing_stream_does_not_allocate_node_resources() {
    let f = Fixture::new();
    f.success("--migrate");
    assert!(!f.run("--follow").status.success());
    assert_eq!(f.calls.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_between_follow_polls_cleans_filters_and_preserves_committed_cursor() {
    use std::process::{Child, Stdio};
    use std::time::Instant;
    struct Guard(Option<Child>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    f.tip.store(101, Ordering::SeqCst);
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut child = Guard(Some(
        f.command("--follow")
            .env("ARB_INGEST_MAX_POLLS", "2")
            .env("ARB_INGEST_POLL_MS", "60000")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if store
            .ingestion_cursor(&f.operator, "stream")
            .await
            .unwrap()
            .revision
            == 1
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "first followed batch did not commit"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let before = f.calls.load(Ordering::SeqCst);
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.0.as_ref().unwrap().id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.0.as_mut().unwrap().try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "follow cancellation or filter cleanup exceeded bound"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let output = child.0.take().unwrap().wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["reason"],
        "CANCELLED"
    );
    assert_eq!(f.calls.load(Ordering::SeqCst), before + 2);
    assert_eq!(count(&f, "eth_uninstallFilter"), 2);
    let cursor = store.ingestion_cursor(&f.operator, "stream").await.unwrap();
    assert_eq!(cursor.state, "ACTIVE");
    assert_eq!(cursor.revision, 1);
    assert_eq!(cursor.checkpoint.number, 101);
    f.tip.store(102, Ordering::SeqCst);
    let resumed = rows(f.run("--follow"));
    assert_eq!(resumed[1]["revision"], "2");
}

/// Wait with a real process deadline and kill/wait on a failed assertion.
#[cfg(unix)]
async fn finished_after_output_loss(child: std::process::Child) -> Output {
    struct Guard(Option<std::process::Child>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let mut guard = Guard(Some(child));
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while guard.0.as_mut().unwrap().try_wait().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "output-loss cleanup exceeded bound"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    guard.0.take().unwrap().wait_with_output().unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn closed_stdout_before_recovery_cleans_both_filters_without_a_false_gap() {
    use std::process::Stdio;
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    f.tip.store(101, Ordering::SeqCst);
    let mut child = f
        .command("--follow")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The first write happens only after filter allocation and polling.
    drop(child.stdout.take());
    let output = finished_after_output_loss(child).await;
    assert_eq!(
        count(&f, "eth_uninstallFilter"),
        2,
        "known filters leaked on stdout failure"
    );
    assert_eq!(count(&f, "eth_getLogs"), 0);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["reason"],
        "OUTPUT_UNAVAILABLE"
    );
    let status = f.success("--status");
    assert_eq!(status["state"], "ACTIVE");
    assert_eq!(status["revision"], "0");
    assert_eq!(status["checkpoint"]["number"], 100);
    f.success("--run");
    assert_eq!(f.success("--status")["checkpoint"]["number"], 101);
}

#[cfg(unix)]
#[tokio::test]
async fn closed_stdout_after_hints_preserves_committed_batch_and_cleans_filters() {
    use std::process::{Child, Stdio};
    struct Guard(Option<Child>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let f = Fixture::new();
    f.success("--migrate");
    f.success("--initialize");
    f.tip.store(101, Ordering::SeqCst);
    // Stop the first canonical log response until stdout is definitely closed.
    // FILTER_HINTS must have been written successfully to reach this request.
    f.recovery_stalled.store(true, Ordering::SeqCst);
    let mut child = Guard(Some(
        f.command("--follow")
            .env("ARB_INGEST_MAX_POLLS", "2")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    while count(&f, "eth_getLogs") == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "recovery did not reach its held response"
        );
        assert!(child.0.as_mut().unwrap().try_wait().unwrap().is_none());
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    drop(child.0.as_mut().unwrap().stdout.take());
    f.recovery_stalled.store(false, Ordering::SeqCst);
    let output = finished_after_output_loss(child.0.take().unwrap()).await;
    assert_eq!(
        count(&f, "eth_uninstallFilter"),
        2,
        "committed output failure bypassed cleanup"
    );
    assert_eq!(
        count(&f, "eth_getFilterChanges"),
        2,
        "output failure must prevent another poll"
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["reason"],
        "OUTPUT_UNAVAILABLE"
    );
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let cursor = store.ingestion_cursor(&f.operator, "stream").await.unwrap();
    assert_eq!(cursor.state, "ACTIVE");
    assert_eq!(cursor.revision, 1);
    assert_eq!(cursor.checkpoint.number, 101);
    assert_eq!(
        store
            .ingestion_batches(&f.operator, "stream", 0, 16)
            .await
            .unwrap()
            .len(),
        1
    );
    // An uncertain output is not permission to repeat an already committed batch.
    f.tip.store(102, Ordering::SeqCst);
    let resumed = rows(f.run("--follow"));
    assert_eq!(resumed[1]["revision"], "2");
    assert_eq!(resumed[1]["checkpoint"]["number"], 102);
}
