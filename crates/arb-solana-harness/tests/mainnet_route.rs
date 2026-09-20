//! Offline litesvm proof that the research-only `SolanaPlan` from
//! `arb_solana::plan` executes a real two-leg Orca Whirlpool cycle against
//! the REAL, unmodified Whirlpool program bytes and REAL pool/vault/mint/
//! tick-array state pinned to one finalized mainnet slot (448820183, see
//! `tests/fixtures/mainnet/accounts.json`'s `provenance`).
//!
//! Everything here runs in-process against `litesvm::LiteSVM`. The only
//! things that are NOT real are the authority's own two token accounts: they
//! are given a SYNTHETIC starting balance (this harness has no wallet with
//! real WSOL/USDC to trade), created fresh inside each test and dropped with
//! it. No RPC is contacted anywhere in this file, no key is ever persisted,
//! and nothing here signs a transaction meant to be sent anywhere. This is a
//! research artifact, not a deployment: it proves the plan's instruction
//! encoding and the crate's offline math against one pinned historical
//! state, not a live trading strategy or current pool conditions.

use arb_solana::math::quote_two_leg_cycle_math;
use arb_solana::plan::{
    AccountMeta as PlanAccountMeta, ComputeBudget, FinalBalanceGuard,
    Instruction as PlanInstruction, Pubkey32, SolanaPlan, WhirlpoolSwapLeg,
};
use arb_solana::{
    FixedTickArray, PoolSnapshot, WhirlpoolState, decode_fixed_tick_array, decode_whirlpool,
};
use base64::Engine;
use litesvm::LiteSVM;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

/// The raw fixture, one finalized `getMultipleAccounts` snapshot at slot
/// 448820183 plus the Whirlpool program ELF (loaded separately below).
const ACCOUNTS_JSON: &str = include_str!("fixtures/mainnet/accounts.json");
const PROGRAM_ELF: &[u8] = include_bytes!("fixtures/mainnet/whirlpool-program.so");

/// Pool addresses fixed in the fixture: two SOL/USDC whirlpools sharing both
/// mints, so a swap on one followed by the reverse swap on the other forms a
/// same-mint cycle.
const POOL_LOW_SPACING: &str = "Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE"; // tick spacing 4
const POOL_HIGH_SPACING: &str = "FpCMFDFGYotvufJ7HrFHsWEiiQCGbkLCtwHiDnh7o28Q"; // tick spacing 2

/// Orca's pinned historical bounds on `sqrt_price_limit`
/// (`orca_whirlpools_core=1.0.4`, the same version `arb_solana::math` uses).
/// Duplicated here as plain constants rather than a direct dependency on
/// `orca_whirlpools_core` from this dev-only crate: `MIN_SQRT_PRICE =
/// 4295048016`, `MAX_SQRT_PRICE = 79226673515401279992447579055`.
const MIN_SQRT_PRICE: u128 = 4_295_048_016;
const MAX_SQRT_PRICE: u128 = 79_226_673_515_401_279_992_447_579_055;

/// Synthetic starting balance for the authority's WSOL account: 1 SOL. This
/// harness has no real funded wallet; every other account in this test
/// (pools, vaults, mints, tick arrays, the program itself) is the real
/// mainnet state from the fixture.
const STARTING_WSOL: u64 = 1_000_000_000;
/// The authority starts with no USDC; the cycle should return to (near) zero.
const STARTING_USDC: u64 = 0;
/// Swap size: 0.1 WSOL. Small relative to both pools' liquidity so the trade
/// stays inside the three loaded tick arrays per leg.
const AMOUNT_IN: u64 = 100_000_000;

/// Parse `tests/fixtures/mainnet/accounts.json` into a generic
/// `serde_json::Value`. Deliberately untyped: this dev-only crate depends on
/// `serde_json` and `base64` only (see `Cargo.toml`), not on `serde`'s derive
/// macros or on `arb-adapter-api` directly.
fn fixture_root() -> serde_json::Value {
    serde_json::from_str(ACCOUNTS_JSON).expect("parse tests/fixtures/mainnet/accounts.json")
}

