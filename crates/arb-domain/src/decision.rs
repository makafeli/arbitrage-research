//! Immutable research decisions. Grouping never removes raw observations.
use crate::{
    AssetId, AtomicAmount, EligibilityChecks, Evidence, FinalityStatus, FundingMode, Mode,
    NetworkId, OpportunityLeg, OpportunityRecord, OpportunitySnapshot, PoolId, SignedAmount,
    SimulationStatus, SourceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt};

/// Maximum source set for a newly published worker decision; legacy readers stay compatible.
pub const MAX_PUBLICATION_CAPTURE_REFS: usize = 8;
pub const DECISION_SCHEMA_VERSION: &str = "1.0.0";
pub const FRESHNESS_DECISION_SCHEMA_VERSION: &str = "1.1.0";
pub const GROUPING_VERSION: &str = "route-size-window-v1";
/// Only named public diagnostics may cross the API; raw provider errors and URLs cannot.
pub const DECISION_REASON_CODES: &[&str] = &[
    "NO_CAPTURE_INPUTS",
    "CAPTURE_UNAVAILABLE",
    "PROVIDER_UNAVAILABLE",
    "CAPTURE_CONFIGURATION_MISMATCH",
    "CAPTURE_ORIGIN_MISMATCH",
    "CAPTURE_IDENTITY_MISMATCH",
    "CAPTURE_CONTEXT_MISMATCH",
    "CAPTURE_PAIR_INCOMPLETE",
    "CAPTURE_DATA_INCOMPLETE",
    "UNSUPPORTED_POOL_MODEL",
    "UNSUPPORTED_TOKEN_BEHAVIOR",
    "UNSUPPORTED_SIZE",
    "UNSUPPORTED_REQUESTED_CAPABILITY",
    "CURRENT_PROTOCOL_EQUIVALENCE_UNQUALIFIED",
    "TOKEN_BEHAVIOR_UNQUALIFIED",
    "RESEARCH_MATH_ONLY",
    "FULL_TRANSACTION_SIMULATION_NOT_RUN",
    "EXTERNAL_COSTS_UNAVAILABLE",
    "VIRTUAL_FUNDING_NOT_RESERVED",
    "NO_CONFIGURED_START_ASSET",
    "NO_CONFIGURED_TRADE_SIZES",
    "NO_ELIGIBLE_POOL_PAIRS",
    "DISTINCT_POOL_REQUIRED",
    "ASSET_CONTINUITY_MISMATCH",
    "ARITHMETIC_OVERFLOW",
    "ZERO_LIQUIDITY",
    "INCOMPLETE_TICK_COVERAGE",
    "MATH_INPUT_REJECTED",
    "OUTPUT_ROUNDS_TO_ZERO",
    "ROUTE_BUDGET_EXHAUSTED",
    "EVALUATION_BUDGET_EXHAUSTED",
    "MAX_POOL_BOUND_EXCEEDED",
    "STALE_INPUT",
    "CHAIN_TIME_UNAVAILABLE",
    "CHAIN_TIME_FUTURE",
    "CHAIN_TIME_STALE",
    "CHAIN_TIME_INVALID",
    "WORK_GENERATION_CANCELLED",
    "DEADLINE_EXPIRED",
    "SNAPSHOT_NOT_ATOMIC",
    "POOLS_OUTSIDE_CONFIG",
    "SOURCE_QUALIFICATION_PENDING",
    "RATE_LIMITED",
    "CAPTURE_LIMIT_REACHED",
    "QUOTE_CAPABILITY_UNQUALIFIED",
    "NO_QUOTED_ROUTES",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DatasetOrigin {
    Synthetic,
    ManuallyConstructed,
    RecordedLive,
}
impl DatasetOrigin {
    pub fn source_kind(self) -> SourceKind {
        match self {
            Self::RecordedLive => SourceKind::CapturedMarketData,
            _ => SourceKind::SyntheticFixture,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Synthetic => "SYNTHETIC",
            Self::ManuallyConstructed => "MANUALLY_CONSTRUCTED",
            Self::RecordedLive => "RECORDED_LIVE",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionCaptureRef {
    pub capture_id: String,
    pub manifest_digest: String,
    /// V1 uses the manifest digest as the immutable snapshot identity.
    pub snapshot_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionLeg {
    pub pool_id: PoolId,
    pub asset_in: AssetId,
    pub asset_out: AssetId,
    pub venue_family: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum DecisionResult {
    /// Positive and negative math results are CANDIDATE, before external costs.
    Quoted {
        quoted_output_minor: AtomicAmount,
        gross_delta_minor: SignedAmount,
        /// Each fee is denominated in the corresponding route leg's input asset.
        included_pool_fees: Vec<AtomicAmount>,
    },
    Rejected {
        reason_codes: Vec<String>,
    },
    NoRoute {
        reason_codes: Vec<String>,
    },
    DataUnavailable {
        reason_codes: Vec<String>,
    },
}
impl DecisionResult {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Quoted { .. } => "QUOTED",
            Self::Rejected { .. } => "REJECTED",
            Self::NoRoute { .. } => "NO_ROUTE",
            Self::DataUnavailable { .. } => "DATA_UNAVAILABLE",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionGrouping {
    pub version: String,
    pub key: String,
    pub window_ms: u64,
    pub window_start_ms: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionTrace {
    pub schema_version: String,
    pub observation_id: String,
    pub session_id: String,
    pub experiment_id: String,
    #[serde(with = "decimal_u64")]
    pub generation: u64,
    pub configuration_digest: String,
    pub calculation_version: String,
    pub strategy_id: String,
    pub network_id: NetworkId,
    pub mode: Mode,
    pub source_kind: SourceKind,
    pub dataset_origin: DatasetOrigin,
    pub observed_at_unix_ms: u64,
    pub input_age_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::freshness::deserialize_present_report"
    )]
    pub chain_freshness: Option<crate::ChainFreshnessReport>,
    pub capture_refs: Vec<DecisionCaptureRef>,
    pub route: Vec<DecisionLeg>,
    pub amount_in_minor: Option<AtomicAmount>,
    pub result: DecisionResult,
    pub grouping: DecisionGrouping,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionError {
    pub field: &'static str,
    pub reason: &'static str,
}
impl fmt::Display for DecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.reason)
    }
}
impl std::error::Error for DecisionError {}
fn invalid(field: &'static str, reason: &'static str) -> DecisionError {
    DecisionError { field, reason }
}
fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
fn is_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn valid_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn allowed_codes(codes: &[String]) -> bool {
    codes.len() <= 64
        && codes
            .iter()
            .all(|value| DECISION_REASON_CODES.contains(&value.as_str()))
}
fn valid_codes(codes: &[String]) -> bool {
    !codes.is_empty() && allowed_codes(codes)
}
impl DecisionTrace {
    /// Validate newly admitted worker output. Historical schema validation and
    /// exact retries remain compatible; missing-data diagnostics may have no inputs.
    pub fn validate_for_publication(&self) -> Result<(), DecisionError> {
        if self.capture_refs.len() > MAX_PUBLICATION_CAPTURE_REFS {
            return Err(invalid(
                "capture_refs",
                "new publication supports at most eight capture references",
            ));
        }
        self.validate()
    }

    /// Content-address one immutable observation. Grouping is an additional key;
    /// it never replaces or deletes raw quoted, rejected or missing-input rows.
    pub fn seal(mut self) -> Result<Self, DecisionError> {
        self.grouping.version = GROUPING_VERSION.into();
        if !(1..=3_600_000).contains(&self.grouping.window_ms) {
            return Err(invalid(
                "grouping.window_ms",
                "grouping window must be 1..=3600000 milliseconds",
            ));
        }
        self.grouping.window_start_ms =
            self.observed_at_unix_ms / self.grouping.window_ms * self.grouping.window_ms;
        self.grouping.key = self.compute_grouping_key()?;
        self.observation_id = self.compute_observation_id()?;
        self.validate()?;
        Ok(self)
    }
    pub fn validate(&self) -> Result<(), DecisionError> {
        if ![DECISION_SCHEMA_VERSION, FRESHNESS_DECISION_SCHEMA_VERSION]
            .contains(&self.schema_version.as_str())
        {
            return Err(invalid("schema_version", "unsupported decision version"));
        }
        for (field, value) in [
            ("session_id", &self.session_id),
            ("experiment_id", &self.experiment_id),
            ("calculation_version", &self.calculation_version),
            ("strategy_id", &self.strategy_id),
        ] {
            if !valid_label(value) {
                return Err(invalid(field, "nonempty bounded identifier required"));
            }
        }
        if self.mode == Mode::Live {
            return Err(invalid(
                "mode",
                "research traces cannot grant LIVE capability",
            ));
        }
        if !is_digest(&self.configuration_digest) || !is_digest(&self.observation_id) {
            return Err(invalid(
                "configuration_digest",
                "canonical configuration and observation digests required",
            ));
        }
        if self.source_kind != self.dataset_origin.source_kind() {
            return Err(invalid(
                "dataset_origin",
                "dataset origin and source kind disagree",
            ));
        }
        if self.observed_at_unix_ms == 0
            || self.observed_at_unix_ms > 253_402_300_799_999
            || self
                .input_age_ms
                .is_some_and(|age| age > 8_640_000_000_000_000)
        {
            return Err(invalid(
                "observed_at_unix_ms",
                "timestamp or age outside supported exact millisecond range",
            ));
        }
        match &self.chain_freshness {
            None if self.schema_version != DECISION_SCHEMA_VERSION
                || self.calculation_version == crate::FRESHNESS_CALCULATION_VERSION =>
            {
                return Err(invalid(
                    "chain_freshness",
                    "freshness schema and calculation require an explicit report",
                ));
            }
            Some(report) => {
                if self.schema_version != FRESHNESS_DECISION_SCHEMA_VERSION
                    || self.calculation_version != crate::FRESHNESS_CALCULATION_VERSION
                    || report.reference_observed_at_unix_ms != self.observed_at_unix_ms
                    || Some(report.evaluation_elapsed_ms) != self.input_age_ms
                    || report.sources.len() != self.capture_refs.len()
                    || report
                        .sources
                        .iter()
                        .zip(&self.capture_refs)
                        .any(|(source, capture)| {
                            source.capture_id != capture.capture_id
                                || source.source.network_id() != self.network_id
                        })
                {
                    return Err(invalid(
                        "chain_freshness",
                        "freshness version, clock, network and ordered capture bindings must match trace",
                    ));
                }
                report.validate()?;
                if let Some(code) = report.status.reason_code() {
                    let has_reason = match &self.result {
                        DecisionResult::Rejected { reason_codes }
                        | DecisionResult::NoRoute { reason_codes }
                        | DecisionResult::DataUnavailable { reason_codes } => {
                            reason_codes.iter().any(|reason| reason == code)
                        }
                        DecisionResult::Quoted { .. } => false,
                    };
                    if !has_reason {
                        return Err(invalid(
                            "chain_freshness",
                            "non-fresh evidence requires matching rejection or missing-data reason",
                        ));
                    }
                }
            }
            None => {}
        }
        let expected_chain_time_reason = self
            .chain_freshness
            .as_ref()
            .and_then(|report| report.status.reason_code());
        let result_reasons = match &self.result {
            DecisionResult::Rejected { reason_codes }
            | DecisionResult::NoRoute { reason_codes }
            | DecisionResult::DataUnavailable { reason_codes } => reason_codes.as_slice(),
            DecisionResult::Quoted { .. } => &[],
        };
        // These codes assert an assessed aggregate status, including when carried
        // as diagnostics. CHAIN_TIME_INVALID instead names an assessment error
        // and does not claim any successfully reproduced report status.
        if result_reasons
            .iter()
            .chain(&self.diagnostics)
            .any(|reason| {
                matches!(
                    reason.as_str(),
                    "CHAIN_TIME_STALE" | "CHAIN_TIME_FUTURE" | "CHAIN_TIME_UNAVAILABLE"
                ) && Some(reason.as_str()) != expected_chain_time_reason
            })
        {
            return Err(invalid(
                "chain_freshness",
                "chain-time result and diagnostic reasons require the matching report status",
            ));
        }
        if self.capture_refs.len() > 64 {
            return Err(invalid("capture_refs", "capture reference bound exceeded"));
        }
        let mut captures = HashSet::new();
        for reference in &self.capture_refs {
            if !valid_label(&reference.capture_id)
                || !is_digest(&reference.manifest_digest)
                || reference.snapshot_id != reference.manifest_digest
                || !captures.insert(&reference.capture_id)
            {
                return Err(invalid(
                    "capture_refs",
                    "distinct capture IDs and matching manifest/snapshot digests required",
                ));
            }
        }
        if !allowed_codes(&self.diagnostics) {
            return Err(invalid(
                "diagnostics",
                "only bounded named public reason codes are accepted",
            ));
        }
        match &self.result {
            DecisionResult::Quoted {
                quoted_output_minor,
                gross_delta_minor,
                included_pool_fees,
            } => {
                self.validate_route()?;
                let input = self
                    .amount_in_minor
                    .as_ref()
                    .ok_or_else(|| invalid("amount_in_minor", "quote input required"))?;
                if quoted_output_minor.is_zero()
                    || included_pool_fees.len() != 2
                    || gross_delta_minor != &SignedAmount::difference(quoted_output_minor, input)
                {
                    return Err(invalid(
                        "result",
                        "positive output, two included pool fees and exact signed gross delta required",
                    ));
                }
                if &included_pool_fees[0] > input {
                    return Err(invalid(
                        "result.included_pool_fees",
                        "first pool fee exceeds input",
                    ));
                }
            }
            DecisionResult::Rejected { reason_codes } => {
                self.validate_route()?;
                if !valid_codes(reason_codes) {
                    return Err(invalid(
                        "result.reason_codes",
                        "named rejection reason required",
                    ));
                }
            }
            DecisionResult::NoRoute { reason_codes } => {
                if self.capture_refs.is_empty()
                    || !self.route.is_empty()
                    || self.amount_in_minor.is_some()
                    || !valid_codes(reason_codes)
                {
                    return Err(invalid(
                        "result",
                        "NO_ROUTE requires captures, no fabricated route or size, and reasons",
                    ));
                }
            }
            DecisionResult::DataUnavailable { reason_codes } => {
                if !self.route.is_empty()
                    || self.amount_in_minor.is_some()
                    || !valid_codes(reason_codes)
                {
                    return Err(invalid(
                        "result",
                        "DATA_UNAVAILABLE requires no fabricated route or size and explicit reasons",
                    ));
                }
            }
        }
        if self.grouping.version != GROUPING_VERSION
            || !(1..=3_600_000).contains(&self.grouping.window_ms)
            || self.grouping.window_start_ms
                != self.observed_at_unix_ms / self.grouping.window_ms * self.grouping.window_ms
            || self.grouping.key != self.compute_grouping_key()?
        {
            return Err(invalid(
                "grouping",
                "invalid versioned grouping key or UTC window",
            ));
        }
        if self.observation_id != self.compute_observation_id()? {
            return Err(invalid(
                "observation_id",
                "decision body differs from its content digest",
            ));
        }
        Ok(())
    }
    fn validate_route(&self) -> Result<(), DecisionError> {
        if self.route.len() != 2
            || self.capture_refs.len() != 2
            || self
                .amount_in_minor
                .as_ref()
                .is_none_or(AtomicAmount::is_zero)
            || self.input_age_ms.is_none()
        {
            return Err(invalid(
                "route",
                "two-leg decisions require two captures, positive input and explicit age",
            ));
        }
        let first = &self.route[0];
        let second = &self.route[1];
        for leg in &self.route {
            if leg.pool_id.network() != self.network_id
                || leg.asset_in.network() != self.network_id
                || leg.asset_out.network() != self.network_id
                || leg.asset_in == leg.asset_out
            {
                return Err(invalid(
                    "route",
                    "same-network pools/assets and asset-changing legs required",
                ));
            }
            let expected = match self.network_id {
                NetworkId::BaseMainnet => "uniswap-v3",
                NetworkId::SolanaMainnet => "orca-whirlpools",
            };
            if leg.venue_family != expected {
                return Err(invalid("route.venue_family", "unsupported pool model"));
            }
        }
        if first.pool_id == second.pool_id
            || first.asset_out != second.asset_in
            || second.asset_out != first.asset_in
        {
            return Err(invalid(
                "route",
                "distinct pools and cyclic continuity required",
            ));
        }
        Ok(())
    }
    fn compute_grouping_key(&self) -> Result<String, DecisionError> {
        let coverage_status = match &self.result {
            DecisionResult::NoRoute { .. } => "NO_ROUTE",
            DecisionResult::DataUnavailable { .. } => "DATA_UNAVAILABLE",
            _ => "ROUTE_OBSERVATION",
        };
        let value = serde_json::json!({
            "version": GROUPING_VERSION, "session_id": self.session_id, "experiment_id": self.experiment_id,
            "configuration_digest": self.configuration_digest, "calculation_version": self.calculation_version,
            "strategy_id": self.strategy_id, "network_id": self.network_id, "mode": self.mode,
            "source_kind": self.source_kind, "dataset_origin": self.dataset_origin, "route": self.route,
            "amount_in_minor": self.amount_in_minor, "window_ms": self.grouping.window_ms,
            "window_start_ms": self.grouping.window_start_ms, "coverage_status": coverage_status,
        });
        Ok(digest(&canonical_json(value)?))
    }
    fn compute_observation_id(&self) -> Result<String, DecisionError> {
        let mut value = serde_json::to_value(self)
            .map_err(|_| invalid("observation_id", "decision serialization failed"))?;
        value
            .as_object_mut()
            .ok_or_else(|| invalid("observation_id", "invalid decision object"))?
            .remove("observation_id");
        Ok(digest(&canonical_json(value)?))
    }
    /// Unknown external costs remain null in schema 1.1. This projection never
    /// manufactures simulation success, net profit, eligibility or a real fill.
    pub fn to_opportunity(&self) -> Result<Option<OpportunityRecord>, DecisionError> {
        self.validate()?;
        let DecisionResult::Quoted {
            quoted_output_minor,
            ..
        } = &self.result
        else {
            return Ok(None);
        };
        let timestamp = i64::try_from(self.observed_at_unix_ms)
            .map_err(|_| invalid("observed_at_unix_ms", "timestamp conversion failed"))?;
        let time = chrono::DateTime::from_timestamp_millis(timestamp)
            .ok_or_else(|| invalid("observed_at_unix_ms", "timestamp outside RFC3339 range"))?;
        let prefix = match self.network_id {
            NetworkId::BaseMainnet => "base",
            NetworkId::SolanaMainnet => "solana",
        };
        let fixture = self.source_kind == SourceKind::SyntheticFixture;
        let asset_id = |asset: &AssetId| {
            if fixture {
                format!("fixture:{prefix}:{}", asset.address())
            } else {
                asset.to_string()
            }
        };
        let route = self
            .route
            .iter()
            .map(|leg| OpportunityLeg {
                pool_id: if fixture {
                    format!("fixture:pool-{prefix}:{}", leg.pool_id.address())
                } else {
                    leg.pool_id.to_string()
                },
                venue_family: leg.venue_family.clone(),
                asset_in: asset_id(&leg.asset_in),
                asset_out: asset_id(&leg.asset_out),
            })
            .collect();
        let mut reasons = self.diagnostics.clone();
        for reason in [
            "RESEARCH_MATH_ONLY",
            "EXTERNAL_COSTS_UNAVAILABLE",
            "FULL_TRANSACTION_SIMULATION_NOT_RUN",
        ] {
            if !reasons.iter().any(|existing| existing == reason) {
                reasons.push(reason.into());
            }
        }
        let record = OpportunityRecord {
            schema_version: "1.1.0".into(),
            source_kind: self.source_kind.clone(),
            dataset_origin: Some(self.dataset_origin),
            opportunity_id: self.observation_id.clone(),
            session_id: self.session_id.clone(),
            experiment_id: self.experiment_id.clone(),
            strategy_revision: AtomicAmount::from(1),
            network_id: self.network_id,
            mode: self.mode,
            evidence_label: Evidence::Candidate,
            observed_at: time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            snapshot: OpportunitySnapshot {
                snapshot_id: self
                    .capture_refs
                    .iter()
                    .map(|r| r.snapshot_id.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                state_reference: self
                    .capture_refs
                    .iter()
                    .map(|r| r.manifest_digest.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                source: format!("decision-trace/{}", self.dataset_origin.label()),
                consistent: false,
                complete: false,
                finality_label: "PAIR_CONTEXT_MATCH_IS_NOT_ATOMIC_EXECUTION_PROOF".into(),
                age_ms: self
                    .input_age_ms
                    .ok_or_else(|| invalid("input_age_ms", "quote age required"))?,
            },
            funding_mode: FundingMode::OwnCapital,
            start_asset_id: asset_id(&self.route[0].asset_in),
            amount_in_minor: self
                .amount_in_minor
                .clone()
                .ok_or_else(|| invalid("amount_in_minor", "quote input required"))?,
            quoted_output_minor: quoted_output_minor.clone(),
            route,
            costs: Vec::new(),
            quoted_output_includes_pool_fees_and_price_impact: true,
            net_after_explicit_costs_minor: None,
            simulation_status: SimulationStatus::NotRun,
            inclusion_scenario_id: None,
            finality_status: FinalityStatus::NotApplicable,
            transaction_id: None,
            reason_codes: reasons,
            eligibility_checks: EligibilityChecks {
                state_fresh_and_coherent: false,
                atomic_route_supported: false,
                final_balance_guard_present: false,
                costs_complete: false,
                principal_and_fee_reservations_valid: false,
                simulation_matches_exact_plan: false,
            },
            execution_plan_digest: None,
        };
        record.validate_research().map_err(|_| {
            invalid(
                "opportunity",
                "projection violates the opportunity contract",
            )
        })?;
        Ok(Some(record))
    }
}
fn canonical_json(value: serde_json::Value) -> Result<Vec<u8>, DecisionError> {
    fn sorted(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let ordered: std::collections::BTreeMap<_, _> = map.into_iter().collect();
                serde_json::Value::Object(
                    ordered
                        .into_iter()
                        .map(|(key, value)| (key, sorted(value)))
                        .collect(),
                )
            }
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(sorted).collect())
            }
            other => other,
        }
    }
    serde_json::to_vec(&sorted(value))
        .map_err(|_| invalid("observation_id", "canonical serialization failed"))
}
mod decimal_u64 {
    use serde::{Deserialize, Deserializer, Serializer, de};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.is_empty()
            || (raw.len() > 1 && raw.starts_with('0'))
            || !raw.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(de::Error::custom(
                "expected canonical unsigned decimal revision string",
            ));
        }
        raw.parse().map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> DecisionTrace {
        let a = AssetId::new(
            NetworkId::BaseMainnet,
            "0x0000000000000000000000000000000000000001",
        )
        .unwrap();
        let b = AssetId::new(
            NetworkId::BaseMainnet,
            "0x0000000000000000000000000000000000000002",
        )
        .unwrap();
        let captures = (3..=4)
            .map(|n| {
                let digest = format!("sha256:{}", n.to_string().repeat(64));
                DecisionCaptureRef {
                    capture_id: format!("capture-{n}"),
                    manifest_digest: digest.clone(),
                    snapshot_id: digest,
                }
            })
            .collect();
        DecisionTrace {
            schema_version: DECISION_SCHEMA_VERSION.into(),
            observation_id: String::new(),
            session_id: "session-research".into(),
            experiment_id: "experiment-a".into(),
            generation: u64::MAX,
            configuration_digest: format!("sha256:{}", "a".repeat(64)),
            calculation_version: "research-math-v1".into(),
            strategy_id: "cyclic-exact-in-2leg-v1".into(),
            network_id: NetworkId::BaseMainnet,
            mode: Mode::Paper,
            source_kind: SourceKind::SyntheticFixture,
            dataset_origin: DatasetOrigin::ManuallyConstructed,
            observed_at_unix_ms: 1_700_000_000_100,
            input_age_ms: Some(17),
            chain_freshness: None,
            capture_refs: captures,
            route: vec![
                DecisionLeg {
                    pool_id: PoolId::new(
                        NetworkId::BaseMainnet,
                        "0x0000000000000000000000000000000000000003",
                    )
                    .unwrap(),
                    asset_in: a.clone(),
                    asset_out: b.clone(),
                    venue_family: "uniswap-v3".into(),
                },
                DecisionLeg {
                    pool_id: PoolId::new(
                        NetworkId::BaseMainnet,
                        "0x0000000000000000000000000000000000000004",
                    )
                    .unwrap(),
                    asset_in: b,
                    asset_out: a,
                    venue_family: "uniswap-v3".into(),
                },
            ],
            amount_in_minor: Some(AtomicAmount::from(100)),
            result: DecisionResult::Quoted {
                quoted_output_minor: AtomicAmount::from(99),
                gross_delta_minor: "-1".parse().unwrap(),
                included_pool_fees: vec![AtomicAmount::from(1), AtomicAmount::from(1)],
            },
            grouping: DecisionGrouping {
                version: String::new(),
                key: String::new(),
                window_ms: 1000,
                window_start_ms: 0,
            },
            diagnostics: vec!["RESEARCH_MATH_ONLY".into()],
        }
        .seal()
        .unwrap()
    }
    #[test]
    fn independent_chain_time_and_historical_cost_fixtures_roundtrip_exactly() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../specs/chain-freshness.example.json"))
                .unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let value = case["record"]["trace"].clone();
            let trace: DecisionTrace = serde_json::from_value(value.clone()).unwrap();
            trace.validate().unwrap();
            assert_eq!(trace.clone().seal().unwrap(), trace);
            assert_eq!(serde_json::to_value(trace).unwrap(), value);
        }
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../specs/cost-assessment.example.json"))
                .unwrap();
        let trace: DecisionTrace =
            serde_json::from_value(fixture["source_decision"]["trace"].clone()).unwrap();
        trace.validate().unwrap();
        assert_eq!(
            trace.observation_id,
            "sha256:c958bdc0502550be216657922b083d8a27b4f345221f7ee874569c6cf1119b32"
        );
        assert!(trace.chain_freshness.is_none());
        assert_eq!(
            serde_json::to_value(trace).unwrap(),
            fixture["source_decision"]["trace"]
        );
    }
    #[test]
    fn chain_time_cannot_be_resealed_with_wrong_bindings_status_or_version() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../specs/chain-freshness.example.json"))
                .unwrap();
        let original: DecisionTrace =
            serde_json::from_value(fixtures["cases"][0]["record"]["trace"].clone()).unwrap();
        for mutation in 0..8 {
            let mut trace = original.clone();
            match mutation {
                0 => trace.schema_version = DECISION_SCHEMA_VERSION.into(),
                1 => trace.chain_freshness = None,
                2 => trace.input_age_ms = Some(1),
                3 => trace.chain_freshness.as_mut().unwrap().sources.swap(0, 1),
                4 => trace.chain_freshness.as_mut().unwrap().sources[0].age_ms = Some(0),
                5 => {
                    trace.chain_freshness.as_mut().unwrap().status =
                        crate::ChainFreshnessStatus::Stale
                }
                6 => trace.calculation_version = "research-math-v1".into(),
                _ => trace.chain_freshness.as_mut().unwrap().sources[0].chain_time_seconds = None,
            }
            assert!(
                trace.seal().is_err(),
                "mutation {mutation} must fail even when resealed"
            );
        }
        let opportunity = original.to_opportunity().unwrap().unwrap();
        assert_eq!(opportunity.evidence_label, Evidence::Candidate);
        assert!(!opportunity.eligibility_checks.state_fresh_and_coherent);
        assert!(!opportunity.eligibility_checks.atomic_route_supported);
        assert_eq!(opportunity.simulation_status, SimulationStatus::NotRun);
    }
    fn with_chain_time(status: crate::ChainFreshnessStatus) -> DecisionTrace {
        let mut trace = sample();
        let seconds = trace.observed_at_unix_ms / 1000;
        let chain_time_seconds = match status {
            crate::ChainFreshnessStatus::WithinPolicy => Some(seconds),
            crate::ChainFreshnessStatus::Stale => Some(seconds - 2),
            crate::ChainFreshnessStatus::Future => Some(seconds + 1),
            crate::ChainFreshnessStatus::Unknown => None,
        };
        let report = crate::ChainFreshnessReport::assess(
            crate::ChainFreshnessPolicy {
                version: crate::CHAIN_FRESHNESS_VERSION.into(),
                max_chain_age_ms: 1000,
            },
            trace.observed_at_unix_ms,
            trace.input_age_ms.unwrap(),
            trace
                .capture_refs
                .iter()
                .map(|capture| crate::ChainTimeInput {
                    capture_id: capture.capture_id.clone(),
                    source: crate::ChainTimeSource::BaseFinalizedBlockTimestamp {
                        block_number: "100".into(),
                        block_hash: format!("0x{}", "a".repeat(64)),
                        parent_hash: format!("0x{}", "b".repeat(64)),
                    },
                    chain_time_seconds,
                })
                .collect(),
        )
        .unwrap();
        assert_eq!(report.status, status);
        trace.schema_version = FRESHNESS_DECISION_SCHEMA_VERSION.into();
        trace.calculation_version = crate::FRESHNESS_CALCULATION_VERSION.into();
        trace.chain_freshness = Some(report);
        if let Some(code) = status.reason_code() {
            trace.result = DecisionResult::Rejected {
                reason_codes: vec![code.into()],
            };
        }
        trace.seal().unwrap()
    }
    fn with_failure_result(
        mut trace: DecisionTrace,
        status: &str,
        reason_codes: Vec<String>,
    ) -> DecisionTrace {
        trace.result = match status {
            "REJECTED" => DecisionResult::Rejected { reason_codes },
            "NO_ROUTE" => DecisionResult::NoRoute { reason_codes },
            "DATA_UNAVAILABLE" => DecisionResult::DataUnavailable { reason_codes },
            _ => unreachable!(),
        };
        if status != "REJECTED" {
            trace.route.clear();
            trace.amount_in_minor = None;
        }
        trace
    }
    #[test]
    fn legacy_decisions_cannot_claim_assessed_chain_time_outcomes() {
        for reason in [
            "CHAIN_TIME_STALE",
            "CHAIN_TIME_FUTURE",
            "CHAIN_TIME_UNAVAILABLE",
        ] {
            for status in ["REJECTED", "NO_ROUTE", "DATA_UNAVAILABLE"] {
                let trace = with_failure_result(sample(), status, vec![reason.into()]);
                assert_eq!(trace.seal().unwrap_err().field, "chain_freshness");
            }
            // A diagnostic must not bypass the same evidence requirement on a quote.
            let mut trace = sample();
            trace.diagnostics.push(reason.into());
            assert_eq!(trace.seal().unwrap_err().field, "chain_freshness");
        }
    }
    #[test]
    fn chain_time_result_and_diagnostic_claims_match_the_aggregate_report() {
        for report_status in [
            crate::ChainFreshnessStatus::WithinPolicy,
            crate::ChainFreshnessStatus::Stale,
            crate::ChainFreshnessStatus::Future,
            crate::ChainFreshnessStatus::Unknown,
        ] {
            let original = with_chain_time(report_status);
            let expected_reason = report_status.reason_code();
            for reason in [
                "CHAIN_TIME_STALE",
                "CHAIN_TIME_FUTURE",
                "CHAIN_TIME_UNAVAILABLE",
            ] {
                for result_status in ["REJECTED", "NO_ROUTE", "DATA_UNAVAILABLE"] {
                    let mut reasons: Vec<String> =
                        expected_reason.into_iter().map(String::from).collect();
                    // Keep the required reason and append another claim: merely
                    // finding the required reason must not allow contradictory ones.
                    reasons.push(reason.into());
                    let trace = with_failure_result(original.clone(), result_status, reasons);
                    assert_eq!(
                        trace.seal().is_ok(),
                        expected_reason == Some(reason),
                        "{report_status:?}/{result_status}/{reason}"
                    );
                }
                let mut trace = original.clone();
                trace.diagnostics.push(reason.into());
                assert_eq!(trace.seal().is_ok(), expected_reason == Some(reason));
            }
            if expected_reason.is_some() {
                let mut quote = original.clone();
                quote.result = sample().result;
                assert_eq!(quote.seal().unwrap_err().field, "chain_freshness");
                let missing_reason = with_failure_result(
                    original,
                    "DATA_UNAVAILABLE",
                    vec!["CAPTURE_UNAVAILABLE".into()],
                );
                assert_eq!(missing_reason.seal().unwrap_err().field, "chain_freshness");
            }
        }
    }
    #[test]
    fn unrelated_chain_time_errors_and_legacy_outcomes_remain_compatible() {
        for original in [
            sample(),
            with_chain_time(crate::ChainFreshnessStatus::WithinPolicy),
        ] {
            for reason in ["CHAIN_TIME_INVALID", "STALE_INPUT", "PROVIDER_UNAVAILABLE"] {
                for status in ["REJECTED", "NO_ROUTE", "DATA_UNAVAILABLE"] {
                    with_failure_result(original.clone(), status, vec![reason.into()])
                        .seal()
                        .unwrap();
                }
                let mut trace = original.clone();
                trace.diagnostics.push(reason.into());
                trace.seal().unwrap();
            }
        }
    }
    #[test]
    fn observation_integrity_and_exact_generation_wire_roundtrip() {
        let trace = sample();
        let value = serde_json::to_value(&trace).unwrap();
        assert_eq!(value["generation"], u64::MAX.to_string());
        assert_eq!(
            serde_json::from_value::<DecisionTrace>(value.clone()).unwrap(),
            trace
        );
        let mut bad = value;
        bad["generation"] = serde_json::json!(1);
        assert!(serde_json::from_value::<DecisionTrace>(bad).is_err());
        let mut changed = trace;
        changed.input_age_ms = Some(18);
        assert!(changed.validate().is_err());
    }
    #[test]
    fn grouping_keeps_raw_outcomes_distinct_without_hiding_rejections() {
        let first = sample();
        let mut next = first.clone();
        next.observed_at_unix_ms += 1;
        next.result = DecisionResult::Rejected {
            reason_codes: vec!["INCOMPLETE_TICK_COVERAGE".into()],
        };
        let next = next.seal().unwrap();
        assert_eq!(first.grouping.key, next.grouping.key);
        assert_ne!(first.observation_id, next.observation_id);
        assert!(next.to_opportunity().unwrap().is_none());
        let mut changed = first.clone();
        changed.observed_at_unix_ms += 1000;
        assert_ne!(changed.seal().unwrap().grouping.key, first.grouping.key);
    }
    #[test]
    fn quoted_projection_retains_unknown_costs_and_explicit_fixture_origin() {
        let trace = sample();
        let record = trace.to_opportunity().unwrap().unwrap();
        assert_eq!(record.schema_version, "1.1.0");
        assert_eq!(record.evidence_label, Evidence::Candidate);
        assert_eq!(
            record.dataset_origin,
            Some(DatasetOrigin::ManuallyConstructed)
        );
        assert_eq!(record.snapshot.age_ms, 17);
        assert!(record.start_asset_id.starts_with("fixture:base:"));
        assert!(record.route[0].pool_id.starts_with("fixture:pool-base:"));
        assert!(record.net_after_explicit_costs_minor.is_none());
        assert!(!record.eligibility_checks.costs_complete);
        let mut fabricated = record.clone();
        fabricated.net_after_explicit_costs_minor = Some("-1".parse().unwrap());
        assert!(fabricated.validate().is_err());
        fabricated = record;
        fabricated.evidence_label = Evidence::Simulated;
        assert!(fabricated.validate().is_err());
        let mut live = trace;
        live.dataset_origin = DatasetOrigin::RecordedLive;
        live.source_kind = SourceKind::CapturedMarketData;
        let captured = live.seal().unwrap().to_opportunity().unwrap().unwrap();
        assert!(captured.start_asset_id.starts_with("base-mainnet:"));
        assert_eq!(captured.dataset_origin, Some(DatasetOrigin::RecordedLive));
    }
    #[test]
    fn rejects_spoofed_origin_unknown_diagnostics_and_broken_capture_route() {
        let trace = sample();
        let mut bad = trace.clone();
        bad.dataset_origin = DatasetOrigin::RecordedLive;
        assert!(bad.seal().is_err());
        let mut bad = trace.clone();
        bad.diagnostics.push("https://secret@rpc.invalid".into());
        assert!(bad.seal().is_err());
        let mut bad = trace.clone();
        bad.route[1].pool_id = bad.route[0].pool_id.clone();
        assert!(bad.seal().is_err());
        let mut bad = trace.clone();
        bad.capture_refs[1].snapshot_id = bad.capture_refs[0].snapshot_id.clone();
        assert!(bad.seal().is_err());
        let mut bad = trace;
        bad.input_age_ms = None;
        assert!(bad.seal().is_err());
    }
    #[test]
    fn outage_has_no_fabricated_amount_and_survives_empty_capture_denominator() {
        let mut trace = sample();
        trace.capture_refs.clear();
        trace.route.clear();
        trace.amount_in_minor = None;
        trace.input_age_ms = None;
        trace.result = DecisionResult::DataUnavailable {
            reason_codes: vec!["PROVIDER_UNAVAILABLE".into()],
        };
        let trace = trace.seal().unwrap();
        assert!(trace.to_opportunity().unwrap().is_none());
        let mut invalid = trace.clone();
        invalid.amount_in_minor = Some(AtomicAmount::from(0));
        assert!(invalid.seal().is_err());
        let mut invalid = trace;
        invalid.result = DecisionResult::NoRoute {
            reason_codes: vec!["NO_ELIGIBLE_POOL_PAIRS".into()],
        };
        assert!(invalid.seal().is_err());
    }
    #[test]
    fn quoted_time_is_always_projectable_as_four_digit_rfc3339_year() {
        let mut trace = sample();
        trace.observed_at_unix_ms = 253_402_300_799_999;
        trace
            .seal()
            .unwrap()
            .to_opportunity()
            .unwrap()
            .unwrap()
            .validate()
            .unwrap();
        let mut trace = sample();
        trace.observed_at_unix_ms = 253_402_300_800_000;
        assert!(trace.seal().is_err());
    }
    #[test]
    fn negative_and_large_signed_outcomes_remain_exact() {
        for input in [1_u64, 1000, 9_007_199_254_740_993, u64::MAX] {
            let mut trace = sample();
            trace.amount_in_minor = Some(AtomicAmount::from(input));
            trace.result = DecisionResult::Quoted {
                quoted_output_minor: AtomicAmount::from(1),
                gross_delta_minor: SignedAmount::difference(
                    &AtomicAmount::from(1),
                    &AtomicAmount::from(input),
                ),
                included_pool_fees: vec![AtomicAmount::from(0), AtomicAmount::from(0)],
            };
            trace.seal().unwrap().validate().unwrap();
        }
    }
}
