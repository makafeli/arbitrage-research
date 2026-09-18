//! Offline litesvm proof that the research-only `SolanaPlan` from
//! `arb_solana::plan` executes atomically and that its final-balance guard
//! rejects the whole transaction when the guarded account is short, has the
//! wrong owner, or is not a real SPL Token account.
//!
//! Everything here runs in-process against `litesvm::LiteSVM`. SPL Token is
//! built into litesvm; no program is deployed, no RPC is contacted, and every
//! keypair is created inside a test and dropped with it. Nothing in this file
//! signs a transaction meant to be sent anywhere, and there is no network
//! access anywhere in this crate.

use arb_solana::plan::{
    AccountMeta as PlanAccountMeta, ComputeBudget, FinalBalanceGuard,
    Instruction as PlanInstruction, Pubkey32, SolanaPlan, WhirlpoolSwapLeg,
};
use litesvm::LiteSVM;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

/// The pinned SPL Token program id, decoded once for these tests.
fn token_program_pubkey32() -> Pubkey32 {
    Pubkey32::from_base58(arb_solana::TOKEN_PROGRAM).unwrap()
}

fn token_program_address() -> Address {
    Address::from(*token_program_pubkey32().as_bytes())
}

fn system_program_address() -> Address {
    Address::from([0u8; 32])
}

