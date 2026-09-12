use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum NetworkId {
    #[serde(rename = "base-mainnet")]
    BaseMainnet,
    #[serde(rename = "solana-mainnet")]
    SolanaMainnet,
}
impl NetworkId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BaseMainnet => "base-mainnet",
            Self::SolanaMainnet => "solana-mainnet",
        }
    }
}
impl fmt::Display for NetworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for NetworkId {
    type Err = IdentityError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "base-mainnet" => Ok(Self::BaseMainnet),
            "solana-mainnet" => Ok(Self::SolanaMainnet),
            _ => Err(IdentityError::UnsupportedNetwork),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    UnsupportedNetwork,
    AddressRequired,
    InvalidAddress,
    FixtureInProduction,
    InvalidFixture,
    InvalidRouteLength,
    CrossNetwork,
    DiscontinuousRoute,
    RepeatedPool,
    NonCyclicRoute,
    SameAssetLeg,
}
impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedNetwork => "network must be base-mainnet or solana-mainnet",
            Self::AddressRequired => {
                "identity requires network and contract/mint address; symbols are not identities"
            }
            Self::InvalidAddress => "invalid canonical address for network",
            Self::FixtureInProduction => {
                "fixture identities are prohibited in production registries"
            }
            Self::InvalidFixture => {
                "synthetic identity must start with fixture: and contain a nonempty safe identifier"
            }
            Self::InvalidRouteLength => "initial research routes require exactly two legs",
            Self::CrossNetwork => "all route pools and assets must belong to one network",
            Self::DiscontinuousRoute => "each route output must equal the next input",
            Self::RepeatedPool => "a route must use distinct pools",
            Self::NonCyclicRoute => "route must finish in its starting asset",
            Self::SameAssetLeg => "a swap leg must change assets",
        })
    }
}
impl std::error::Error for IdentityError {}

fn validate_address(network: NetworkId, address: &str) -> Result<String, IdentityError> {
    if address.starts_with("fixture:") {
        return Err(IdentityError::FixtureInProduction);
    }
    match network {
        NetworkId::BaseMainnet => {
            let body = address
                .strip_prefix("0x")
                .ok_or(IdentityError::InvalidAddress)?;
            if body.len() != 40
                || !body.bytes().all(|b| b.is_ascii_hexdigit())
                || body.bytes().all(|b| b == b'0')
            {
                return Err(IdentityError::InvalidAddress);
            }
            // The key is case-insensitive; checksum/provenance checks belong to registry qualification.
            Ok(address.to_ascii_lowercase())
        }
        NetworkId::SolanaMainnet => {
            if !(32..=44).contains(&address.len()) {
                return Err(IdentityError::InvalidAddress);
            }
            let bytes = bs58::decode(address)
                .into_vec()
                .map_err(|_| IdentityError::InvalidAddress)?;
            if bytes.len() != 32
                || bs58::encode(&bytes).into_string() != address
                || bytes.iter().all(|b| *b == 0)
            {
                return Err(IdentityError::InvalidAddress);
            }
            Ok(address.to_owned())
        }
    }
}
macro_rules! identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $name {
            network: NetworkId,
            address: String,
        }
        impl $name {
            pub fn new(network: NetworkId, address: &str) -> Result<Self, IdentityError> {
                Ok(Self {
                    network,
                    address: validate_address(network, address)?,
                })
            }
            pub fn network(&self) -> NetworkId {
                self.network
            }
            pub fn address(&self) -> &str {
                &self.address
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.network, self.address)
            }
        }
        impl FromStr for $name {
            type Err = IdentityError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.starts_with("fixture:") {
                    return Err(IdentityError::FixtureInProduction);
                }
                let (network, address) = value
                    .split_once(':')
                    .ok_or(IdentityError::AddressRequired)?;
                Self::new(network.parse()?, address)
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_string())
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                String::deserialize(d)?.parse().map_err(de::Error::custom)
            }
        }
    };
}
identity!(AssetId);
identity!(PoolId);

