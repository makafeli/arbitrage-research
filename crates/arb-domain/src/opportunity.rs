use crate::{
    AssetId, AtomicAmount, Decimals, Evidence, FixtureId, Mode, NetworkId, PoolId, SignedAmount,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceKind {
    SyntheticFixture,
    CapturedMarketData,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SimulationStatus {
    NotRun,
    Passed,
    Failed,
    Unsupported,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinalityStatus {
    NotApplicable,
    Provisional,
    Finalized,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FundingMode {
    OwnCapital,
    FlashLoan,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CostKind {
    NetworkFee,
    PriorityFee,
    RelayTip,
    BorrowFee,
    AccountSetup,
    OtherExplicitCost,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAmount {
    pub asset_id: String,
    pub amount_minor: AtomicAmount,
    pub decimals: Decimals,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplicitCost {
    pub kind: CostKind,
    pub native_amount: NativeAmount,
    pub in_start_asset_minor: AtomicAmount,
    pub valuation_reference: String,
}
/// Missing evidence has no implicit zero amount. Callers must retain the reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum CostEstimate {
    Known { costs: Vec<ExplicitCost> },
    Missing { reason: String },
}
/// Timeout or uncertain transport remains UNKNOWN until positive reconciliation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum SubmissionOutcome {
    NotSubmitted,
    Unknown {
        reason: String,
    },
    RejectedBeforeSubmission {
        reason: String,
    },
    Included {
        transaction_id: String,
        finality: FinalityStatus,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpportunitySnapshot {
    pub snapshot_id: String,
    pub state_reference: String,
    pub source: String,
    pub consistent: bool,
    pub complete: bool,
    pub finality_label: String,
    pub age_ms: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpportunityLeg {
    pub pool_id: String,
    pub venue_family: String,
    pub asset_in: String,
    pub asset_out: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EligibilityChecks {
    pub state_fresh_and_coherent: bool,
    pub atomic_route_supported: bool,
    pub final_balance_guard_present: bool,
    pub costs_complete: bool,
    pub principal_and_fee_reservations_valid: bool,
    pub simulation_matches_exact_plan: bool,
}
/// Untrusted boundary DTO matching opportunity.schema.json. Call `validate_research`
/// before persistence/publication. Booleans assert evidence; they do not create it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpportunityRecord {
    pub schema_version: String,
    pub source_kind: SourceKind,
    pub opportunity_id: String,
    pub session_id: String,
    pub experiment_id: String,
    pub strategy_revision: AtomicAmount,
    pub network_id: NetworkId,
    pub mode: Mode,
    pub evidence_label: Evidence,
    pub observed_at: String,
    pub snapshot: OpportunitySnapshot,
    pub funding_mode: FundingMode,
    pub start_asset_id: String,
    pub amount_in_minor: AtomicAmount,
    pub quoted_output_minor: AtomicAmount,
    pub route: Vec<OpportunityLeg>,
    pub costs: Vec<ExplicitCost>,
    pub quoted_output_includes_pool_fees_and_price_impact: bool,
    pub net_after_explicit_costs_minor: SignedAmount,
    pub simulation_status: SimulationStatus,
    pub inclusion_scenario_id: Option<String>,
    pub finality_status: FinalityStatus,
    pub transaction_id: Option<String>,
    pub reason_codes: Vec<String>,
    pub eligibility_checks: EligibilityChecks,
    pub execution_plan_digest: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpportunityError {
    pub field: &'static str,
    pub reason: &'static str,
}
impl fmt::Display for OpportunityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.reason)
    }
}
impl std::error::Error for OpportunityError {}
fn err(field: &'static str, reason: &'static str) -> OpportunityError {
    OpportunityError { field, reason }
}
fn present(value: &Option<String>) -> bool {
    value.as_ref().is_some_and(|s| !s.trim().is_empty())
}

impl OpportunityRecord {
    /// Research publication gate. LIVE and production transaction identifiers are
    /// unavailable, including otherwise shape-valid REALIZED records.
    pub fn validate_research(&self) -> Result<(), OpportunityError> {
        if self.mode == Mode::Live {
            return Err(err(
                "mode",
                "LIVE publication requires the separate execution and ledger capability",
            ));
        }
        self.validate()
    }
    /// Validate wire/domain relationships. This verifies consistency of supplied
    /// evidence, never the truth of a provider response or ledger reconciliation.
    pub fn validate(&self) -> Result<(), OpportunityError> {
        if self.schema_version != "1.0.0" {
            return Err(err("schema_version", "unsupported opportunity version"));
        }
        if self.mode != Mode::Live
            && (self.evidence_label == Evidence::Realized
                || self.transaction_id.is_some()
                || self.finality_status != FinalityStatus::NotApplicable)
        {
            return Err(err(
                "evidence_label",
                "research results cannot be REALIZED or contain production transaction/finality evidence",
            ));
        }
        if self.evidence_label == Evidence::Realized
            && (self.mode != Mode::Live
                || !present(&self.transaction_id)
                || self.finality_status == FinalityStatus::NotApplicable)
        {
            return Err(err(
                "evidence_label",
                "REALIZED requires LIVE transaction and finality evidence",
            ));
        }
        if self.source_kind == SourceKind::SyntheticFixture && self.mode == Mode::Live {
            return Err(err(
                "source_kind",
                "synthetic fixtures cannot be LIVE evidence",
            ));
        }
        if self.amount_in_minor.is_zero() {
            return Err(err("amount_in_minor", "route input must be positive"));
        }
        if self.route.len() != 2 {
            return Err(err(
                "route",
                "initial supported strategy has exactly two legs",
            ));
        }
        self.validate_asset(&self.start_asset_id)?;
        let mut seen = HashSet::new();
        let mut previous = &self.start_asset_id;
        for leg in &self.route {
            self.validate_asset(&leg.asset_in)?;
            self.validate_asset(&leg.asset_out)?;
            let normalized_pool = match self.source_kind {
                SourceKind::SyntheticFixture => leg
                    .pool_id
                    .parse::<FixtureId>()
                    .map(|id| id.as_str().to_owned())
                    .map_err(|_| err("route.pool_id", "invalid synthetic pool identity"))?,
                SourceKind::CapturedMarketData => {
                    let pool = leg.pool_id.parse::<PoolId>().map_err(|_| {
                        err(
                            "route.pool_id",
                            "production pool requires canonical network/address identity",
                        )
                    })?;
                    if pool.network() != self.network_id {
                        return Err(err("route.pool_id", "cross-network pool"));
                    }
                    pool.to_string()
                }
            };
            if !seen.insert(normalized_pool) {
                return Err(err("route.pool_id", "pools must be distinct"));
            }
            if &leg.asset_in != previous {
                return Err(err("route.asset_in", "route continuity violated"));
            }
            if leg.asset_in == leg.asset_out {
                return Err(err("route.asset_out", "swap must change asset"));
            }
            if leg.venue_family.trim().is_empty() {
                return Err(err("route.venue_family", "venue family required"));
            }
            previous = &leg.asset_out;
        }
        if previous != &self.start_asset_id {
            return Err(err("route", "route must finish in starting asset"));
        }
        if !self.quoted_output_includes_pool_fees_and_price_impact {
            return Err(err(
                "quoted_output_includes_pool_fees_and_price_impact",
                "pool fees and impact must already be included in quote",
            ));
        }
        let mut net = SignedAmount::difference(&self.quoted_output_minor, &self.amount_in_minor);
        for cost in &self.costs {
            // Native fee currency has no ERC-20 contract or SPL mint. Wrapped
            // native tokens remain separate token identities and cannot substitute.
            let native_id = format!("{}:native", self.network_id);
            if self.source_kind != SourceKind::CapturedMarketData
                || cost.native_amount.asset_id != native_id
            {
                self.validate_asset(&cost.native_amount.asset_id)?;
            }
            if cost.valuation_reference.trim().is_empty() {
                return Err(err(
                    "costs.valuation_reference",
                    "conversion or direct-asset valuation reference required",
                ));
            }
            net = net
                .checked_sub_cost(&cost.in_start_asset_minor)
                .map_err(|_| err("costs", "net result overflows supported signed magnitude"))?;
        }
        if net != self.net_after_explicit_costs_minor {
            return Err(err(
                "net_after_explicit_costs_minor",
                "net must equal quoted output minus input minus explicit costs exactly",
            ));
        }
        if matches!(
            self.evidence_label,
            Evidence::Simulated | Evidence::EstimatedExecutable
        ) && (self.simulation_status != SimulationStatus::Passed
            || !present(&self.execution_plan_digest)
            || !self.snapshot.consistent
            || !self.snapshot.complete
            || !self.eligibility_checks.atomic_route_supported
            || !self.eligibility_checks.simulation_matches_exact_plan)
        {
            return Err(err(
                "simulation_status",
                "SIMULATED requires successful complete atomic transaction simulation, exact plan digest and coherent complete state",
            ));
        }
        if self.evidence_label == Evidence::EstimatedExecutable {
            let e = &self.eligibility_checks;
            if !present(&self.inclusion_scenario_id)
                || !e.state_fresh_and_coherent
                || !e.final_balance_guard_present
                || !e.costs_complete
                || !e.principal_and_fee_reservations_valid
            {
                return Err(err(
                    "eligibility_checks",
                    "ESTIMATED_EXECUTABLE requires named inclusion scenario, freshness, balance guard, complete costs and valid reservations",
                ));
            }
        }
        Ok(())
    }
    fn validate_asset(&self, value: &str) -> Result<(), OpportunityError> {
        match self.source_kind {
            SourceKind::SyntheticFixture => {
                value
                    .parse::<FixtureId>()
                    .map_err(|_| err("asset_id", "synthetic record requires fixture identity"))?;
                let prefix = match self.network_id {
                    NetworkId::BaseMainnet => "fixture:base:",
                    NetworkId::SolanaMainnet => "fixture:solana:",
                };
                if !value.starts_with(prefix) {
                    return Err(err(
                        "asset_id",
                        "synthetic asset must name the record network",
                    ));
                }
            }
            SourceKind::CapturedMarketData => {
                let asset = value.parse::<AssetId>().map_err(|_| {
                    err(
                        "asset_id",
                        "production asset requires canonical network/address identity",
                    )
                })?;
                if asset.network() != self.network_id {
                    return Err(err("asset_id", "cross-network asset"));
                }
                if asset.to_string() != value {
                    return Err(err(
                        "asset_id",
                        "wire asset identity must use its canonical representation",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> OpportunityRecord {
        serde_json::from_str(include_str!("../../../specs/opportunity.example.json")).unwrap()
    }
    #[test]
    fn shared_schema_example_roundtrips_exactly() {
        let sample = sample();
        sample.validate_research().unwrap();
        let original: serde_json::Value =
            serde_json::from_str(include_str!("../../../specs/opportunity.example.json")).unwrap();
        assert_eq!(serde_json::to_value(sample).unwrap(), original);
    }
    #[test]
    fn arithmetic_is_not_simulation() {
        let mut sample = sample();
        sample.evidence_label = Evidence::Simulated;
        assert!(sample.validate_research().is_err());
        sample.simulation_status = SimulationStatus::Passed;
        assert!(sample.validate_research().is_err());
        sample.execution_plan_digest = Some("fixture:exact-plan".into());
        sample.eligibility_checks.atomic_route_supported = true;
        sample.eligibility_checks.simulation_matches_exact_plan = true;
        sample.validate_research().unwrap();
        sample.snapshot.complete = false;
        assert!(sample.validate_research().is_err());
    }
    #[test]
    fn estimates_require_every_declared_gate() {
        let mut sample = sample();
        sample.evidence_label = Evidence::EstimatedExecutable;
        sample.simulation_status = SimulationStatus::Passed;
        sample.execution_plan_digest = Some("fixture:plan".into());
        sample.eligibility_checks = EligibilityChecks {
            state_fresh_and_coherent: true,
            atomic_route_supported: true,
            final_balance_guard_present: true,
            costs_complete: true,
            principal_and_fee_reservations_valid: true,
            simulation_matches_exact_plan: true,
        };
        assert!(sample.validate_research().is_err());
        sample.inclusion_scenario_id = Some("delay-50ms".into());
        sample.validate_research().unwrap();
        for index in 0..6 {
            let mut bad = sample.clone();
            let e = &mut bad.eligibility_checks;
            match index {
                0 => e.state_fresh_and_coherent = false,
                1 => e.atomic_route_supported = false,
                2 => e.final_balance_guard_present = false,
                3 => e.costs_complete = false,
                4 => e.principal_and_fee_reservations_valid = false,
                _ => e.simulation_matches_exact_plan = false,
            }
            assert!(bad.validate_research().is_err());
        }
    }
    #[test]
    fn captured_native_fees_are_distinct_from_wrapped_principal() {
        let mut record = sample();
        record.source_kind = SourceKind::CapturedMarketData;
        let asset_a = "base-mainnet:0x0000000000000000000000000000000000000001";
        let asset_b = "base-mainnet:0x0000000000000000000000000000000000000002";
        record.start_asset_id = asset_a.into();
        record.route[0].asset_in = asset_a.into();
        record.route[0].asset_out = asset_b.into();
        record.route[1].asset_in = asset_b.into();
        record.route[1].asset_out = asset_a.into();
        record.route[0].pool_id = "base-mainnet:0x0000000000000000000000000000000000000003".into();
        record.route[1].pool_id = "base-mainnet:0x0000000000000000000000000000000000000004".into();
        record.costs[0].native_amount.asset_id = "base-mainnet:native".into();
        record.validate_research().unwrap();
        record.costs[0].native_amount.asset_id = "solana-mainnet:native".into();
        assert!(record.validate_research().is_err());
    }

    #[test]
    fn fictional_live_and_broken_financial_relationships_fail() {
        for mode in [Mode::Observe, Mode::Paper, Mode::Replay] {
            let mut bad = sample();
            bad.mode = mode;
            bad.evidence_label = Evidence::Realized;
            assert!(bad.validate().is_err());
        }
        let mut bad = sample();
        bad.net_after_explicit_costs_minor = "60000".parse().unwrap();
        assert!(bad.validate().is_err());
        let mut bad = sample();
        bad.route[1].pool_id = bad.route[0].pool_id.clone();
        assert!(bad.validate().is_err());
        let mut bad = sample();
        bad.source_kind = SourceKind::CapturedMarketData;
        assert!(bad.validate().is_err());
        let unknown = SubmissionOutcome::Unknown {
            reason: "provider timeout".into(),
        };
        assert_eq!(serde_json::to_value(unknown).unwrap()["status"], "UNKNOWN");
        let missing = CostEstimate::Missing {
            reason: "fee conversion absent".into(),
        };
        assert!(
            serde_json::to_value(missing)
                .unwrap()
                .get("costs")
                .is_none()
        );
    }
}
