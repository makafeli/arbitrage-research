//! Exact hypothetical economics and replayable virtual inventory. No signing,
//! network transport, real fills or implicit promotion to execution eligibility.
mod cost;
mod portfolio;
pub use cost::*;
pub use portfolio::*;
