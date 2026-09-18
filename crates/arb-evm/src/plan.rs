//! Research-only description of one intended atomic Base transaction.
//!
//! `BasePlan` is a pure data/validation artifact: it can be built, checked
//! against an allowlist and hashed into a canonical digest, but nothing in
//! this module executes, signs or broadcasts anything. It has no RPC calls
//! and no network access.
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

pub use primitive_types::{H160 as Address20, U256};

/// The account whose starting-asset balance funds the route. Distinct from
/// any fee account: see [`PlanRejection::FeeAccountOnly`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpendingAccount {
    pub address: Address20,
    pub principal: U256,
}

/// A token approval a leg's pool relies on to pull `token` from the spending
/// account (or a prior leg's proceeds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allowance {
    pub token: Address20,
    pub spender: Address20,
    pub amount: U256,
}

/// One exact-input Uniswap V3 swap in the route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapLeg {
    pub pool: Address20,
    pub token_in: Address20,
    pub token_out: Address20,
    pub fee_tier: u32,
    pub exact_in: U256,
    pub min_out: U256,
}

/// The only addresses allowed to call back into the on-chain guard: the
/// route's own pools, nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackAuthorization {
    pub pools: Vec<Address20>,
}

/// A research-only description of one intended atomic Base transaction: a
/// cyclic route of Uniswap V3 swaps that starts and ends in `starting_asset`,
/// guarded by a final minimum-balance check.
///
/// This type cannot execute or submit anything. It must never gain a private
/// key, signature or raw signed transaction field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasePlan {
    pub chain_id: u64,
    pub spending_account: SpendingAccount,
    pub starting_asset: Address20,
    pub legs: Vec<SwapLeg>,
    pub allowances: Vec<Allowance>,
    pub deadline_unix: u64,
    pub min_final_balance: U256,
    pub callback_authorization: CallbackAuthorization,
}

/// Why a [`BasePlan`] failed pre-flight validation. This is a rejection
/// classification, not an on-chain revert reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanRejection {
    UnsupportedTarget {
        address: Address20,
    },
    RouteNotCyclic,
    LegDiscontinuity {
        index: usize,
    },
    InsufficientPrincipal,
    InsufficientFinalBalance {
        required: U256,
        guaranteed: U256,
    },
    UnauthorizedCallback {
        address: Address20,
    },
    MissingAllowance {
        token: Address20,
        spender: Address20,
    },
    FeeAccountOnly,
}
impl fmt::Display for PlanRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget { address } => {
                write!(f, "target {address:#x} is not in the pool allowlist")
            }
            Self::RouteNotCyclic => write!(f, "route does not start and end in the same asset"),
            Self::LegDiscontinuity { index } => {
                write!(f, "leg {index} does not chain from the previous leg")
            }
            Self::InsufficientPrincipal => {
                write!(
                    f,
                    "spending account principal is below the first leg's input"
                )
            }
            Self::InsufficientFinalBalance {
                required,
                guaranteed,
            } => write!(
                f,
                "guaranteed final balance {guaranteed} is below required minimum {required}"
            ),
            Self::UnauthorizedCallback { address } => {
                write!(
                    f,
                    "callback address {address:#x} is not an authorized leg pool"
                )
            }
            Self::MissingAllowance { token, spender } => write!(
                f,
                "no sufficient allowance for token {token:#x} to spender {spender:#x}"
            ),
            Self::FeeAccountOnly => write!(f, "spending account has zero principal"),
        }
    }
}
impl std::error::Error for PlanRejection {}

/// Pools and tokens a [`BasePlan`] is permitted to reference. Built from
/// reviewed registry data or literal addresses; never populated from a plan
/// itself.
#[derive(Clone, Debug, Default)]
pub struct PoolAllowlist {
    pools: BTreeSet<Address20>,
    tokens: BTreeSet<Address20>,
}
impl PoolAllowlist {
    pub fn new(
        pools: impl IntoIterator<Item = Address20>,
        tokens: impl IntoIterator<Item = Address20>,
    ) -> Self {
        Self {
            pools: pools.into_iter().collect(),
            tokens: tokens.into_iter().collect(),
        }
    }
    fn allows_pool(&self, address: Address20) -> bool {
        self.pools.contains(&address)
    }
    fn allows_token(&self, address: Address20) -> bool {
        self.tokens.contains(&address)
    }
}

/// `"sha256:" + hex(sha256(canonical_bytes))` over a plan's ABI-style encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanDigest(pub String);
impl fmt::Display for PlanDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn word_u64(value: u64) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}
fn word_address(value: Address20) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[12..].copy_from_slice(value.as_bytes());
    word
}
fn word_u256(value: U256) -> [u8; 32] {
    value.to_big_endian()
}

