//! Tests for `arb_evm::simulation`: the manifest contract and the
//! exact-plan evidence gate `bind()`. One test per fixture in
//! `tests/fixtures/simulation/`, plus targeted tests for the mutation,
//! malformed-input and serde-boundary behavior that a single fixture per
//! branch cannot exercise on its own.
//!
//! The `BasePlan` used throughout is parsed from the same committed fixture
//! `plan_parity.rs` already checks byte-for-byte against Solidity
//! (`contracts/base-guard/test/fixtures/plan-digest.json`), so `plan.digest()`
//! is a known, reviewed value (`expected_digest` in that file). This test
//! binary duplicates the minimal JSON-to-`BasePlan` parsing `plan_parity.rs`
//! uses rather than sharing code across test binaries (Rust integration
//! tests cannot import each other).
use arb_evm::plan::{Allowance, BasePlan, CallbackAuthorization, SpendingAccount, SwapLeg};
use arb_evm::simulation::{
    ArtifactIdentity, EvidenceGap, OverrideKind, SimulationManifest, SimulationOutcome,
    StateIdentity, StateKind, StateOverride, bind,
};
use primitive_types::{H160 as Address20, U256};
use serde_json::Value;

const PLAN_FIXTURE: &str =
    include_str!("../../../contracts/base-guard/test/fixtures/plan-digest.json");

const FIXTURE_REAL_FUNDING: &str =
    include_str!("fixtures/simulation/executed-fork-real-funding.json");
const FIXTURE_SYNTHETIC_FUNDING: &str =
    include_str!("fixtures/simulation/executed-fork-synthetic-funding.json");
const FIXTURE_REVERTED: &str = include_str!("fixtures/simulation/reverted-fork.json");
const FIXTURE_FIXTURE_STATE: &str = include_str!("fixtures/simulation/executed-fixture-state.json");
const FIXTURE_NO_BLOCK_HASH: &str =
    include_str!("fixtures/simulation/executed-fork-no-block-hash.json");
const FIXTURE_DIGEST_MISMATCH: &str =
    include_str!("fixtures/simulation/executed-digest-mismatch.json");

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

/// Mirrors `plan_parity.rs`'s `plan_from_fixture`: parses the committed,
/// reviewed plan fixture into the same `BasePlan` this crate's Solidity
/// parity test already validates.
fn base_plan() -> BasePlan {
    let value: Value = serde_json::from_str(PLAN_FIXTURE).expect("fixture must be valid JSON");
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
        executor: addr(str_field(&value, "executor")),
        spending_account: SpendingAccount {
            address: addr(str_field(spending_account, "address")),
            principal: amount(str_field(spending_account, "principal")),
        },
        starting_asset: addr(str_field(&value, "starting_asset")),
        legs,
        allowances,
        deadline_unix: value["deadline_unix"]
            .as_u64()
            .expect("fixture `deadline_unix` must be an integer"),
        min_final_balance: amount(str_field(&value, "min_final_balance")),
        callback_authorization: CallbackAuthorization { pools },
    }
}

fn manifest(json: &str) -> SimulationManifest {
    serde_json::from_str(json).expect("fixture manifest must deserialize")
}

fn set_final_balance(m: &mut SimulationManifest, value: &str) {
    match &mut m.outcome {
        SimulationOutcome::Executed { final_balance, .. } => *final_balance = value.to_string(),
        SimulationOutcome::Reverted { .. } => panic!("fixture must be Executed"),
    }
}

fn set_emitted_digest(m: &mut SimulationManifest, value: &str) {
    match &mut m.outcome {
        SimulationOutcome::Executed { emitted_digest, .. } => {
            *emitted_digest = value.to_string();
        }
        SimulationOutcome::Reverted { .. } => panic!("fixture must be Executed"),
    }
}

