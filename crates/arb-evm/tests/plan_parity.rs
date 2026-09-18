//! Byte-for-byte parity between `BasePlan::canonical_bytes` (Rust) and
//! `PlanEncoding.canonicalBytes` (Solidity, `contracts/base-guard/src/PlanEncoding.sol`).
//! Both sides parse the same literal, committed fixtures and derive their own
//! digest from them; this test only checks the Rust side against each
//! fixture's `expected_digest`. `contracts/base-guard/test/PlanEncoding.t.sol`
//! performs the matching Solidity-side check against the same files, so
//! neither encoding can silently drift from the other without one of these
//! tests failing.
//!
//! Two fixtures, not one: `plan-digest.json` ("regular") happens to have
//! `principal == legs[0].exact_in`, one allowance per leg and
//! `callback_authorization.pools` equal to the leg pools in leg order — so it
//! cannot by itself catch a bug that silently swaps `principal` for
//! `legs[0].exact_in`, or that encodes callback pools in the wrong order or
//! count. `plan-digest-irregular.json` deliberately breaks every one of
//! those coincidences (zero principal, one allowance across three legs, a
//! reordered and partial callback pool set).
use arb_evm::plan::{Allowance, BasePlan, CallbackAuthorization, SpendingAccount, SwapLeg};
use primitive_types::{H160 as Address20, U256};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../contracts/base-guard/test/fixtures/plan-digest.json");
const FIXTURE_IRREGULAR: &str =
    include_str!("../../../contracts/base-guard/test/fixtures/plan-digest-irregular.json");

fn addr(s: &str) -> Address20 {
    let bytes = hex::decode(s.trim_start_matches("0x")).expect("valid hex address");
    Address20::from_slice(&bytes)
}

fn amount(s: &str) -> U256 {
    U256::from_dec_str(s).expect("valid decimal amount")
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("fixture field `{key}` must be a string"))
}

fn plan_from_fixture(value: &Value) -> BasePlan {
    let spending_account = &value["spending_account"];
    let legs = value["legs"]
        .as_array()
        .expect("fixture `legs` must be an array")
        .iter()
        .map(|leg| SwapLeg {
            pool: addr(str_field(leg, "pool")),
            token_in: addr(str_field(leg, "token_in")),
            token_out: addr(str_field(leg, "token_out")),
            fee_tier: leg["fee_tier"]
                .as_u64()
                .expect("leg `fee_tier` must be an integer") as u32,
            exact_in: amount(str_field(leg, "exact_in")),
            min_out: amount(str_field(leg, "min_out")),
        })
        .collect();
    let allowances = value["allowances"]
        .as_array()
        .expect("fixture `allowances` must be an array")
        .iter()
        .map(|allowance| Allowance {
            token: addr(str_field(allowance, "token")),
            spender: addr(str_field(allowance, "spender")),
            amount: amount(str_field(allowance, "amount")),
        })
        .collect();
    let pools = value["callback_authorization"]["pools"]
        .as_array()
        .expect("fixture `callback_authorization.pools` must be an array")
        .iter()
        .map(|pool| addr(pool.as_str().expect("pool address must be a string")))
        .collect();
    BasePlan {
        chain_id: value["chain_id"]
            .as_u64()
            .expect("fixture `chain_id` must be an integer"),
        spending_account: SpendingAccount {
            address: addr(str_field(spending_account, "address")),
            principal: amount(str_field(spending_account, "principal")),
        },
        starting_asset: addr(str_field(value, "starting_asset")),
        legs,
        allowances,
        deadline_unix: value["deadline_unix"]
            .as_u64()
            .expect("fixture `deadline_unix` must be an integer"),
        min_final_balance: amount(str_field(value, "min_final_balance")),
        callback_authorization: CallbackAuthorization { pools },
    }
}

fn assert_fixture_digest_matches(fixture: &str) {
    let value: Value = serde_json::from_str(fixture).expect("fixture must be valid JSON");
    let plan = plan_from_fixture(&value);

    let expected_digest = str_field(&value, "expected_digest").to_string();
    assert_eq!(
        plan.digest().0,
        expected_digest,
        "Rust BasePlan::digest() must match the committed fixture digest"
    );
}

#[test]
fn fixture_digest_matches_expected() {
    assert_fixture_digest_matches(FIXTURE);
}

#[test]
fn irregular_fixture_digest_matches_expected() {
    assert_fixture_digest_matches(FIXTURE_IRREGULAR);
}
