//! Declared chain-time research assumptions, never execution or reorg evidence.
use crate::{DecisionError, NetworkId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CHAIN_FRESHNESS_VERSION: &str = "finalized-chain-time-v1";
pub const FRESHNESS_CALCULATION_VERSION: &str =
    "capture-pair-research-v2;bounds8x63;group1000;finalized-chain-time-v1";
pub const MAX_CHAIN_TIMESTAMP_SECONDS: u64 = 253_402_300_799;
pub const MAX_CHAIN_REFERENCE_MS: u64 = 253_402_300_799_999;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainFreshnessPolicy {
    pub version: String,
    pub max_chain_age_ms: u64,
}
impl ChainFreshnessPolicy {
    pub fn validate(&self) -> Result<(), DecisionError> {
        if self.version != CHAIN_FRESHNESS_VERSION
            || !(1..=86_400_000).contains(&self.max_chain_age_ms)
        {
            return Err(invalid("unsupported chain-time policy or age limit"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChainFreshnessStatus {
    Unknown,
    Future,
    Stale,
    WithinPolicy,
}
impl ChainFreshnessStatus {
    pub fn reason_code(self) -> Option<&'static str> {
        match self {
            Self::Unknown => Some("CHAIN_TIME_UNAVAILABLE"),
            Self::Future => Some("CHAIN_TIME_FUTURE"),
            Self::Stale => Some("CHAIN_TIME_STALE"),
            Self::WithinPolicy => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum ChainTimeSource {
    BaseFinalizedBlockTimestamp {
        block_number: String,
        block_hash: String,
        parent_hash: String,
    },
    SolanaEstimatedBlockTime {
        slot: String,
        genesis_hash: String,
        account_context: String,
    },
}
impl ChainTimeSource {
    pub fn network_id(&self) -> NetworkId {
        match self {
            Self::BaseFinalizedBlockTimestamp { .. } => NetworkId::BaseMainnet,
            Self::SolanaEstimatedBlockTime { .. } => NetworkId::SolanaMainnet,
        }
    }
    fn validate(&self) -> Result<(), DecisionError> {
        let canonical_height =
            |value: &str| value.parse::<u64>().is_ok_and(|n| n.to_string() == value);
        let hash = |value: &str| {
            value.strip_prefix("0x").is_some_and(|hex| {
                hex.len() == 64
                    && hex
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        };
        let valid = match self {
            Self::BaseFinalizedBlockTimestamp {
                block_number,
                block_hash,
                parent_hash,
            } => canonical_height(block_number) && hash(block_hash) && hash(parent_hash),
            Self::SolanaEstimatedBlockTime {
                slot,
                genesis_hash,
                account_context,
            } => {
                canonical_height(slot)
                    && label(account_context)
                    && bs58::decode(genesis_hash).into_vec().is_ok_and(|bytes| {
                        bytes.len() == 32
                            && bytes.iter().any(|b| *b != 0)
                            && bs58::encode(&bytes).into_string() == *genesis_hash
                    })
            }
        };
        if !valid {
            return Err(invalid("invalid chain-time source context"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainTimeInput {
    pub capture_id: String,
    pub source: ChainTimeSource,
    pub chain_time_seconds: Option<u64>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainTimeObservation {
    pub capture_id: String,
    pub source: ChainTimeSource,
    pub chain_time_seconds: Option<u64>,
    pub age_ms: Option<u64>,
    pub status: ChainFreshnessStatus,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainFreshnessReport {
    pub policy: ChainFreshnessPolicy,
    pub reference_observed_at_unix_ms: u64,
    pub evaluation_elapsed_ms: u64,
    pub sources: Vec<ChainTimeObservation>,
    pub status: ChainFreshnessStatus,
}
pub(crate) fn deserialize_present_report<'de, D>(
    deserializer: D,
) -> Result<Option<ChainFreshnessReport>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ChainFreshnessReport::deserialize(deserializer).map(Some)
}
fn invalid(reason: &'static str) -> DecisionError {
    DecisionError {
        field: "chain_freshness",
        reason,
    }
}
fn label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
impl ChainFreshnessReport {
    pub fn assess(
        policy: ChainFreshnessPolicy,
        reference_observed_at_unix_ms: u64,
        evaluation_elapsed_ms: u64,
        inputs: Vec<ChainTimeInput>,
    ) -> Result<Self, DecisionError> {
        policy.validate()?;
        let now = reference_observed_at_unix_ms
            .checked_add(evaluation_elapsed_ms)
            .filter(|now| reference_observed_at_unix_ms > 0 && *now <= MAX_CHAIN_REFERENCE_MS)
            .ok_or_else(|| invalid("invalid or overflowing reference and elapsed time"))?;
        if inputs.len() > 8 {
            return Err(invalid("chain-time source bound exceeded"));
        }
        let mut captures = HashSet::new();
        let mut sources = Vec::with_capacity(inputs.len());
        for input in inputs {
            if !label(&input.capture_id) || !captures.insert(input.capture_id.clone()) {
                return Err(invalid("distinct bounded capture identifiers required"));
            }
            input.source.validate()?;
            let (age_ms, status) = match input.chain_time_seconds {
                None => (None, ChainFreshnessStatus::Unknown),
                Some(seconds) => {
                    if seconds > MAX_CHAIN_TIMESTAMP_SECONDS {
                        return Err(invalid("chain timestamp outside supported exact UTC range"));
                    }
                    let timestamp = seconds
                        .checked_mul(1000)
                        .ok_or_else(|| invalid("chain timestamp conversion overflow"))?;
                    match now.checked_sub(timestamp) {
                        None => (None, ChainFreshnessStatus::Future),
                        Some(age) if age > policy.max_chain_age_ms => {
                            (Some(age), ChainFreshnessStatus::Stale)
                        }
                        Some(age) => (Some(age), ChainFreshnessStatus::WithinPolicy),
                    }
                }
            };
            sources.push(ChainTimeObservation {
                capture_id: input.capture_id,
                source: input.source,
                chain_time_seconds: input.chain_time_seconds,
                age_ms,
                status,
            });
        }
        // Failures stay visible even in mixed evidence: future, unknown, then stale.
        let status = if sources
            .iter()
            .any(|s| s.status == ChainFreshnessStatus::Future)
        {
            ChainFreshnessStatus::Future
        } else if sources.is_empty()
            || sources
                .iter()
                .any(|s| s.status == ChainFreshnessStatus::Unknown)
        {
            ChainFreshnessStatus::Unknown
        } else if sources
            .iter()
            .any(|s| s.status == ChainFreshnessStatus::Stale)
        {
            ChainFreshnessStatus::Stale
        } else {
            ChainFreshnessStatus::WithinPolicy
        };
        Ok(Self {
            policy,
            reference_observed_at_unix_ms,
            evaluation_elapsed_ms,
            sources,
            status,
        })
    }
    pub fn validate(&self) -> Result<(), DecisionError> {
        let inputs = self
            .sources
            .iter()
            .map(|source| ChainTimeInput {
                capture_id: source.capture_id.clone(),
                source: source.source.clone(),
                chain_time_seconds: source.chain_time_seconds,
            })
            .collect();
        let expected = Self::assess(
            self.policy.clone(),
            self.reference_observed_at_unix_ms,
            self.evaluation_elapsed_ms,
            inputs,
        )?;
        if self != &expected {
            return Err(invalid(
                "chain-time arithmetic or status differs from evidence",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> ChainFreshnessPolicy {
        ChainFreshnessPolicy {
            version: CHAIN_FRESHNESS_VERSION.into(),
            max_chain_age_ms: 1000,
        }
    }
    fn input(n: u64, timestamp: Option<u64>) -> ChainTimeInput {
        ChainTimeInput {
            capture_id: format!("capture-{n}"),
            source: ChainTimeSource::BaseFinalizedBlockTimestamp {
                block_number: n.to_string(),
                block_hash: format!("0x{}", "a".repeat(64)),
                parent_hash: format!("0x{}", "b".repeat(64)),
            },
            chain_time_seconds: timestamp,
        }
    }
    #[test]
    fn boundary_future_unknown_and_monotone_processing_age_are_exact() {
        for (reference, elapsed, timestamp, status, age) in [
            (
                10_000,
                1000,
                Some(10),
                ChainFreshnessStatus::WithinPolicy,
                Some(1000),
            ),
            (
                10_000,
                1001,
                Some(10),
                ChainFreshnessStatus::Stale,
                Some(1001),
            ),
            (10_000, 999, Some(11), ChainFreshnessStatus::Future, None),
            (
                10_000,
                1000,
                Some(11),
                ChainFreshnessStatus::WithinPolicy,
                Some(0),
            ),
            (10_000, 10, None, ChainFreshnessStatus::Unknown, None),
        ] {
            let report = ChainFreshnessReport::assess(
                policy(),
                reference,
                elapsed,
                vec![input(1, timestamp)],
            )
            .unwrap();
            assert_eq!(report.status, status);
            assert_eq!(report.sources[0].age_ms, age);
            report.validate().unwrap();
        }
    }
    #[test]
    fn malformed_ranges_duplicate_sources_and_overflow_never_become_fresh() {
        for (reference, elapsed, timestamp) in [
            (0, 0, Some(0)),
            (u64::MAX, 1, Some(1)),
            (MAX_CHAIN_REFERENCE_MS, 1, Some(1)),
            (10_000, 0, Some(u64::MAX)),
            (10_000, 0, Some(MAX_CHAIN_TIMESTAMP_SECONDS + 1)),
        ] {
            assert!(
                ChainFreshnessReport::assess(
                    policy(),
                    reference,
                    elapsed,
                    vec![input(1, timestamp)]
                )
                .is_err()
            );
        }
        assert!(
            ChainFreshnessReport::assess(
                policy(),
                10_000,
                0,
                vec![input(1, Some(10)), input(1, Some(10))]
            )
            .is_err()
        );
        assert!(
            ChainFreshnessReport::assess(
                policy(),
                10_000,
                0,
                (0..9).map(|n| input(n, Some(10))).collect()
            )
            .is_err()
        );
        for limit in [0, 86_400_001, u64::MAX] {
            let mut p = policy();
            p.max_chain_age_ms = limit;
            assert!(p.validate().is_err());
        }
    }
    #[test]
    fn aggregate_precedence_and_tamper_checks_are_deterministic() {
        let mixed = vec![input(1, Some(1)), input(2, None), input(3, Some(11))];
        let mut report = ChainFreshnessReport::assess(policy(), 10_000, 0, mixed).unwrap();
        assert_eq!(report.status, ChainFreshnessStatus::Future);
        report.sources.pop();
        report.status = ChainFreshnessStatus::Unknown;
        report.validate().unwrap();
        report.sources.pop();
        report.status = ChainFreshnessStatus::Stale;
        report.validate().unwrap();
        report.sources[0].age_ms = Some(0);
        assert!(report.validate().is_err());
        let empty = ChainFreshnessReport::assess(policy(), 10_000, 0, vec![]).unwrap();
        assert_eq!(empty.status, ChainFreshnessStatus::Unknown);
    }
}