#[test]
fn executed_fork_real_funding_is_passed() {
    let plan = base_plan();
    let m = manifest(FIXTURE_REAL_FUNDING);
    let evidence = bind(&m, &plan).expect("real-funding manifest must bind");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Passed
    );
    assert!(evidence.simulation_matches_exact_plan);
    assert!(!evidence.synthetic_funding);
    assert!(evidence.realized_market_claim_allowed);
    assert!(evidence.reason_codes.is_empty());
    assert_eq!(evidence.plan_digest, plan.digest());
}

#[test]
fn executed_fork_synthetic_funding_is_passed_but_not_realized() {
    let plan = base_plan();
    let m = manifest(FIXTURE_SYNTHETIC_FUNDING);
    let evidence = bind(&m, &plan).expect("synthetic-funding manifest must bind");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Passed
    );
    assert!(evidence.simulation_matches_exact_plan);
    assert!(evidence.synthetic_funding);
    assert!(!evidence.realized_market_claim_allowed);
    assert_eq!(evidence.reason_codes, vec!["SYNTHETIC_FUNDING_OVERRIDE"]);
}

#[test]
fn reverted_fork_is_failed() {
    let plan = base_plan();
    let m = manifest(FIXTURE_REVERTED);
    let evidence = bind(&m, &plan).expect("reverted manifest still binds to evidence");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Failed
    );
    assert!(!evidence.simulation_matches_exact_plan);
    assert_eq!(evidence.reason_codes, vec!["SIMULATION_REVERTED"]);
}

#[test]
fn executed_fixture_state_is_unsupported() {
    let plan = base_plan();
    let m = manifest(FIXTURE_FIXTURE_STATE);
    let evidence = bind(&m, &plan).expect("fixture-state manifest still binds to evidence");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Unsupported
    );
    assert!(!evidence.simulation_matches_exact_plan);
    assert!(!evidence.realized_market_claim_allowed);
    assert_eq!(evidence.reason_codes, vec!["SYNTHETIC_FIXTURE_STATE"]);
}

#[test]
fn executed_fork_no_block_hash_is_unsupported() {
    let plan = base_plan();
    let m = manifest(FIXTURE_NO_BLOCK_HASH);
    let evidence = bind(&m, &plan).expect("no-block-hash manifest still binds to evidence");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Unsupported
    );
    assert_eq!(evidence.reason_codes, vec!["FORK_STATE_UNAVAILABLE"]);
}

#[test]
fn executed_digest_mismatch_is_rejected() {
    let plan = base_plan();
    let m = manifest(FIXTURE_DIGEST_MISMATCH);
    let err = bind(&m, &plan).expect_err("mismatched plan_digest must be rejected");
    assert!(matches!(err, EvidenceGap::DigestMismatch { .. }));
}

type PlanMutation = fn(&mut BasePlan);

#[test]
fn changing_any_plan_field_invalidates_evidence() {
    let m = manifest(FIXTURE_REAL_FUNDING);
    let mutations: &[PlanMutation] = &[
        |p| p.legs[0].exact_in += U256::one(),
        |p| p.legs[1].min_out += U256::one(),
        |p| p.executor = Address20::repeat_byte(0xEE),
        |p| p.legs[0].fee_tier += 1,
        |p| p.min_final_balance += U256::one(),
        |p| p.deadline_unix += 1,
    ];
    for mutate in mutations {
        let mut plan = base_plan();
        mutate(&mut plan);
        let err = bind(&m, &plan).expect_err("mutated plan must invalidate evidence");
        assert!(matches!(err, EvidenceGap::DigestMismatch { .. }));
    }
}

