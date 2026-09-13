//! Immutable, capability-limited research configuration. No RPC, secret loading,
//! signing or broadcasting. Syntax validation cannot qualify real chain state.
use arb_domain::{
    AssetId, AtomicAmount, ChainFreshnessPolicy, Evidence, Mode, NetworkId, PoolId, State,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt, net::IpAddr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    pub field: String,
    pub reason: &'static str,
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.reason)
    }
}
impl std::error::Error for ConfigError {}
fn error(field: impl Into<String>, reason: &'static str) -> ConfigError {
    ConfigError {
        field: field.into(),
        reason,
    }
}
fn require(ok: bool, field: &str, reason: &'static str) -> Result<(), ConfigError> {
    if ok {
        Ok(())
    } else {
        Err(error(field, reason))
    }
}

/// Secret references are names, never endpoints, credentials or resolved values.
/// Accepted configured form is env:UPPERCASE_VARIABLE_NAME.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SecretReference(String);
impl SecretReference {
    pub fn is_configured(&self) -> bool {
        self.0 != "UNCONFIGURED"
    }
    pub fn name(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SecretReference {
    type Error = ConfigError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value == "UNCONFIGURED" {
            return Ok(Self(value));
        }
        let name=value.strip_prefix("env:").ok_or_else(||error("secret_reference","use UNCONFIGURED or env:UPPERCASE_VARIABLE_NAME; inline secrets and URLs are rejected"))?;
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            || name.as_bytes()[0].is_ascii_digit()
        {
            return Err(error(
                "secret_reference",
                "invalid environment variable name; values are never accepted",
            ));
        }
        Ok(Self(value))
    }
}
impl From<SecretReference> for String {
    fn from(value: SecretReference) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResearchConfig {
    schema_version: String,
    deployment: Deployment,
    execution: Execution,
    research: Research,
    evidence: EvidencePolicy,
    networks: Networks,
    resources: Resources,
    storage: Storage,
    control: Control,
    #[serde(default)]
    simulation: Simulation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Deployment {
    profile: String,
    operator_model: String,
    mode: Mode,
    on_restart: State,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Execution {
    enabled: bool,
    signer_enabled: bool,
    broadcast_enabled: bool,
    flash_loans_enabled: bool,
    live_notional_limit_minor: AtomicAmount,
    live_fee_budget_minor: AtomicAmount,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Research {
    max_hops: u8,
    funding_mode: String,
    deduplicate_routes: bool,
    require_distinct_pools: bool,
    require_exact_protocol_math: bool,
    require_state_provenance: bool,
    record_rejections: bool,
    trade_sizes_minor: Vec<AtomicAmount>,
    default_starting_asset_symbol: String,
    #[serde(default = "default_strategies")]
    strategy_ids: Vec<String>,
}
fn default_strategies() -> Vec<String> {
    vec!["cyclic-exact-in-2leg-v1".into()]
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidencePolicy {
    default_label: Evidence,
    full_transaction_simulation_required_for_simulated_label: bool,
    inclusion_scenario_required_for_estimated_executable_label: bool,
    allow_realized_in_paper: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Networks {
    base: BaseNetwork,
    solana: SolanaNetwork,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseNetwork {
    registry_id: NetworkId,
    expected_evm_chain_id: u64,
    enabled: bool,
    candidate_venue: String,
    verified_pool_ids: Vec<PoolId>,
    verified_asset_ids: Vec<AssetId>,
    rpc_secret_reference: SecretReference,
    #[serde(default)]
    registry_qualification_digest: Option<String>,
    #[serde(default)]
    starting_asset_id: Option<AssetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chain_freshness: Option<ChainFreshnessPolicy>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SolanaNetwork {
    registry_id: NetworkId,
    expected_genesis_identity: String,
    enabled: bool,
    candidate_venue: String,
    verified_pool_ids: Vec<PoolId>,
    verified_asset_ids: Vec<AssetId>,
    rpc_secret_reference: SecretReference,
    #[serde(default)]
    registry_qualification_digest: Option<String>,
    #[serde(default)]
    starting_asset_id: Option<AssetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chain_freshness: Option<ChainFreshnessPolicy>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    cpu_evaluation_threads: u16,
    maximum_queued_evaluations: u32,
    maximum_inflight_evaluations_per_network: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Storage {
    database_secret_reference: SecretReference,
    critical_journal_durability: String,
    capture_directory: String,
    capture_quota_bytes: u64,
    on_capture_quota_exceeded: String,
    #[serde(default = "default_retention")]
    capture_retention_days: u16,
}
fn default_retention() -> u16 {
    30
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    bind_address: IpAddr,
    authentication_required: bool,
    csrf_protection_required: bool,
    command_acknowledgement: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Simulation {
    delay_scenarios_ms: Vec<u32>,
    fee_buffer_bps: u16,
    maximum_state_age_ms: u32,
}
impl Default for Simulation {
    fn default() -> Self {
        Self {
            delay_scenarios_ms: vec![0, 50, 100, 250],
            fee_buffer_bps: 1000,
            maximum_state_age_ms: 1000,
        }
    }
}

/// Validated immutable snapshot. It contains only safe references, never secret
/// values. Store digest and effective_json with an experiment before evaluating.
#[derive(Clone, Debug)]
pub struct ValidatedConfig {
    inner: ResearchConfig,
    digest: String,
    effective_json: String,
}
impl ValidatedConfig {
    pub fn from_toml(input: &str) -> Result<Self, ConfigError> {
        require(
            input.len() <= 1024 * 1024,
            "configuration",
            "configuration exceeds 1 MiB input limit",
        )?;
        let inner:ResearchConfig=toml::from_str(input).map_err(|failure:toml::de::Error|{
            let offset=failure.span().map_or(0,|s|s.start).min(input.len());
            let line=input.as_bytes()[..offset].iter().filter(|b|**b==b'\n').count()+1;
            // Never interpolate parser messages: they can contain a supplied secret.
            error(format!("configuration line {line}"),"invalid TOML, unknown field, missing field or invalid field type; compare the versioned example")
        })?;
        Self::from_inner(inner)
    }
    /// Validate a recorded effective configuration without resolving any secret
    /// reference or making network requests. Use the returned canonical digest to
    /// bind it to a capture; JSON shape alone never qualifies a configuration.
    pub fn from_effective_json(input: &str) -> Result<Self, ConfigError> {
        require(
            input.len() <= 1024 * 1024,
            "configuration",
            "configuration exceeds 1 MiB input limit",
        )?;
        let inner: ResearchConfig = serde_json::from_str(input).map_err(|failure| {
            error(format!("configuration line {} column {}", failure.line(), failure.column()),
                "invalid effective JSON, unknown field, missing field or invalid field type; compare the versioned configuration")
        })?;
        Self::from_inner(inner)
    }
    fn from_inner(mut inner: ResearchConfig) -> Result<Self, ConfigError> {
        inner.validate()?;
        // These are sets, so equivalent orderings have one canonical hash.
        inner.research.trade_sizes_minor.sort();
        inner.research.strategy_ids.sort();
        inner.simulation.delay_scenarios_ms.sort();
        inner
            .networks
            .base
            .verified_asset_ids
            .sort_by_key(ToString::to_string);
        inner
            .networks
            .base
            .verified_pool_ids
            .sort_by_key(ToString::to_string);
        inner
            .networks
            .solana
            .verified_asset_ids
            .sort_by_key(ToString::to_string);
        inner
            .networks
            .solana
            .verified_pool_ids
            .sort_by_key(ToString::to_string);
        // Struct fields have a fixed serialization order and no unordered maps.
        let effective_json = serde_json::to_string(&inner)
            .map_err(|_| error("configuration", "canonical serialization failed"))?;
        let digest = format!(
            "sha256:{}",
            Sha256::digest(effective_json.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        Ok(Self {
            inner,
            digest,
            effective_json,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn effective_json(&self) -> &str {
        &self.effective_json
    }
    pub fn mode(&self) -> Mode {
        self.inner.deployment.mode
    }
    pub fn strategy_ids(&self) -> &[String] {
        &self.inner.research.strategy_ids
    }
    /// Configured does not imply reachable/qualified/fresh. The adapter must verify
    /// actual network and registry ownership before worker readiness is granted.
    pub fn network_enabled(&self, network: NetworkId) -> bool {
        match network {
            NetworkId::BaseMainnet => self.inner.networks.base.enabled,
            NetworkId::SolanaMainnet => self.inner.networks.solana.enabled,
        }
    }
    pub fn rpc_secret_reference(&self, network: NetworkId) -> &SecretReference {
        match network {
            NetworkId::BaseMainnet => &self.inner.networks.base.rpc_secret_reference,
            NetworkId::SolanaMainnet => &self.inner.networks.solana.rpc_secret_reference,
        }
    }
    pub fn expected_genesis_identity(&self) -> &str {
        &self.inner.networks.solana.expected_genesis_identity
    }
    pub fn expected_evm_chain_id(&self) -> u64 {
        self.inner.networks.base.expected_evm_chain_id
    }
    /// An explicit frozen research assumption; absence makes no chain-time claim.
    pub fn chain_freshness(&self, network: NetworkId) -> Option<&ChainFreshnessPolicy> {
        match network {
            NetworkId::BaseMainnet => self.inner.networks.base.chain_freshness.as_ref(),
            NetworkId::SolanaMainnet => self.inner.networks.solana.chain_freshness.as_ref(),
        }
    }
    pub fn starting_asset(&self, network: NetworkId) -> Option<&AssetId> {
        match network {
            NetworkId::BaseMainnet => self.inner.networks.base.starting_asset_id.as_ref(),
            NetworkId::SolanaMainnet => self.inner.networks.solana.starting_asset_id.as_ref(),
        }
    }
    pub fn registry_qualification_digest(&self, network: NetworkId) -> Option<&str> {
        match network {
            NetworkId::BaseMainnet => self
                .inner
                .networks
                .base
                .registry_qualification_digest
                .as_deref(),
            NetworkId::SolanaMainnet => self
                .inner
                .networks
                .solana
                .registry_qualification_digest
                .as_deref(),
        }
    }
    pub fn verified_assets(&self, network: NetworkId) -> &[AssetId] {
        match network {
            NetworkId::BaseMainnet => &self.inner.networks.base.verified_asset_ids,
            NetworkId::SolanaMainnet => &self.inner.networks.solana.verified_asset_ids,
        }
    }
    pub fn verified_pools(&self, network: NetworkId) -> &[PoolId] {
        match network {
            NetworkId::BaseMainnet => &self.inner.networks.base.verified_pool_ids,
            NetworkId::SolanaMainnet => &self.inner.networks.solana.verified_pool_ids,
        }
    }
    pub fn trade_sizes(&self) -> &[AtomicAmount] {
        &self.inner.research.trade_sizes_minor
    }
    pub fn delay_scenarios_ms(&self) -> &[u32] {
        &self.inner.simulation.delay_scenarios_ms
    }
    pub fn maximum_state_age_ms(&self) -> u32 {
        self.inner.simulation.maximum_state_age_ms
    }
    pub fn fee_buffer_bps(&self) -> u16 {
        self.inner.simulation.fee_buffer_bps
    }
    pub fn cpu_evaluation_threads(&self) -> u16 {
        self.inner.resources.cpu_evaluation_threads
    }
    pub fn maximum_queued_evaluations(&self) -> u32 {
        self.inner.resources.maximum_queued_evaluations
    }
    pub fn maximum_inflight_evaluations_per_network(&self) -> u32 {
        self.inner
            .resources
            .maximum_inflight_evaluations_per_network
    }
    pub fn capture_directory(&self) -> &str {
        &self.inner.storage.capture_directory
    }
    pub fn capture_quota_bytes(&self) -> u64 {
        self.inner.storage.capture_quota_bytes
    }
    pub fn capture_retention_days(&self) -> u16 {
        self.inner.storage.capture_retention_days
    }
    pub fn database_secret_reference(&self) -> &SecretReference {
        &self.inner.storage.database_secret_reference
    }
    pub fn control_bind_address(&self) -> IpAddr {
        self.inner.control.bind_address
    }
    pub fn freeze_session(&self, network: NetworkId) -> Result<FrozenSessionConfig, ConfigError> {
        require(
            self.network_enabled(network),
            "session.network_id",
            "network is disabled in this configuration",
        )?;
        Ok(FrozenSessionConfig {
            mode: self.mode(),
            network,
            starting_asset: self.starting_asset(network).cloned(),
            configuration_digest: self.digest.clone(),
        })
    }
}

/// No mutation methods: mode/network/asset/configuration version identify one session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FrozenSessionConfig {
    mode: Mode,
    network: NetworkId,
    starting_asset: Option<AssetId>,
    configuration_digest: String,
}
impl FrozenSessionConfig {
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn network(&self) -> NetworkId {
        self.network
    }
    pub fn starting_asset(&self) -> Option<&AssetId> {
        self.starting_asset.as_ref()
    }
    pub fn configuration_digest(&self) -> &str {
        &self.configuration_digest
    }
    /// Caller creates a distinct session record; the old session is never mutated.
    pub fn replacement(
        &self,
        prior_state: State,
        next: &ValidatedConfig,
        network: NetworkId,
    ) -> Result<Self, ConfigError> {
        require(
            prior_state == State::Stopped,
            "session.state",
            "stop the prior session before creating a different mode, asset, network or configuration",
        )?;
        next.freeze_session(network)
    }
}
fn valid_genesis(value: &str) -> bool {
    if !(32..=44).contains(&value.len()) {
        return false;
    }
    bs58::decode(value).into_vec().is_ok_and(|bytes| {
        bytes.len() == 32
            && bytes.iter().any(|b| *b != 0)
            && bs58::encode(&bytes).into_string() == value
    })
}
fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
impl ResearchConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        for (field, policy) in [
            (
                "networks.base.chain_freshness",
                &self.networks.base.chain_freshness,
            ),
            (
                "networks.solana.chain_freshness",
                &self.networks.solana.chain_freshness,
            ),
        ] {
            if let Some(policy) = policy {
                policy.validate().map_err(|_| {
                    error(
                        field,
                        "unsupported chain-time policy or max_chain_age_ms outside 1..86400000",
                    )
                })?;
            }
        }
        require(
            self.schema_version == "0.1.0",
            "schema_version",
            "unsupported configuration schema version",
        )?;
        require(
            self.deployment.profile == "private-research",
            "deployment.profile",
            "only private-research is supported",
        )?;
        require(
            self.deployment.operator_model == "single-operator",
            "deployment.operator_model",
            "only single-operator is supported",
        )?;
        require(
            self.deployment.mode != Mode::Live,
            "deployment.mode",
            "LIVE capability is unavailable in this research build",
        )?;
        require(
            self.deployment.on_restart == State::Stopped,
            "deployment.on_restart",
            "restart policy must be STOPPED after recovery",
        )?;
        let x = &self.execution;
        for (field, value) in [
            ("execution.enabled", x.enabled),
            ("execution.signer_enabled", x.signer_enabled),
            ("execution.broadcast_enabled", x.broadcast_enabled),
            ("execution.flash_loans_enabled", x.flash_loans_enabled),
        ] {
            require(
                !value,
                field,
                "execution, signing, broadcast and flash-loan capabilities are unavailable",
            )?;
        }
        require(
            x.live_notional_limit_minor.is_zero(),
            "execution.live_notional_limit_minor",
            "live notional must be zero",
        )?;
        require(
            x.live_fee_budget_minor.is_zero(),
            "execution.live_fee_budget_minor",
            "live fee budget must be zero",
        )?;
        let r = &self.research;
        require(
            r.max_hops == 2,
            "research.max_hops",
            "initial supported strategy requires exactly two legs",
        )?;
        require(
            r.funding_mode == "OWN_CAPITAL",
            "research.funding_mode",
            "paper research uses virtual own-capital inventory",
        )?;
        for (field, value) in [
            ("research.deduplicate_routes", r.deduplicate_routes),
            ("research.require_distinct_pools", r.require_distinct_pools),
            (
                "research.require_exact_protocol_math",
                r.require_exact_protocol_math,
            ),
            (
                "research.require_state_provenance",
                r.require_state_provenance,
            ),
            ("research.record_rejections", r.record_rejections),
        ] {
            require(
                value,
                field,
                "required research invariant cannot be disabled",
            )?;
        }
        require(
            !r.default_starting_asset_symbol.is_empty()
                && r.default_starting_asset_symbol.len() <= 16
                && r.default_starting_asset_symbol
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric()),
            "research.default_starting_asset_symbol",
            "display symbol must be 1..16 ASCII letters/digits and is not an identity",
        )?;
        require(
            r.trade_sizes_minor.len() <= 256,
            "research.trade_sizes_minor",
            "at most 256 experiment sizes are supported",
        )?;
        let mut sizes = HashSet::new();
        for size in &r.trade_sizes_minor {
            require(
                !size.is_zero() && sizes.insert(size),
                "research.trade_sizes_minor",
                "sizes must be unique positive canonical base-unit amounts",
            )?;
        }
        require(
            r.strategy_ids == default_strategies(),
            "research.strategy_ids",
            "only cyclic-exact-in-2leg-v1 is implemented by the initial strategy contract",
        )?;
        let e = &self.evidence;
        require(
            e.default_label == Evidence::Candidate,
            "evidence.default_label",
            "new arithmetic quotes start as CANDIDATE",
        )?;
        require(
            e.full_transaction_simulation_required_for_simulated_label,
            "evidence.full_transaction_simulation_required_for_simulated_label",
            "complete transaction simulation is mandatory",
        )?;
        require(
            e.inclusion_scenario_required_for_estimated_executable_label,
            "evidence.inclusion_scenario_required_for_estimated_executable_label",
            "named inclusion scenarios are mandatory",
        )?;
        require(
            !e.allow_realized_in_paper,
            "evidence.allow_realized_in_paper",
            "PAPER cannot produce REALIZED evidence",
        )?;
        let b = &self.networks.base;
        let s = &self.networks.solana;
        require(
            b.registry_id == NetworkId::BaseMainnet && b.expected_evm_chain_id == 8453,
            "networks.base",
            "Base registry and chain ID must match base-mainnet/8453",
        )?;
        require(
            s.registry_id == NetworkId::SolanaMainnet,
            "networks.solana.registry_id",
            "Solana registry must be solana-mainnet",
        )?;
        require(
            b.candidate_venue == "uniswap-v3",
            "networks.base.candidate_venue",
            "only uniswap-v3 is in the initial adapter scope",
        )?;
        require(
            s.candidate_venue == "orca-whirlpools",
            "networks.solana.candidate_venue",
            "only orca-whirlpools is in the initial adapter scope",
        )?;
        self.validate_network(
            NetworkId::BaseMainnet,
            b.enabled,
            &b.verified_pool_ids,
            &b.verified_asset_ids,
            &b.rpc_secret_reference,
            b.registry_qualification_digest.as_deref(),
            b.starting_asset_id.as_ref(),
        )?;
        self.validate_network(
            NetworkId::SolanaMainnet,
            s.enabled,
            &s.verified_pool_ids,
            &s.verified_asset_ids,
            &s.rpc_secret_reference,
            s.registry_qualification_digest.as_deref(),
            s.starting_asset_id.as_ref(),
        )?;
        if s.enabled {
            require(
                valid_genesis(&s.expected_genesis_identity),
                "networks.solana.expected_genesis_identity",
                "enabled network requires recorded mainnet genesis identity, verified by the adapter before readiness",
            )?;
        }
        let q = &self.resources;
        require(
            (1..=64).contains(&q.cpu_evaluation_threads),
            "resources.cpu_evaluation_threads",
            "evaluation threads must be in 1..=64; benchmark against actual host capacity",
        )?;
        require(
            (1..=1_000_000).contains(&q.maximum_queued_evaluations),
            "resources.maximum_queued_evaluations",
            "queue bound must be in 1..=1000000",
        )?;
        require(
            q.maximum_inflight_evaluations_per_network > 0
                && q.maximum_inflight_evaluations_per_network <= q.maximum_queued_evaluations,
            "resources.maximum_inflight_evaluations_per_network",
            "inflight bound must be positive and no larger than queue bound",
        )?;
        require(
            self.storage.critical_journal_durability == "synchronous-local-wal",
            "storage.critical_journal_durability",
            "critical journal requires synchronous-local-wal",
        )?;
        require(
            !self.storage.capture_directory.trim().is_empty()
                && !self.storage.capture_directory.contains('\0'),
            "storage.capture_directory",
            "capture directory must be a nonempty valid path",
        )?;
        require(
            self.storage.capture_quota_bytes > 0,
            "storage.capture_quota_bytes",
            "capture quota must be positive bytes",
        )?;
        require(
            (1..=3650).contains(&self.storage.capture_retention_days),
            "storage.capture_retention_days",
            "retention must be 1..=3650 days",
        )?;
        require(
            self.storage.on_capture_quota_exceeded
                == "mark-experiment-incomplete-and-pause-capture",
            "storage.on_capture_quota_exceeded",
            "quota exhaustion must preserve incompleteness and pause capture",
        )?;
        require(
            self.control.bind_address.is_loopback(),
            "control.bind_address",
            "initial research API binds to loopback; remote exposure requires reviewed transport configuration",
        )?;
        require(
            self.control.authentication_required,
            "control.authentication_required",
            "authentication cannot be disabled",
        )?;
        require(
            self.control.csrf_protection_required,
            "control.csrf_protection_required",
            "CSRF protection cannot be disabled",
        )?;
        require(
            self.control.command_acknowledgement == "worker-applied-fence",
            "control.command_acknowledgement",
            "acceptance is not a worker acknowledgement",
        )?;
        let sim = &self.simulation;
        require(
            !sim.delay_scenarios_ms.is_empty()
                && sim.delay_scenarios_ms.len() <= 32
                && sim.delay_scenarios_ms.iter().all(|d| *d <= 60_000),
            "simulation.delay_scenarios_ms",
            "provide 1..32 delay scenarios bounded to 60000 milliseconds",
        )?;
        let unique: HashSet<_> = sim.delay_scenarios_ms.iter().collect();
        require(
            unique.len() == sim.delay_scenarios_ms.len(),
            "simulation.delay_scenarios_ms",
            "delay scenarios must be unique",
        )?;
        require(
            sim.fee_buffer_bps <= 10_000,
            "simulation.fee_buffer_bps",
            "fee buffer must be 0..=10000 basis points",
        )?;
        require(
            (1..=60_000).contains(&sim.maximum_state_age_ms),
            "simulation.maximum_state_age_ms",
            "state age bound must be 1..=60000 milliseconds",
        )?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn validate_network(
        &self,
        network: NetworkId,
        enabled: bool,
        pools: &[PoolId],
        assets: &[AssetId],
        rpc: &SecretReference,
        qualification: Option<&str>,
        starting_asset: Option<&AssetId>,
    ) -> Result<(), ConfigError> {
        let field = format!("networks.{network}");
        require(
            pools.iter().all(|p| p.network() == network)
                && assets.iter().all(|a| a.network() == network),
            &field,
            "registry entries must belong to the configured network",
        )?;
        let pool_set: HashSet<_> = pools.iter().collect();
        let asset_set: HashSet<_> = assets.iter().collect();
        require(
            pool_set.len() == pools.len() && asset_set.len() == assets.len(),
            &field,
            "registry allowlists cannot contain duplicate identities",
        )?;
        if let Some(start) = starting_asset {
            require(
                start.network() == network && assets.contains(start),
                &field,
                "starting asset must belong to this network and its verified asset allowlist",
            )?;
        }
        if enabled {
            require(
                pools.len() >= 2 && assets.len() >= 2,
                &field,
                "enabled research network requires at least two distinct qualified pools and assets",
            )?;
            require(
                rpc.is_configured(),
                &field,
                "enabled network requires an environment secret reference",
            )?;
            require(
                qualification.is_some_and(valid_digest),
                &field,
                "enabled network requires sha256 registry qualification digest; adapter must verify matching registry evidence",
            )?;
            if matches!(self.deployment.mode, Mode::Paper | Mode::Replay) {
                require(
                    starting_asset.is_some(),
                    &field,
                    "PAPER/REPLAY requires an explicit qualified starting asset address",
                )?;
                require(
                    !self.research.trade_sizes_minor.is_empty(),
                    "research.trade_sizes_minor",
                    "enabled PAPER/REPLAY network requires positive experiment sizes",
                )?;
            }
        } else if let Some(digest) = qualification {
            require(
                valid_digest(digest),
                &field,
                "registry qualification digest must be sha256 plus 64 lowercase hexadecimal characters",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &str = include_str!("../../../config/research.example.toml");
    fn enabled_base(mode: &str) -> String {
        let mut input = EXAMPLE.replace("mode = \"PAPER\"", &format!("mode = \"{mode}\""));
        input = input.replace(
            "trade_sizes_minor = []",
            "trade_sizes_minor = [\"9007199254740993\", \"1000000\"]",
        );
        let begin = input.find("[networks.base]").unwrap();
        let end = input.find("[networks.solana]").unwrap();
        let mut base = input[begin..end].replace("enabled = false", "enabled = true");
        base=base.replace("verified_pool_ids = []","verified_pool_ids = [\"base-mainnet:0x0000000000000000000000000000000000000001\", \"base-mainnet:0x0000000000000000000000000000000000000002\"]");
        base=base.replace("verified_asset_ids = []","verified_asset_ids = [\"base-mainnet:0x0000000000000000000000000000000000000003\", \"base-mainnet:0x0000000000000000000000000000000000000004\"]");
        base=base.replace("rpc_secret_reference = \"UNCONFIGURED\"","rpc_secret_reference = \"env:BASE_RPC_URL\"\nregistry_qualification_digest = \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\nstarting_asset_id = \"base-mainnet:0x0000000000000000000000000000000000000003\"");
        input.replace_range(begin..end, &base);
        input
    }
    #[test]
    fn legacy_effective_bytes_and_digest_survive_optional_policy() {
        let legacy = include_str!("../tests/fixtures/legacy-effective-config.json");
        let config = ValidatedConfig::from_toml(EXAMPLE).unwrap();
        assert_eq!(config.effective_json(), legacy);
        assert_eq!(
            config.digest(),
            "sha256:27e1d1300ba5d5fc77d3c6fd9933df3f880b7345b9071b129eb63d2d8e6b9050"
        );
        let decoded = ValidatedConfig::from_effective_json(legacy).unwrap();
        assert_eq!(decoded.effective_json(), legacy);
        for network in [NetworkId::BaseMainnet, NetworkId::SolanaMainnet] {
            assert!(decoded.chain_freshness(network).is_none());
        }
    }
    #[test]
    fn per_network_policy_is_explicit_frozen_versioned_and_bounded() {
        let example = ValidatedConfig::from_toml(include_str!(
            "../../../config/chain-freshness.example.toml"
        ))
        .unwrap();
        assert!(!example.network_enabled(NetworkId::BaseMainnet));
        assert!(!example.network_enabled(NetworkId::SolanaMainnet));
        assert_eq!(
            example
                .chain_freshness(NetworkId::BaseMainnet)
                .unwrap()
                .max_chain_age_ms,
            1_800_000
        );
        assert_eq!(
            example
                .chain_freshness(NetworkId::SolanaMainnet)
                .unwrap()
                .max_chain_age_ms,
            60_000
        );
        let original = ValidatedConfig::from_toml(EXAMPLE).unwrap();
        for network in ["base", "solana"] {
            let mut json: serde_json::Value =
                serde_json::from_str(original.effective_json()).unwrap();
            json["networks"][network]["chain_freshness"] = serde_json::json!({
                "version": arb_domain::CHAIN_FRESHNESS_VERSION, "max_chain_age_ms": 30000,
            });
            let with_policy = ValidatedConfig::from_effective_json(&json.to_string()).unwrap();
            assert_ne!(with_policy.digest(), original.digest());
            assert_eq!(
                ValidatedConfig::from_effective_json(with_policy.effective_json())
                    .unwrap()
                    .digest(),
                with_policy.digest()
            );
            for invalid in [
                serde_json::json!({"version":"unknown","max_chain_age_ms":30000}),
                serde_json::json!({"version":arb_domain::CHAIN_FRESHNESS_VERSION,"max_chain_age_ms":0}),
                serde_json::json!({"version":arb_domain::CHAIN_FRESHNESS_VERSION,"max_chain_age_ms":86400001}),
                serde_json::json!({"version":arb_domain::CHAIN_FRESHNESS_VERSION,"max_chain_age_ms":30000,"extra":true}),
            ] {
                json["networks"][network]["chain_freshness"] = invalid;
                assert!(ValidatedConfig::from_effective_json(&json.to_string()).is_err());
            }
        }
    }
    #[test]
    fn example_is_valid_inert_and_effective_defaults_are_visible() {
        let c = ValidatedConfig::from_toml(EXAMPLE).unwrap();
        assert_eq!(c.mode(), Mode::Paper);
        assert!(!c.network_enabled(NetworkId::BaseMainnet));
        assert!(!c.network_enabled(NetworkId::SolanaMainnet));
        assert!(c.trade_sizes().is_empty());
        assert!(c.freeze_session(NetworkId::BaseMainnet).is_err());
        assert!(valid_digest(c.digest()));
        let effective: serde_json::Value = serde_json::from_str(c.effective_json()).unwrap();
        assert_eq!(effective["execution"]["broadcast_enabled"], false);
        assert_eq!(effective["simulation"]["fee_buffer_bps"], 1000);
        assert_eq!(effective["storage"]["capture_retention_days"], 30);
    }
    #[test]
    fn hash_is_stable_across_equivalent_toml_and_set_order() {
        let a = enabled_base("PAPER");
        let b = a.replace(
            "[\"9007199254740993\", \"1000000\"]",
            "[\"1000000\",\"9007199254740993\"]",
        );
        let c = ValidatedConfig::from_toml(&a).unwrap();
        let d = ValidatedConfig::from_toml(&format!("# equivalent experiment\n{b}\n\n")).unwrap();
        assert_eq!(c.digest(), d.digest());
        assert_eq!(c.effective_json(), d.effective_json());
        let changed = ValidatedConfig::from_toml(&a.replace(
            "maximum_queued_evaluations = 256",
            "maximum_queued_evaluations = 512",
        ))
        .unwrap();
        assert_ne!(c.digest(), changed.digest());
    }
    #[test]
    fn capabilities_limits_and_unknown_fields_fail_closed() {
        for (before, after) in [
            ("mode = \"PAPER\"", "mode = \"LIVE\""),
            ("signer_enabled = false", "signer_enabled = true"),
            ("broadcast_enabled = false", "broadcast_enabled = true"),
            (
                "live_fee_budget_minor = \"0\"",
                "live_fee_budget_minor = \"1\"",
            ),
            (
                "live_notional_limit_minor = \"0\"",
                "live_notional_limit_minor = \"1\"",
            ),
            ("cpu_evaluation_threads = 2", "cpu_evaluation_threads = 0"),
            (
                "maximum_queued_evaluations = 256",
                "maximum_queued_evaluations = 2",
            ),
            (
                "capture_quota_bytes = 10737418240",
                "capture_quota_bytes = 0",
            ),
            (
                "allow_realized_in_paper = false",
                "allow_realized_in_paper = true",
            ),
            ("max_hops = 2", "max_hops = 3"),
            (
                "require_exact_protocol_math = true",
                "require_exact_protocol_math = false",
            ),
            ("trade_sizes_minor = []", "trade_sizes_minor = [\"0\"]"),
            ("trade_sizes_minor = []", "trade_sizes_minor = [1]"),
            ("bind_address = \"127.0.0.1\"", "bind_address = \"0.0.0.0\""),
            (
                "schema_version = \"0.1.0\"",
                "schema_version = \"0.1.0\"\nunknown_option = true",
            ),
            ("[resources]", "[resources]\nunbounded_queue = true"),
        ] {
            assert!(
                ValidatedConfig::from_toml(&EXAMPLE.replace(before, after)).is_err(),
                "mutation must be rejected: {before}"
            );
        }
    }
    #[test]
    fn unqualified_registry_and_fixture_addresses_are_rejected() {
        assert!(
            ValidatedConfig::from_toml(&EXAMPLE.replace("enabled = false", "enabled = true"))
                .is_err()
        );
        let valid = enabled_base("PAPER");
        assert!(
            ValidatedConfig::from_toml(&valid)
                .unwrap()
                .network_enabled(NetworkId::BaseMainnet)
        );
        for bad in [
            valid.replace(
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "UNVERIFIED",
            ),
            valid.replace(
                "base-mainnet:0x0000000000000000000000000000000000000003",
                "fixture:base:usdc",
            ),
            valid.replace("env:BASE_RPC_URL", "UNCONFIGURED"),
            valid.replace(
                "trade_sizes_minor = [\"9007199254740993\", \"1000000\"]",
                "trade_sizes_minor = []",
            ),
        ] {
            assert!(ValidatedConfig::from_toml(&bad).is_err());
        }
    }
    #[test]
    fn errors_never_echo_credentials_even_parser_errors() {
        let secret = "VERY_SENSITIVE_RPC_TOKEN";
        for bad in [
            EXAMPLE.replace(
                "rpc_secret_reference = \"UNCONFIGURED\"",
                &format!("rpc_secret_reference = \"https://user:{secret}@rpc.invalid\""),
            ),
            EXAMPLE.replace("mode = \"PAPER\"", &format!("mode = \"{secret}\"")),
            format!("{EXAMPLE}\nbroken = \"{secret}"),
        ] {
            let e = ValidatedConfig::from_toml(&bad).unwrap_err();
            assert!(!e.to_string().contains(secret));
            assert!(!format!("{e:?}").contains(secret));
            assert!(!e.field.is_empty());
            assert!(!e.reason.is_empty());
        }
        let c = ValidatedConfig::from_toml(&enabled_base("PAPER")).unwrap();
        assert!(!c.effective_json().contains(secret));
        assert!(c.effective_json().contains("env:BASE_RPC_URL"));
    }
    #[test]
    fn effective_json_revalidates_the_same_frozen_configuration_without_io() {
        let original = ValidatedConfig::from_toml(&enabled_base("PAPER")).unwrap();
        let decoded = ValidatedConfig::from_effective_json(original.effective_json()).unwrap();
        assert_eq!(decoded.digest(), original.digest());
        assert_eq!(decoded.effective_json(), original.effective_json());
        assert_eq!(decoded.mode(), original.mode());
        assert_eq!(
            decoded.verified_assets(NetworkId::BaseMainnet),
            original.verified_assets(NetworkId::BaseMainnet)
        );
        let mut json: serde_json::Value = serde_json::from_str(original.effective_json()).unwrap();
        json["deployment"]["mode"] = serde_json::json!("LIVE");
        assert!(ValidatedConfig::from_effective_json(&json.to_string()).is_err());
        json["deployment"]["mode"] = serde_json::json!("PAPER");
        json["networks"]["base"]["rpc_secret_reference"] =
            serde_json::json!("https://SECRET_TOKEN.invalid");
        let error = ValidatedConfig::from_effective_json(&json.to_string()).unwrap_err();
        assert!(!error.to_string().contains("SECRET_TOKEN"));
        assert!(ValidatedConfig::from_effective_json(r#"{"mode":"offline-fixture"}"#).is_err());
        assert!(ValidatedConfig::from_effective_json(&" ".repeat(1024 * 1024 + 1)).is_err());
    }

    #[test]
    fn sessions_are_frozen_and_replacement_requires_stopped() {
        let old = ValidatedConfig::from_toml(&enabled_base("PAPER")).unwrap();
        let next = ValidatedConfig::from_toml(&enabled_base("OBSERVE")).unwrap();
        let session = old.freeze_session(NetworkId::BaseMainnet).unwrap();
        for state in [
            State::Running,
            State::Paused,
            State::Pausing,
            State::Recovering,
            State::Draining,
            State::Faulted,
        ] {
            assert!(
                session
                    .replacement(state, &next, NetworkId::BaseMainnet)
                    .is_err()
            );
        }
        let replacement = session
            .replacement(State::Stopped, &next, NetworkId::BaseMainnet)
            .unwrap();
        assert_eq!(session.mode(), Mode::Paper);
        assert_eq!(replacement.mode(), Mode::Observe);
        assert_ne!(
            session.configuration_digest(),
            replacement.configuration_digest()
        );
    }
}
