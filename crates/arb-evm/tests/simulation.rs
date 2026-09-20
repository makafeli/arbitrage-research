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
    EvidenceGap, OverrideKind, SimulationManifest, SimulationOutcome, StateKind, StateOverride,
    bind,
};
use primitive_types::{H160 as Address20, U256};
use serde_json::Value;

const TEST_TOKEN: &str = "0x2222222222222222222222222222222222222222";
const TEST_SPENDER: &str = "0x7777777777777777777777777777777777777777";
const TEST_AMOUNT: &str = "1000";
const TEST_ACCOUNT: &str = "0x1111111111111111111111111111111111111111";

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
    assert_eq!(evidence.state, m.state);
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
    assert_eq!(evidence.state, m.state);
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
    assert!(!evidence.realized_market_claim_allowed);
    assert_eq!(evidence.reason_codes, vec!["SIMULATION_REVERTED"]);
    assert_eq!(evidence.state, m.state);
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
    assert_eq!(evidence.state, m.state);
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
    assert!(!evidence.realized_market_claim_allowed);
    assert_eq!(evidence.reason_codes, vec!["FORK_STATE_UNAVAILABLE"]);
    assert_eq!(evidence.state, m.state);
}

#[test]
fn executed_digest_mismatch_is_rejected() {
    let plan = base_plan();
    let m = manifest(FIXTURE_DIGEST_MISMATCH);
    let err = bind(&m, &plan).expect_err("mismatched plan_digest must be rejected");
    match err {
        EvidenceGap::DigestMismatch { expected, found } => {
            assert_eq!(expected, plan.digest());
            assert_eq!(
                found,
                "sha256:038bab648afbf7a33f35f4453f6347c6b6800ceaa6295cce1ba48c210a646c8a"
            );
        }
        other => panic!("expected DigestMismatch, got {other:?}"),
    }
}

#[test]
fn reverted_manifest_for_another_plan_is_rejected() {
    let plan = base_plan();
    let mut m = manifest(FIXTURE_REVERTED);
    m.plan_digest =
        "sha256:038bab648afbf7a33f35f4453f6347c6b6800ceaa6295cce1ba48c210a646c8a".into();
    let err =
        bind(&m, &plan).expect_err("wrong plan_digest must be rejected even for a reverted run");
    assert!(matches!(err, EvidenceGap::DigestMismatch { .. }));
}

#[test]
fn schema_version_mismatch_is_rejected() {
    let plan = base_plan();
    let mut m = manifest(FIXTURE_REAL_FUNDING);
    m.schema_version = 2;
    let err = bind(&m, &plan).expect_err("schema version mismatch must be rejected");
    assert_eq!(err, EvidenceGap::SchemaVersion { found: 2 });
}

#[test]
fn malformed_block_hash_is_rejected() {
    let plan = base_plan();
    for bad in [
        format!("0x{}", "c".repeat(63)),
        format!("0X{}", "c".repeat(64)),
        format!("0x{}", "z".repeat(64)),
    ] {
        let mut m = manifest(FIXTURE_REAL_FUNDING);
        m.state.block_hash = Some(bad);
        let err = bind(&m, &plan).expect_err("malformed block_hash must be rejected");
        assert_eq!(
            err,
            EvidenceGap::MalformedField {
                field: "state.block_hash"
            }
        );
    }
}

#[test]
fn malformed_shapes_are_rejected() {
    let plan = base_plan();

    let mut bad_digest_len = manifest(FIXTURE_REAL_FUNDING);
    bad_digest_len.plan_digest = format!("sha256:{}", "b".repeat(63));
    assert_eq!(
        bind(&bad_digest_len, &plan).unwrap_err(),
        EvidenceGap::MalformedField {
            field: "plan_digest"
        }
    );

    let mut bad_digest_prefix = manifest(FIXTURE_REAL_FUNDING);
    bad_digest_prefix.plan_digest = "b".repeat(64);
    assert_eq!(
        bind(&bad_digest_prefix, &plan).unwrap_err(),
        EvidenceGap::MalformedField {
            field: "plan_digest"
        }
    );

    let mut bad_artifact = manifest(FIXTURE_REAL_FUNDING);
    bad_artifact.artifact.guard_code_hash = format!("0x{}", "a".repeat(63));
    assert_eq!(
        bind(&bad_artifact, &plan).unwrap_err(),
        EvidenceGap::MalformedField {
            field: "artifact.guard_code_hash"
        }
    );

    let mut bad_account = manifest(FIXTURE_REAL_FUNDING);
    bad_account.overrides[0].account = format!("0x{}", "7".repeat(39));
    assert_eq!(
        bind(&bad_account, &plan).unwrap_err(),
        EvidenceGap::MalformedField {
            field: "override.account"
        }
    );

    let mut bad_token = manifest(FIXTURE_SYNTHETIC_FUNDING);
    bad_token.overrides[1].token = Some(format!("0x{}", "2".repeat(39)));
    assert_eq!(
        bind(&bad_token, &plan).unwrap_err(),
        EvidenceGap::MalformedField {
            field: "override.token"
        }
    );

    let mut bad_spender = manifest(FIXTURE_SYNTHETIC_FUNDING);
    bad_spender.overrides[2].spender = Some(format!("0x{}", "7".repeat(39)));
    assert_eq!(
        bind(&bad_spender, &plan).unwrap_err(),
        EvidenceGap::MalformedField {
            field: "override.spender"
        }
    );
}

