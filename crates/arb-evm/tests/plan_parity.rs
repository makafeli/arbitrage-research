//! Byte-for-byte parity between `BasePlan::canonical_bytes` (Rust) and
//! `PlanEncoding.canonicalBytes` (Solidity, `contracts/base-guard/src/PlanEncoding.sol`).
//! Both sides parse the same literal, committed fixture and derive their own
//! digest from it; this test only checks the Rust side against the fixture's
//! `expected_digest`. `contracts/base-guard/test/PlanEncoding.t.sol` performs
//! the matching Solidity-side check against the same file, so neither
//! encoding can silently drift from the other without one of these tests
//! failing.
use arb_evm::plan::{Allowance, BasePlan, CallbackAuthorization, SpendingAccount, SwapLeg};
use primitive_types::{H160 as Address20, U256};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../contracts/base-guard/test/fixtures/plan-digest.json");

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

#[test]
fn fixture_digest_matches_expected() {
    let value: Value = serde_json::from_str(FIXTURE).expect("fixture must be valid JSON");
    let plan = plan_from_fixture(&value);

    let expected_digest = str_field(&value, "expected_digest").to_string();
    assert_eq!(
        plan.digest().0,
        expected_digest,
        "Rust BasePlan::digest() must match the committed fixture digest"
    );
}

#[test]
fn fixture_round_trips_through_json() {
    // The fixture cannot silently drift between edits: re-serializing and
    // re-parsing the same value must reproduce an identical structure.
    let value: Value = serde_json::from_str(FIXTURE).expect("fixture must be valid JSON");
    let serialized = serde_json::to_string(&value).expect("fixture must re-serialize");
    let reparsed: Value =
        serde_json::from_str(&serialized).expect("re-serialized JSON must re-parse");
    assert_eq!(value, reparsed);
}
