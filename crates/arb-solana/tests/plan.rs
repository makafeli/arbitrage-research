//! Fixtures and rejection cases for the research-only Solana transaction plan.

use arb_solana::plan::{
    COMPUTE_BUDGET_PROGRAM, ComputeBudget, FinalBalanceGuard, PlanRejection, Pubkey32, SolanaPlan,
    WhirlpoolSwapLeg, swap_discriminator,
};
use arb_solana::{TOKEN_PROGRAM, WHIRLPOOL_PROGRAM};
use sha2::{Digest, Sha256};

fn key(byte: u8) -> Pubkey32 {
    Pubkey32::from_bytes([byte; 32])
}

fn valid_leg0() -> WhirlpoolSwapLeg {
    WhirlpoolSwapLeg {
        whirlpool: key(7),
        token_authority: key(2),
        // a_to_b: true, so this is the leg's input account — must equal the
        // plan's starting_token_account (key(4)).
        token_owner_account_a: key(4),
        token_vault_a: key(11),
        token_owner_account_b: key(12),
        token_vault_b: key(13),
        tick_arrays: [key(14), key(15), key(16)],
        oracle: key(17),
        amount: 500,
        other_amount_threshold: 480,
        sqrt_price_limit: 1,
        amount_specified_is_input: true,
        a_to_b: true,
    }
}

fn valid_leg1() -> WhirlpoolSwapLeg {
    WhirlpoolSwapLeg {
        whirlpool: key(8),
        token_authority: key(2),
        // a_to_b: false, so this is the leg's output account — must equal
        // the plan's starting_token_account (key(4)), closing the cycle.
        token_owner_account_a: key(4),
        token_vault_a: key(19),
        token_owner_account_b: key(20),
        token_vault_b: key(21),
        tick_arrays: [key(22), key(23), key(24)],
        oracle: key(25),
        amount: 480,
        other_amount_threshold: 1000,
        sqrt_price_limit: u128::MAX,
        amount_specified_is_input: true,
        a_to_b: false,
    }
}

fn valid_plan() -> SolanaPlan {
    SolanaPlan {
        payer: key(1),
        authority: key(2),
        starting_mint: key(3),
        starting_token_account: key(4),
        token_program: Pubkey32::from_base58(TOKEN_PROGRAM).unwrap(),
        compute_budget: ComputeBudget {
            units: 200_000,
            micro_lamports_per_unit: 1,
        },
        legs: [valid_leg0(), valid_leg1()],
        guard: FinalBalanceGuard {
            token_account: key(4),
            owner: key(2),
            min_balance: 900,
        },
        recent_blockhash_placeholder: [0; 32],
    }
}

/// (input, output) mint pair per leg. Leg 0: starting_mint(3) -> intermediate(6).
/// Leg 1: intermediate(6) -> starting_mint(3), closing the cycle.
fn valid_leg_mints() -> [(Pubkey32, Pubkey32); 2] {
    [(key(3), key(6)), (key(6), key(3))]
}

fn valid_allowlist() -> arb_solana::plan::ProgramAllowlist {
    arb_solana::plan::ProgramAllowlist {
        programs: vec![
            Pubkey32::from_base58(COMPUTE_BUDGET_PROGRAM).unwrap(),
            Pubkey32::from_base58(WHIRLPOOL_PROGRAM).unwrap(),
            Pubkey32::from_base58(TOKEN_PROGRAM).unwrap(),
        ],
        whirlpools: vec![key(7), key(8)],
        mints: vec![key(3), key(6)],
    }
}

#[test]
fn valid_plan_has_five_instructions_and_a_stable_changing_digest() {
    let plan = valid_plan();
    let instructions = plan.instructions();
    assert_eq!(instructions.len(), 5);

    // Compute budget prefix, in order.
    assert_eq!(
        instructions[0].program_id,
        Pubkey32::from_base58(COMPUTE_BUDGET_PROGRAM).unwrap()
    );
    assert_eq!(instructions[0].data[0], 2);
    assert_eq!(
        instructions[1].program_id,
        Pubkey32::from_base58(COMPUTE_BUDGET_PROGRAM).unwrap()
    );
    assert_eq!(instructions[1].data[0], 3);

    // Both swap legs use the Whirlpool program and the independently
    // computed Anchor `global:swap` discriminator.
    let expected_discriminator: [u8; 8] = Sha256::digest(b"global:swap")[..8].try_into().unwrap();
    assert_eq!(swap_discriminator(), expected_discriminator);
    for swap_ix in &instructions[2..4] {
        assert_eq!(
            swap_ix.program_id,
            Pubkey32::from_base58(WHIRLPOOL_PROGRAM).unwrap()
        );
        assert_eq!(&swap_ix.data[..8], &expected_discriminator);
    }

    // Guard instruction last, running under the plan's token program.
    assert_eq!(
        instructions[4].program_id,
        Pubkey32::from_base58(TOKEN_PROGRAM).unwrap()
    );

    // Digest is stable across two independently built, identical plans.
    let digest_a = plan.digest();
    let digest_b = valid_plan().digest();
    assert_eq!(digest_a, digest_b);
    assert!(digest_a.0.starts_with("sha256:"));

    // Digest changes when a swap parameter changes.
    let mut changed_threshold = plan;
    changed_threshold.legs[1].other_amount_threshold += 1;
    assert_ne!(plan.digest(), changed_threshold.digest());

    // Digest changes when the compute budget changes.
    let mut changed_units = plan;
    changed_units.compute_budget.units += 1;
    assert_ne!(plan.digest(), changed_units.digest());
}

