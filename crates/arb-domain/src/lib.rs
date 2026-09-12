//! Deterministic research-domain foundation. No I/O, signing or transport.
//!
//! This is an in-memory model, not a durable command processor. Callers must
//! serialize it with their actual worker gate and journal before integration.

mod amount;
mod lifecycle;

pub use amount::{AtomicAmount, AmountError};
pub use lifecycle::{Action, ControlError, Mode, Progress, Session, State};

/// Evidence names are classifications, not an automatic proof of eligibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