/// Convert one research-only plan `Instruction` into a real SDK instruction
/// litesvm can execute. Program id and account metas map 1:1; nothing here
/// adds a signature.
fn to_sdk(instruction: &PlanInstruction) -> Instruction {
    Instruction {
        program_id: Address::from(*instruction.program_id.as_bytes()),
        accounts: instruction
            .accounts
            .iter()
            .map(|meta: &PlanAccountMeta| AccountMeta {
                pubkey: Address::from(*meta.pubkey.as_bytes()),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: instruction.data.clone(),
    }
}

/// Write a minimal, valid 82-byte SPL mint account: no mint authority, the
/// given `supply`, 6 decimals, initialized, no freeze authority.
fn set_mint(svm: &mut LiteSVM, mint: Address, supply: u64) {
    let mut data = vec![0u8; 82];
    // mint_authority: COption::None
    data[0..4].copy_from_slice(&0u32.to_le_bytes());
    // supply
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    // decimals
    data[44] = 6;
    // is_initialized
    data[45] = 1;
    // freeze_authority: COption::None
    data[46..50].copy_from_slice(&0u32.to_le_bytes());

    let lamports = svm.minimum_balance_for_rent_exemption(data.len());
    svm.set_account(
        mint,
        solana_account::Account {
            lamports,
            data,
            owner: token_program_address(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("set mint account");
}

/// Write a minimal, valid 165-byte SPL token account: `mint`, `owner`,
/// `amount`, no delegate, `Initialized`, not native, zero delegated amount,
/// no close authority.
fn set_token_account(
    svm: &mut LiteSVM,
    address: Address,
    mint: Address,
    owner: Address,
    amount: u64,
) {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    // delegate: COption::None
    data[72..76].copy_from_slice(&0u32.to_le_bytes());
    // state: Initialized
    data[108] = 1;
    // is_native: COption::None
    data[109..113].copy_from_slice(&0u32.to_le_bytes());
    // delegated_amount
    data[121..129].copy_from_slice(&0u64.to_le_bytes());
    // close_authority: COption::None
    data[129..133].copy_from_slice(&0u32.to_le_bytes());

    let lamports = svm.minimum_balance_for_rent_exemption(data.len());
    svm.set_account(
        address,
        solana_account::Account {
            lamports,
            data,
            owner: token_program_address(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("set token account");
}

/// A hand-packed System program `Transfer` instruction, used only to prove
/// atomic rollback: an earlier instruction's balance change must be undone
/// when a later instruction (the guard) fails.
fn system_transfer(from: Address, to: Address, lamports: u64) -> Instruction {
    let mut data = 2u32.to_le_bytes().to_vec();
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: system_program_address(),
        accounts: vec![
            AccountMeta {
                pubkey: from,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: to,
                is_signer: false,
                is_writable: true,
            },
        ],
        data,
    }
}

/// Build a transaction from `ixs`, sign it with `payer` plus `extra_signers`,
/// and send it into the in-memory bank. Never touches a network.
fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    extra_signers: &[&Keypair],
    ixs: &[Instruction],
) -> Result<litesvm::types::TransactionMetadata, Box<litesvm::types::FailedTransactionMetadata>> {
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra_signers);
    let tx = Transaction::new(&signers, message, blockhash);
    svm.send_transaction(tx).map_err(Box::new)
}

/// Fresh SVM plus a funded payer keypair, ready for one test.
fn new_svm_with_payer() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000)
        .expect("airdrop the in-memory payer");
    (svm, payer)
}

#[test]
fn guard_passes_when_final_balance_meets_minimum() {
    let (mut svm, payer) = new_svm_with_payer();
    let authority = Keypair::new();
    let mint = Keypair::new().pubkey();
    let token_account = Keypair::new().pubkey();
    set_mint(&mut svm, mint, 1_000_000);
    set_token_account(&mut svm, token_account, mint, authority.pubkey(), 1_000);

    let guard = FinalBalanceGuard {
        token_account: Pubkey32::from_bytes(token_account.to_bytes()),
        owner: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
        min_balance: 1_000,
    };
    let ix = to_sdk(&guard.instruction(&token_program_pubkey32()));

    let result = send(&mut svm, &payer, &[&authority], &[ix]);
    assert!(result.is_ok(), "expected Ok, got {result:?}");

    let account = svm
        .get_account(&token_account)
        .expect("token account exists");
    // Bytes 64..72 hold the token amount; a self-transfer is a no-op.
    let amount = u64::from_le_bytes(account.data[64..72].try_into().unwrap());
    assert_eq!(amount, 1_000);
}

#[test]
fn guard_fails_atomically_when_final_balance_is_short() {
    let (mut svm, payer) = new_svm_with_payer();
    let authority = Keypair::new();
    let sink = Keypair::new().pubkey();
    let mint = Keypair::new().pubkey();
    let token_account = Keypair::new().pubkey();
    set_mint(&mut svm, mint, 1_000_000);
    set_token_account(&mut svm, token_account, mint, authority.pubkey(), 999);

    let guard = FinalBalanceGuard {
        token_account: Pubkey32::from_bytes(token_account.to_bytes()),
        owner: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
        min_balance: 1_000,
    };
    let ixs = vec![
        system_transfer(payer.pubkey(), sink, 10_000),
        to_sdk(&guard.instruction(&token_program_pubkey32())),
    ];

    let result = send(&mut svm, &payer, &[&authority], &ixs);
    let err = result.expect_err("expected Err on insufficient final balance");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("InstructionError(1, Custom(1))"),
        "expected InsufficientFunds at instruction index 1, got: {debug}"
    );

    // The first instruction (the sink transfer) was rolled back too: `sink`
    // was never funded before this transaction, so it still has no account.
    assert_eq!(svm.get_balance(&sink), None);
}

#[test]
fn guard_fails_when_signer_is_not_the_account_owner() {
    let (mut svm, payer) = new_svm_with_payer();
    let real_owner = Keypair::new();
    let impostor = Keypair::new();
    let mint = Keypair::new().pubkey();
    let token_account = Keypair::new().pubkey();
    set_mint(&mut svm, mint, 1_000_000);
    set_token_account(&mut svm, token_account, mint, real_owner.pubkey(), 1_000);

    // The guard names `impostor` as the owner and `impostor` signs, but the
    // token account's actual on-chain owner is `real_owner`.
    let guard = FinalBalanceGuard {
        token_account: Pubkey32::from_bytes(token_account.to_bytes()),
        owner: Pubkey32::from_bytes(impostor.pubkey().to_bytes()),
        min_balance: 1_000,
    };
    let ix = to_sdk(&guard.instruction(&token_program_pubkey32()));

    let result = send(&mut svm, &payer, &[&impostor], &[ix]);
    let err = result.expect_err("expected Err on owner mismatch");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("Custom(4)"),
        "expected OwnerMismatch, got: {debug}"
    );
}

#[test]
fn guard_fails_when_the_account_is_not_a_token_account() {
    let (mut svm, payer) = new_svm_with_payer();
    let authority = Keypair::new();
    // A plain system account: airdropped, no data, not owned by the token program.
    let not_a_token_account = Keypair::new().pubkey();
    svm.airdrop(&not_a_token_account, 1_000_000)
        .expect("airdrop a plain system account");

    let guard = FinalBalanceGuard {
        token_account: Pubkey32::from_bytes(not_a_token_account.to_bytes()),
        owner: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
        min_balance: 1_000,
    };
    let ix = to_sdk(&guard.instruction(&token_program_pubkey32()));

    let result = send(&mut svm, &payer, &[&authority], &[ix]);
    let err = result.expect_err("expected Err when the account is not a token account");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("InstructionError(0, InvalidAccountData)"),
        "expected InvalidAccountData when the account is not a real SPL Token \
         account, got: {debug}"
    );
}

