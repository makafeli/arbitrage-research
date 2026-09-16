//! Real executable and PostgreSQL with transient synthetic provider failures.
use super::*;

fn records(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn method_count(fixture: &Fixture, method: &str) -> usize {
    fixture
        .observed
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["method"] == method)
        .count()
}
async fn batches(fixture: &Fixture) -> Vec<Value> {
    Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap()
        .ingestion_batches(&fixture.operator, "stream", 0, 16)
        .await
        .unwrap()
}
fn initialized() -> Fixture {
    let fixture = Fixture::new();
    fixture.success("--migrate");
    fixture.success("--initialize");
    fixture.tip.store(103, Ordering::SeqCst);
    fixture
}

#[tokio::test]
async fn default_transport_failure_still_halts_without_reconnect() {
    let fixture = initialized();
    fixture.failures.store(1, Ordering::SeqCst);
    let output = fixture.run("--run");
    assert!(!output.status.success());
    assert!(
        !records(&output)
            .iter()
            .any(|r| r["status"] == "RECONNECT_SCHEDULED")
    );
    let status = fixture.success("--status");
    assert_eq!(status["state"], "HALTED");
    assert_eq!(status["halt_reason"], "PROVIDER_FAILURE");
    assert_eq!(status["checkpoint"]["number"], 100);
    assert!(batches(&fixture).await.is_empty());
    assert_eq!(method_count(&fixture, "eth_getLogs"), 1);
}

#[tokio::test]
async fn transient_follow_failure_reopens_filters_and_recovers_every_missing_block_once() {
    let fixture = initialized();
    fixture.failures.store(1, Ordering::SeqCst);
    let output = fixture
        .command("--follow")
        .env("ARB_INGEST_MAX_RECONNECTS", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = records(&output);
    let retries: Vec<_> = rows
        .iter()
        .filter(|r| r["status"] == "RECONNECT_SCHEDULED")
        .collect();
    assert_eq!(retries.len(), 1);
    assert_eq!(retries[0]["checkpoint"]["number"], 100);
    assert_eq!(retries[0]["revision"], "0");
    assert_eq!(retries[0]["backoff_ms"], 1000);
    assert_eq!(rows.last().unwrap()["checkpoint"]["number"], 103);
    let saved = batches(&fixture).await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0]["from_checkpoint"]["number"], 100);
    let blocks = saved[0]["blocks"].as_array().unwrap();
    assert_eq!(
        blocks
            .iter()
            .map(|b| b["header"]["number"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![101, 102, 103]
    );
    assert_eq!(fixture.success("--status")["revision"], "1");
    assert_eq!(method_count(&fixture, "eth_getLogs"), 4);
    assert_eq!(method_count(&fixture, "eth_newBlockFilter"), 2);
    assert_eq!(method_count(&fixture, "eth_uninstallFilter"), 4);
    let requests = fixture.observed.lock().unwrap();
    let opens: Vec<_> = requests
        .iter()
        .enumerate()
        .filter(|(_, r)| r["method"] == "eth_newBlockFilter")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        requests[opens[0]..opens[1]]
            .iter()
            .filter(|r| r["method"] == "eth_uninstallFilter")
            .count(),
        2
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE_PROVIDER_ERROR"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_PROVIDER_ERROR"));
}

#[tokio::test]
async fn exhausted_reconnect_budget_halts_and_does_not_rearm_persisted_halt() {
    let fixture = initialized();
    fixture.failures.store(20, Ordering::SeqCst);
    let output = fixture
        .command("--follow")
        .env("ARB_INGEST_MAX_RECONNECTS", "2")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        records(&output)
            .iter()
            .filter(|r| r["status"] == "RECONNECT_SCHEDULED")
            .count(),
        2
    );
    assert_eq!(method_count(&fixture, "eth_getLogs"), 3);
    assert_eq!(method_count(&fixture, "eth_uninstallFilter"), 6);
    assert!(batches(&fixture).await.is_empty());
    let status = fixture.success("--status");
    assert_eq!(status["checkpoint"]["number"], 100);
    assert_eq!(status["halt_reason"], "PROVIDER_FAILURE");
    let before = fixture.calls.load(Ordering::SeqCst);
    assert!(
        !fixture
            .command("--follow")
            .env("ARB_INGEST_MAX_RECONNECTS", "3")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fixture.calls.load(Ordering::SeqCst), before);
}

#[tokio::test]
async fn rate_limit_or_access_refusal_never_uses_the_opted_in_budget() {
    for status in [429, 401, 403] {
        let fixture = initialized();
        fixture.failure_status.store(status, Ordering::SeqCst);
        fixture.failures.store(10, Ordering::SeqCst);
        let output = fixture
            .command("--follow")
            .env("ARB_INGEST_MAX_RECONNECTS", "3")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(method_count(&fixture, "eth_getLogs"), 1);
        assert_eq!(method_count(&fixture, "eth_uninstallFilter"), 2);
        assert!(
            !records(&output)
                .iter()
                .any(|r| r["status"] == "RECONNECT_SCHEDULED")
        );
        assert!(batches(&fixture).await.is_empty());
    }
}

#[tokio::test]
async fn changed_checkpoint_never_retries_or_skips_history() {
    let fixture = initialized();
    fixture.reorg.store(true, Ordering::SeqCst);
    let output = fixture
        .command("--follow")
        .env("ARB_INGEST_MAX_RECONNECTS", "3")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        !records(&output)
            .iter()
            .any(|r| r["status"] == "RECONNECT_SCHEDULED")
    );
    assert_eq!(method_count(&fixture, "eth_newBlockFilter"), 1);
    assert_eq!(method_count(&fixture, "eth_uninstallFilter"), 2);
    assert_eq!(
        fixture.success("--status")["halt_reason"],
        "CONTINUITY_LOST"
    );
    assert!(batches(&fixture).await.is_empty());
}

#[tokio::test]
async fn invalid_reconnect_setting_performs_no_provider_request() {
    let fixture = initialized();
    for value in ["4", "01", "+1", "-1", ""] {
        let before = fixture.calls.load(Ordering::SeqCst);
        let output = fixture
            .command("--follow")
            .env("ARB_INGEST_MAX_RECONNECTS", value)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("INVALID_RECONNECT_BOUND"));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), before);
    }
    assert_eq!(fixture.success("--status")["state"], "ACTIVE");
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_during_backoff_keeps_checkpoint_and_prevents_another_request() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        sync::mpsc,
    };
    let fixture = initialized();
    fixture.failures.store(1, Ordering::SeqCst);
    let mut child = fixture
        .command("--follow")
        .env("ARB_INGEST_MAX_RECONNECTS", "3")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let line = line.unwrap();
            if line.contains("RECONNECT_SCHEDULED") {
                let _ = tx.send(());
            }
        }
    });
    if rx.recv_timeout(Duration::from_secs(8)).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        panic!("process did not reach bounded backoff");
    }
    let before = fixture.calls.load(Ordering::SeqCst);
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            panic!("backoff did not cancel");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CANCELLED"));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), before);
    assert_eq!(method_count(&fixture, "eth_uninstallFilter"), 2);
    let status = fixture.success("--status");
    assert_eq!(status["state"], "ACTIVE");
    assert_eq!(status["checkpoint"]["number"], 100);
    assert!(batches(&fixture).await.is_empty());
    // A later explicit invocation recovers the original retained checkpoint.
    assert!(fixture.run("--follow").status.success());
    assert_eq!(fixture.success("--status")["checkpoint"]["number"], 103);
}
