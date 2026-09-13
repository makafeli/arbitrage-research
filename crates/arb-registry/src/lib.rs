//! Bounded registry documents shared by capture and offline replay.
//! Structural authorization never establishes protocol or provider qualification.
use arb_config::ValidatedConfig;
use arb_domain::NetworkId;
use serde::Deserialize;
use std::{collections::BTreeSet, fmt};

pub const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
pub const MAX_POOLS: usize = 8;

#[derive(Clone, Debug)]
pub enum PoolRegistry {
    Base(arb_evm::PoolRegistry),
    Solana(arb_solana::PoolRegistry),
}
impl PoolRegistry {
    pub fn pool(&self) -> &str {
        match self {
            Self::Base(r) => &r.pool,
            Self::Solana(r) => &r.pool,
        }
    }
    pub fn assets(&self) -> [&str; 2] {
        match self {
            Self::Base(r) => [&r.token0, &r.token1],
            Self::Solana(r) => [&r.mint_a, &r.mint_b],
        }
    }
    pub fn network(&self) -> NetworkId {
        match self {
            Self::Base(_) => NetworkId::BaseMainnet,
            Self::Solana(_) => NetworkId::SolanaMainnet,
        }
    }
    fn validate(&self) -> Result<(), RegistryError> {
        match self {
            Self::Base(r) => r
                .validate()
                .map_err(|_| RegistryError("invalid Base pool registry")),
            Self::Solana(r) => r
                .validate()
                .map_err(|_| RegistryError("invalid Solana pool registry")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentFormat {
    LegacySinglePool,
    PoolSetV1,
}

#[derive(Debug)]
pub struct RegistryError(pub &'static str);
impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for RegistryError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolSet<T> {
    schema_version: u32,
    network_id: NetworkId,
    pools: Vec<T>,
}

#[derive(Clone, Debug)]
pub struct RegistryDocument {
    network: NetworkId,
    format: DocumentFormat,
    digest: String,
    pools: Vec<PoolRegistry>,
}
impl RegistryDocument {
    pub fn from_bytes(bytes: &[u8], network: NetworkId) -> Result<Self, RegistryError> {
        if bytes.is_empty() || bytes.len() > MAX_REGISTRY_BYTES {
            return Err(RegistryError("registry document exceeds byte bounds"));
        }
        let shape: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| RegistryError("invalid registry JSON"))?;
        let is_set = shape.get("pools").is_some();
        // Deserialize original bytes again to reject duplicate struct fields.
        let (format, version, declared_network, pools) = match (network, is_set) {
            (NetworkId::BaseMainnet, true) => {
                let set: PoolSet<arb_evm::PoolRegistry> = serde_json::from_slice(bytes)
                    .map_err(|_| RegistryError("invalid Base pool set"))?;
                (
                    DocumentFormat::PoolSetV1,
                    set.schema_version,
                    set.network_id,
                    set.pools
                        .into_iter()
                        .map(PoolRegistry::Base)
                        .collect::<Vec<_>>(),
                )
            }
            (NetworkId::SolanaMainnet, true) => {
                let set: PoolSet<arb_solana::PoolRegistry> = serde_json::from_slice(bytes)
                    .map_err(|_| RegistryError("invalid Solana pool set"))?;
                (
                    DocumentFormat::PoolSetV1,
                    set.schema_version,
                    set.network_id,
                    set.pools
                        .into_iter()
                        .map(PoolRegistry::Solana)
                        .collect::<Vec<_>>(),
                )
            }
            (NetworkId::BaseMainnet, false) => {
                let pool = serde_json::from_slice(bytes)
                    .map_err(|_| RegistryError("invalid legacy Base registry"))?;
                (
                    DocumentFormat::LegacySinglePool,
                    1,
                    network,
                    vec![PoolRegistry::Base(pool)],
                )
            }
            (NetworkId::SolanaMainnet, false) => {
                let pool = serde_json::from_slice(bytes)
                    .map_err(|_| RegistryError("invalid legacy Solana registry"))?;
                (
                    DocumentFormat::LegacySinglePool,
                    1,
                    network,
                    vec![PoolRegistry::Solana(pool)],
                )
            }
        };
        if version != 1
            || declared_network != network
            || pools.is_empty()
            || pools.len() > MAX_POOLS
        {
            return Err(RegistryError(
                "unsupported registry version, network or pool count",
            ));
        }
        let mut unique = BTreeSet::new();
        for pool in &pools {
            pool.validate()?;
            if !unique.insert(canonical(network, pool.pool())) {
                return Err(RegistryError("duplicate pool in registry document"));
            }
        }
        Ok(Self {
            network,
            format,
            digest: arb_capture::digest(bytes),
            pools,
        })
    }
    pub fn network(&self) -> NetworkId {
        self.network
    }
    pub fn format(&self) -> DocumentFormat {
        self.format
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn pools(&self) -> &[PoolRegistry] {
        &self.pools
    }
    pub fn select(&self, address: &str) -> Result<&PoolRegistry, RegistryError> {
        let key = canonical(self.network, address);
        self.pools
            .iter()
            .find(|p| canonical(self.network, p.pool()) == key)
            .ok_or(RegistryError(
                "snapshot pool is absent from registry document",
            ))
    }
    pub fn adapter_version(&self) -> &'static str {
        match (self.network, self.format) {
            (NetworkId::BaseMainnet, DocumentFormat::LegacySinglePool) => "arb_evm-v1",
            (NetworkId::SolanaMainnet, DocumentFormat::LegacySinglePool) => "arb_solana-v1",
            (NetworkId::BaseMainnet, DocumentFormat::PoolSetV1) => "arb_evm-pool-set-v2",
            (NetworkId::SolanaMainnet, DocumentFormat::PoolSetV1) => "arb_solana-pool-set-v2",
        }
    }
    /// Historical pool-set captures retained an independent transcript per pool.
    /// Registry schema remains v1; the adapter version distinguishes acquisition.
    pub fn legacy_adapter_version(&self) -> &'static str {
        match (self.network, self.format) {
            (NetworkId::BaseMainnet, DocumentFormat::PoolSetV1) => "arb_evm-pool-set-v1",
            (NetworkId::SolanaMainnet, DocumentFormat::PoolSetV1) => "arb_solana-pool-set-v1",
            _ => self.adapter_version(),
        }
    }
    pub fn authorize(&self, config: &ValidatedConfig) -> Result<(), RegistryError> {
        if !config.network_enabled(self.network)
            || config.registry_qualification_digest(self.network) != Some(self.digest())
        {
            return Err(RegistryError(
                "registry differs from enabled immutable configuration",
            ));
        }
        for pool in &self.pools {
            if !config.verified_pools(self.network).iter().any(|allowed| {
                canonical(self.network, allowed.address()) == canonical(self.network, pool.pool())
            }) || pool.assets().iter().any(|asset| {
                !config.verified_assets(self.network).iter().any(|allowed| {
                    canonical(self.network, allowed.address()) == canonical(self.network, asset)
                })
            }) {
                return Err(RegistryError(
                    "registry pool or asset is outside immutable allowlists",
                ));
            }
            if let PoolRegistry::Solana(r) = pool
                && r.expected_genesis_hash != config.expected_genesis_identity()
            {
                return Err(RegistryError(
                    "registry genesis differs from immutable configuration",
                ));
            }
        }
        Ok(())
    }
}
fn canonical(network: NetworkId, value: &str) -> String {
    match network {
        NetworkId::BaseMainnet => value.to_ascii_lowercase(),
        NetworkId::SolanaMainnet => value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn pool() -> Value {
        serde_json::from_str(include_str!("../../arb-evm/tests/fixtures/registry.json")).unwrap()
    }
    fn document(pools: Vec<Value>) -> Vec<u8> {
        serde_json::to_vec(&json!({"schema_version":1,"network_id":"base-mainnet","pools":pools}))
            .unwrap()
    }
    #[test]
    fn legacy_and_pool_set_are_distinct_and_exactly_addressed() {
        let p = pool();
        let legacy =
            RegistryDocument::from_bytes(&serde_json::to_vec(&p).unwrap(), NetworkId::BaseMainnet)
                .unwrap();
        assert_eq!(legacy.format(), DocumentFormat::LegacySinglePool);
        assert_eq!(legacy.adapter_version(), "arb_evm-v1");
        let mut second = p.clone();
        second["pool"] = json!("0x9999999999999999999999999999999999999999");
        let bytes = document(vec![p.clone(), second.clone()]);
        let set = RegistryDocument::from_bytes(&bytes, NetworkId::BaseMainnet).unwrap();
        assert_eq!(set.format(), DocumentFormat::PoolSetV1);
        assert_eq!(set.digest(), arb_capture::digest(&bytes));
        assert_eq!(set.adapter_version(), "arb_evm-pool-set-v2");
        assert_eq!(set.legacy_adapter_version(), "arb_evm-pool-set-v1");
        assert_ne!(set.adapter_version(), legacy.adapter_version());
        assert_eq!(set.pools().len(), 2);
        assert_eq!(
            set.select(second["pool"].as_str().unwrap()).unwrap().pool(),
            second["pool"].as_str().unwrap()
        );
        assert!(
            set.select("0x8888888888888888888888888888888888888888")
                .is_err()
        );
    }
    #[test]
    fn duplicate_unknown_or_unbounded_documents_are_rejected() {
        let p = pool();
        for pools in [vec![], vec![p.clone(), p.clone()], vec![p.clone(); 9]] {
            assert!(
                RegistryDocument::from_bytes(&document(pools), NetworkId::BaseMainnet).is_err()
            );
        }
        let mut set = json!({"schema_version":1,"network_id":"base-mainnet","pools":[p]});
        set["extra"] = json!(true);
        assert!(
            RegistryDocument::from_bytes(
                &serde_json::to_vec(&set).unwrap(),
                NetworkId::BaseMainnet
            )
            .is_err()
        );
        let bytes = document(vec![pool()]);
        assert!(RegistryDocument::from_bytes(&bytes, NetworkId::SolanaMainnet).is_err());
        let duplicate = String::from_utf8(bytes).unwrap().replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
            1,
        );
        assert!(
            RegistryDocument::from_bytes(duplicate.as_bytes(), NetworkId::BaseMainnet).is_err()
        );
    }
    #[test]
    fn structural_validation_never_enables_disabled_configuration() {
        let config =
            ValidatedConfig::from_toml(include_str!("../../../config/research.example.toml"))
                .unwrap();
        let registry =
            RegistryDocument::from_bytes(&document(vec![pool()]), NetworkId::BaseMainnet).unwrap();
        assert!(registry.authorize(&config).is_err());
    }
}
