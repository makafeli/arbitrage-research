//! The Python auditor consumes files produced by the actual Rust capture writer.
//! These are storage interoperability tests, not qualified market observations.
use arb_adapter_api::{Chain, StateContext};
use arb_capture::{CaptureManifest, Origin, digest, write_bundle};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "arb-capture-audit-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn run_audit(network: Chain, now: u64, remove_object: bool) -> (i32, Value) {
    let root = Fixture::new();
    let captures = root.0.join("captures");
    fs::create_dir(&captures).unwrap();
    let context = match network {
        Chain::BaseMainnet => StateContext::Evm {
            block_number: 1,
            block_hash: format!("0x{}", "a".repeat(64)),
            parent_hash: format!("0x{}", "b".repeat(64)),
            block_timestamp_seconds: 1,
            finality: "finalized".into(),
        },
        Chain::SolanaMainnet => StateContext::Solana {
            slot: 1,
            genesis_hash: "synthetic-storage-fixture".into(),
            commitment: "finalized".into(),
            account_context: "single-getMultipleAccounts-response".into(),
        },
    };
    let manifest = CaptureManifest {
        schema_version: 1,
        capture_id: "synthetic-audit".into(),
        origin: Origin::Synthetic,
        network,
        provider_alias: "fixture".into(),
        adapter_version: "test-v1".into(),
        adapter_source_commit: "a".repeat(40),
        build_digest: digest(b"fixture"),
        config_digest: digest(b"config"),
        created_at_ms: 100,
        raw_expires_at_ms: Some(200),
        context,
        first_sequence: 0,
        last_sequence: 0,
        required_inputs: vec!["state".into()],
        missing_inputs: vec![],
        coherent: true,
        complete_for_quote: true,
        objects: vec![],
    };
    let checksum = write_bundle(
        &captures.join("synthetic-audit"),
        manifest,
        vec![("rpc.json".into(), b"{\"synthetic\":true}".to_vec())],
        4096,
    )
    .unwrap();
    let request_path = root.0.join("request.json");
    fs::write(
        &request_path,
        serde_json::to_vec(&json!({
            "schema_version": 1, "network": network, "config_digest": digest(b"config"),
            "captures": [{"capture_id": "synthetic-audit", "manifest_digest": checksum}]
        }))
        .unwrap(),
    )
    .unwrap();
    if remove_object {
        fs::remove_file(captures.join("synthetic-audit/rpc.json")).unwrap();
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let output = Command::new("python3")
        .arg(repository.join("scripts/capture_audit.py"))
        .arg("--root")
        .arg(&captures)
        .arg("--request")
        .arg(request_path)
        .arg("--now-ms")
        .arg(now.to_string())
        .output()
        .expect("Python 3.11+ is required for the capture audit interoperability test");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "audit did not return JSON: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), report)
}

#[test]
fn rust_base_and_solana_bundles_pass_python_storage_audit() {
    for network in [Chain::BaseMainnet, Chain::SolanaMainnet] {
        let (code, report) = run_audit(network, 150, false);
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["dependencies"][0]["raw_artifact_status"], "AVAILABLE");
        assert_eq!(report["dependencies"][0]["expiration_status"], "NOT_EXPIRED");
        assert_eq!(report["dependencies"][0]["origin"], "synthetic");
        assert_eq!(report["dependencies"][0]["market_performance_eligible"], false);
        assert_eq!(report["execution_authorized"], false);
    }
}

#[test]
fn expired_rust_bundle_stays_visible_with_or_without_raw_objects() {
    for remove_object in [false, true] {
        let (code, report) = run_audit(Chain::BaseMainnet, 200, remove_object);
        assert_eq!(code, 1, "{report}");
        assert_eq!(report["dependencies"][0]["expiration_status"], "EXPIRED");
        assert_eq!(
            report["dependencies"][0]["raw_artifact_status"],
            if remove_object { "MISSING" } else { "AVAILABLE" }
        );
        assert_eq!(report["references_reported"], 1);
    }
}