impl BasePlan {
    /// Validates route continuity, allowlist membership, principal/allowance
    /// funding and the callback/final-balance guard. Returns the first
    /// applicable [`PlanRejection`]; does not read chain state or execute
    /// anything.
    pub fn validate(&self, allowlist: &PoolAllowlist) -> Result<(), PlanRejection> {
        if self.legs.is_empty() {
            return Err(PlanRejection::RouteNotCyclic);
        }
        for leg in &self.legs {
            if !allowlist.allows_pool(leg.pool) {
                return Err(PlanRejection::UnsupportedTarget { address: leg.pool });
            }
            if !allowlist.allows_token(leg.token_in) {
                return Err(PlanRejection::UnsupportedTarget {
                    address: leg.token_in,
                });
            }
            if !allowlist.allows_token(leg.token_out) {
                return Err(PlanRejection::UnsupportedTarget {
                    address: leg.token_out,
                });
            }
        }
        for (index, pair) in self.legs.windows(2).enumerate() {
            if pair[0].token_out != pair[1].token_in {
                return Err(PlanRejection::LegDiscontinuity { index: index + 1 });
            }
        }
        let first_leg = &self.legs[0];
        let last_leg = &self.legs[self.legs.len() - 1];
        if first_leg.token_in != self.starting_asset || last_leg.token_out != self.starting_asset {
            return Err(PlanRejection::RouteNotCyclic);
        }
        // A funded fee account is insufficient: zero principal is rejected
        // before comparing it against the first leg's input, regardless of
        // whether allowances are otherwise present and correct.
        if self.spending_account.principal.is_zero() {
            return Err(PlanRejection::FeeAccountOnly);
        }
        if first_leg.exact_in > self.spending_account.principal {
            return Err(PlanRejection::InsufficientPrincipal);
        }
        let guaranteed = self
            .spending_account
            .principal
            .checked_sub(first_leg.exact_in)
            .expect("checked above: exact_in <= principal")
            .saturating_add(last_leg.min_out);
        if guaranteed < self.min_final_balance {
            return Err(PlanRejection::InsufficientFinalBalance {
                required: self.min_final_balance,
                guaranteed,
            });
        }
        let leg_pools: BTreeSet<Address20> = self.legs.iter().map(|leg| leg.pool).collect();
        let authorized_pools: BTreeSet<Address20> =
            self.callback_authorization.pools.iter().copied().collect();
        for pool in &self.callback_authorization.pools {
            if !leg_pools.contains(pool) {
                return Err(PlanRejection::UnauthorizedCallback { address: *pool });
            }
        }
        for pool in &leg_pools {
            if !authorized_pools.contains(pool) {
                return Err(PlanRejection::UnauthorizedCallback { address: *pool });
            }
        }
        for leg in &self.legs {
            let has_allowance = self.allowances.iter().any(|allowance| {
                allowance.token == leg.token_in
                    && allowance.spender == leg.pool
                    && allowance.amount >= leg.exact_in
            });
            if !has_allowance {
                return Err(PlanRejection::MissingAllowance {
                    token: leg.token_in,
                    spender: leg.pool,
                });
            }
        }
        Ok(())
    }

    /// Deterministic ABI-style encoding: every field as a 32-byte big-endian
    /// word (addresses left-padded), dynamic arrays as a length word followed
    /// by their elements, in struct field order. Never serde JSON.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&word_u64(self.chain_id));
        bytes.extend_from_slice(&word_address(self.spending_account.address));
        bytes.extend_from_slice(&word_u256(self.spending_account.principal));
        bytes.extend_from_slice(&word_address(self.starting_asset));
        bytes.extend_from_slice(&word_u64(self.legs.len() as u64));
        for leg in &self.legs {
            bytes.extend_from_slice(&word_address(leg.pool));
            bytes.extend_from_slice(&word_address(leg.token_in));
            bytes.extend_from_slice(&word_address(leg.token_out));
            bytes.extend_from_slice(&word_u64(u64::from(leg.fee_tier)));
            bytes.extend_from_slice(&word_u256(leg.exact_in));
            bytes.extend_from_slice(&word_u256(leg.min_out));
        }
        bytes.extend_from_slice(&word_u64(self.allowances.len() as u64));
        for allowance in &self.allowances {
            bytes.extend_from_slice(&word_address(allowance.token));
            bytes.extend_from_slice(&word_address(allowance.spender));
            bytes.extend_from_slice(&word_u256(allowance.amount));
        }
        bytes.extend_from_slice(&word_u64(self.deadline_unix));
        bytes.extend_from_slice(&word_u256(self.min_final_balance));
        bytes.extend_from_slice(&word_u64(self.callback_authorization.pools.len() as u64));
        for pool in &self.callback_authorization.pools {
            bytes.extend_from_slice(&word_address(*pool));
        }
        bytes
    }

    /// `sha256:` plus the hex-encoded SHA-256 digest of [`Self::canonical_bytes`].
    pub fn digest(&self) -> PlanDigest {
        let hash = Sha256::digest(self.canonical_bytes());
        PlanDigest(format!("sha256:{}", hex::encode(hash)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address20 {
        Address20::repeat_byte(byte)
    }

    #[test]
    fn allowlist_rejects_addresses_outside_its_sets() {
        let allowlist = PoolAllowlist::new([addr(1)], [addr(2)]);
        assert!(allowlist.allows_pool(addr(1)));
        assert!(!allowlist.allows_pool(addr(9)));
        assert!(allowlist.allows_token(addr(2)));
        assert!(!allowlist.allows_token(addr(9)));
    }

    #[test]
    fn word_helpers_left_pad_addresses_and_big_endian_integers() {
        assert_eq!(word_u64(1)[31], 1);
        assert_eq!(&word_u64(1)[..24], &[0_u8; 24]);
        let a = addr(0xAB);
        let w = word_address(a);
        assert_eq!(&w[..12], &[0_u8; 12]);
        assert_eq!(&w[12..], a.as_bytes());
        let expected = U256::from(300).to_big_endian();
        assert_eq!(word_u256(U256::from(300)), expected);
    }
}
