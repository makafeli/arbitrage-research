//! Immutable startup diagnostics. Structural scope is never protocol qualification.
use super::{AppState, Json, State};
use arb_config::ValidatedConfig;
use arb_domain::{ChainFreshnessPolicy, NetworkId};
use arb_registry::{PoolRegistry, RegistryDocument};
use serde::Serialize;
use std::{collections::BTreeSet, io::Read};

const MAX_CONFIGURATIONS: usize = 16;
const MAX_REGISTRIES: usize = 16;
const MAX_DECLARED_ASSETS: usize = 16;

#[derive(Clone, Debug, Serialize)]
pub(super) struct AdapterSupport {
    schema_version: &'static str,
    catalog_version: &'static str,
    configurations: Vec<ConfigurationSupport>,
}
impl AdapterSupport {
    #[cfg(test)]
    pub(super) fn empty() -> Self {
        Self {
            schema_version: "1.0.0",
            catalog_version: "immutable-scope-v1",
            configurations: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
struct ConfigurationSupport {
    configuration_digest: String,
    networks: Vec<NetworkSupport>,
}
#[derive(Clone, Debug, Serialize)]
struct NetworkSupport {
    network_id: NetworkId,
    configured_enabled: bool,
    declared_scope: DeclaredScope,
    registry: RegistrySupport,
    chain_freshness: FreshnessSupport,
    capability: arb_engine::CapabilityReport,
}
#[derive(Clone, Debug, Serialize)]
struct DeclaredScope {
    asset_ids: Vec<String>,
    pool_ids: Vec<String>,
    registry_digest: Option<String>,
    asset_count: usize,
    pool_count: usize,
    identities_expanded: bool,
    reason_codes: Vec<&'static str>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RegistryStatus {
    NotLoaded,
    LoadedAuthorized,
    LoadedBlocked,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RegistryReason {
    RegistryNotLoaded,
    NetworkDisabled,
    RegistryDigestMismatch,
    RegistryScopeMismatch,
}
#[derive(Clone, Debug, Serialize)]
struct RegistrySupport {
    status: RegistryStatus,
    available_digests: Vec<String>,
    loaded_digest: Option<String>,
    reason_codes: Vec<RegistryReason>,
    pools: Vec<PoolSupport>,
}
#[derive(Clone, Debug, Serialize)]
struct PoolSupport {
    pool_id: String,
    asset_ids: [String; 2],
    venue_family: &'static str,
    programs: [ProgramSupport; 2],
}
#[derive(Clone, Debug, Serialize)]
struct ProgramSupport {
    role: ProgramRole,
    address: String,
    runtime_digest: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ProgramRole {
    Factory,
    PoolRuntime,
    VenueProgram,
    ProgramData,
}
#[derive(Clone, Debug, Serialize)]
struct FreshnessSupport {
    status: FreshnessStatus,
    policy: Option<ChainFreshnessPolicy>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum FreshnessStatus {
    NotConfigured,
    Configured,
}

pub(super) async fn adapter_support(State(state): State<AppState>) -> Json<AdapterSupport> {
    Json(state.0.config.adapter_support.clone())
}

pub(super) fn load_from_env() -> Result<Vec<RegistryDocument>, String> {
    let setting = std::env::var("ARB_SUPPORT_REGISTRY_FILES").ok();
    load_registry_documents(setting.as_deref(), |path| {
        let file = std::fs::File::open(path)
            .map_err(|_| "An adapter support registry could not be read".to_owned())?;
        let mut bytes = Vec::new();
        file.take(arb_registry::MAX_REGISTRY_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "An adapter support registry could not be read".to_owned())?;
        Ok(bytes)
    })
}

fn load_registry_documents(
    setting: Option<&str>,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<RegistryDocument>, String> {
    let Some(setting) = setting else {
        return Ok(Vec::new());
    };
    if setting.len() > 32768 {
        return Err("ARB_SUPPORT_REGISTRY_FILES exceeds its setting bound".into());
    }
    let entries: Vec<_> = setting.split(',').map(str::trim).collect();
    if entries.is_empty() || entries.len() > MAX_REGISTRIES {
        return Err("ARB_SUPPORT_REGISTRY_FILES requires 1 through 16 entries".into());
    }
    let mut registries = Vec::new();
    let mut unique = BTreeSet::new();
    for entry in entries {
        let (network, path) = entry
            .split_once('=')
            .ok_or("ARB_SUPPORT_REGISTRY_FILES entries require network=path")?;
        let network = match network {
            "base-mainnet" => NetworkId::BaseMainnet,
            "solana-mainnet" => NetworkId::SolanaMainnet,
            _ => return Err("Adapter support registry network is unsupported".into()),
        };
        if path.is_empty() || path.len() > 2048 {
            return Err("Adapter support registry path is empty or exceeds bounds".into());
        }
        let document = RegistryDocument::from_bytes(&read(path)?, network)
            .map_err(|_| "An adapter support registry failed structural validation".to_owned())?;
        if !unique.insert((network.to_string(), document.digest().to_owned())) {
            return Err("Duplicate adapter support registry document".into());
        }
        registries.push(document);
    }
    Ok(registries)
}

pub(super) fn build(
    configs: &[ValidatedConfig],
    registries: &[RegistryDocument],
) -> Result<AdapterSupport, String> {
    if configs.len() > MAX_CONFIGURATIONS || registries.len() > MAX_REGISTRIES {
        return Err("Adapter support catalog exceeds configuration or registry bounds".into());
    }
    let mut configurations = Vec::new();
    let mut digests = BTreeSet::new();
    for config in configs {
        if !digests.insert(config.digest()) {
            continue;
        }
        let mut networks = Vec::new();
        for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
            let asset_count = config.verified_assets(network).len();
            let pool_count = config.verified_pools(network).len();
            let identities_expanded =
                asset_count <= MAX_DECLARED_ASSETS && pool_count <= arb_registry::MAX_POOLS;
            let available: Vec<_> = registries
                .iter()
                .filter(|document| document.network() == network)
                .collect();
            let selected = available.iter().copied().find(|document| {
                config.registry_qualification_digest(network) == Some(document.digest())
            });
            let (status, reason_codes) = if available.is_empty() {
                (
                    RegistryStatus::NotLoaded,
                    vec![RegistryReason::RegistryNotLoaded],
                )
            } else if !config.network_enabled(network) {
                (
                    RegistryStatus::LoadedBlocked,
                    vec![RegistryReason::NetworkDisabled],
                )
            } else if selected.is_none() {
                (
                    RegistryStatus::LoadedBlocked,
                    vec![RegistryReason::RegistryDigestMismatch],
                )
            } else if selected.is_some_and(|document| document.authorize(config).is_err()) {
                (
                    RegistryStatus::LoadedBlocked,
                    vec![RegistryReason::RegistryScopeMismatch],
                )
            } else {
                (RegistryStatus::LoadedAuthorized, Vec::new())
            };
            let capability = arb_engine::capabilities(network);
            let pools = if matches!(status, RegistryStatus::LoadedAuthorized) {
                selected
                    .expect("authorized registry exists")
                    .pools()
                    .iter()
                    .map(|pool| pool_support(pool, capability.venue_family))
                    .collect()
            } else {
                Vec::new()
            };
            let mut available_digests: Vec<_> = available
                .iter()
                .map(|document| document.digest().to_owned())
                .collect();
            available_digests.sort();
            let policy = config.chain_freshness(network).cloned();
            networks.push(NetworkSupport {
                network_id: network,
                configured_enabled: config.network_enabled(network),
                declared_scope: DeclaredScope {
                    asset_ids: if identities_expanded {
                        config
                            .verified_assets(network)
                            .iter()
                            .map(ToString::to_string)
                            .collect()
                    } else {
                        Vec::new()
                    },
                    pool_ids: if identities_expanded {
                        config
                            .verified_pools(network)
                            .iter()
                            .map(ToString::to_string)
                            .collect()
                    } else {
                        Vec::new()
                    },
                    registry_digest: config
                        .registry_qualification_digest(network)
                        .map(str::to_owned),
                    asset_count,
                    pool_count,
                    identities_expanded,
                    reason_codes: if identities_expanded {
                        Vec::new()
                    } else {
                        vec!["DECLARED_SCOPE_NOT_EXPANDED"]
                    },
                },
                registry: RegistrySupport {
                    status,
                    available_digests,
                    loaded_digest: selected.map(|document| document.digest().to_owned()),
                    reason_codes,
                    pools,
                },
                chain_freshness: FreshnessSupport {
                    status: if policy.is_some() {
                        FreshnessStatus::Configured
                    } else {
                        FreshnessStatus::NotConfigured
                    },
                    policy,
                },
                capability,
            });
        }
        configurations.push(ConfigurationSupport {
            configuration_digest: config.digest().to_owned(),
            networks,
        });
    }
    configurations.sort_by(|a, b| a.configuration_digest.cmp(&b.configuration_digest));
    Ok(AdapterSupport {
        schema_version: "1.0.0",
        catalog_version: "immutable-scope-v1",
        configurations,
    })
}
fn address(network: NetworkId, raw: &str) -> String {
    match network {
        NetworkId::BaseMainnet => raw.to_ascii_lowercase(),
        NetworkId::SolanaMainnet => raw.to_owned(),
    }
}
fn pool_support(pool: &PoolRegistry, venue_family: &'static str) -> PoolSupport {
    let network = pool.network();
    let program = pool.venue_program_address();
    let programs = match pool {
        PoolRegistry::Base(registry) => [
            ProgramSupport {
                role: ProgramRole::Factory,
                address: address(network, program),
                runtime_digest: Some(registry.factory_runtime_sha256.to_ascii_lowercase()),
            },
            ProgramSupport {
                role: ProgramRole::PoolRuntime,
                address: address(network, &registry.pool),
                runtime_digest: Some(registry.pool_runtime_sha256.to_ascii_lowercase()),
            },
        ],
        PoolRegistry::Solana(registry) => [
            ProgramSupport {
                role: ProgramRole::VenueProgram,
                address: address(network, program),
                runtime_digest: None,
            },
            ProgramSupport {
                role: ProgramRole::ProgramData,
                address: address(network, &registry.program_data),
                runtime_digest: Some(registry.program_data_sha256.to_ascii_lowercase()),
            },
        ],
    };
    PoolSupport {
        pool_id: format!("{network}:{}", address(network, pool.pool())),
        asset_ids: pool
            .assets()
            .map(|asset| format!("{network}:{}", address(network, asset))),
        venue_family,
        programs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture(network: NetworkId) -> (ValidatedConfig, RegistryDocument) {
        let inert =
            ValidatedConfig::from_toml(include_str!("../../../config/research.example.toml"))
                .unwrap();
        let raw = match network {
            NetworkId::BaseMainnet => {
                include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json").as_slice()
            }
            NetworkId::SolanaMainnet => {
                include_bytes!("../../../crates/arb-solana/tests/fixtures/registry.json").as_slice()
            }
        };
        let first: Value = serde_json::from_slice(raw).unwrap();
        let mut second = first.clone();
        second["pool"] = match network {
            NetworkId::BaseMainnet => json!("0x9999999999999999999999999999999999999999"),
            NetworkId::SolanaMainnet => json!("US517G5965aydkZ46HS38QLi7UQiSojurfbQfKCELFx"),
        };
        if network == NetworkId::SolanaMainnet {
            second["tick_arrays"] = json!([first["pool"]]);
        }
        let registry = RegistryDocument::from_bytes(
            &serde_json::to_vec(
                &json!({"schema_version":1,"network_id":network,"pools":[first,second]}),
            )
            .unwrap(),
            network,
        )
        .unwrap();
        let mut config: Value = serde_json::from_str(inert.effective_json()).unwrap();
        config["research"]["trade_sizes_minor"] = json!(["100"]);
        let key = if network == NetworkId::BaseMainnet {
            "base"
        } else {
            "solana"
        };
        let section = &mut config["networks"][key];
        section["enabled"] = json!(true);
        section["verified_pool_ids"] = json!(
            registry
                .pools()
                .iter()
                .map(|p| format!("{network}:{}", p.pool()))
                .collect::<Vec<_>>()
        );
        section["verified_asset_ids"] = json!(
            registry.pools()[0]
                .assets()
                .map(|a| format!("{network}:{a}"))
        );
        section["starting_asset_id"] =
            json!(format!("{network}:{}", registry.pools()[0].assets()[0]));
        section["rpc_secret_reference"] = json!("env:PRIVATE_PROVIDER_CREDENTIAL");
        section["registry_qualification_digest"] = json!(registry.digest());
        section["chain_freshness"] =
            json!({"version":"finalized-chain-time-v1","max_chain_age_ms":90000});
        if let PoolRegistry::Solana(registry) = &registry.pools()[0] {
            section["expected_genesis_identity"] = json!(registry.expected_genesis_hash);
        }
        (
            ValidatedConfig::from_effective_json(&config.to_string()).unwrap(),
            registry,
        )
    }
    fn report(
        config: &ValidatedConfig,
        registries: &[RegistryDocument],
        network: NetworkId,
    ) -> Value {
        let value =
            serde_json::to_value(build(std::slice::from_ref(config), registries).unwrap()).unwrap();
        value["configurations"][0]["networks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["network_id"] == network.as_str())
            .unwrap()
            .clone()
    }
    #[test]
    fn structural_catalog_cannot_promote_capabilities_and_exposes_only_authorized_relationships() {
        for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
            let (config, registry) = fixture(network);
            let loaded = report(&config, std::slice::from_ref(&registry), network);
            assert_eq!(loaded["registry"]["status"], "LOADED_AUTHORIZED");
            assert_eq!(loaded["registry"]["loaded_digest"], registry.digest());
            assert_eq!(loaded["registry"]["reason_codes"], json!([]));
            assert_eq!(loaded["registry"]["pools"].as_array().unwrap().len(), 2);
            assert_eq!(loaded["declared_scope"]["asset_count"], 2);
            assert_eq!(loaded["declared_scope"]["identities_expanded"], true);
            assert_eq!(
                loaded["chain_freshness"]["policy"]["max_chain_age_ms"],
                90000
            );
            assert_eq!(
                loaded["capability"],
                serde_json::to_value(arb_engine::capabilities(network)).unwrap()
            );
            for capability in [
                "qualified_quote",
                "transaction_build",
                "full_transaction_simulation",
                "submit",
            ] {
                assert_eq!(loaded["capability"][capability], false);
            }
            let serialized = loaded.to_string();
            for excluded in [
                "PRIVATE_PROVIDER_CREDENTIAL",
                "qualification_reference",
                "MANUALLY-CONSTRUCTED",
                "rpc_secret_reference",
            ] {
                assert!(!serialized.contains(excluded));
            }
            let programs = &loaded["registry"]["pools"][0]["programs"];
            assert_eq!(
                programs[0]["address"],
                registry.pools()[0].venue_program_address()
            );
            assert_eq!(programs.as_array().unwrap().len(), 2);
        }
    }
    #[test]
    fn missing_disabled_mismatched_and_outside_scope_registries_have_no_invented_pairs() {
        let (config, registry) = fixture(NetworkId::BaseMainnet);
        let missing = report(&config, &[], NetworkId::BaseMainnet);
        assert_eq!(missing["registry"]["status"], "NOT_LOADED");
        assert_eq!(
            missing["registry"]["reason_codes"],
            json!(["REGISTRY_NOT_LOADED"])
        );
        for (field, replacement, expected) in [
            ("enabled", json!(false), "NETWORK_DISABLED"),
            (
                "registry_qualification_digest",
                json!(format!("sha256:{}", "a".repeat(64))),
                "REGISTRY_DIGEST_MISMATCH",
            ),
            (
                "verified_pool_ids",
                json!([
                    "base-mainnet:0x8888888888888888888888888888888888888888",
                    "base-mainnet:0x7777777777777777777777777777777777777777"
                ]),
                "REGISTRY_SCOPE_MISMATCH",
            ),
        ] {
            let mut modified: Value = serde_json::from_str(config.effective_json()).unwrap();
            modified["networks"]["base"][field] = replacement;
            let config = ValidatedConfig::from_effective_json(&modified.to_string()).unwrap();
            let blocked = report(
                &config,
                std::slice::from_ref(&registry),
                NetworkId::BaseMainnet,
            );
            assert_eq!(blocked["registry"]["status"], "LOADED_BLOCKED");
            assert_eq!(blocked["registry"]["reason_codes"], json!([expected]));
            assert_eq!(blocked["registry"]["pools"], json!([]));
        }
    }
    #[test]
    fn larger_existing_allowlists_and_duplicate_configs_keep_control_startup_compatible() {
        let (config, registry) = fixture(NetworkId::BaseMainnet);
        let mut value: Value = serde_json::from_str(config.effective_json()).unwrap();
        let pools = value["networks"]["base"]["verified_pool_ids"]
            .as_array_mut()
            .unwrap();
        for n in 10..18 {
            pools.push(json!(format!("base-mainnet:0x{n:040x}")));
        }
        let config = ValidatedConfig::from_effective_json(&value.to_string()).unwrap();
        let built = build(
            &[config.clone(), config.clone()],
            std::slice::from_ref(&registry),
        )
        .unwrap();
        assert_eq!(built.configurations.len(), 1);
        let loaded = report(&config, &[registry], NetworkId::BaseMainnet);
        assert_eq!(loaded["declared_scope"]["pool_count"], 10);
        assert_eq!(loaded["declared_scope"]["asset_count"], 2);
        assert_eq!(loaded["declared_scope"]["identities_expanded"], false);
        assert_eq!(loaded["declared_scope"]["pool_ids"], json!([]));
        assert_eq!(loaded["declared_scope"]["asset_ids"], json!([]));
        assert_eq!(
            loaded["declared_scope"]["reason_codes"],
            json!(["DECLARED_SCOPE_NOT_EXPANDED"])
        );
        assert_eq!(loaded["registry"]["status"], "LOADED_AUTHORIZED");
        assert_eq!(loaded["registry"]["pools"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn registry_loading_is_bounded_rejects_duplicates_and_never_echoes_paths_or_content() {
        let raw = include_bytes!("../../../crates/arb-evm/tests/fixtures/registry.json").to_vec();
        let mut calls = 0;
        assert!(
            load_registry_documents(None, |_| {
                calls += 1;
                Ok(raw.clone())
            })
            .unwrap()
            .is_empty()
        );
        assert_eq!(calls, 0);
        for input in [
            String::new(),
            "base-mainnet".into(),
            "base-mainnet=".into(),
            "ethereum-mainnet=/private/path".into(),
            "base-mainnet=/private/path,base-mainnet=/other/path".into(),
            vec!["base-mainnet=/private/path"; 17].join(","),
        ] {
            let error = load_registry_documents(Some(&input), |_| Ok(raw.clone())).unwrap_err();
            assert!(!error.contains("/private/path"));
            assert!(!error.contains("/other/path"));
        }
        assert!(
            load_registry_documents(Some("base-mainnet=/private/path"), |_| Ok(vec![
                0;
                arb_registry::MAX_REGISTRY_BYTES
                    + 1
            ]))
            .is_err()
        );
        assert!(
            load_registry_documents(Some("solana-mainnet=/private/path"), |_| Ok(raw.clone()))
                .is_err()
        );
        assert_eq!(
            load_registry_documents(Some("base-mainnet=/private/path"), |_| Ok(raw.clone()))
                .unwrap()
                .len(),
            1
        );
        let mut bad: Value = serde_json::from_slice(&raw).unwrap();
        bad["submit"] = json!(true);
        assert!(
            load_registry_documents(Some("base-mainnet=/private/path"), |_| Ok(
                serde_json::to_vec(&bad).unwrap()
            ))
            .is_err()
        );
    }
}
