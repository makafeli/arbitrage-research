//! The Base simulation manifest contract and the exact-plan evidence gate.
//!
//! A manifest is the record of one simulation run of one exact plan against
//! one identified state. This module does not run a simulation: it decides
//! what a manifest is *allowed to claim* once it exists. Hand-written
//! manifests in this crate's tests are test inputs exercising that gate, not
//! evidence of anything real. There is no fork runner here, no RPC call and
//! no filesystem or network access: `crates/arb-evm/tests/simulation.rs`
//! builds every manifest either from a committed JSON fixture or in-process,
//! never from a live run.
use crate::plan::{BasePlan, PlanDigest};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

pub const REASON_SYNTHETIC_FIXTURE_STATE: &str = "SYNTHETIC_FIXTURE_STATE";
pub const REASON_FORK_STATE_UNAVAILABLE: &str = "FORK_STATE_UNAVAILABLE";
pub const REASON_STATE_CHAIN_MISMATCH: &str = "STATE_CHAIN_MISMATCH";
pub const REASON_SIMULATION_REVERTED: &str = "SIMULATION_REVERTED";
pub const REASON_SYNTHETIC_FUNDING_OVERRIDE: &str = "SYNTHETIC_FUNDING_OVERRIDE";
pub const REASON_FINAL_BALANCE_BELOW_FLOOR: &str = "FINAL_BALANCE_BELOW_FLOOR";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StateKind {
    PinnedFork,
    SyntheticFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateIdentity {
    pub kind: StateKind,
    pub chain_id: u64,
    pub block_number: u64,
    /// `None` means the run could not identify the state (see
    /// [`REASON_FORK_STATE_UNAVAILABLE`]).
    #[serde(default)]
    pub block_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    /// `"0x"` + 64 hex: keccak of the deployed guard runtime code.
    pub guard_code_hash: String,
    /// Free text, e.g. `"foundry 1.8.3 / solc 0.8.28"`.
    pub toolchain: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverrideKind {
    NativeBalance,
    TokenBalance,
    Allowance,
    Code,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateOverride {
    pub kind: OverrideKind,
    pub account: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub spender: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum SimulationOutcome {
    Executed {
        emitted_digest: String,
        final_balance: String,
        gas_used: u64,
    },
    /// Selector name or raw revert data, retained verbatim.
    Reverted { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationManifest {
    pub schema_version: u32,
    /// `"sha256:"` + 64 hex: the plan the run executed.
    pub plan_digest: String,
    pub state: StateIdentity,
    pub artifact: ArtifactIdentity,
    pub overrides: Vec<StateOverride>,
    pub outcome: SimulationOutcome,
    /// RFC 3339.
    pub executed_at: String,
}

/// What a manifest is allowed to claim about one [`BasePlan`], never more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactPlanEvidence {
    pub plan_digest: PlanDigest,
    /// Never `NotRun`: `bind()` only ever returns evidence for a manifest
    /// that named an outcome.
    pub simulation_status: arb_domain::SimulationStatus,
    pub simulation_matches_exact_plan: bool,
    pub synthetic_funding: bool,
    pub realized_market_claim_allowed: bool,
    pub state: StateIdentity,
    pub reason_codes: Vec<&'static str>,
}

/// A hard rejection: no [`ExactPlanEvidence`] is produced at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvidenceGap {
    SchemaVersion {
        found: u32,
    },
    /// `manifest.plan_digest` or `Executed.emitted_digest` did not match
    /// `plan.digest()`. Any change to the plan changes the digest, which is
    /// exactly the "changed route/amount/guard/fee invalidates prior
    /// evidence" criterion.
    DigestMismatch {
        expected: PlanDigest,
        found: String,
    },
    MalformedField {
        field: &'static str,
    },
}
impl fmt::Display for EvidenceGap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaVersion { found } => {
                write!(
                    f,
                    "manifest schema version {found} does not match {MANIFEST_SCHEMA_VERSION}"
                )
            }
            Self::DigestMismatch { expected, found } => {
                write!(
                    f,
                    "manifest digest {found} does not match plan digest {expected}"
                )
            }
            Self::MalformedField { field } => write!(f, "malformed manifest field: {field}"),
        }
    }
}
impl std::error::Error for EvidenceGap {}

fn is_hex(value: &str, prefix: &str, len: usize) -> bool {
    value.strip_prefix(prefix).is_some_and(|hex_part| {
        hex_part.len() == len && hex_part.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

fn malformed(field: &'static str) -> EvidenceGap {
    EvidenceGap::MalformedField { field }
}

fn check_override_shape(o: &StateOverride) -> Result<(), EvidenceGap> {
    if !is_hex(&o.account, "0x", 40) {
        return Err(malformed("override.account"));
    }
    if let Some(token) = &o.token
        && !is_hex(token, "0x", 40)
    {
        return Err(malformed("override.token"));
    }
    if let Some(spender) = &o.spender
        && !is_hex(spender, "0x", 40)
    {
        return Err(malformed("override.spender"));
    }
    if let Some(amount) = &o.amount
        && U256::from_dec_str(amount).is_err()
    {
        return Err(malformed("override.amount"));
    }
    // ponytail: one match per kind, not a generic "required fields" table —
    // there are four kinds and the table would be harder to read than this.
    match o.kind {
        OverrideKind::NativeBalance => {
            if o.amount.is_none() {
                return Err(malformed("override.amount"));
            }
        }
        OverrideKind::TokenBalance => {
            if o.token.is_none() || o.amount.is_none() {
                return Err(malformed("override.token"));
            }
        }
        OverrideKind::Allowance => {
            if o.token.is_none() || o.spender.is_none() || o.amount.is_none() {
                return Err(malformed("override.spender"));
            }
        }
        OverrideKind::Code => {}
    }
    Ok(())
}

fn check_manifest_shape(manifest: &SimulationManifest) -> Result<(), EvidenceGap> {
    if !is_hex(&manifest.plan_digest, "sha256:", 64) {
        return Err(malformed("plan_digest"));
    }
    if let Some(block_hash) = &manifest.state.block_hash
        && !is_hex(block_hash, "0x", 64)
    {
        return Err(malformed("state.block_hash"));
    }
    if !is_hex(&manifest.artifact.guard_code_hash, "0x", 64) {
        return Err(malformed("artifact.guard_code_hash"));
    }
    for o in &manifest.overrides {
        check_override_shape(o)?;
    }
    Ok(())
}

fn is_synthetic_funding(overrides: &[StateOverride]) -> bool {
    overrides.iter().any(|o| {
        matches!(
            o.kind,
            OverrideKind::NativeBalance | OverrideKind::TokenBalance | OverrideKind::Allowance
        )
    })
}

/// Binds a manifest to a plan, deciding what evidence the manifest is
/// allowed to produce. Never executes anything; `manifest` is assumed to
/// already exist. See the module doc for what a hard rejection ([`EvidenceGap`])
/// means versus a named, non-passing [`arb_domain::SimulationStatus`].
pub fn bind(
    manifest: &SimulationManifest,
    plan: &BasePlan,
) -> Result<ExactPlanEvidence, EvidenceGap> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(EvidenceGap::SchemaVersion {
            found: manifest.schema_version,
        });
    }
    check_manifest_shape(manifest)?;

    let plan_digest = plan.digest();
    if manifest.plan_digest != plan_digest.0 {
        return Err(EvidenceGap::DigestMismatch {
            expected: plan_digest,
            found: manifest.plan_digest.clone(),
        });
    }
    if let SimulationOutcome::Executed { emitted_digest, .. } = &manifest.outcome
        && *emitted_digest != plan_digest.0
    {
        return Err(EvidenceGap::DigestMismatch {
            expected: plan_digest,
            found: emitted_digest.clone(),
        });
    }

    let mut reason_codes = Vec::new();
    // `Unsupported` wins over the outcome: a fixture that "executed" is
    // still `Unsupported`, checked before looking at the outcome at all.
    let mut status = if manifest.state.kind == StateKind::SyntheticFixture {
        reason_codes.push(REASON_SYNTHETIC_FIXTURE_STATE);
        arb_domain::SimulationStatus::Unsupported
    } else if manifest.state.block_hash.is_none() {
        reason_codes.push(REASON_FORK_STATE_UNAVAILABLE);
        arb_domain::SimulationStatus::Unsupported
    } else if manifest.state.chain_id != plan.chain_id {
        reason_codes.push(REASON_STATE_CHAIN_MISMATCH);
        arb_domain::SimulationStatus::Unsupported
    } else {
        match &manifest.outcome {
            SimulationOutcome::Reverted { .. } => {
                reason_codes.push(REASON_SIMULATION_REVERTED);
                arb_domain::SimulationStatus::Failed
            }
            SimulationOutcome::Executed { .. } => arb_domain::SimulationStatus::Passed,
        }
    };

    let synthetic_funding = is_synthetic_funding(&manifest.overrides);
    let realized_market_claim_allowed =
        !synthetic_funding && manifest.state.kind == StateKind::PinnedFork;
    if synthetic_funding {
        reason_codes.push(REASON_SYNTHETIC_FUNDING_OVERRIDE);
    }

    // A manifest claiming `Passed` still owes a final balance at or above the
    // plan's floor; an unparseable amount cannot establish that and fails
    // safe rather than passing.
    if status == arb_domain::SimulationStatus::Passed
        && let SimulationOutcome::Executed { final_balance, .. } = &manifest.outcome
    {
        let meets_floor = U256::from_dec_str(final_balance)
            .is_ok_and(|balance| balance >= plan.min_final_balance);
        if !meets_floor {
            status = arb_domain::SimulationStatus::Failed;
            reason_codes.push(REASON_FINAL_BALANCE_BELOW_FLOOR);
        }
    }

    let simulation_matches_exact_plan = status == arb_domain::SimulationStatus::Passed;
    Ok(ExactPlanEvidence {
        plan_digest: plan.digest(),
        simulation_status: status,
        simulation_matches_exact_plan,
        synthetic_funding,
        realized_market_claim_allowed,
        state: manifest.state.clone(),
        reason_codes,
    })
}
