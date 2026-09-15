//! Real Unix process signals against synthetic loopback inputs and PostgreSQL.
use super::*;
use std::process::{Child, Stdio};
use std::time::Instant;

struct RunningChild(Option<Child>);
impl RunningChild {
    fn terminate(&self) {
        let id = self.0.as_ref().unwrap().id().to_string();
        assert!(
            Command::new("kill")
                .args(["-TERM", &id])
                .status()
                .unwrap()
                .success()
        );
    }

    fn finish(&mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "ingestion did not process SIGTERM"
            );
            thread::sleep(Duration::from_millis(5));
        }
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}
impl Drop for RunningChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn assert_cancelled(output: Output) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected handled cancellation, not signal death"
    );
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["status"], "STOPPED_WITH_ERROR");
    assert_eq!(value["reason"], "CANCELLED");
    assert_eq!(value["execution_authorized"], false);
}

#[tokio::test]
async fn terminate_during_rpc_uses_the_same_fence_and_keeps_the_saved_cursor() {
    let fixture = Fixture::new();
    fixture.success("--migrate");
    fixture.success("--initialize");
    fixture.tip.store(101, Ordering::SeqCst);
    fixture.stalled.store(true, Ordering::SeqCst);
    let before = fixture.calls.load(Ordering::SeqCst);
    let mut child = RunningChild(Some(
        fixture
            .command("--run")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    while fixture.calls.load(Ordering::SeqCst) == before {
        assert!(Instant::now() < deadline, "ingestion did not enter its RPC");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    child.terminate();
    tokio::time::sleep(Duration::from_millis(100)).await;
    fixture.stalled.store(false, Ordering::SeqCst);
    assert_cancelled(child.finish());
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let cursor = store
        .ingestion_cursor(&fixture.operator, "stream")
        .await
        .unwrap();
    assert_eq!(cursor.state, "ACTIVE");
    assert_eq!(cursor.revision, 0);
    assert_eq!(cursor.checkpoint.number, 100);
    assert!(
        store
            .ingestion_batches(&fixture.operator, "stream", 0, 16)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.calls.load(Ordering::SeqCst), before + 1);
    // A fresh process reuses the unchanged checkpoint; cancellation did not
    // invent a permanent provider gap or reset coverage to the current tip.
    assert_eq!(fixture.success("--run")["checkpoint"]["number"], 101);
}

#[tokio::test]
async fn terminate_between_polls_preserves_committed_data_and_prevents_the_next_rpc() {
    let fixture = Fixture::new();
    fixture.success("--migrate");
    fixture.success("--initialize");
    fixture.tip.store(101, Ordering::SeqCst);
    let store = Store::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut child = RunningChild(Some(
        fixture
            .command("--run")
            .env("ARB_INGEST_MAX_POLLS", "2")
            .env("ARB_INGEST_POLL_MS", "60000")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let cursor = store
            .ingestion_cursor(&fixture.operator, "stream")
            .await
            .unwrap();
        if cursor.revision == 1 {
            break;
        }
        assert!(Instant::now() < deadline, "first batch was not committed");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let calls = fixture.calls.load(Ordering::SeqCst);
    child.terminate();
    assert_cancelled(child.finish());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), calls);
    let cursor = store
        .ingestion_cursor(&fixture.operator, "stream")
        .await
        .unwrap();
    assert_eq!(cursor.state, "ACTIVE");
    assert_eq!(cursor.revision, 1);
    assert_eq!(cursor.checkpoint.number, 101);
    assert_eq!(
        store
            .ingestion_batches(&fixture.operator, "stream", 0, 16)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.tip.store(102, Ordering::SeqCst);
    assert_eq!(fixture.success("--run")["revision"], "2");
}