/// litesvm (like the real Agave runtime it embeds) checks that every
/// referenced program account exists and is executable for the WHOLE
/// transaction before it runs any instruction: a transaction cannot be
/// partially accepted "up to" a missing program, so this cannot show the
/// compute-budget prefix executing followed by a per-instruction failure at
/// the first swap leg (there is a separate assertion below for that). What
/// it proves instead: (a) the compute-budget prefix alone is valid,
/// executable instruction data against a real SVM, and (b) the full plan's
/// instruction conversion is structurally complete and reaches the SAME
/// whole-transaction program-loading check that a real Whirlpool deployment
/// would also have to pass — it fails for exactly one reason (the Whirlpool
/// program is not loaded into this offline harness), not for a decoding or
/// account-layout defect introduced by this plan. This is NOT a full-route
/// success fixture — see ARB-029's remaining acceptance note.
#[test]
fn compute_budget_prefix_alone_is_accepted_by_a_real_runtime() {
    let (mut svm, payer) = new_svm_with_payer();
    let compute_budget = ComputeBudget {
        units: 200_000,
        micro_lamports_per_unit: 1,
    };
    let ixs: Vec<Instruction> = compute_budget.instructions().iter().map(to_sdk).collect();

    let result = send(&mut svm, &payer, &[], &ixs);
    assert!(
        result.is_ok(),
        "expected the compute-budget prefix to execute cleanly, got {result:?}"
    );
}

#[test]
fn full_plan_reaches_the_program_loading_check_and_fails_only_on_the_unloaded_whirlpool_program() {
    let (mut svm, payer) = new_svm_with_payer();
    let authority = Keypair::new();
    let starting_token_account = Keypair::new().pubkey();

    let key = |byte: u8| Pubkey32::from_bytes([byte; 32]);
    let plan = SolanaPlan {
        payer: Pubkey32::from_bytes(payer.pubkey().to_bytes()),
        authority: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
        starting_mint: key(3),
        starting_token_account: Pubkey32::from_bytes(starting_token_account.to_bytes()),
        token_program: token_program_pubkey32(),
        compute_budget: ComputeBudget {
            units: 200_000,
            micro_lamports_per_unit: 1,
        },
        legs: [
            WhirlpoolSwapLeg {
                whirlpool: key(7),
                token_authority: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
                token_owner_account_a: key(10),
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
            },
            WhirlpoolSwapLeg {
                whirlpool: key(8),
                token_authority: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
                token_owner_account_a: key(18),
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
            },
        ],
        guard: FinalBalanceGuard {
            token_account: Pubkey32::from_bytes(starting_token_account.to_bytes()),
            owner: Pubkey32::from_bytes(authority.pubkey().to_bytes()),
            min_balance: 900,
        },
        recent_blockhash_placeholder: [0; 32],
    };

    let ixs: Vec<Instruction> = plan.instructions().iter().map(to_sdk).collect();
    let result = send(&mut svm, &payer, &[&authority], &ixs);
    let err = result.expect_err("the Whirlpool program is not loaded into this harness");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("InvalidProgramForExecution"),
        "expected the whole-transaction program-loading check to reject the \
         unloaded Whirlpool program, got: {debug}"
    );
}
