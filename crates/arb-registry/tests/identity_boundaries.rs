//! Catalogue data is observed identity evidence, never an executable registry.
use arb_domain::NetworkId;
use arb_registry::RegistryDocument;
use serde_json::{Value, json};

#[test]
fn reviewed_inventory_is_not_accepted_as_a_runtime_registry() {
    let bytes = include_bytes!("../../../docs/registries/initial-identities.json");
    for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
        assert!(RegistryDocument::from_bytes(bytes, network).is_err());
    }
}

#[test]
fn base_production_parser_rejects_fixture_pool_and_asset_identities() {
    let source = include_str!("../../arb-evm/tests/fixtures/registry.json");
    let original: Value = serde_json::from_str(source).unwrap();
    for field in ["pool", "token0", "token1"] {
        let mut pool = original.clone();
        pool[field] = json!("fixture:identity");
        let bytes = serde_json::to_vec(&pool).unwrap();
        assert!(RegistryDocument::from_bytes(&bytes, NetworkId::BaseMainnet).is_err());
    }
}

#[test]
fn solana_production_parser_rejects_fixture_accounts_and_genesis() {
    let source = include_str!("../../arb-solana/tests/fixtures/registry.json");
    let original: Value = serde_json::from_str(source).unwrap();
    for field in [
        "pool",
        "expected_genesis_hash",
        "whirlpools_config",
        "program_data",
        "mint_a",
        "mint_b",
        "vault_a",
        "vault_b",
    ] {
        let mut pool = original.clone();
        pool[field] = json!("fixture:identity");
        let bytes = serde_json::to_vec(&pool).unwrap();
        assert!(RegistryDocument::from_bytes(&bytes, NetworkId::SolanaMainnet).is_err());
    }
    let mut pool = original;
    pool["tick_arrays"] = json!(["fixture:tick-array"]);
    let bytes = serde_json::to_vec(&pool).unwrap();
    assert!(RegistryDocument::from_bytes(&bytes, NetworkId::SolanaMainnet).is_err());
}