/// Find one account record by pubkey in an already-parsed fixture root.
fn fixture_account<'a>(root: &'a serde_json::Value, target: &str) -> &'a serde_json::Value {
    root["accounts"]
        .as_array()
        .expect("fixture has an accounts array")
        .iter()
        .find(|a| a["pubkey"] == target)
        .unwrap_or_else(|| panic!("fixture account {target} not found"))
}

/// Base64-decode an existing fixture account's `data_base64` field.
fn fixture_account_data(root: &serde_json::Value, target: &str) -> Vec<u8> {
    let record = fixture_account(root, target);
    assert_eq!(
        record["exists"], true,
        "fixture account {target} must exist"
    );
    let data_base64 = record["data_base64"]
        .as_str()
        .unwrap_or_else(|| panic!("fixture account {target} has no data_base64"));
    base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .expect("valid base64 fixture account data")
}

fn token_program_pubkey32() -> Pubkey32 {
    Pubkey32::from_base58(arb_solana::TOKEN_PROGRAM).unwrap()
}

fn token_program_address() -> Address {
    Address::from(*token_program_pubkey32().as_bytes())
}

fn address_of(base58: &str) -> Address {
    Address::from(*Pubkey32::from_base58(base58).unwrap().as_bytes())
}

/// The well-known `Clock` sysvar address (`SysvarC1ock11111111111111111111111111111111`).
fn clock_sysvar_address() -> Address {
    address_of("SysvarC1ock11111111111111111111111111111111")
}

/// Raise the in-process `Clock` sysvar's `unix_timestamp` to at least
/// `at_least`, leaving every other field untouched.
///
/// `litesvm::LiteSVM::new()` starts `unix_timestamp` at 0 (see its
/// `set_sysvars`, `Clock { slot: MAINNET_DEFAULT_SLOT, ..Default::default() }`).
/// The fixture's pools carry a real `reward_last_updated_timestamp` field
/// (part of the standard Whirlpool account layout, right after
/// `fee_growth_global_b`) set to the real wall-clock time the fixture was
/// snapshotted — since Solana's clock tracks real time, that is always far
/// past 0. Orca's swap instruction requires the current `Clock` to be at
/// least that recorded timestamp ("Timestamp should be greater than the last
/// updated timestamp", Anchor error 6022) before it will touch reward
/// accounting, even though this harness never funds or claims a reward.
///
/// Hand-decoded rather than imported: `Clock`'s wire format is the fixed,
/// stable sysvar layout (`slot: u64, epoch_start_timestamp: i64, epoch: u64,
/// leader_schedule_epoch: u64, unix_timestamp: i64`, 40 bytes, no padding);
/// adding `solana-clock` as a new direct dependency is out of scope for this
/// dev-only crate's restricted `Cargo.toml` (`serde_json` and `base64` only).
/// This does not touch the fixture: it overwrites the VM's own `Clock`
/// sysvar account, not any fixture-sourced account.
fn set_clock_unix_timestamp_at_least(svm: &mut LiteSVM, at_least: i64) {
    let address = clock_sysvar_address();
    let account = svm
        .get_account(&address)
        .expect("Clock sysvar account exists");
    let mut data = account.data.clone();
    assert_eq!(
        data.len(),
        40,
        "Clock sysvar must be the fixed 40-byte layout"
    );
    let current = i64::from_le_bytes(data[32..40].try_into().unwrap());
    if current >= at_least {
        return;
    }
    data[32..40].copy_from_slice(&at_least.to_le_bytes());
    svm.set_account(
        address,
        solana_account::Account {
            lamports: account.lamports,
            data,
            owner: account.owner,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
        },
    )
    .expect("bump Clock sysvar unix_timestamp");
}

/// Read a pool account's `reward_last_updated_timestamp` field: a real `i64`
/// unix timestamp at byte offset 261 in the standard Whirlpool layout
/// (immediately after `fee_growth_global_b`, which itself follows
/// `token_vault_b` at the offsets `arb_solana::decode_whirlpool` uses).
fn reward_last_updated_timestamp(root: &serde_json::Value, pool_address: &str) -> i64 {
    let data = fixture_account_data(root, pool_address);
    i64::from_le_bytes(data[261..269].try_into().unwrap())
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

/// Write a minimal, valid 165-byte SPL token account for a SYNTHETIC balance:
/// `mint` and `owner` are real fixture addresses, `amount` is made up by this
/// harness (see module docs).
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
    .expect("set synthetic token account");
}