#[test]
fn rejects_unsupported_program_whirlpool_and_mint() {
    let plan = valid_plan();
    let leg_mints = valid_leg_mints();

    let mut missing_program = valid_allowlist();
    let token_program = Pubkey32::from_base58(TOKEN_PROGRAM).unwrap();
    missing_program.programs.retain(|p| *p != token_program);
    assert_eq!(
        plan.validate(&missing_program, &leg_mints),
        Err(PlanRejection::UnsupportedProgram {
            program: token_program
        })
    );

    let mut missing_whirlpool = valid_allowlist();
    missing_whirlpool.whirlpools.retain(|w| *w != key(7));
    assert_eq!(
        plan.validate(&missing_whirlpool, &leg_mints),
        Err(PlanRejection::UnsupportedWhirlpool { whirlpool: key(7) })
    );

    let mut missing_mint = valid_allowlist();
    missing_mint.mints.retain(|m| *m != key(3));
    assert_eq!(
        plan.validate(&missing_mint, &leg_mints),
        Err(PlanRejection::UnsupportedMint { mint: key(3) })
    );
}

#[test]
fn rejects_authority_and_guard_owner_mismatch() {
    let leg_mints = valid_leg_mints();
    let allowlist = valid_allowlist();

    let mut bad_authority = valid_plan();
    bad_authority.legs[0].token_authority = key(99);
    assert_eq!(
        bad_authority.validate(&allowlist, &leg_mints),
        Err(PlanRejection::AuthorityMismatch)
    );

    let mut bad_guard_owner = valid_plan();
    bad_guard_owner.guard.owner = key(99);
    assert_eq!(
        bad_guard_owner.validate(&allowlist, &leg_mints),
        Err(PlanRejection::GuardOwnerMismatch)
    );
}

#[test]
fn rejects_guard_watching_a_different_account_than_the_starting_token_account() {
    let leg_mints = valid_leg_mints();
    let allowlist = valid_allowlist();

    let mut watches_wrong_account = valid_plan();
    watches_wrong_account.guard.token_account = key(98);
    assert_eq!(
        watches_wrong_account.validate(&allowlist, &leg_mints),
        Err(PlanRejection::GuardAccountMismatch)
    );
}

#[test]
fn rejects_starting_account_not_bound_to_the_legs() {
    let leg_mints = valid_leg_mints();
    let allowlist = valid_allowlist();

    // Leg 0's input account (a_to_b: true -> token_owner_account_a) no
    // longer matches starting_token_account.
    let mut leg0_input_mismatch = valid_plan();
    leg0_input_mismatch.legs[0].token_owner_account_a = key(97);
    assert_eq!(
        leg0_input_mismatch.validate(&allowlist, &leg_mints),
        Err(PlanRejection::StartingAccountMismatch)
    );

    // Leg 1's output account (a_to_b: false -> token_owner_account_a) no
    // longer matches starting_token_account.
    let mut leg1_output_mismatch = valid_plan();
    leg1_output_mismatch.legs[1].token_owner_account_a = key(97);
    assert_eq!(
        leg1_output_mismatch.validate(&allowlist, &leg_mints),
        Err(PlanRejection::StartingAccountMismatch)
    );
}

#[test]
fn rejects_non_cyclic_route() {
    let plan = valid_plan();
    let allowlist = valid_allowlist();
    // Both mints stay allowlisted; only the closing leg's output mint changes,
    // so this exercises RouteNotCyclic specifically rather than UnsupportedMint.
    let non_cyclic = [(key(3), key(6)), (key(6), key(6))];
    assert_eq!(
        plan.validate(&allowlist, &non_cyclic),
        Err(PlanRejection::RouteNotCyclic)
    );

    // Leg 0's input mint no longer matches the plan's starting mint, even
    // though every mint involved stays allowlisted.
    let leg0_input_not_starting = [(key(6), key(6)), (key(6), key(3))];
    assert_eq!(
        plan.validate(&allowlist, &leg0_input_not_starting),
        Err(PlanRejection::RouteNotCyclic)
    );
}