fn override_with(
    kind: OverrideKind,
    token: Option<&str>,
    spender: Option<&str>,
    amount: Option<&str>,
) -> StateOverride {
    StateOverride {
        kind,
        account: TEST_ACCOUNT.into(),
        token: token.map(String::from),
        spender: spender.map(String::from),
        amount: amount.map(String::from),
    }
}

type OverrideFieldCase = (
    OverrideKind,
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
    &'static str,
);

#[test]
fn override_field_rules_are_exact() {
    let plan = base_plan();
    let base = manifest(FIXTURE_REAL_FUNDING);

    let cases: &[OverrideFieldCase] = &[
        // NativeBalance: amount required; token/spender must be absent.
        (
            OverrideKind::NativeBalance,
            None,
            None,
            None,
            "override.amount",
        ),
        (
            OverrideKind::NativeBalance,
            Some(TEST_TOKEN),
            None,
            Some(TEST_AMOUNT),
            "override.token",
        ),
        (
            OverrideKind::NativeBalance,
            None,
            Some(TEST_SPENDER),
            Some(TEST_AMOUNT),
            "override.spender",
        ),
        // TokenBalance: token+amount required; spender must be absent.
        (
            OverrideKind::TokenBalance,
            None,
            None,
            Some(TEST_AMOUNT),
            "override.token",
        ),
        (
            OverrideKind::TokenBalance,
            Some(TEST_TOKEN),
            None,
            None,
            "override.amount",
        ),
        (
            OverrideKind::TokenBalance,
            Some(TEST_TOKEN),
            Some(TEST_SPENDER),
            Some(TEST_AMOUNT),
            "override.spender",
        ),
        // Allowance: token+spender+amount all required.
        (
            OverrideKind::Allowance,
            None,
            Some(TEST_SPENDER),
            Some(TEST_AMOUNT),
            "override.token",
        ),
        (
            OverrideKind::Allowance,
            Some(TEST_TOKEN),
            None,
            Some(TEST_AMOUNT),
            "override.spender",
        ),
        (
            OverrideKind::Allowance,
            Some(TEST_TOKEN),
            Some(TEST_SPENDER),
            None,
            "override.amount",
        ),
        // Code: token/spender/amount must all be absent.
        (
            OverrideKind::Code,
            Some(TEST_TOKEN),
            None,
            None,
            "override.token",
        ),
        (
            OverrideKind::Code,
            None,
            Some(TEST_SPENDER),
            None,
            "override.spender",
        ),
        (
            OverrideKind::Code,
            None,
            None,
            Some(TEST_AMOUNT),
            "override.amount",
        ),
    ];

    for (kind, token, spender, amount, expected_field) in cases.iter().copied() {
        let mut m = base.clone();
        m.overrides
            .push(override_with(kind, token, spender, amount));
        let err = bind(&m, &plan).expect_err("bad override shape must be rejected");
        assert_eq!(
            err,
            EvidenceGap::MalformedField {
                field: expected_field
            },
            "case: {kind:?} token={token:?} spender={spender:?} amount={amount:?}"
        );
    }
}