/// Read a token account's raw `amount` field (bytes 64..72).
fn token_balance(svm: &LiteSVM, address: &Address) -> u64 {
    let account = svm.get_account(address).expect("token account exists");
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
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

/// Fresh SVM with the real Whirlpool program and every fixture account
/// loaded, plus a funded in-memory payer. No RPC, no network.
fn new_svm_with_fixture() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000)
        .expect("airdrop the in-memory payer");

    let whirlpool_program = address_of(arb_solana::WHIRLPOOL_PROGRAM);
    svm.add_program(whirlpool_program, PROGRAM_ELF)
        .expect("load the real Whirlpool program ELF");
    assert!(
        svm.get_account(&whirlpool_program)
            .expect("program account exists after add_program")
            .executable,
        "the loaded Whirlpool program account must be executable"
    );

    let root = fixture_root();
    for record in root["accounts"].as_array().expect("fixture accounts array") {
        if record["exists"] != true {
            continue;
        }
        let pubkey = record["pubkey"].as_str().expect("account pubkey");
        let owner = record["owner"]
            .as_str()
            .expect("existing account has an owner");
        let data_base64 = record["data_base64"]
            .as_str()
            .expect("existing account has data");
        let data = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .expect("valid base64 fixture account data");
        svm.set_account(
            address_of(pubkey),
            solana_account::Account {
                lamports: record["lamports"]
                    .as_u64()
                    .expect("existing account has lamports"),
                data,
                owner: address_of(owner),
                executable: record["executable"].as_bool().unwrap_or(false),
                rent_epoch: record["rent_epoch"].as_u64().unwrap_or(0),
            },
        )
        .unwrap_or_else(|e| panic!("set fixture account {pubkey}: {e:?}"));
    }

    // Both pools' `reward_last_updated_timestamp` reflect the real wall-clock
    // time the fixture was snapshotted; litesvm's default Clock starts at
    // unix_timestamp 0, which is always behind that. See
    // `set_clock_unix_timestamp_at_least` for why the swap needs this.
    let newest_pool_timestamp = [POOL_LOW_SPACING, POOL_HIGH_SPACING]
        .into_iter()
        .map(|pool| reward_last_updated_timestamp(&root, pool))
        .max()
        .expect("at least one pool timestamp");
    set_clock_unix_timestamp_at_least(&mut svm, newest_pool_timestamp + 1);

    (svm, payer)
}

/// The three fixture tick arrays around the current tick for `pool`, in the
/// direction `a_to_b` swaps in: descending starts (`[s, s-span, s-span*2]`)
/// selling A for B, ascending starts (`[s, s+span, s+span*2]`) buying A with
/// B. Returns `(addresses in swap-instruction order, decoded arrays)`.
fn selected_tick_arrays(pool: &PoolFixture, a_to_b: bool) -> (Vec<String>, Vec<FixedTickArray>) {
    // `tick_array_starts[2]` is the array holding the current tick for both
    // fixture pools (see module docs / issue #43).
    let indices: [usize; 3] = if a_to_b { [2, 1, 0] } else { [2, 3, 4] };
    let mut addresses = Vec::with_capacity(3);
    let mut arrays = Vec::with_capacity(3);
    for &i in &indices {
        let address = pool.tick_arrays[i].clone();
        let mut array = decode_fixed_tick_array(
            &pool.tick_array_data[i],
            &pool.pool,
            pool.state.tick_spacing,
        )
        .expect("decode fixture tick array");
        array.address = address.clone();
        addresses.push(address);
        arrays.push(array);
    }
    (addresses, arrays)
}

/// One pool's decoded state plus its five fixture tick arrays (raw bytes),
/// read once per test from the fixture JSON.
struct PoolFixture {
    pool: String,
    state: WhirlpoolState,
    tick_arrays: [String; 5],
    tick_array_data: [Vec<u8>; 5],
    oracle: String,
}