#[test]
fn rejects_broken_middle_of_route() {
    let plan = valid_plan();
    let allowlist = valid_allowlist();
    // Leg 0 starts at the starting mint and leg 1 ends at it, but leg 0's
    // output mint does not match leg 1's input mint: the two legs do not
    // actually compose into one route. All mints stay allowlisted so this
    // isolates RouteNotCyclic from UnsupportedMint.
    let broken_middle = [(key(3), key(6)), (key(3), key(3))];
    assert_eq!(
        plan.validate(&allowlist, &broken_middle),
        Err(PlanRejection::RouteNotCyclic)
    );
}

#[test]
fn rejects_insufficient_final_balance() {
    let allowlist = valid_allowlist();
    let leg_mints = valid_leg_mints();
    let mut plan = valid_plan();
    // leg[1] guarantees other_amount_threshold = 1000 (amount_specified_is_input).
    plan.guard.min_balance = 1_500;
    assert_eq!(
        plan.validate(&allowlist, &leg_mints),
        Err(PlanRejection::InsufficientFinalBalance {
            required: 1_500,
            guaranteed: 1_000,
        })
    );
}

#[test]
fn rejects_compute_budget_out_of_bounds() {
    let allowlist = valid_allowlist();
    let leg_mints = valid_leg_mints();

    let mut zero_units = valid_plan();
    zero_units.compute_budget.units = 0;
    assert_eq!(
        zero_units.validate(&allowlist, &leg_mints),
        Err(PlanRejection::ComputeBudgetOutOfBounds)
    );

    let mut too_many_units = valid_plan();
    too_many_units.compute_budget.units = 1_400_001;
    assert_eq!(
        too_many_units.validate(&allowlist, &leg_mints),
        Err(PlanRejection::ComputeBudgetOutOfBounds)
    );
}

#[test]
fn canonical_bytes_length_matches_hand_computed_sum() {
    let plan = valid_plan();

    const HEADER: usize = 32 + 2; // program_id + u16 account count
    const PER_ACCOUNT: usize = 32 + 1; // pubkey + flags byte
    const DATA_LEN_FIELD: usize = 4; // u32 LE data length

    // SetComputeUnitLimit: 1 discriminator byte + u32 = 5 bytes of data, no accounts.
    let cu_limit_len = HEADER + DATA_LEN_FIELD + 5;
    // SetComputeUnitPrice: 1 discriminator byte + u64 = 9 bytes of data, no accounts.
    let cu_price_len = HEADER + DATA_LEN_FIELD + 9;
    // Whirlpool swap: 11 accounts; data = 8-byte discriminator + u64 + u64 + u128 + bool + bool.
    let swap_data_len = 8 + 8 + 8 + 16 + 1 + 1;
    let swap_len = HEADER + 11 * PER_ACCOUNT + DATA_LEN_FIELD + swap_data_len;
    // Guard: an SPL Token Transfer with 3 accounts (token_account twice, owner);
    // data = 1 discriminator byte + u64 amount.
    let guard_data_len = 1 + 8;
    let guard_len = HEADER + 3 * PER_ACCOUNT + DATA_LEN_FIELD + guard_data_len;

    let expected = cu_limit_len + cu_price_len + 2 * swap_len + guard_len;
    assert_eq!(plan.canonical_bytes().len(), expected);
}

#[test]
fn guard_instruction_is_an_spl_token_self_transfer() {
    let plan = valid_plan();
    let ix = plan.guard.instruction(&plan.token_program);

    assert_eq!(ix.program_id, Pubkey32::from_base58(TOKEN_PROGRAM).unwrap());

    assert_eq!(ix.accounts.len(), 3);
    assert_eq!(ix.accounts[0].pubkey, plan.guard.token_account);
    assert!(!ix.accounts[0].is_signer);
    assert!(ix.accounts[0].is_writable);
    assert_eq!(ix.accounts[1].pubkey, plan.guard.token_account);
    assert!(!ix.accounts[1].is_signer);
    assert!(ix.accounts[1].is_writable);
    assert_eq!(ix.accounts[2].pubkey, plan.guard.owner);
    assert!(ix.accounts[2].is_signer);
    assert!(!ix.accounts[2].is_writable);

    let mut expected_data = vec![3u8];
    expected_data.extend_from_slice(&plan.guard.min_balance.to_le_bytes());
    assert_eq!(ix.data, expected_data);
}
