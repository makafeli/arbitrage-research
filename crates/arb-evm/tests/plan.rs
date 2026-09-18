//! Research-only Base transaction plan: validation and canonical digest fixtures.
//! Every address here is synthetic (`Address20::repeat_byte(n)`), not a deployed
//! contract or observed market identity.
use arb_evm::plan::{
    Allowance, BasePlan, CallbackAuthorization, PlanRejection, PoolAllowlist, SpendingAccount,
    SwapLeg,
};
use primitive_types::{H160 as Address20, U256};

const USDC: u8 = 0x01;
const WETH: u8 = 0x02;
const POOL_A: u8 = 0x10;
const POOL_B: u8 = 0x11;
const SPENDER: u8 = 0x99;

fn addr(byte: u8) -> Address20 {
    Address20::repeat_byte(byte)
}

fn allowlist() -> PoolAllowlist {
    PoolAllowlist::new([addr(POOL_A), addr(POOL_B)], [addr(USDC), addr(WETH)])
}

/// A valid two-leg cycle: USDC -[pool A]-> WETH -[pool B]-> USDC.
/// principal(1000) - exact_in0(1000) + min_out1(1500) = guaranteed final balance 1500.
fn valid_plan() -> BasePlan {
    BasePlan {
        chain_id: 8453,
        spending_account: SpendingAccount {
            address: addr(SPENDER),
            principal: U256::from(1000),
        },
        starting_asset: addr(USDC),
        legs: vec![
            SwapLeg {
                pool: addr(POOL_A),
                token_in: addr(USDC),
                token_out: addr(WETH),
                fee_tier: 500,
                exact_in: U256::from(1000),
                min_out: U256::from(2000),
            },
            SwapLeg {
                pool: addr(POOL_B),
                token_in: addr(WETH),
                token_out: addr(USDC),
                fee_tier: 3000,
                exact_in: U256::from(2000),
                min_out: U256::from(1500),
            },
        ],
        allowances: vec![
            Allowance {
                token: addr(USDC),
                spender: addr(POOL_A),
                amount: U256::from(1000),
            },
            Allowance {
                token: addr(WETH),
                spender: addr(POOL_B),
                amount: U256::from(2000),
            },
        ],
        deadline_unix: 9_999_999_999,
        min_final_balance: U256::from(1500),
        callback_authorization: CallbackAuthorization {
            pools: vec![addr(POOL_A), addr(POOL_B)],
        },
    }
}

#[test]
fn valid_two_leg_cycle_validates_and_digest_is_stable_and_sensitive() {
    let plan = valid_plan();
    assert_eq!(plan.validate(&allowlist()), Ok(()));

    let same_plan = valid_plan();
    assert_eq!(plan.digest(), same_plan.digest());

    let mut changed_min_out = valid_plan();
    changed_min_out.legs[1].min_out = U256::from(1600);
    assert_ne!(plan.digest(), changed_min_out.digest());

    let mut swapped_legs = valid_plan();
    swapped_legs.legs.swap(0, 1);
    assert_ne!(plan.digest(), swapped_legs.digest());
}

#[test]
fn pool_not_in_allowlist_is_rejected() {
    let restricted = PoolAllowlist::new([addr(POOL_A)], [addr(USDC), addr(WETH)]);
    assert_eq!(
        valid_plan().validate(&restricted),
        Err(PlanRejection::UnsupportedTarget {
            address: addr(POOL_B)
        })
    );
}

#[test]
fn token_not_in_allowlist_is_rejected() {
    let restricted = PoolAllowlist::new([addr(POOL_A), addr(POOL_B)], [addr(USDC)]);
    assert_eq!(
        valid_plan().validate(&restricted),
        Err(PlanRejection::UnsupportedTarget {
            address: addr(WETH)
        })
    );
}

#[test]
fn non_cyclic_route_is_rejected() {
    const OTHER: u8 = 0x03;
    let allowlist = PoolAllowlist::new(
        [addr(POOL_A), addr(POOL_B)],
        [addr(USDC), addr(WETH), addr(OTHER)],
    );
    let mut plan = valid_plan();
    plan.legs[1].token_out = addr(OTHER);
    assert_eq!(
        plan.validate(&allowlist),
        Err(PlanRejection::RouteNotCyclic)
    );
}

#[test]
fn broken_leg_chain_is_rejected() {
    const OTHER: u8 = 0x04;
    let allowlist = PoolAllowlist::new(
        [addr(POOL_A), addr(POOL_B)],
        [addr(USDC), addr(WETH), addr(OTHER)],
    );
    let mut plan = valid_plan();
    plan.legs[1].token_in = addr(OTHER);
    assert_eq!(
        plan.validate(&allowlist),
        Err(PlanRejection::LegDiscontinuity { index: 1 })
    );
}

#[test]
fn insufficient_final_balance_is_rejected_with_amounts() {
    let mut plan = valid_plan();
    plan.min_final_balance = U256::from(2000);
    assert_eq!(
        plan.validate(&allowlist()),
        Err(PlanRejection::InsufficientFinalBalance {
            required: U256::from(2000),
            guaranteed: U256::from(1500),
        })
    );
}

#[test]
fn callback_authorization_with_non_leg_address_is_rejected() {
    const OUTSIDER: u8 = 0x12;
    let mut plan = valid_plan();
    plan.callback_authorization.pools.push(addr(OUTSIDER));
    assert_eq!(
        plan.validate(&allowlist()),
        Err(PlanRejection::UnauthorizedCallback {
            address: addr(OUTSIDER)
        })
    );
}

#[test]
fn missing_allowance_is_rejected() {
    let mut plan = valid_plan();
    plan.allowances.remove(1);
    assert_eq!(
        plan.validate(&allowlist()),
        Err(PlanRejection::MissingAllowance {
            token: addr(WETH),
            spender: addr(POOL_B),
        })
    );
}

#[test]
fn zero_principal_is_fee_account_only_even_with_allowances() {
    let mut plan = valid_plan();
    plan.spending_account.principal = U256::zero();
    assert_eq!(
        plan.validate(&allowlist()),
        Err(PlanRejection::FeeAccountOnly)
    );
}

#[test]
fn canonical_bytes_length_matches_expected_word_count() {
    let plan = valid_plan();
    // chain_id, spending address, principal, starting_asset = 4 words.
    // legs: 1 length word + 6 words per leg.
    // allowances: 1 length word + 3 words per allowance.
    // deadline_unix, min_final_balance = 2 words.
    // callback_authorization.pools: 1 length word + 1 word per pool.
    let expected_words = 4
        + (1 + 6 * plan.legs.len())
        + (1 + 3 * plan.allowances.len())
        + 2
        + (1 + plan.callback_authorization.pools.len());
    assert_eq!(plan.canonical_bytes().len(), expected_words * 32);
}