fn load_pool_fixture(pool_address: &str, tick_arrays: [&str; 5], oracle: &str) -> PoolFixture {
    let root = fixture_root();
    let pool_data = fixture_account_data(&root, pool_address);
    let state = decode_whirlpool(&pool_data).expect("decode fixture whirlpool state");
    let tick_array_data = std::array::from_fn(|i| fixture_account_data(&root, tick_arrays[i]));
    PoolFixture {
        pool: pool_address.to_string(),
        state,
        tick_arrays: tick_arrays.map(String::from),
        tick_array_data,
        oracle: oracle.to_string(),
    }
}

fn pool_low_spacing() -> PoolFixture {
    load_pool_fixture(
        POOL_LOW_SPACING,
        [
            "7T6JQngtMoLfPoxT6ZpbzY1uGiWkGPgwLC2vR1sTjPsn",
            "32wMhfqGgeaftnPacPR6pqBPL3agbd7to1oUsqo6y14F",
            "8NPFeBD52yqJWnsmBNma9qXXGEjXa6WatYcLMXjzSeyK",
            "D3461zSTVPNdBFPRk2b6zpqQ93g2LW5Kw2potgMdxNJP",
            "6hA1LN1fzCiXqymDiQXeBFn5da1b7STP1L7JmDc6hR3M",
        ],
        "FoKYKtRpD25TKzBMndysKpgPqbj8AdLXjfpYHXn9PGTX",
    )
}

fn pool_high_spacing() -> PoolFixture {
    load_pool_fixture(
        POOL_HIGH_SPACING,
        [
            "4bDQwhomEfGetMSpmAWvjSxvy7KmkhtBzWGBMUZDd8Ja",
            "7BbKG8YVD4P7me8CHwzFx7B56UVfvuYa7yuhEoh9wQak",
            "337KZvwgFwqHz5SFR328M8EGwXrgjbiFktXn3qFKXQGG",
            "G5duKHWhVbEsf7RZGcjtA8YE1CvoatvgTr3virDW2Gmd",
            "2A9neMejdY42C216L4cBFweXiLNXQFZAeNWMmkmBBUrv",
        ],
        "923j69hYbT5Set5kYfiQr1D8jPL6z15tbfTbVLSwUWJD",
    )
}

/// Build one pool's `PoolSnapshot` for the offline math quote out of the same
/// three fixture tick arrays selected for the executed swap leg. Metadata
/// fields not read by `arb_solana::math` (context slot, quality) are filled
/// with the fixture's real provenance where available and otherwise inert
/// placeholders documented inline.
fn snapshot_for_quote(pool: &PoolFixture, arrays: Vec<FixedTickArray>) -> PoolSnapshot {
    // Built through `serde_json` rather than naming `arb_adapter_api`'s
    // `StateContext`/`SnapshotQuality` types directly: this dev-only crate
    // depends on `arb-solana`, `serde_json` and `base64` only (see
    // `Cargo.toml`), and `PoolSnapshot` (like every type nested in it) is
    // already `Serialize + Deserialize`.
    let value = serde_json::json!({
        "pool": pool.pool,
        // Both pools were read in the SAME finalized getMultipleAccounts
        // response (slot 448820183); `quote_two_leg_cycle_math` requires
        // identical contexts across the two legs.
        "context": {
            "kind": "Solana",
            "slot": 448_820_183u64,
            "genesis_hash": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
            "commitment": "finalized",
            "account_context": "fixture:accounts.json",
        },
        "state": pool.state,
        "tick_arrays": arrays,
        "account_write_provenance": "fixture:accounts.json",
        // Not evaluated by `quote_exact_input_math`; the quote's own
        // `evidence: "CANDIDATE"` tag is the operative qualification marker.
        "quality": {
            "coherent": true,
            "complete_for_quote": true,
            "quote_implementation_qualified": true,
            "observed_at_ms": 0,
            "max_age_ms": 0,
            "reasons": [],
        },
    });
    serde_json::from_value(value).expect("construct PoolSnapshot for the offline quote")
}