#[test]
fn empty_or_non_decimal_amount_is_rejected() {
    let plan = base_plan();
    let base = manifest(FIXTURE_REAL_FUNDING);

    for bad in ["", "+1", " 1", "0x10", &"9".repeat(79)] {
        let mut m = base.clone();
        m.overrides.push(StateOverride {
            kind: OverrideKind::NativeBalance,
            account: TEST_ACCOUNT.into(),
            token: None,
            spender: None,
            amount: Some(bad.into()),
        });
        let err = bind(&m, &plan).expect_err("non-decimal amount must be rejected");
        assert_eq!(
            err,
            EvidenceGap::MalformedField {
                field: "override.amount"
            }
        );
    }

    for bad in ["", "abc"] {
        let mut m = base.clone();
        set_final_balance(&mut m, bad);
        let err = bind(&m, &plan).expect_err("non-decimal final_balance must be rejected");
        assert_eq!(
            err,
            EvidenceGap::MalformedField {
                field: "outcome.final_balance"
            }
        );
    }
}

#[test]
fn each_funding_kind_alone_is_synthetic() {
    let plan = base_plan();
    let base = manifest(FIXTURE_REAL_FUNDING);

    let cases = [
        StateOverride {
            kind: OverrideKind::NativeBalance,
            account: TEST_ACCOUNT.into(),
            token: None,
            spender: None,
            amount: Some(TEST_AMOUNT.into()),
        },
        StateOverride {
            kind: OverrideKind::TokenBalance,
            account: TEST_ACCOUNT.into(),
            token: Some(TEST_TOKEN.into()),
            spender: None,
            amount: Some(TEST_AMOUNT.into()),
        },
        StateOverride {
            kind: OverrideKind::Allowance,
            account: TEST_ACCOUNT.into(),
            token: Some(TEST_TOKEN.into()),
            spender: Some(TEST_SPENDER.into()),
            amount: Some(TEST_AMOUNT.into()),
        },
    ];

    for extra in cases {
        let mut m = base.clone();
        m.overrides.push(extra);
        let evidence = bind(&m, &plan).expect("well-formed synthetic override still binds");
        assert_eq!(
            evidence.simulation_status,
            arb_domain::SimulationStatus::Passed
        );
        assert!(evidence.synthetic_funding);
        assert!(!evidence.realized_market_claim_allowed);
        assert_eq!(evidence.reason_codes, vec!["SYNTHETIC_FUNDING_OVERRIDE"]);
    }
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
    assert!(!evidence.realized_market_claim_allowed);
    assert_eq!(evidence.reason_codes, vec!["FINAL_BALANCE_BELOW_FLOOR"]);
    assert_eq!(evidence.state, m.state);
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
    assert!(!evidence.realized_market_claim_allowed);
    assert_eq!(evidence.reason_codes, vec!["STATE_CHAIN_MISMATCH"]);
    assert_eq!(evidence.state, m.state);
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
    fn assert_rejected(mutate: impl FnOnce(&mut Value)) {
        let mut value: Value = serde_json::from_str(FIXTURE_REAL_FUNDING).unwrap();
        mutate(&mut value);
        let text = serde_json::to_string(&value).unwrap();
        assert!(serde_json::from_str::<SimulationManifest>(&text).is_err());
    }

    assert_rejected(|v| {
        v.as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), Value::Bool(true));
    });
    assert_rejected(|v| {
        v["state"]
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), Value::Bool(true));
    });
    assert_rejected(|v| {
        v["artifact"]
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), Value::Bool(true));
    });
    assert_rejected(|v| {
        v["overrides"][0]
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), Value::Bool(true));
    });
    assert_rejected(|v| {
        v["outcome"]
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".into(), Value::Bool(true));
    });
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
fn unsupported_wins_over_reverted_outcome() {
    let plan = base_plan();

    let mut fixture_state = manifest(FIXTURE_REVERTED);
    fixture_state.state.kind = StateKind::SyntheticFixture;
    let evidence =
        bind(&fixture_state, &plan).expect("reverted+fixture-state manifest still binds");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Unsupported
    );
    assert_eq!(evidence.reason_codes, vec!["SYNTHETIC_FIXTURE_STATE"]);

    let mut no_block_hash = manifest(FIXTURE_REVERTED);
    no_block_hash.state.block_hash = None;
    let evidence =
        bind(&no_block_hash, &plan).expect("reverted+no-block-hash manifest still binds");
    assert_eq!(
        evidence.simulation_status,
        arb_domain::SimulationStatus::Unsupported
    );
    assert_eq!(evidence.reason_codes, vec!["FORK_STATE_UNAVAILABLE"]);
}