/// Fixture IDs deliberately cannot coerce into an AssetId or PoolId.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureId(String);
impl FromStr for FixtureId {
    type Err = IdentityError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let suffix = value
            .strip_prefix("fixture:")
            .ok_or(IdentityError::InvalidFixture)?;
        if suffix.is_empty()
            || suffix.len() > 128
            || !suffix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b":-_".contains(&b))
        {
            return Err(IdentityError::InvalidFixture);
        }
        Ok(Self(value.into()))
    }
}
impl FixtureId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteLeg {
    pub pool_id: PoolId,
    pub asset_in: AssetId,
    pub asset_out: AssetId,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<RouteLeg>", into = "Vec<RouteLeg>")]
pub struct Route(Vec<RouteLeg>);
impl Route {
    pub fn new(legs: Vec<RouteLeg>) -> Result<Self, IdentityError> {
        if legs.len() != 2 {
            return Err(IdentityError::InvalidRouteLength);
        }
        let network = legs[0].asset_in.network();
        for (index, leg) in legs.iter().enumerate() {
            if leg.pool_id.network() != network
                || leg.asset_in.network() != network
                || leg.asset_out.network() != network
            {
                return Err(IdentityError::CrossNetwork);
            }
            if leg.asset_in == leg.asset_out {
                return Err(IdentityError::SameAssetLeg);
            }
            if legs[..index]
                .iter()
                .any(|prior| prior.pool_id == leg.pool_id)
            {
                return Err(IdentityError::RepeatedPool);
            }
            if index > 0 && legs[index - 1].asset_out != leg.asset_in {
                return Err(IdentityError::DiscontinuousRoute);
            }
        }
        if legs[0].asset_in != legs[legs.len() - 1].asset_out {
            return Err(IdentityError::NonCyclicRoute);
        }
        Ok(Self(legs))
    }
    pub fn legs(&self) -> &[RouteLeg] {
        &self.0
    }
    pub fn network(&self) -> NetworkId {
        self.0[0].asset_in.network()
    }
    pub fn starting_asset(&self) -> &AssetId {
        &self.0[0].asset_in
    }
}
impl TryFrom<Vec<RouteLeg>> for Route {
    type Error = IdentityError;
    fn try_from(value: Vec<RouteLeg>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Route> for Vec<RouteLeg> {
    fn from(route: Route) -> Self {
        route.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn asset(n: u8) -> AssetId {
        AssetId::new(NetworkId::BaseMainnet, &format!("0x{n:040x}")).unwrap()
    }
    fn pool(n: u8) -> PoolId {
        PoolId::new(NetworkId::BaseMainnet, &format!("0x{n:040x}")).unwrap()
    }
    fn legs() -> Vec<RouteLeg> {
        vec![
            RouteLeg {
                pool_id: pool(1),
                asset_in: asset(3),
                asset_out: asset(4),
            },
            RouteLeg {
                pool_id: pool(2),
                asset_in: asset(4),
                asset_out: asset(3),
            },
        ]
    }
    #[test]
    fn address_identity_is_not_ticker_identity() {
        for value in [
            "USDC",
            "fixture:base:usdc",
            "base-mainnet:USDC",
            "base-mainnet:0x123",
            "ethereum:0x123",
        ] {
            assert!(value.parse::<AssetId>().is_err());
        }
        assert_eq!(
            AssetId::new(
                NetworkId::BaseMainnet,
                "0x00000000000000000000000000000000000000AF"
            )
            .unwrap()
            .address(),
            "0x00000000000000000000000000000000000000af"
        );
        assert!(
            AssetId::new(
                NetworkId::SolanaMainnet,
                "So11111111111111111111111111111111111111112"
            )
            .is_ok()
        );
        assert!(
            AssetId::new(NetworkId::SolanaMainnet, "11111111111111111111111111111111").is_err()
        );
        assert!("fixture:base:usdc".parse::<FixtureId>().is_ok());
        assert!("USDC".parse::<FixtureId>().is_err());
    }
    #[test]
    fn route_is_validated_on_construction_and_deserialization() {
        let route = Route::new(legs()).unwrap();
        let wire = serde_json::to_string(&route).unwrap();
        assert_eq!(serde_json::from_str::<Route>(&wire).unwrap(), route);
        let mut duplicate = legs();
        duplicate[1].pool_id = duplicate[0].pool_id.clone();
        assert_eq!(
            Route::new(duplicate.clone()),
            Err(IdentityError::RepeatedPool)
        );
        assert!(
            serde_json::from_str::<Route>(&serde_json::to_string(&duplicate).unwrap()).is_err()
        );
        let mut broken = legs();
        broken[1].asset_in = asset(5);
        assert_eq!(Route::new(broken), Err(IdentityError::DiscontinuousRoute));
        let mut cross = legs();
        cross[1].asset_out = AssetId::new(
            NetworkId::SolanaMainnet,
            "So11111111111111111111111111111111111111112",
        )
        .unwrap();
        assert_eq!(Route::new(cross), Err(IdentityError::CrossNetwork));
        let mut open = legs();
        open[1].asset_out = asset(5);
        assert_eq!(Route::new(open), Err(IdentityError::NonCyclicRoute));
    }
    #[test]
    fn bounded_route_mutation_properties() {
        for pool_id in 1..16u8 {
            for final_asset in 3..16u8 {
                let mut sample = legs();
                sample[1].pool_id = pool(pool_id);
                sample[1].asset_out = asset(final_asset);
                assert_eq!(Route::new(sample).is_ok(), pool_id != 1 && final_asset == 3);
            }
        }
    }
}
