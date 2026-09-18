//! End-to-end fixture-capture check for the one-shot CLI. The acceptance
//! criterion requires the account decode version and source provenance to be
//! stored *with captures*: this exercises the real `fixture` subcommand
//! against the shared arb-solana fixtures and reads the written manifest,
//! instead of only asserting on the in-memory `CaptureManifest` construction.
use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> String {
    format!(
        "{}/../../crates/arb-solana/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn unique_capture_dir(label: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    dir.push(format!(
        "arb-solana-worker-test-{label}-{}-{nanos}",
        std::process::id()
    ));
    dir
}

#[test]
fn fixture_capture_stores_adapter_version_source_commit_and_solana_context() {
    let capture_dir = unique_capture_dir("provenance");
    let output = Command::new(env!("CARGO_BIN_EXE_solana-worker"))
        .args([
            "fixture",
            &fixture_path("registry.json"),
            &fixture_path("rpc.json"),
        ])
        .arg(&capture_dir)
        .output()
        .expect("solana-worker fixture command runs");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest_bytes =
        std::fs::read(capture_dir.join("manifest.json")).expect("manifest.json is written");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).expect("manifest.json is valid JSON");

    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["origin"], "manually-constructed");
    assert_eq!(manifest["network"], "solana-mainnet");
    // Decode version and pinned source provenance, per acceptance criterion.
    assert_eq!(manifest["adapter_version"], "arb_solana-v1");
    assert_eq!(manifest["adapter_source_commit"], arb_solana::SOURCE_COMMIT);
    assert!(
        manifest["build_digest"]
            .as_str()
            .is_some_and(|s| s.starts_with("sha256:"))
    );
    // Account context/slot provenance is retained alongside the decode version.
    assert_eq!(manifest["context"]["kind"], "Solana");
    assert_eq!(manifest["context"]["commitment"], "finalized");
    assert_eq!(
        manifest["context"]["account_context"],
        "single-getMultipleAccounts-response"
    );
    // Missing/incomplete account context can never be reported as quote-ready.
    assert_eq!(manifest["coherent"], false);
    assert_eq!(manifest["complete_for_quote"], false);
    assert!(
        manifest["missing_inputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "qualified-quote-math")
    );

    let inspect = Command::new(env!("CARGO_BIN_EXE_solana-worker"))
        .args(["inspect"])
        .arg(&capture_dir)
        .output()
        .expect("solana-worker inspect command runs");
    assert!(
        inspect.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let inspected: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(
        inspected["manifest"]["adapter_source_commit"],
        arb_solana::SOURCE_COMMIT
    );
    assert_eq!(inspected["quote_ready"], false);

    let _ = std::fs::remove_dir_all(&capture_dir);
}
