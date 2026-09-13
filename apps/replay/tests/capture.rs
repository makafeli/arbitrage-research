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
    bundle.manifest.adapter_version = "arb_evm-pool-set-v2".into();
    let mislabeled = source_path.with_extension("mislabeled-v2");
    let hash = write_bundle(
        &mislabeled,
        bundle.manifest.clone(),
        bundle.objects.clone(),
        64 * 1024 * 1024,
    )
    .unwrap();
    assert!(replay::verify_capture(&mislabeled, &hash, 101).is_err());
    fs::remove_dir_all(mislabeled).unwrap();
    bundle.manifest.adapter_version = "arb_evm-v1".into();
    let wrong = source_path.with_extension("wrong-format");
    let hash = write_bundle(&wrong, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
    assert!(replay::verify_capture(&wrong, &hash, 101).is_err());
    for path in [source_path, good, wrong] {
        fs::remove_dir_all(path).unwrap();
    }
}

/// Real-shaped synthetic batch transcripts are kept verbatim, including the
/// Solana union account response and the Base shared anchor/final recheck.
fn batch_fixture(chain: Chain, selected: usize) -> (PathBuf, String) {
    let (registries, transcript, network) = match chain {
        Chain::BaseMainnet => (
            include_bytes!("../../../crates/arb-evm/tests/fixtures/batch-registries.json")
                .as_slice(),
            include_bytes!("../../../crates/arb-evm/tests/fixtures/batch-rpc.json").as_slice(),
            arb_domain::NetworkId::BaseMainnet,
        ),
        Chain::SolanaMainnet => (
            include_bytes!("../../../crates/arb-solana/tests/fixtures/batch-registries.json")
                .as_slice(),
            include_bytes!("../../../crates/arb-solana/tests/fixtures/batch-rpc.json").as_slice(),
            arb_domain::NetworkId::SolanaMainnet,
        ),
    };
    let (source, source_hash) = fixture(chain, |_, _| {});
    let mut bundle = arb_capture::load_bundle(&source, Some(&source_hash), 101).unwrap();
    let records: Vec<RpcRecord> = serde_json::from_slice(transcript).unwrap();
    let mut rpc = TranscriptRpc::new(records.clone());
    let snapshot = match chain {
        Chain::BaseMainnet => serde_json::to_value(
            arb_evm::capture_pools(
                &mut rpc,
                &serde_json::from_slice::<Vec<_>>(registries).unwrap(),
                100,
            )
            .unwrap()
            .remove(selected),
        )
        .unwrap(),
        Chain::SolanaMainnet => serde_json::to_value(
            arb_solana::capture_pools(
                &mut rpc,
                &serde_json::from_slice::<Vec<_>>(registries).unwrap(),
                100,
            )
            .unwrap()
            .remove(selected),
        )
        .unwrap(),
    };
    rpc.finish().unwrap();
    let registry = serde_json::to_vec(&json!({
        "schema_version": 1, "network_id": network,
        "pools": serde_json::from_slice::<Value>(registries).unwrap()
    }))
    .unwrap();
    bundle.manifest.adapter_version =
        arb_registry::RegistryDocument::from_bytes(&registry, network)
            .unwrap()
            .adapter_version()
            .into();
    bundle.manifest.context = serde_json::from_value(snapshot["context"].clone()).unwrap();
    bundle.manifest.coherent = snapshot["quality"]["coherent"].as_bool().unwrap();
    bundle.manifest.last_sequence = records.len() as u64 - 1;
    bundle.manifest.objects.clear();
    for (name, bytes) in &mut bundle.objects {
        match name.as_str() {
            "registry.json" => *bytes = registry.clone(),
            "rpc.json" => *bytes = transcript.to_vec(),
            "snapshot.json" => *bytes = serde_json::to_vec(&snapshot).unwrap(),
            _ => {}
        }
    }
    let path = source.with_extension("batch-v2");
    let hash = write_bundle(&path, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
    fs::remove_dir_all(source).unwrap();
    (path, hash)
}

#[test]
fn shared_batch_replay_consumes_the_actual_full_transcript_for_either_selected_pool() {
    for chain in [Chain::BaseMainnet, Chain::SolanaMainnet] {
        for selected in [0, 1] {
            let (path, hash) = batch_fixture(chain, selected);
            let report = replay::verify_capture(&path, &hash, 101).unwrap();
            let expected_records = if chain == Chain::SolanaMainnet { 2 } else { 29 };
            assert_eq!(report["rpc_records_consumed"], expected_records);
            assert_eq!(report["network_requests"], 0);
            assert_eq!(report["origin"], "manually-constructed");
            assert_eq!(report["paper_pnl_available"], false);
            assert_eq!(report["quality"]["quote_implementation_qualified"], false);
            let bundle = arb_capture::load_bundle(&path, Some(&hash), 101).unwrap();
            let retained = &bundle
                .objects
                .iter()
                .find(|(name, _)| name == "rpc.json")
                .unwrap()
                .1;
            let expected = if chain == Chain::SolanaMainnet {
                include_bytes!("../../../crates/arb-solana/tests/fixtures/batch-rpc.json")
                    .as_slice()
            } else {
                include_bytes!("../../../crates/arb-evm/tests/fixtures/batch-rpc.json").as_slice()
            };
            assert_eq!(
                retained, expected,
                "original full RPC response bytes retained"
            );
            fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn shared_batch_replay_rejects_unselected_pool_corruption_and_unconsumed_calls() {
    for chain in [Chain::BaseMainnet, Chain::SolanaMainnet] {
        for append in [false, true] {
            let (source, source_hash) = batch_fixture(chain, 0);
            let mut bundle = arb_capture::load_bundle(&source, Some(&source_hash), 101).unwrap();
            let entry = bundle
                .objects
                .iter_mut()
                .find(|(name, _)| name == "rpc.json")
                .unwrap();
            let mut records: Vec<RpcRecord> = serde_json::from_slice(&entry.1).unwrap();
            if append {
                let mut extra = records.last().unwrap().clone();
                extra.sequence = records.len() as u64;
                let mut response: Value = serde_json::from_str(&extra.response).unwrap();
                response["id"] = json!(extra.sequence);
                extra.response = response.to_string();
                records.push(extra);
            } else if chain == Chain::SolanaMainnet {
                // Last union account belongs only to the unselected second pool.
                let record = records.last_mut().unwrap();
                let mut response: Value = serde_json::from_str(&record.response).unwrap();
                *response["result"]["value"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap() = Value::Null;
                record.response = response.to_string();
            } else {
                let record = records
                    .iter_mut()
                    .find(|record| {
                        record.method == arb_adapter_api::ReadMethod::EthGetCode
                            && record.params[0] == "0x0404040404040404040404040404040404040404"
                    })
                    .unwrap();
                let mut response: Value = serde_json::from_str(&record.response).unwrap();
                response["result"] = json!("0x");
                record.response = response.to_string();
            }
            entry.1 = serde_json::to_vec(&records).unwrap();
            bundle.manifest.last_sequence = records.len() as u64 - 1;
            bundle.manifest.objects.clear();
            let path = source.with_extension("adversarial-batch");
            let hash =
                write_bundle(&path, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
            let failure = replay::verify_capture(&path, &hash, 101).unwrap_err();
            if append {
                assert_eq!(failure.0, "replay left unconsumed RPC records");
            } else {
                assert!(failure.0.ends_with("batch transcript replay failed"));
            }
            fs::remove_dir_all(source).unwrap();
            fs::remove_dir_all(path).unwrap();
        }
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

/// Version 3 freezes the exact account-context slot's estimated time and policy.
/// All data is manually constructed; the timestamps model history, not uptime.
fn chain_time_fixture(selected: usize, block_time: Value) -> (PathBuf, String) {
    let (source, source_hash) = batch_fixture(Chain::SolanaMainnet, selected);
    let mut bundle = arb_capture::load_bundle(&source, Some(&source_hash), 101).unwrap();
    let registry_bytes = bundle
        .objects
        .iter()
        .find(|(n, _)| n == "registry.json")
        .unwrap()
        .1
        .clone();
    let document = arb_registry::RegistryDocument::from_bytes(
        &registry_bytes,
        arb_domain::NetworkId::SolanaMainnet,
    )
    .unwrap();
    let registries = document
        .pools()
        .iter()
        .map(|pool| {
            let arb_registry::PoolRegistry::Solana(registry) = pool else {
                panic!("Solana fixture")
            };
            registry.clone()
        })
        .collect::<Vec<_>>();
    let baseline = arb_config::ValidatedConfig::from_toml(include_str!(
        "../../../config/research.example.toml"
    ))
    .unwrap();
    let mut config: Value = serde_json::from_str(baseline.effective_json()).unwrap();
    config["deployment"]["mode"] = json!("OBSERVE");
    config["research"]["trade_sizes_minor"] = json!(["10000"]);
    config["simulation"]["maximum_state_age_ms"] = json!(60000);
    let solana = &mut config["networks"]["solana"];
    solana["enabled"] = json!(true);
    solana["rpc_secret_reference"] = json!("env:OFFLINE_TEST_RPC");
    solana["expected_genesis_identity"] = json!(registries[0].expected_genesis_hash);
    solana["registry_qualification_digest"] = json!(digest(&registry_bytes));
    solana["verified_pool_ids"] = json!(
        registries
            .iter()
            .map(|r| format!("solana-mainnet:{}", r.pool))
            .collect::<Vec<_>>()
    );
    solana["verified_asset_ids"] = json!(
        registries
            .iter()
            .flat_map(|r| [&r.mint_a, &r.mint_b])
            .map(|mint| format!("solana-mainnet:{mint}"))
            .collect::<std::collections::BTreeSet<_>>()
    );
    solana["starting_asset_id"] = json!(format!("solana-mainnet:{}", registries[0].mint_a));
    solana["chain_freshness"] =
        json!({"version":"finalized-chain-time-v1","max_chain_age_ms":30000});
    let config = arb_config::ValidatedConfig::from_effective_json(&config.to_string()).unwrap();
    document.authorize(&config).unwrap();
    let mut records: Vec<RpcRecord> = serde_json::from_slice(
        &bundle
            .objects
            .iter()
            .find(|(n, _)| n == "rpc.json")
            .unwrap()
            .1,
    )
    .unwrap();
    let account_response: Value = serde_json::from_str(&records[1].response).unwrap();
    let sequence = records.len() as u64;
    records.push(RpcRecord {
        sequence,
        method: arb_adapter_api::ReadMethod::GetBlockTime,
        params: json!([account_response["result"]["context"]["slot"]]),
        response: json!({"jsonrpc":"2.0","id":sequence,"result":block_time}).to_string(),
    });
    let mut rpc = TranscriptRpc::new(records.clone());
    let snapshot = arb_solana::capture_pools_with_chain_time(&mut rpc, &registries, 100000)
        .unwrap()
        .remove(selected);
    rpc.finish().unwrap();
    bundle.manifest.adapter_version = "arb_solana-pool-set-v3".into();
    bundle.manifest.capture_id = format!("chain-time-source-{selected}");
    bundle.manifest.config_digest = config.digest().into();
    bundle.manifest.created_at_ms = 100000;
    bundle.manifest.raw_expires_at_ms = Some(1000000);
    bundle.manifest.last_sequence = sequence;
    bundle.manifest.objects.clear();
    for (name, bytes) in &mut bundle.objects {
        match name.as_str() {
            "effective-config.json" => *bytes = config.effective_json().as_bytes().to_vec(),
            "rpc.json" => *bytes = serde_json::to_vec(&records).unwrap(),
            "snapshot.json" => *bytes = serde_json::to_vec(&snapshot).unwrap(),
            _ => {}
        }
    }
    let path = source.with_extension("chain-time-v3");
    let hash = write_bundle(&path, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
    fs::remove_dir_all(source).unwrap();
    (path, hash)
}

#[test]
fn solana_chain_time_replay_preserves_estimated_and_unknown_time_without_network() {
    for block_time in [json!(90), Value::Null] {
        for selected in [0, 1] {
            let (path, hash) = chain_time_fixture(selected, block_time.clone());
            let first = replay::verify_capture(&path, &hash, 100001).unwrap();
            assert_eq!(first, replay::verify_capture(&path, &hash, 999999).unwrap());
            assert_eq!(first["rpc_records_consumed"], 3);
            assert_eq!(first["network_requests"], 0);
            assert_eq!(first["quality"]["coherent"], false);
            assert_eq!(first["quality"]["quote_implementation_qualified"], false);
            let (pool, config) = replay::load_evaluation_capture(&path, &hash, 100001).unwrap();
            assert!(
                config
                    .chain_freshness(arb_domain::NetworkId::SolanaMainnet)
                    .is_some()
            );
            let arb_engine::PoolState::Solana { snapshot, .. } = pool.state else {
                panic!("Solana")
            };
            assert_eq!(snapshot.block_time_seconds, block_time.as_u64());
            fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn solana_chain_time_replay_rejects_tampering_trailing_calls_and_policy_downgrade() {
    for mutation in 0..7 {
        let (source, source_hash) = chain_time_fixture(0, json!(90));
        let mut bundle = arb_capture::load_bundle(&source, Some(&source_hash), 100001).unwrap();
        let entry = bundle
            .objects
            .iter_mut()
            .find(|(n, _)| n == "rpc.json")
            .unwrap();
        let mut records: Vec<RpcRecord> = serde_json::from_slice(&entry.1).unwrap();
        match mutation {
            0 => records[2].params = json!([999999]),
            1 => records[2].response = json!({"jsonrpc":"2.0","id":2,"result":91}).to_string(),
            2 => records[2].response = json!({"jsonrpc":"2.0","id":2,"result":"90"}).to_string(),
            3 => {
                records.pop();
            }
            4 => {
                let mut extra = records[2].clone();
                extra.sequence = 3;
                extra.response = json!({"jsonrpc":"2.0","id":3,"result":90}).to_string();
                records.push(extra);
            }
            5 => bundle.manifest.adapter_version = "arb_solana-pool-set-v2".into(),
            6 => {
                bundle.manifest.adapter_version = "arb_solana-pool-set-v2".into();
                records.pop();
            }
            _ => unreachable!(),
        }
        entry.1 = serde_json::to_vec(&records).unwrap();
        if mutation == 6 {
            let entry = bundle
                .objects
                .iter_mut()
                .find(|(n, _)| n == "snapshot.json")
                .unwrap();
            let mut snapshot: Value = serde_json::from_slice(&entry.1).unwrap();
            snapshot
                .as_object_mut()
                .unwrap()
                .remove("block_time_seconds");
            entry.1 = serde_json::to_vec(&snapshot).unwrap();
        }
        bundle.manifest.last_sequence = records.len() as u64 - 1;
        bundle.manifest.objects.clear();
        let path = source.with_extension("tampered-time");
        let hash = write_bundle(&path, bundle.manifest, bundle.objects, 64 * 1024 * 1024).unwrap();
        assert!(
            replay::verify_capture(&path, &hash, 100001).is_err(),
            "mutation {mutation}"
        );
        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn solana_chain_time_economic_replay_uses_frozen_historical_reference_and_age() {
    for block_time in [json!(90), Value::Null] {
        let fixtures = [
            chain_time_fixture(0, block_time.clone()),
            chain_time_fixture(1, block_time.clone()),
        ];
        let (_, config) =
            replay::load_evaluation_capture(&fixtures[0].0, &fixtures[0].1, 100001).unwrap();
        let mut request = replay::ReplayEvaluationRequest {
            schema_version: 1,
            session_id: "offline-chain-time".into(),
            experiment_id: "manual-chain-time-fixture".into(),
            strategy_id: config.strategy_ids()[0].clone(),
            network_id: arb_domain::NetworkId::SolanaMainnet,
            generation: 1,
            observed_at_unix_ms: 100000,
            input_age_ms: 1,
            captures: fixtures
                .iter()
                .map(|(path, hash)| replay::ReplayCaptureInput {
                    path: path.to_str().unwrap().into(),
                    manifest_digest: hash.clone(),
                })
                .collect(),
        };
        let first = replay::evaluate_captures(&request, 100001).unwrap();
        assert_eq!(first, replay::evaluate_captures(&request, 999999).unwrap());
        assert_eq!(first["network_requests"], 0);
        for decision in first["decisions"].as_array().unwrap() {
            assert_eq!(decision["schema_version"], "1.1.0");
            assert_eq!(
                decision["chain_freshness"]["reference_observed_at_unix_ms"],
                100000
            );
            assert_eq!(decision["chain_freshness"]["evaluation_elapsed_ms"], 1);
            assert_eq!(
                decision["chain_freshness"]["status"],
                if block_time.is_null() {
                    "UNKNOWN"
                } else {
                    "WITHIN_POLICY"
                }
            );
        }
        request.input_age_ms = 20001;
        let aged = replay::evaluate_captures(&request, 100001).unwrap();
        assert_eq!(aged["decisions"].as_array().unwrap().len(), 1);
        let decision = &aged["decisions"][0];
        assert_eq!(decision["result"]["status"], "DATA_UNAVAILABLE");
        assert_eq!(
            decision["result"]["reason_codes"],
            json!([if block_time.is_null() {
                "CHAIN_TIME_UNAVAILABLE"
            } else {
                "CHAIN_TIME_STALE"
            }])
        );
        assert_eq!(decision["chain_freshness"]["evaluation_elapsed_ms"], 20001);
        assert_ne!(
            first["decisions"][0]["observation_id"],
            decision["observation_id"]
        );
        for (path, _) in fixtures {
            fs::remove_dir_all(path).unwrap();
        }
    }
}