#[test]
fn emitted_digest_must_match_plan_digest() {
    let plan = base_plan();
    let mut m = manifest(FIXTURE_REAL_FUNDING);
    set_emitted_digest(
        &mut m,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    let err = bind(&m, &plan).expect_err("wrong emitted_digest must be rejected");
    assert!(matches!(err, EvidenceGap::DigestMismatch { .. }));
}

#[test]
fn final_balance_below_floor_cannot_pass() {
    let plan = base_plan();
    let mut m = manifest(FIXTURE_REAL_FUNDING);
    // plan.min_final_balance is "1050"; one below the floor.
    set_final_balance(&mut m, "1049");
    let evidence = bind(&m, &plan).expect("below-floor manifest still binds to evidence");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Failed
    );
    assert!(!evidence.simulation_matches_exact_plan);
    assert_eq!(evidence.reason_codes, vec!["FINAL_BALANCE_BELOW_FLOOR"]);
}

#[test]
fn state_chain_mismatch_is_unsupported() {
    let plan = base_plan();
    let mut m = manifest(FIXTURE_REAL_FUNDING);
    m.state.chain_id = 1;
    let evidence = bind(&m, &plan).expect("chain-mismatch manifest still binds to evidence");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Unsupported
    );
    assert_eq!(evidence.reason_codes, vec!["STATE_CHAIN_MISMATCH"]);
}

#[test]
fn malformed_override_is_rejected() {
    let plan = base_plan();
    let base = manifest(FIXTURE_REAL_FUNDING);

    // Allowance without a spender.
    let mut missing_spender = base.clone();
    missing_spender.overrides.push(StateOverride {
        kind: OverrideKind::Allowance,
        account: "0x1111111111111111111111111111111111111111".into(),
        token: Some("0x2222222222222222222222222222222222222222".into()),
        spender: None,
        amount: Some("1000".into()),
    });
    assert!(matches!(
        bind(&missing_spender, &plan),
        Err(EvidenceGap::MalformedField { .. })
    ));

    // Amount that is not a decimal string.
    let mut bad_amount = base;
    bad_amount.overrides.push(StateOverride {
        kind: OverrideKind::NativeBalance,
        account: "0x1111111111111111111111111111111111111111".into(),
        token: None,
        spender: None,
        amount: Some("not-a-number".into()),
    });
    assert!(matches!(
        bind(&bad_amount, &plan),
        Err(EvidenceGap::MalformedField { .. })
    ));
}

#[test]
fn manifest_round_trips_through_serde() {
    let m = manifest(FIXTURE_SYNTHETIC_FUNDING);
    let text = serde_json::to_string(&m).expect("manifest must serialize");
    let round_tripped: SimulationManifest =
        serde_json::from_str(&text).expect("serialized manifest must deserialize");
    assert_eq!(m, round_tripped);
}

#[test]
fn unknown_manifest_field_is_rejected() {
    let mut value: Value = serde_json::from_str(FIXTURE_REAL_FUNDING).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unexpected_field".into(), Value::Bool(true));
    let text = serde_json::to_string(&value).unwrap();
    assert!(serde_json::from_str::<SimulationManifest>(&text).is_err());
}

#[test]
fn unsupported_wins_over_executed_outcome() {
    let plan = base_plan();
    let m = manifest(FIXTURE_FIXTURE_STATE);
    assert!(matches!(m.outcome, SimulationOutcome::Executed { .. }));
    let evidence = bind(&m, &plan).expect("fixture-state manifest still binds to evidence");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Unsupported
    );
}

#[test]
fn manifest_types_are_reachable_directly() {
    // Sanity check on the public surface named in the design: constructing
    // each type directly (not only via fixture JSON) must compile.
    let state = StateIdentity {
        kind: StateKind::PinnedFork,
        chain_id: 8453,
        block_number: 1,
        block_hash: Some(format!("0x{}", "a".repeat(64))),
    };
    let artifact = ArtifactIdentity {
        guard_code_hash: format!("0x{}", "b".repeat(64)),
        toolchain: "foundry 1.8.3 / solc 0.8.28".into(),
    };
    assert_eq!(state.kind, StateKind::PinnedFork);
    assert_eq!(artifact.toolchain, "foundry 1.8.3 / solc 0.8.28");
}
