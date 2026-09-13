//! Versioned manual economics bound to an immutable historical decision. A cost
//! scenario adds assumptions; it supplies neither quotes nor execution evidence.
use crate::{
    AccountingAsset, CostReport, Expense, ExpenseAmount, ExpenseKind, FundingAssumption,
    OverheadAllocation, PaperError, Quote, Valuation, evaluate_costs, paper_error,
};
use arb_domain::{AssetId, AtomicAmount, DatasetOrigin, DecisionResult, DecisionTrace, NetworkId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const COST_SCENARIO_SCHEMA_VERSION: &str = "1.0.0";
pub const COST_CALCULATION_VERSION: &str = "decision-bound-exact-costs-v1";
pub const MAX_SCENARIO_VALUATION_AGE_MS: u64 = 86_400_000;
const COMPONENTS: [ExpenseKind; 7] = [
    ExpenseKind::NetworkExecution,
    ExpenseKind::BaseL1Data,
    ExpenseKind::PriorityFee,
    ExpenseKind::RelayTip,
    ExpenseKind::Funding,
    ExpenseKind::AccountSetup,
    ExpenseKind::Other,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CostScenarioOrigin {
    ManuallyConstructed,
}
/// These are accounting decompositions, not estimates of current chain fees.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FeeComposition {
    /// NetworkExecution includes the priority part; BaseL1Data is separate.
    BaseExecutionIncludesPriority,
    /// NetworkExecution is the base/signature fee; PriorityFee is separate.
    SolanaBaseExcludesPriority,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostScenario {
    pub schema_version: String,
    pub scenario_id: String,
    pub version: String,
    pub origin: CostScenarioOrigin,
    pub provenance_reference: String,
    pub valuation_max_age_ms: u64,
    pub fee_composition: FeeComposition,
    pub expenses: Vec<Expense>,
    pub funding: FundingAssumption,
    pub overhead: OverheadAllocation,
}
/// Only the validated server-loaded trace can supply these fields. Generation
/// stays a decimal string to preserve the complete u64 range through JSON.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostDecisionBinding {
    pub observation_id: String,
    pub decision_digest: String,
    pub session_id: String,
    pub experiment_id: String,
    pub generation: String,
    pub configuration_digest: String,
    pub decision_calculation_version: String,
    pub dataset_origin: DatasetOrigin,
    pub network_id: NetworkId,
    pub starting_asset: AssetId,
    pub amount_in_minor: AtomicAmount,
    pub quoted_output_minor: AtomicAmount,
    pub observed_at_unix_ms: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CostAssessmentEvidence {
    Candidate,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostAssessment {
    pub schema_version: String,
    pub calculation_version: String,
    pub assessment_id: String,
    pub scenario_digest: String,
    pub scenario: CostScenario,
    pub binding: CostDecisionBinding,
    pub report: CostReport,
    pub evidence: CostAssessmentEvidence,
}
impl CostAssessment {
    /// Revalidate the original trace and recompute every binding, amount,
    /// unknown, version, evidence label and digest. No current RPC data or clock.
    pub fn replay(&self, trace: &DecisionTrace) -> Result<(), PaperError> {
        let expected = assess_cost_scenario(trace, &self.scenario)?;
        if self != &expected {
            return Err(paper_error(
                "assessment",
                "retained assessment does not reproduce from its immutable decision and scenario",
            ));
        }
        Ok(())
    }
}
impl CostScenario {
    pub fn validate(&self, trace: &DecisionTrace) -> Result<(), PaperError> {
        if self.schema_version != COST_SCENARIO_SCHEMA_VERSION {
            return Err(paper_error(
                "scenario.schema_version",
                "unsupported scenario version",
            ));
        }
        label(&self.scenario_id, 64, "scenario.scenario_id")?;
        label(&self.version, 64, "scenario.version")?;
        reference(&self.provenance_reference, "scenario.provenance_reference")?;
        if !(1..=MAX_SCENARIO_VALUATION_AGE_MS).contains(&self.valuation_max_age_ms) {
            return Err(paper_error(
                "scenario.valuation_max_age_ms",
                "maximum valuation age must be 1..=86400000 milliseconds",
            ));
        }
        let excluded = match (trace.network_id, self.fee_composition) {
            (NetworkId::BaseMainnet, FeeComposition::BaseExecutionIncludesPriority) => {
                ExpenseKind::PriorityFee
            }
            (NetworkId::SolanaMainnet, FeeComposition::SolanaBaseExcludesPriority) => {
                ExpenseKind::BaseL1Data
            }
            _ => {
                return Err(paper_error(
                    "scenario.fee_composition",
                    "fee decomposition must match the decision network",
                ));
            }
        };
        if self.expenses.len() > COMPONENTS.len() {
            return Err(paper_error(
                "scenario.expenses",
                "at most seven distinct expense components are supported",
            ));
        }
        let mut seen = HashSet::new();
        for expense in &self.expenses {
            if !seen.insert(expense.kind) {
                return Err(paper_error(
                    "scenario.expenses",
                    "duplicate expense components cannot be counted twice",
                ));
            }
            if expense.kind == excluded
                && !matches!(&expense.amount, ExpenseAmount::Known { amount, .. } if amount.is_zero())
            {
                return Err(paper_error(
                    "scenario.expenses",
                    "component is already included or not applicable; only omission or explicit zero is valid",
                ));
            }
            match &expense.amount {
                ExpenseAmount::Missing { reason } => {
                    label(reason, 128, "scenario.expenses.reason")?
                }
                ExpenseAmount::Known {
                    asset, valuation, ..
                } => {
                    if asset.network() != trace.network_id {
                        return Err(paper_error(
                            "scenario.expenses.asset",
                            "fee currency must belong to the decision network",
                        ));
                    }
                    if matches!(
                        expense.kind,
                        ExpenseKind::NetworkExecution
                            | ExpenseKind::BaseL1Data
                            | ExpenseKind::PriorityFee
                            | ExpenseKind::RelayTip
                    ) && asset != &AccountingAsset::Native(trace.network_id)
                    {
                        return Err(paper_error(
                            "scenario.expenses.asset",
                            "chain execution, data, priority and relay expenses require the native currency",
                        ));
                    }
                    if matches!(valuation, Valuation::Ratio { .. })
                        && trace.route.first().is_some_and(|leg| {
                            asset == &AccountingAsset::Token(leg.asset_in.clone())
                        })
                    {
                        return Err(paper_error(
                            "scenario.expenses.valuation",
                            "identical token currencies require same-asset valuation",
                        ));
                    }
                    match valuation {
                        Valuation::Missing { reason } => {
                            label(reason, 128, "scenario.expenses.valuation.reason")?
                        }
                        Valuation::SameAsset {
                            reference: value,
                            valued_at_unix_ms,
                        }
                        | Valuation::Ratio {
                            reference: value,
                            valued_at_unix_ms,
                            ..
                        } => {
                            reference(value, "scenario.expenses.valuation.reference")?;
                            if *valued_at_unix_ms == 0
                                || *valued_at_unix_ms > trace.observed_at_unix_ms
                            {
                                return Err(paper_error(
                                    "scenario.expenses.valuation.valued_at_unix_ms",
                                    "valuation must be positive and no later than the historical decision",
                                ));
                            }
                            if trace.observed_at_unix_ms - valued_at_unix_ms
                                > self.valuation_max_age_ms
                            {
                                return Err(paper_error(
                                    "scenario.expenses.valuation.valued_at_unix_ms",
                                    "valuation exceeds the declared historical age limit",
                                ));
                            }
                        }
                    }
                }
            }
        }
        if let FundingAssumption::Unknown { reason } = &self.funding {
            label(reason, 128, "scenario.funding.reason")?;
        }
        match &self.overhead {
            OverheadAllocation::Allocated {
                method,
                version,
                reference: value,
                ..
            } => {
                label(method, 64, "scenario.overhead.method")?;
                label(version, 64, "scenario.overhead.version")?;
                reference(value, "scenario.overhead.reference")?;
            }
            OverheadAllocation::Unknown { reason } => {
                label(reason, 128, "scenario.overhead.reason")?
            }
            OverheadAllocation::NotAllocated => {}
        }
        Ok(())
    }
}
/// Derive quotes from a sealed QUOTED trace. Every applicable omitted cost is
/// added as an explicit unknown; no optimistic default prices or zero costs.
pub fn assess_cost_scenario(
    trace: &DecisionTrace,
    scenario: &CostScenario,
) -> Result<CostAssessment, PaperError> {
    trace
        .validate()
        .map_err(|_| paper_error("decision", "valid immutable decision trace required"))?;
    scenario.validate(trace)?;
    let DecisionResult::Quoted {
        quoted_output_minor,
        ..
    } = &trace.result
    else {
        return Err(paper_error(
            "decision.result",
            "cost assessment requires a retained QUOTED decision",
        ));
    };
    let starting_asset = trace
        .route
        .first()
        .ok_or_else(|| paper_error("decision.route", "quoted starting asset required"))?
        .asset_in
        .clone();
    let amount_in = trace
        .amount_in_minor
        .clone()
        .ok_or_else(|| paper_error("decision.amount_in_minor", "quoted input required"))?;
    let mut expenses = scenario.expenses.clone();
    let excluded = match trace.network_id {
        NetworkId::BaseMainnet => ExpenseKind::PriorityFee,
        NetworkId::SolanaMainnet => ExpenseKind::BaseL1Data,
    };
    for kind in COMPONENTS {
        if kind != excluded && !expenses.iter().any(|expense| expense.kind == kind) {
            expenses.push(Expense {
                kind,
                amount: ExpenseAmount::Missing {
                    reason: "SCENARIO_COMPONENT_NOT_DECLARED".into(),
                },
            });
        }
    }
    let report = evaluate_costs(
        &Quote {
            starting_asset: starting_asset.clone(),
            amount_in: amount_in.clone(),
            amount_out: quoted_output_minor.clone(),
            includes_pool_fees_and_impact: true,
            quote_reference: trace.observation_id.clone(),
        },
        &expenses,
        scenario.funding.clone(),
        scenario.overhead.clone(),
    )?;
    let binding = CostDecisionBinding {
        observation_id: trace.observation_id.clone(),
        decision_digest: canonical_digest(trace)?,
        session_id: trace.session_id.clone(),
        experiment_id: trace.experiment_id.clone(),
        generation: trace.generation.to_string(),
        configuration_digest: trace.configuration_digest.clone(),
        decision_calculation_version: trace.calculation_version.clone(),
        dataset_origin: trace.dataset_origin,
        network_id: trace.network_id,
        starting_asset,
        amount_in_minor: amount_in,
        quoted_output_minor: quoted_output_minor.clone(),
        observed_at_unix_ms: trace.observed_at_unix_ms,
    };
    let mut assessment = CostAssessment {
        schema_version: COST_SCENARIO_SCHEMA_VERSION.into(),
        calculation_version: COST_CALCULATION_VERSION.into(),
        assessment_id: String::new(),
        scenario_digest: canonical_digest(scenario)?,
        scenario: scenario.clone(),
        binding,
        report,
        evidence: CostAssessmentEvidence::Candidate,
    };
    // The versioned preimage includes the empty assessment_id field. The
    // scenario and complete sealed trace are independently content addressed.
    assessment.assessment_id = canonical_digest(&assessment)?;
    Ok(assessment)
}
fn label(value: &str, limit: usize, field: &'static str) -> Result<(), PaperError> {
    if value.is_empty()
        || value.len() > limit
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
    {
        return Err(paper_error(
            field,
            "bounded ASCII identifier required; URLs, whitespace and control characters are prohibited",
        ));
    }
    Ok(())
}
fn reference(value: &str, field: &'static str) -> Result<(), PaperError> {
    label(value, 128, field)?;
    if let Some(hex) = value.strip_prefix("sha256:")
        && (hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    {
        return Err(paper_error(
            field,
            "sha256 references require 64 lowercase hexadecimal digits",
        ));
    }
    Ok(())
}
fn canonical_digest(value: &impl Serialize) -> Result<String, PaperError> {
    fn sorted(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(values) => {
                let entries: std::collections::BTreeMap<_, _> = values
                    .into_iter()
                    .map(|(key, value)| (key, sorted(value)))
                    .collect();
                serde_json::Value::Object(entries.into_iter().collect())
            }
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(sorted).collect())
            }
            value => value,
        }
    }
    let value = serde_json::to_value(value)
        .map_err(|_| paper_error("digest", "scenario serialization failed"))?;
    let bytes = serde_json::to_vec(&sorted(value))
        .map_err(|_| paper_error("digest", "canonical serialization failed"))?;
    Ok(format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_domain::{
        DECISION_SCHEMA_VERSION, DecisionCaptureRef, DecisionGrouping, DecisionLeg, Mode, PoolId,
        SignedAmount, SourceKind,
    };
    fn trace() -> DecisionTrace {
        let network = NetworkId::BaseMainnet;
        let asset = |n| AssetId::new(network, &format!("0x{n:040x}")).unwrap();
        let pool = |n| PoolId::new(network, &format!("0x{n:040x}")).unwrap();
        DecisionTrace {
            schema_version: DECISION_SCHEMA_VERSION.into(),
            observation_id: String::new(),
            session_id: "cost-session".into(),
            experiment_id: "cost-experiment".into(),
            generation: u64::MAX,
            configuration_digest: format!("sha256:{}", "a".repeat(64)),
            calculation_version: "research-math-v1".into(),
            strategy_id: "cyclic-exact-in-2leg-v1".into(),
            network_id: network,
            mode: Mode::Paper,
            source_kind: SourceKind::SyntheticFixture,
            dataset_origin: DatasetOrigin::ManuallyConstructed,
            observed_at_unix_ms: 1_700_000_000_100,
            input_age_ms: Some(17),
            capture_refs: (3..=4)
                .map(|n| {
                    let digest = format!("sha256:{}", n.to_string().repeat(64));
                    DecisionCaptureRef {
                        capture_id: format!("capture-{n}"),
                        manifest_digest: digest.clone(),
                        snapshot_id: digest,
                    }
                })
                .collect(),
            route: vec![
                DecisionLeg {
                    pool_id: pool(3),
                    asset_in: asset(1),
                    asset_out: asset(2),
                    venue_family: "uniswap-v3".into(),
                },
                DecisionLeg {
                    pool_id: pool(4),
                    asset_in: asset(2),
                    asset_out: asset(1),
                    venue_family: "uniswap-v3".into(),
                },
            ],
            amount_in_minor: Some(100.into()),
            result: DecisionResult::Quoted {
                quoted_output_minor: 99.into(),
                gross_delta_minor: "-1".parse().unwrap(),
                included_pool_fees: vec![1.into(), 1.into()],
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
    fn scenario(trace: &DecisionTrace) -> CostScenario {
        let excluded = match trace.network_id {
            NetworkId::BaseMainnet => ExpenseKind::PriorityFee,
            NetworkId::SolanaMainnet => ExpenseKind::BaseL1Data,
        };
        CostScenario {
            schema_version: COST_SCENARIO_SCHEMA_VERSION.into(),
            scenario_id: "manual-fee-study".into(),
            version: "1".into(),
            origin: CostScenarioOrigin::ManuallyConstructed,
            provenance_reference: "manual-assumptions-1".into(),
            valuation_max_age_ms: 1000,
            fee_composition: match trace.network_id {
                NetworkId::BaseMainnet => FeeComposition::BaseExecutionIncludesPriority,
                NetworkId::SolanaMainnet => FeeComposition::SolanaBaseExcludesPriority,
            },
            expenses: COMPONENTS
                .into_iter()
                .filter(|kind| *kind != excluded)
                .map(|kind| Expense {
                    kind,
                    amount: ExpenseAmount::Known {
                        asset: AccountingAsset::Native(trace.network_id),
                        amount: AtomicAmount::zero(),
                        valuation: Valuation::Ratio {
                            numerator: 1.into(),
                            denominator: 1.into(),
                            reference: "manual-unit-ratio".into(),
                            valued_at_unix_ms: trace.observed_at_unix_ms,
                        },
                    },
                })
                .collect(),
            funding: FundingAssumption::OwnVirtualCapital,
            overhead: OverheadAllocation::NotAllocated,
        }
    }
    fn amount(scenario: &mut CostScenario, kind: ExpenseKind, value: u64) {
        let expense = scenario
            .expenses
            .iter_mut()
            .find(|expense| expense.kind == kind)
            .unwrap();
        let ExpenseAmount::Known { amount, .. } = &mut expense.amount else {
            panic!("known fixture")
        };
        *amount = value.into();
    }
    #[test]
    fn bound_negative_cost_assessment_replays_exactly_with_separate_overhead() {
        let trace = trace();
        let mut scenario = scenario(&trace);
        amount(&mut scenario, ExpenseKind::NetworkExecution, 10);
        amount(&mut scenario, ExpenseKind::BaseL1Data, 20);
        scenario.overhead = OverheadAllocation::Allocated {
            amount_in_start_asset: 5.into(),
            method: "per-attempt".into(),
            version: "1".into(),
            reference: "manual-allocation".into(),
        };
        let assessment = assess_cost_scenario(&trace, &scenario).unwrap();
        assert_eq!(
            assessment
                .report
                .gross_after_quote_included_costs
                .to_string(),
            "-1"
        );
        assert_eq!(
            assessment
                .report
                .transaction_net
                .as_ref()
                .unwrap()
                .to_string(),
            "-31"
        );
        assert_eq!(
            assessment
                .report
                .fully_allocated_net
                .as_ref()
                .unwrap()
                .to_string(),
            "-36"
        );
        assert_eq!(assessment.binding.generation, u64::MAX.to_string());
        assert_eq!(assessment.binding.observation_id, trace.observation_id);
        assert_ne!(assessment.binding.decision_digest, trace.observation_id);
        assert!(assessment.report.costs_complete());
        let wire = serde_json::to_value(&assessment).unwrap();
        assert_eq!(wire["binding"]["amount_in_minor"], "100");
        assert_eq!(wire["report"]["transaction_net"], "-31");
        let retained: CostAssessment = serde_json::from_value(wire).unwrap();
        retained.replay(&trace).unwrap();
    }
    #[test]
    fn omitted_expenses_unknown_valuation_and_explicit_zero_remain_distinct() {
        let trace = trace();
        let complete = scenario(&trace);
        let zero = assess_cost_scenario(&trace, &complete).unwrap();
        assert_eq!(
            zero.report.transaction_net.as_ref().unwrap().to_string(),
            "-1"
        );
        assert!(zero.report.fully_allocated_net.is_none());
        let mut omitted = complete.clone();
        omitted
            .expenses
            .retain(|expense| expense.kind != ExpenseKind::RelayTip);
        let report = assess_cost_scenario(&trace, &omitted).unwrap().report;
        assert!(report.transaction_net.is_none());
        assert!(
            report
                .expenses
                .iter()
                .any(|expense| expense.expense.kind == ExpenseKind::RelayTip
                    && expense.in_start_asset.is_none())
        );
        let mut missing = complete.clone();
        missing.expenses[0].amount = ExpenseAmount::Missing {
            reason: "FEE_UNKNOWN".into(),
        };
        assert!(
            assess_cost_scenario(&trace, &missing)
                .unwrap()
                .report
                .transaction_net
                .is_none()
        );
        let mut missing_valuation = complete;
        if let ExpenseAmount::Known { valuation, .. } = &mut missing_valuation.expenses[0].amount {
            *valuation = Valuation::Missing {
                reason: "PRICE_UNKNOWN".into(),
            };
        }
        let result = assess_cost_scenario(&trace, &missing_valuation).unwrap();
        assert!(result.report.transaction_net.is_none()); // Even an explicit zero cannot invent valuation evidence.
        assert!(serde_json::to_value(result).unwrap()["report"]["transaction_net"].is_null());
    }
    #[test]
    fn unknown_funding_and_overhead_are_retained_without_optimistic_defaults() {
        let trace = trace();
        let mut scenario = scenario(&trace);
        scenario.funding = FundingAssumption::Unknown {
            reason: "FUNDING_NOT_MODELED".into(),
        };
        scenario.overhead = OverheadAllocation::Unknown {
            reason: "ALLOCATION_UNKNOWN".into(),
        };
        let assessment = assess_cost_scenario(&trace, &scenario).unwrap();
        assert!(assessment.report.transaction_net.is_none());
        assert!(assessment.report.fully_allocated_net.is_none());
        assert_eq!(assessment.scenario.funding, scenario.funding);
        assert_eq!(assessment.report.overhead, scenario.overhead);
        assert!(
            assessment
                .report
                .incomplete_reasons
                .iter()
                .any(|reason| reason.contains("FUNDING_NOT_MODELED"))
        );
    }
    #[test]
    fn historical_valuation_rejects_future_stale_and_zero_but_accepts_exact_boundary() {
        let trace = trace();
        for (time, accepted) in [
            (trace.observed_at_unix_ms, true),
            (trace.observed_at_unix_ms - 1000, true),
            (trace.observed_at_unix_ms - 1001, false),
            (trace.observed_at_unix_ms + 1, false),
            (u64::MAX, false),
            (0, false),
        ] {
            let mut scenario = scenario(&trace);
            if let ExpenseAmount::Known {
                valuation:
                    Valuation::Ratio {
                        valued_at_unix_ms, ..
                    },
                ..
            } = &mut scenario.expenses[0].amount
            {
                *valued_at_unix_ms = time;
            }
            assert_eq!(
                assess_cost_scenario(&trace, &scenario).is_ok(),
                accepted,
                "time {time}"
            );
        }
    }
    #[test]
    fn sealed_trace_and_all_retained_assessment_fields_are_verified_on_replay() {
        let trace = trace();
        let scenario = scenario(&trace);
        let assessment = assess_cost_scenario(&trace, &scenario).unwrap();
        let mut bad_trace = trace.clone();
        bad_trace.configuration_digest = format!("sha256:{}", "b".repeat(64));
        assert!(assess_cost_scenario(&bad_trace, &scenario).is_err());
        assert!(assessment.replay(&bad_trace.seal().unwrap()).is_err());
        let mut bad_trace = trace.clone();
        bad_trace.experiment_id = "different-experiment".into();
        assert!(assessment.replay(&bad_trace.seal().unwrap()).is_err());
        let original = serde_json::to_value(&assessment).unwrap();
        for (pointer, value) in [
            ("/binding/amount_in_minor", "101"),
            ("/binding/quoted_output_minor", "1000000000"),
            ("/binding/generation", "1"),
            ("/binding/session_id", "another-session"),
            ("/binding/configuration_digest", "sha256:bad"),
            ("/binding/observation_id", "sha256:bad"),
            ("/binding/decision_digest", "sha256:bad"),
            ("/scenario/provenance_reference", "different-source"),
            ("/scenario_digest", "sha256:bad"),
            ("/assessment_id", "sha256:bad"),
            ("/calculation_version", "future-version"),
            ("/report/transaction_net", "9999999"),
        ] {
            let mut wire = original.clone();
            *wire.pointer_mut(pointer).unwrap() = value.into();
            let modified: CostAssessment = serde_json::from_value(wire).unwrap();
            assert!(modified.replay(&trace).is_err(), "tamper at {pointer}");
        }
    }
    #[test]
    fn complete_manual_costs_preserve_candidate_evidence_even_for_recorded_quotes() {
        let mut trace = trace();
        trace.dataset_origin = DatasetOrigin::RecordedLive;
        trace.source_kind = SourceKind::CapturedMarketData;
        let trace = trace.seal().unwrap();
        let assessment = assess_cost_scenario(&trace, &scenario(&trace)).unwrap();
        assert!(assessment.report.costs_complete());
        assert_eq!(assessment.evidence, CostAssessmentEvidence::Candidate);
        assert_eq!(
            assessment.scenario.origin,
            CostScenarioOrigin::ManuallyConstructed
        );
        assert_eq!(
            assessment.binding.dataset_origin,
            DatasetOrigin::RecordedLive
        );
        assert_eq!(
            trace.to_opportunity().unwrap().unwrap().evidence_label,
            arb_domain::Evidence::Candidate
        );
        let mut promoted = serde_json::to_value(assessment).unwrap();
        promoted["evidence"] = "SIMULATED".into();
        assert!(serde_json::from_value::<CostAssessment>(promoted).is_err());
    }
    #[test]
    fn fee_decomposition_rejects_duplicates_double_count_and_cross_network_inputs() {
        let trace = trace();
        let mut duplicate = scenario(&trace);
        duplicate.expenses.push(duplicate.expenses[0].clone());
        assert!(assess_cost_scenario(&trace, &duplicate).is_err());
        let mut priority = scenario(&trace);
        let mut fee = priority.expenses[0].clone();
        fee.kind = ExpenseKind::PriorityFee;
        priority.expenses.push(fee);
        assess_cost_scenario(&trace, &priority).unwrap();
        amount(&mut priority, ExpenseKind::PriorityFee, 1);
        assert!(assess_cost_scenario(&trace, &priority).is_err());
        let mut composition = scenario(&trace);
        composition.fee_composition = FeeComposition::SolanaBaseExcludesPriority;
        assert!(assess_cost_scenario(&trace, &composition).is_err());
        let mut currency = scenario(&trace);
        if let ExpenseAmount::Known { asset, .. } = &mut currency.expenses[0].amount {
            *asset = AccountingAsset::Native(NetworkId::SolanaMainnet);
        }
        assert!(assess_cost_scenario(&trace, &currency).is_err());
    }
    #[test]
    fn strict_scenario_wire_rejects_overrides_unknown_fields_and_unsafe_references() {
        let trace = trace();
        let original = serde_json::to_value(scenario(&trace)).unwrap();
        for field in [
            "network_id",
            "starting_asset",
            "amount_in_minor",
            "quoted_output_minor",
            "unexpected",
        ] {
            let mut wire = original.clone();
            wire[field] = "override".into();
            assert!(
                serde_json::from_value::<CostScenario>(wire).is_err(),
                "field {field}"
            );
        }
        let mut wire = original.clone();
        wire["expenses"][0]["amount"]["valuation"]["hidden"] = true.into();
        assert!(serde_json::from_value::<CostScenario>(wire).is_err());
        let mut wire = original;
        wire["origin"] = "RECORDED_LIVE".into();
        assert!(serde_json::from_value::<CostScenario>(wire).is_err());
        for reference in [
            "",
            "https://rpc.example/secret",
            "reference\nsecret",
            "unicode-ä",
            "sha256:bad",
            "=FORMULA",
            " space",
        ] {
            let mut scenario = scenario(&trace);
            scenario.provenance_reference = reference.into();
            assert!(
                assess_cost_scenario(&trace, &scenario).is_err(),
                "reference {reference:?}"
            );
        }
        let mut scenario = scenario(&trace);
        scenario.version = "v".repeat(65);
        assert!(assess_cost_scenario(&trace, &scenario).is_err());
        scenario.version = "1".into();
        for age in [0, MAX_SCENARIO_VALUATION_AGE_MS + 1, u64::MAX] {
            scenario.valuation_max_age_ms = age;
            assert!(assess_cost_scenario(&trace, &scenario).is_err());
        }
    }
    #[test]
    fn assessment_uses_existing_exact_upward_rounding_and_overflow_guards() {
        let trace = trace();
        let mut scenario = scenario(&trace);
        if let ExpenseAmount::Known {
            amount, valuation, ..
        } = &mut scenario.expenses[0].amount
        {
            *amount = 1.into();
            *valuation = Valuation::Ratio {
                numerator: 1.into(),
                denominator: 3.into(),
                reference: "manual-thirds".into(),
                valued_at_unix_ms: trace.observed_at_unix_ms,
            };
        }
        let assessment = assess_cost_scenario(&trace, &scenario).unwrap();
        assert_eq!(assessment.report.transaction_net.unwrap().to_string(), "-2");
        if let ExpenseAmount::Known {
            amount, valuation, ..
        } = &mut scenario.expenses[0].amount
        {
            *amount =
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
                    .parse()
                    .unwrap();
            *valuation = Valuation::Ratio {
                numerator: 2.into(),
                denominator: 1.into(),
                reference: "manual-overflow".into(),
                valued_at_unix_ms: trace.observed_at_unix_ms,
            };
        }
        assert!(assess_cost_scenario(&trace, &scenario).is_err());
    }
    #[test]
    fn same_asset_funding_is_valid_but_native_chain_fees_cannot_use_wrapped_identity() {
        let trace = trace();
        let mut scenario = scenario(&trace);
        let funding = scenario
            .expenses
            .iter_mut()
            .find(|expense| expense.kind == ExpenseKind::Funding)
            .unwrap();
        funding.amount = ExpenseAmount::Known {
            asset: AccountingAsset::Token(trace.route[0].asset_in.clone()),
            amount: 2.into(),
            valuation: Valuation::SameAsset {
                reference: "manual-own-capital-cost".into(),
                valued_at_unix_ms: trace.observed_at_unix_ms,
            },
        };
        assert_eq!(
            assess_cost_scenario(&trace, &scenario)
                .unwrap()
                .report
                .transaction_net
                .unwrap()
                .to_string(),
            "-3"
        );
        let execution = scenario
            .expenses
            .iter_mut()
            .find(|expense| expense.kind == ExpenseKind::NetworkExecution)
            .unwrap();
        execution.amount = ExpenseAmount::Known {
            asset: AccountingAsset::Token(trace.route[0].asset_in.clone()),
            amount: 2.into(),
            valuation: Valuation::SameAsset {
                reference: "wrong-native-identity".into(),
                valued_at_unix_ms: trace.observed_at_unix_ms,
            },
        };
        assert!(assess_cost_scenario(&trace, &scenario).is_err());
    }
    #[test]
    fn same_token_expense_cannot_discount_its_own_units_with_a_conversion_ratio() {
        let trace = trace();
        let mut scenario = scenario(&trace);
        let funding = scenario
            .expenses
            .iter_mut()
            .find(|expense| expense.kind == ExpenseKind::Funding)
            .unwrap();
        funding.amount = ExpenseAmount::Known {
            asset: AccountingAsset::Token(trace.route[0].asset_in.clone()),
            amount: 100.into(),
            valuation: Valuation::Ratio {
                numerator: 1.into(),
                denominator: 100.into(),
                reference: "invalid-own-currency-discount".into(),
                valued_at_unix_ms: trace.observed_at_unix_ms,
            },
        };
        assert!(scenario.validate(&trace).is_err());
        assert!(assess_cost_scenario(&trace, &scenario).is_err());
        let quote = Quote {
            starting_asset: trace.route[0].asset_in.clone(),
            amount_in: 100.into(),
            amount_out: 99.into(),
            includes_pool_fees_and_impact: true,
            quote_reference: trace.observation_id.clone(),
        };
        assert!(
            evaluate_costs(
                &quote,
                &scenario.expenses,
                scenario.funding.clone(),
                scenario.overhead.clone()
            )
            .is_err()
        );
        let funding = scenario
            .expenses
            .iter_mut()
            .find(|expense| expense.kind == ExpenseKind::Funding)
            .unwrap();
        if let ExpenseAmount::Known { valuation, .. } = &mut funding.amount {
            *valuation = Valuation::SameAsset {
                reference: "same-token-units".into(),
                valued_at_unix_ms: trace.observed_at_unix_ms,
            };
        }
        assert_eq!(
            assess_cost_scenario(&trace, &scenario)
                .unwrap()
                .report
                .transaction_net
                .unwrap()
                .to_string(),
            "-101"
        );
    }
    #[test]
    fn nonquoted_decisions_cannot_fabricate_costed_quotes() {
        let mut trace = trace();
        trace.result = DecisionResult::Rejected {
            reason_codes: vec!["CAPTURE_CONTEXT_MISMATCH".into()],
        };
        let trace = trace.seal().unwrap();
        assert!(assess_cost_scenario(&trace, &scenario(&trace)).is_err());
    }
    #[test]
    fn solana_base_priority_and_tip_are_separate_and_l1_data_is_not_double_counted() {
        let mut trace = trace();
        let network = NetworkId::SolanaMainnet;
        let addresses = [
            "So11111111111111111111111111111111111111112",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        ];
        trace.network_id = network;
        for (i, leg) in trace.route.iter_mut().enumerate() {
            leg.asset_in = AssetId::new(network, addresses[i]).unwrap();
            leg.asset_out = AssetId::new(network, addresses[1 - i]).unwrap();
            leg.pool_id = PoolId::new(network, addresses[i]).unwrap();
            leg.venue_family = "orca-whirlpools".into();
        }
        trace.result = DecisionResult::Quoted {
            quoted_output_minor: 110.into(),
            gross_delta_minor: SignedAmount::difference(&110.into(), &100.into()),
            included_pool_fees: vec![1.into(), 1.into()],
        };
        let trace = trace.seal().unwrap();
        let mut scenario = scenario(&trace);
        amount(&mut scenario, ExpenseKind::NetworkExecution, 5);
        amount(&mut scenario, ExpenseKind::PriorityFee, 4);
        amount(&mut scenario, ExpenseKind::RelayTip, 3);
        let assessment = assess_cost_scenario(&trace, &scenario).unwrap();
        assert_eq!(assessment.report.transaction_net.unwrap().to_string(), "-2");
        let mut l1 = scenario.expenses[0].clone();
        l1.kind = ExpenseKind::BaseL1Data;
        scenario.expenses.push(l1);
        assert!(assess_cost_scenario(&trace, &scenario).is_err());
    }
    #[test]
    fn independent_python_fixture_reproduces_full_assessment_and_content_hashes() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../specs/cost-assessment.example.json"))
                .unwrap();
        let trace: DecisionTrace =
            serde_json::from_value(fixture["source_decision"]["trace"].clone()).unwrap();
        let scenario: CostScenario =
            serde_json::from_value(fixture["request"]["scenario"].clone()).unwrap();
        let expected: CostAssessment =
            serde_json::from_value(fixture["record"]["assessment"].clone()).unwrap();
        trace.validate().unwrap();
        let actual = assess_cost_scenario(&trace, &scenario).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual.report.transaction_net.as_ref().unwrap().to_string(),
            "-2001"
        );
        assert_eq!(
            actual
                .report
                .fully_allocated_net
                .as_ref()
                .unwrap()
                .to_string(),
            "-2501"
        );
        assert_eq!(
            actual
                .report
                .expenses
                .iter()
                .find(|expense| expense.expense.kind == ExpenseKind::NetworkExecution)
                .unwrap()
                .in_start_asset
                .as_ref()
                .unwrap()
                .to_string(),
            "10001"
        );
        expected.replay(&trace).unwrap();
    }
    #[test]
    fn canonical_hashes_do_not_depend_on_input_object_key_order() {
        let trace = trace();
        let scenario = scenario(&trace);
        let encoded = serde_json::to_value(&scenario).unwrap();
        let mut object = encoded.as_object().unwrap().iter().collect::<Vec<_>>();
        object.reverse();
        let reversed = format!(
            "{{{}}}",
            object
                .into_iter()
                .map(|(key, value)| format!("{}:{}", serde_json::to_string(key).unwrap(), value))
                .collect::<Vec<_>>()
                .join(",")
        );
        let decoded: CostScenario = serde_json::from_str(&reversed).unwrap();
        let first = assess_cost_scenario(&trace, &scenario).unwrap();
        assert_eq!(first, assess_cost_scenario(&trace, &decoded).unwrap());
        assert_eq!(first, assess_cost_scenario(&trace, &scenario).unwrap());
    }
}
