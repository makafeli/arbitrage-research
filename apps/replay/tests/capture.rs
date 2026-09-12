use arb_adapter_api::{Chain, RpcRecord, TranscriptRpc};
use arb_capture::{CaptureManifest, Origin, digest, write_bundle};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);
fn fixture(
    chain: Chain,
    change: impl FnOnce(&mut CaptureManifest, &mut Value),
) -> (PathBuf, String) {
    let (registry, transcript, commit) = match chain {
        Chain::BaseMainnet => (
            include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json").as_slice(),
            include_bytes!("../../../crates/arb-evm/tests/fixtures/rpc.json").as_slice(),
            arb_evm::SOURCE_COMMIT,
        ),
        Chain::SolanaMainnet => (
            include_bytes!("../../../crates/arb-solana/tests/fixtures/registry.json").as_slice(),
            include_bytes!("../../../crates/arb-solana/tests/fixtures/rpc.json").as_slice(),
            arb_solana::SOURCE_COMMIT,
        ),
    };
    let records: Vec<RpcRecord> = serde_json::from_slice(transcript).unwrap();
    let mut rpc = TranscriptRpc::new(records.clone());
    let mut snapshot = match chain {
        Chain::BaseMainnet => serde_json::to_value(
            arb_evm::capture_pool(&mut rpc, &serde_json::from_slice(registry).unwrap(), 100)
                .unwrap(),
        )
        .unwrap(),
        Chain::SolanaMainnet => serde_json::to_value(
            arb_solana::capture_pool(&mut rpc, &serde_json::from_slice(registry).unwrap(), 100)
                .unwrap(),
        )
        .unwrap(),
    };
    rpc.finish().unwrap();
    let config = br#"{"mode":"offline-fixture"}"#;
    let mut manifest = CaptureManifest {
        schema_version: 1,
        capture_id: "offline-redecode-test".into(),
        origin: Origin::ManuallyConstructed,
        network: chain,
        provider_alias: "manual-fixture".into(),
        adapter_version: "test-v1".into(),
        adapter_source_commit: commit.into(),
        build_digest: digest(b"test-build"),
        config_digest: digest(config),
        created_at_ms: 100,
        raw_expires_at_ms: Some(1000),
        context: serde_json::from_value(snapshot["context"].clone()).unwrap(),
        first_sequence: 0,
        last_sequence: records.len() as u64 - 1,
        required_inputs: vec!["qualified-math".into()],
        missing_inputs: vec!["qualified-math".into()],
        coherent: snapshot["quality"]["coherent"].as_bool().unwrap(),
        complete_for_quote: false,
        objects: vec![],
    };
    change(&mut manifest, &mut snapshot);
    let path = std::env::temp_dir().join(format!(
        "arb-redecode-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let hash = write_bundle(
        &path,
        manifest,
        vec![
            ("effective-config.json".into(), config.to_vec()),
            ("registry.json".into(), registry.to_vec()),
            ("rpc.json".into(), transcript.to_vec()),
            (
                "snapshot.json".into(),
                serde_json::to_vec(&snapshot).unwrap(),
            ),
        ],
        64 * 1024 * 1024,
    )
    .unwrap();
    (path, hash)
}

#[test]
fn both_protocol_transcripts_redecode_identically_without_network_or_pnl() {
    for chain in [Chain::BaseMainnet, Chain::SolanaMainnet] {
        let (path, hash) = fixture(chain, |_, _| {});
        let first = replay::verify_capture(&path, &hash, 101).unwrap();
        assert_eq!(first, replay::verify_capture(&path, &hash, 102).unwrap());
        assert_eq!(first["network_requests"], 0);
        assert_eq!(first["origin"], "manually-constructed");
        assert_eq!(first["paper_pnl_available"], false);
        assert_eq!(first["quote_replay_available"], false);
        assert!(replay::verify_capture(&path, &hash, 1000).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn a_self_consistently_rehashed_but_wrong_snapshot_is_rejected() {
    let (path, hash) = fixture(Chain::BaseMainnet, |_, snapshot| {
        snapshot["liquidity"] = json!("999999")
    });
    assert!(replay::verify_capture(&path, &hash, 101).is_err());
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn source_revision_and_config_digest_are_bound_to_the_replay() {
    for kind in 0..3 {
        let (path, hash) = fixture(Chain::BaseMainnet, |manifest, _| match kind {
            0 => manifest.adapter_source_commit = "0".repeat(40),
            1 => manifest.config_digest = digest(b"different-config"),
            _ => manifest.last_sequence += 1,
        });
        assert!(replay::verify_capture(&path, &hash, 101).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn claimed_recorded_solana_capture_must_match_case_sensitive_allowlists_and_genesis() {
    // These remain synthetic fixtures. The adversarial origin label exercises
    // validation of a claimed recorded capture without making network requests.
    for mutation in 0..4 {
        let (source_path, source_hash) = fixture(Chain::SolanaMainnet, |_, _| {});
        let mut bundle = arb_capture::load_bundle(&source_path, Some(&source_hash), 101).unwrap();
        let registry_bytes = &bundle
            .objects
            .iter()
            .find(|(name, _)| name == "registry.json")
            .unwrap()
            .1;
        let registry: arb_solana::PoolRegistry = serde_json::from_slice(registry_bytes).unwrap();
        let baseline = arb_config::ValidatedConfig::from_toml(include_str!(
            "../../../config/research.example.toml"
        ))
        .unwrap();
        let mut config: Value = serde_json::from_str(baseline.effective_json()).unwrap();
        config["deployment"]["mode"] = json!("OBSERVE");
        let solana = &mut config["networks"]["solana"];
        solana["enabled"] = json!(true);
        solana["rpc_secret_reference"] = json!("env:OFFLINE_TEST_RPC");
        solana["registry_qualification_digest"] = json!(digest(registry_bytes));
        solana["expected_genesis_identity"] = json!(registry.expected_genesis_hash);
        let mut pool = registry.pool.clone();
        let mut mint = registry.mint_a.clone();
        if mutation == 1 {
            // Alter a trailing letter: changing a leading Base58 digit can
            // change the decoded byte length and exercise address parsing
            // instead of the case-sensitive replay authorization boundary.
            let index = pool.len() - 2;
            assert_eq!(&pool[index..index + 1], "K");
            pool.replace_range(index..index + 1, "k");
        }
        if mutation == 2 {
            let index = mint.len() - 2;
            assert_eq!(&mint[index..index + 1], "y");
            mint.replace_range(index..index + 1, "Y");
        }
        if mutation == 3 {
            solana["expected_genesis_identity"] = json!(registry.program_data);
        }
        solana["verified_pool_ids"] = json!([
            format!("solana-mainnet:{pool}"),
            format!("solana-mainnet:{}", registry.vault_a)
        ]);
        solana["verified_asset_ids"] = json!([
            format!("solana-mainnet:{mint}"),
            format!("solana-mainnet:{}", registry.mint_b)
        ]);
        let config = arb_config::ValidatedConfig::from_effective_json(
            &serde_json::to_string(&config).unwrap(),
        )
        .unwrap_or_else(|error| panic!("synthetic config mutation {mutation}: {error:?}"));
        bundle.manifest.origin = Origin::RecordedLive;
        bundle.manifest.adapter_version = "arb_solana-v1".into();
        bundle.manifest.config_digest = config.digest().into();
        bundle.manifest.objects.clear();
        bundle
            .objects
            .iter_mut()
            .find(|(name, _)| name == "effective-config.json")
            .unwrap()
            .1 = config.effective_json().as_bytes().to_vec();
        let path = source_path.with_extension("claimed-recorded");
        let hash = write_bundle(&path, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
        let outcome = replay::verify_capture(&path, &hash, 101);
        if mutation == 0 {
            assert!(
                outcome.is_ok(),
                "matching recorded authorization: {outcome:?}"
            );
        } else {
            assert_eq!(
                outcome.unwrap_err().0,
                "replay registry is outside recorded configuration"
            );
        }
        fs::remove_dir_all(source_path).unwrap();
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn pool_set_replay_selects_only_captured_pool_and_requires_its_format() {
    let (source_path, source_hash) = fixture(Chain::BaseMainnet, |_, _| {});
    let mut bundle = arb_capture::load_bundle(&source_path, Some(&source_hash), 101).unwrap();
    let entry = bundle
        .objects
        .iter_mut()
        .find(|(name, _)| name == "registry.json")
        .unwrap();
    let first: Value = serde_json::from_slice(&entry.1).unwrap();
    let mut second = first.clone();
    second["pool"] = json!("0x9999999999999999999999999999999999999999");
    entry.1 = serde_json::to_vec(
        &json!({"schema_version":1,"network_id":"base-mainnet","pools":[first, second]}),
    )
    .unwrap();
    bundle.manifest.adapter_version = "arb_evm-pool-set-v1".into();
    bundle.manifest.objects.clear();
    let good = source_path.with_extension("pool-set");
    let hash = write_bundle(
        &good,
        bundle.manifest.clone(),
        bundle.objects.clone(),
        64 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(
        replay::verify_capture(&good, &hash, 101).unwrap()["verification"],
        "ACQUISITION_REDECODE_MATCHED"
    );
    bundle.manifest.adapter_version = "arb_evm-v1".into();
    let wrong = source_path.with_extension("wrong-format");
    let hash = write_bundle(&wrong, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
    assert!(replay::verify_capture(&wrong, &hash, 101).is_err());
    for path in [source_path, good, wrong] {
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn economic_replay_cannot_promote_unvalidated_fixture_configuration() {
    let (path, hash) = fixture(Chain::BaseMainnet, |_, _| {});
    assert!(replay::load_evaluation_capture(&path, &hash, 101).is_err());
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn economic_replay_preserves_origin_and_explicit_historical_age() {
    let (source_path, source_hash) = fixture(Chain::BaseMainnet, |_, _| {});
    let mut bundle = arb_capture::load_bundle(&source_path, Some(&source_hash), 101).unwrap();
    let registry_bytes = bundle
        .objects
        .iter()
        .find(|(n, _)| n == "registry.json")
        .unwrap()
        .1
        .clone();
    let registry: arb_evm::PoolRegistry = serde_json::from_slice(&registry_bytes).unwrap();
    let baseline = arb_config::ValidatedConfig::from_toml(include_str!(
        "../../../config/research.example.toml"
    ))
    .unwrap();
    let mut config: Value = serde_json::from_str(baseline.effective_json()).unwrap();
    config["deployment"]["mode"] = json!("OBSERVE");
    config["networks"]["base"]["enabled"] = json!(true);
    config["networks"]["base"]["rpc_secret_reference"] = json!("env:OFFLINE_TEST_RPC");
    config["networks"]["base"]["registry_qualification_digest"] = json!(digest(&registry_bytes));
    config["networks"]["base"]["verified_pool_ids"] = json!([
        format!("base-mainnet:{}", registry.pool),
        "base-mainnet:0x9999999999999999999999999999999999999999"
    ]);
    config["networks"]["base"]["verified_asset_ids"] = json!([
        format!("base-mainnet:{}", registry.token0),
        format!("base-mainnet:{}", registry.token1)
    ]);
    let config =
        arb_config::ValidatedConfig::from_effective_json(&serde_json::to_string(&config).unwrap())
            .unwrap();
    bundle.manifest.config_digest = config.digest().into();
    bundle.manifest.objects.clear();
    bundle
        .objects
        .iter_mut()
        .find(|(n, _)| n == "effective-config.json")
        .unwrap()
        .1 = config.effective_json().as_bytes().to_vec();
    let path = source_path.with_extension("economic");
    let hash = write_bundle(&path, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
    let request = replay::ReplayEvaluationRequest {
        schema_version: 1,
        session_id: "offline-evaluation".into(),
        experiment_id: "manual-fixture".into(),
        strategy_id: config.strategy_ids()[0].clone(),
        network_id: arb_domain::NetworkId::BaseMainnet,
        generation: 1,
        observed_at_unix_ms: 100,
        input_age_ms: 1,
        captures: vec![replay::ReplayCaptureInput {
            path: path.to_str().unwrap().into(),
            manifest_digest: hash,
        }],
    };
    let first = replay::evaluate_captures(&request, 101).unwrap();
    let second = replay::evaluate_captures(&request, 102).unwrap();
    assert_eq!(first, second);
    assert_eq!(first["dataset_origin"], "MANUALLY_CONSTRUCTED");
    assert_eq!(first["input_age_ms"], 1);
    assert_eq!(first["network_requests"], 0);
    assert_eq!(first["full_transaction_simulation_available"], false);
    assert!(!first["decisions"].as_array().unwrap().is_empty());
    for p in [source_path, path] {
        fs::remove_dir_all(p).unwrap();
    }
}