/// Compute the offline two-leg cycle quote from the exact fixture bytes,
/// selling `amount_in` WSOL on the low-spacing pool then buying WSOL back on
/// the high-spacing pool. Returns `(leg1 output, leg2 output)` in atomic
/// units of USDC then WSOL respectively.
fn offline_quote(amount_in: u64) -> (u64, u64) {
    let low = pool_low_spacing();
    let high = pool_high_spacing();
    let (_, low_arrays) = selected_tick_arrays(&low, true);
    let (_, high_arrays) = selected_tick_arrays(&high, false);
    let low_snapshot = snapshot_for_quote(&low, low_arrays);
    let high_snapshot = snapshot_for_quote(&high, high_arrays);

    let quote = quote_two_leg_cycle_math(&low_snapshot, &high_snapshot, amount_in, true)
        .expect("offline two-leg quote against real fixture state");
    let leg1_out: u64 = quote.first_leg.amount_out.parse().unwrap();
    let leg2_out: u64 = quote.second_leg.amount_out.parse().unwrap();
    (leg1_out, leg2_out)
}

/// Build the real two-leg `SolanaPlan`: sell `amount_in` WSOL on the
/// low-spacing pool, buy WSOL back on the high-spacing pool with the exact
/// intermediate USDC amount the offline quote predicts. `other_amount_threshold`
/// is 0 on both legs on purpose: this proves whether the executed route
/// matches the offline quote (see `offline_quote_matches_the_executed_leg_outputs`)
/// rather than relying on a slippage check to hide any mismatch.
fn build_plan(
    authority: &Keypair,
    payer: &Keypair,
    wsol_account: Address,
    usdc_account: Address,
    amount_in: u64,
    leg2_amount: u64,
    guard_min_balance: u64,
) -> SolanaPlan {
    let low = pool_low_spacing();
    let high = pool_high_spacing();
    let (low_tick_arrays, _) = selected_tick_arrays(&low, true);
    let (high_tick_arrays, _) = selected_tick_arrays(&high, false);
    let key = |b58: &str| Pubkey32::from_base58(b58).unwrap();
    let pk32 = |a: Address| Pubkey32::from_bytes(a.to_bytes());

    SolanaPlan {
        payer: pk32(payer.pubkey()),
        authority: pk32(authority.pubkey()),
        starting_mint: key(&low.state.mint_a),
        starting_token_account: pk32(wsol_account),
        token_program: token_program_pubkey32(),
        compute_budget: ComputeBudget {
            units: 600_000,
            micro_lamports_per_unit: 1,
        },
        legs: [
            WhirlpoolSwapLeg {
                whirlpool: key(&low.pool),
                token_authority: pk32(authority.pubkey()),
                token_owner_account_a: pk32(wsol_account),
                token_vault_a: key(&low.state.vault_a),
                token_owner_account_b: pk32(usdc_account),
                token_vault_b: key(&low.state.vault_b),
                tick_arrays: [
                    key(&low_tick_arrays[0]),
                    key(&low_tick_arrays[1]),
                    key(&low_tick_arrays[2]),
                ],
                oracle: key(&low.oracle),
                amount: amount_in,
                other_amount_threshold: 0,
                sqrt_price_limit: MIN_SQRT_PRICE,
                amount_specified_is_input: true,
                a_to_b: true,
            },
            WhirlpoolSwapLeg {
                whirlpool: key(&high.pool),
                token_authority: pk32(authority.pubkey()),
                token_owner_account_a: pk32(wsol_account),
                token_vault_a: key(&high.state.vault_a),
                token_owner_account_b: pk32(usdc_account),
                token_vault_b: key(&high.state.vault_b),
                tick_arrays: [
                    key(&high_tick_arrays[0]),
                    key(&high_tick_arrays[1]),
                    key(&high_tick_arrays[2]),
                ],
                oracle: key(&high.oracle),
                amount: leg2_amount,
                other_amount_threshold: 0,
                sqrt_price_limit: MAX_SQRT_PRICE,
                amount_specified_is_input: true,
                a_to_b: false,
            },
        ],
        guard: FinalBalanceGuard {
            token_account: pk32(wsol_account),
            owner: pk32(authority.pubkey()),
            min_balance: guard_min_balance,
        },
        recent_blockhash_placeholder: [0; 32],
    }
}

