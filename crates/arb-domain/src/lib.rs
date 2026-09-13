//! Deterministic research-domain foundation. No I/O, signing or transport.
//!
//! This is an in-memory model, not a durable command processor. Callers must
//! serialize it with their actual worker gate and journal before integration.

mod amount;
mod decision;
mod freshness;
mod identity;
mod lifecycle;
mod opportunity;

pub use amount::{AmountError, AtomicAmount, Decimals, Rounding, SignedAmount};
pub use decision::*;
pub use freshness::*;
pub use identity::{AssetId, FixtureId, IdentityError, NetworkId, PoolId, Route, RouteLeg};
pub use lifecycle::{Action, ControlError, Mode, Progress, Session, SessionSnapshot, State};
pub use opportunity::*;

/// Evidence names are classifications, not an automatic proof of eligibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Evidence {
    Candidate,
    Simulated,
    EstimatedExecutable,
    Realized,
}

/// A necessary mode check only. Full simulation, funding and accounting
/// evidence validation belongs to the planned simulation and ledger layers.
pub fn validate_evidence_mode(mode: Mode, evidence: Evidence) -> Result<(), ControlError> {
    if evidence == Evidence::Realized && mode != Mode::Live {
        return Err(ControlError::RealizedRequiresLive);
    }
    Ok(())
}
