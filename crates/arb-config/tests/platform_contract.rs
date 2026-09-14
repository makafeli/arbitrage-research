//! Cross-boundary configuration regressions. All configurations are synthetic;
//! no registry or provider is qualified and no secret reference is resolved.
use arb_config::ValidatedConfig;
use arb_domain::{Mode, NetworkId, State};
use serde_json::{Value, json};

const A: &str = "base-mainnet:0x0000000000000000000000000000000000000003";
const B: &str = "base-mainnet:0x0000000000000000000000000000000000000004";

fn config(mode: &str, starting_asset: &str) -> ValidatedConfig {
    let inert =
        ValidatedConfig::from_toml(include_str!("../../../config/research.example.toml")).unwrap();
    let mut value: Value = serde_json::from_str(inert.effective_json()).unwrap();
    value["deployment"]["mode"] = json!(mode);
    value["research"]["trade_sizes_minor"] = json!(["1000000", "9007199254740993"]);
    value["networks"]["base"]["enabled"] = json!(true);
    value["networks"]["base"]["verified_asset_ids"] = json!([A, B]);
    value["networks"]["base"]["verified_pool_ids"] = json!([
        "base-mainnet:0x0000000000000000000000000000000000000001",
        "base-mainnet:0x0000000000000000000000000000000000000002"
    ]);
    value["networks"]["base"]["rpc_secret_reference"] = json!("env:SYNTHETIC_ONLY_RPC");
    value["networks"]["base"]["registry_qualification_digest"] =
        json!(format!("sha256:{}", "a".repeat(64)));
    value["networks"]["base"]["starting_asset_id"] = json!(starting_asset);
    ValidatedConfig::from_effective_json(&value.to_string()).unwrap()
}

#[test]
fn changed_starting_asset_and_mode_require_stopped_and_keep_old_snapshot_intact() {
    let original = config("PAPER", A);
    let session = original.freeze_session(NetworkId::BaseMainnet).unwrap();
    let original_bytes = original.effective_json().to_owned();
    let original_frozen = serde_json::to_vec(&session).unwrap();
    for replacement_config in [
        config("PAPER", B),
        config("OBSERVE", A),
        config("REPLAY", B),
    ] {
        for state in [
            State::Recovering,
            State::Running,
            State::Pausing,
            State::Paused,
            State::Draining,
            State::Faulted,
        ] {
            let error = session
                .replacement(state, &replacement_config, NetworkId::BaseMainnet)
                .unwrap_err();
            assert_eq!(error.field, "session.state");
        }
        let replacement = session
            .replacement(State::Stopped, &replacement_config, NetworkId::BaseMainnet)
            .unwrap();
        assert_eq!(
            replacement.configuration_digest(),
            replacement_config.digest()
        );
        assert_eq!(
            replacement.starting_asset(),
            replacement_config.starting_asset(NetworkId::BaseMainnet)
        );
        assert_ne!(
            replacement.configuration_digest(),
            session.configuration_digest()
        );
        assert_eq!(original.effective_json(), original_bytes);
        assert_eq!(serde_json::to_vec(&session).unwrap(), original_frozen);
    }
    assert_eq!(session.mode(), Mode::Paper);
    assert_eq!(session.starting_asset().unwrap().to_string(), A);
}

#[test]
fn persisted_effective_configuration_keeps_exact_units_and_set_canonicalization() {
    let original = config("PAPER", A);
    let mut reordered: Value = serde_json::from_str(original.effective_json()).unwrap();
    reordered["networks"]["base"]["verified_asset_ids"] = json!([B, A]);
    reordered["networks"]["base"]["verified_pool_ids"]
        .as_array_mut()
        .unwrap()
        .reverse();
    reordered["research"]["trade_sizes_minor"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let pretty = serde_json::to_string_pretty(&reordered).unwrap();
    let restored = ValidatedConfig::from_effective_json(&pretty).unwrap();
    assert_eq!(restored.digest(), original.digest());
    assert_eq!(restored.effective_json(), original.effective_json());
    assert_eq!(restored.trade_sizes()[1].to_string(), "9007199254740993");
    assert_eq!(
        restored.freeze_session(NetworkId::BaseMainnet).unwrap(),
        original.freeze_session(NetworkId::BaseMainnet).unwrap(),
    );
}

#[test]
fn invalid_effective_configuration_errors_never_echo_inline_secrets() {
    let original = config("PAPER", A);
    for pointer in [
        "/networks/base/rpc_secret_reference",
        "/storage/database_secret_reference",
    ] {
        let mut value: Value = serde_json::from_str(original.effective_json()).unwrap();
        *value.pointer_mut(pointer).unwrap() =
            json!("https://user:NEVER_EXPORT_THIS@example.invalid");
        let error = ValidatedConfig::from_effective_json(&value.to_string()).unwrap_err();
        assert!(!error.to_string().contains("NEVER_EXPORT_THIS"));
        assert!(!format!("{error:?}").contains("NEVER_EXPORT_THIS"));
        assert!(!error.field.is_empty());
        assert!(!error.reason.is_empty());
    }
}