#[test]
fn real_whirlpool_program_loads_and_executes_a_two_leg_cycle_with_synthetic_funding() {
    let (mut svm, payer) = new_svm_with_fixture();
    let authority = Keypair::new();
    let wsol_account = Keypair::new().pubkey();
    let usdc_account = Keypair::new().pubkey();
    let low = pool_low_spacing();
    set_token_account(
        &mut svm,
        wsol_account,
        address_of(&low.state.mint_a),
        authority.pubkey(),
        STARTING_WSOL,
    );
    set_token_account(
        &mut svm,
        usdc_account,
        address_of(&low.state.mint_b),
        authority.pubkey(),
        STARTING_USDC,
    );

    let (leg1_out, leg2_out) = offline_quote(AMOUNT_IN);
    // Any positive leg-2 output clears this: the guard only proves the
    // transaction reaches its final instruction with the route intact.
    let guard_min_balance = STARTING_WSOL - AMOUNT_IN + 1;
    let plan = build_plan(
        &authority,
        &payer,
        wsol_account,
        usdc_account,
        AMOUNT_IN,
        leg1_out,
        guard_min_balance,
    );

    let ixs: Vec<Instruction> = plan.instructions().iter().map(to_sdk).collect();
    let result = send(&mut svm, &payer, &[&authority], &ixs);
    let compute_units = result
        .as_ref()
        .map(|meta| meta.compute_units_consumed)
        .unwrap_or(0);
    assert!(
        result.is_ok(),
        "expected the real two-leg route to execute, got {result:?}"
    );

    let final_wsol = token_balance(&svm, &wsol_account);
    let final_usdc = token_balance(&svm, &usdc_account);
    println!(
        "executed route: amount_in={AMOUNT_IN} leg1_out(predicted)={leg1_out} \
         leg2_out(predicted)={leg2_out} final_wsol={final_wsol} final_usdc={final_usdc} \
         compute_units_consumed={compute_units}"
    );

    assert_eq!(
        final_wsol,
        STARTING_WSOL - AMOUNT_IN + leg2_out,
        "final WSOL balance must equal starting balance minus the swap-in amount plus leg 2's output"
    );
    assert_eq!(
        final_usdc, STARTING_USDC,
        "the intermediate USDC leg must round-trip back to zero: leg 1's output is entirely consumed by leg 2's input"
    );
}

#[test]
fn final_balance_guard_reverts_the_whole_real_route_when_short() {
    let (mut svm, payer) = new_svm_with_fixture();
    let authority = Keypair::new();
    let wsol_account = Keypair::new().pubkey();
    let usdc_account = Keypair::new().pubkey();
    let low = pool_low_spacing();
    let high = pool_high_spacing();
    set_token_account(
        &mut svm,
        wsol_account,
        address_of(&low.state.mint_a),
        authority.pubkey(),
        STARTING_WSOL,
    );
    set_token_account(
        &mut svm,
        usdc_account,
        address_of(&low.state.mint_b),
        authority.pubkey(),
        STARTING_USDC,
    );

    let low_vault_a = address_of(&low.state.vault_a);
    let low_vault_b = address_of(&low.state.vault_b);
    let high_vault_a = address_of(&high.state.vault_a);
    let high_vault_b = address_of(&high.state.vault_b);
    let before_wsol = svm.get_account(&wsol_account).unwrap().data;
    let before_usdc = svm.get_account(&usdc_account).unwrap().data;
    let before_low_vault_a = svm.get_account(&low_vault_a).unwrap().data;
    let before_low_vault_b = svm.get_account(&low_vault_b).unwrap().data;
    let before_high_vault_a = svm.get_account(&high_vault_a).unwrap().data;
    let before_high_vault_b = svm.get_account(&high_vault_b).unwrap().data;

    let (leg1_out, _leg2_out) = offline_quote(AMOUNT_IN);
    // Unreachable: strictly more than the starting balance, so no swap
    // outcome can ever satisfy it.
    let guard_min_balance = STARTING_WSOL + 1;
    let plan = build_plan(
        &authority,
        &payer,
        wsol_account,
        usdc_account,
        AMOUNT_IN,
        leg1_out,
        guard_min_balance,
    );

    let ixs: Vec<Instruction> = plan.instructions().iter().map(to_sdk).collect();
    let result = send(&mut svm, &payer, &[&authority], &ixs);
    let err = result.expect_err("expected the whole real route to fail on an unreachable guard");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("InstructionError(4, Custom(1))"),
        "expected InsufficientFunds at instruction index 4 (the guard), got: {debug}"
    );

    assert_eq!(
        svm.get_account(&wsol_account).unwrap().data,
        before_wsol,
        "WSOL account must be byte-for-byte unchanged after an atomic rollback"
    );
    assert_eq!(
        svm.get_account(&usdc_account).unwrap().data,
        before_usdc,
        "USDC account must be byte-for-byte unchanged after an atomic rollback"
    );
    assert_eq!(
        svm.get_account(&low_vault_a).unwrap().data,
        before_low_vault_a,
        "low-spacing pool's vault A must be byte-for-byte unchanged after an atomic rollback"
    );
    assert_eq!(
        svm.get_account(&low_vault_b).unwrap().data,
        before_low_vault_b,
        "low-spacing pool's vault B must be byte-for-byte unchanged after an atomic rollback"
    );
    assert_eq!(
        svm.get_account(&high_vault_a).unwrap().data,
        before_high_vault_a,
        "high-spacing pool's vault A must be byte-for-byte unchanged after an atomic rollback"
    );
    assert_eq!(
        svm.get_account(&high_vault_b).unwrap().data,
        before_high_vault_b,
        "high-spacing pool's vault B must be byte-for-byte unchanged after an atomic rollback"
    );
}

#[test]
fn offline_quote_matches_the_executed_leg_outputs() {
    let (mut svm, payer) = new_svm_with_fixture();
    let authority = Keypair::new();
    let wsol_account = Keypair::new().pubkey();
    let usdc_account = Keypair::new().pubkey();
    let low = pool_low_spacing();
    set_token_account(
        &mut svm,
        wsol_account,
        address_of(&low.state.mint_a),
        authority.pubkey(),
        STARTING_WSOL,
    );
    set_token_account(
        &mut svm,
        usdc_account,
        address_of(&low.state.mint_b),
        authority.pubkey(),
        STARTING_USDC,
    );

    let (leg1_out, leg2_out) = offline_quote(AMOUNT_IN);
    let guard_min_balance = STARTING_WSOL - AMOUNT_IN + 1;
    let plan = build_plan(
        &authority,
        &payer,
        wsol_account,
        usdc_account,
        AMOUNT_IN,
        leg1_out,
        guard_min_balance,
    );

    let ixs: Vec<Instruction> = plan.instructions().iter().map(to_sdk).collect();
    let result = send(&mut svm, &payer, &[&authority], &ixs);
    assert!(
        result.is_ok(),
        "expected the route to execute, got {result:?}"
    );

    let final_wsol = token_balance(&svm, &wsol_account);
    let final_usdc = token_balance(&svm, &usdc_account);
    let executed_leg2_out = final_wsol - (STARTING_WSOL - AMOUNT_IN);
    println!(
        "offline quote vs executed: leg1_out(quote)={leg1_out} leg2_out(quote)={leg2_out} \
         leg2_out(executed)={executed_leg2_out} final_usdc={final_usdc}"
    );

    // Leg 1's output was consumed EXACTLY as leg 2's input by construction
    // (`build_plan` uses the quoted `leg1_out` as leg 2's `amount`), so the
    // intermediate USDC balance returning to zero already proves leg 1
    // executed for exactly the quoted output.
    assert_eq!(
        final_usdc, STARTING_USDC,
        "leg 1's executed output must equal the quoted amount consumed whole by leg 2"
    );
    // `orca_whirlpools_core=1.0.4` is a faithful, unmodified port of the same
    // swap math the real deployed program runs; on this pinned static-fee
    // state the two must agree exactly, not just within a rounding unit.
    assert_eq!(
        executed_leg2_out, leg2_out,
        "leg 2's executed output must equal the offline quote's predicted output"
    );
}
