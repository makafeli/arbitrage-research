//! Research-only Solana transaction plan.
//!
//! `SolanaPlan` describes ONE intended atomic Solana transaction: a
//! compute-budget prefix, two Orca Whirlpool swap legs that start and end in
//! the same mint, and a final-balance guard instruction. The guard is an SPL
//! Token `Transfer` of a token account to itself; no custom on-chain program
//! is required or deployed (see [`FinalBalanceGuard::instruction`]). It is a
//! research artifact only. Every type in this module holds no keypair, no
//! signature and no serialized signed transaction; nothing here executes,
//! simulates or broadcasts anything. Signing and live execution are a later,
//! separately reviewed increment. The `arb-solana-harness` crate proves
//! offline (litesvm) that the guard fails atomically, that the plan
//! assembles into one transaction, and (`tests/mainnet_route.rs`, ARB-029)
//! that a real two-leg cycle executes against the real Whirlpool program and
//! real pinned mainnet state; nothing runs on a live cluster.

use crate::WHIRLPOOL_PROGRAM;
use arb_adapter_api::{AdapterError, Result};
use sha2::{Digest, Sha256};
use std::fmt;

/// The pinned Solana Compute Budget program id.
pub const COMPUTE_BUDGET_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";

/// Lower/upper bound accepted for `ComputeBudget::units` (Solana's per-transaction
/// compute unit ceiling).
pub const MIN_COMPUTE_UNITS: u32 = 1;
pub const MAX_COMPUTE_UNITS: u32 = 1_400_000;

/// A 32-byte Solana public key. Carries no signing capability: there is no way
/// to construct a signature or private key from this type.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Pubkey32([u8; 32]);

impl Pubkey32 {
    /// Build a key directly from 32 raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Pubkey32(bytes)
    }

    /// Decode a base58-encoded Solana address into a 32-byte key.
    pub fn from_base58(s: &str) -> Result<Self> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|_| AdapterError("invalid base58 Solana pubkey"))?;
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| AdapterError("Solana pubkey must be exactly 32 bytes"))?;
        Ok(Pubkey32(array))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Pubkey32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", bs58::encode(self.0).into_string())
    }
}

/// Decode a pinned, known-valid base58 program id. Only used for the fixed
/// program constants in this module; panics only on a programmer error
/// (a malformed pinned constant), never on caller input.
fn program_pubkey(base58: &str) -> Pubkey32 {
    Pubkey32::from_base58(base58).expect("pinned program id is valid base58")
}

/// One account reference within an [`Instruction`]. Carries no signature.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AccountMeta {
    pub pubkey: Pubkey32,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// One instruction within the plan. Unsigned and unsubmittable: this type has
/// no method that produces a signature or a wire-ready transaction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Instruction {
    pub program_id: Pubkey32,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

/// Compute-budget request: a unit limit and a priority fee price. Expands to
/// two Compute Budget program instructions with no accounts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ComputeBudget {
    pub units: u32,
    pub micro_lamports_per_unit: u64,
}

impl ComputeBudget {
    /// `SetComputeUnitLimit` (discriminator byte 2 + u32 LE) followed by
    /// `SetComputeUnitPrice` (discriminator byte 3 + u64 LE).
    pub fn instructions(&self) -> [Instruction; 2] {
        let mut limit_data = vec![2u8];
        limit_data.extend_from_slice(&self.units.to_le_bytes());
        let mut price_data = vec![3u8];
        price_data.extend_from_slice(&self.micro_lamports_per_unit.to_le_bytes());
        let program_id = program_pubkey(COMPUTE_BUDGET_PROGRAM);
        [
            Instruction {
                program_id,
                accounts: Vec::new(),
                data: limit_data,
            },
            Instruction {
                program_id,
                accounts: Vec::new(),
                data: price_data,
            },
        ]
    }
}

/// One Orca Whirlpool `swap` leg. Account order and writable/signer flags
/// follow Orca v1's `swap` instruction exactly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WhirlpoolSwapLeg {
    pub whirlpool: Pubkey32,
    pub token_authority: Pubkey32,
    pub token_owner_account_a: Pubkey32,
    pub token_vault_a: Pubkey32,
    pub token_owner_account_b: Pubkey32,
    pub token_vault_b: Pubkey32,
    pub tick_arrays: [Pubkey32; 3],
    pub oracle: Pubkey32,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool,
}

/// Anchor global-instruction discriminator: the first 8 bytes of
/// `sha256("global:swap")`.
pub fn swap_discriminator() -> [u8; 8] {
    let mut out = [0u8; 8];
    out.copy_from_slice(&Sha256::digest(b"global:swap")[..8]);
    out
}

impl WhirlpoolSwapLeg {
    /// The writable accounts within this leg's swap instruction: the
    /// whirlpool, its four token accounts/vaults and its three tick arrays.
    fn writable_accounts(&self) -> [Pubkey32; 8] {
        [
            self.whirlpool,
            self.token_owner_account_a,
            self.token_vault_a,
            self.token_owner_account_b,
            self.token_vault_b,
            self.tick_arrays[0],
            self.tick_arrays[1],
            self.tick_arrays[2],
        ]
    }

    /// This leg's input token account: the owner account debited when this
    /// leg executes, derived from `a_to_b` the same way a caller derives the
    /// leg's input mint.
    fn input_account(&self) -> Pubkey32 {
        if self.a_to_b {
            self.token_owner_account_a
        } else {
            self.token_owner_account_b
        }
    }

    /// This leg's output token account: the owner account credited when this
    /// leg executes, derived from `a_to_b` the same way a caller derives the
    /// leg's output mint.
    fn output_account(&self) -> Pubkey32 {
        if self.a_to_b {
            self.token_owner_account_b
        } else {
            self.token_owner_account_a
        }
    }

    /// Build this leg's `swap` instruction against the given SPL token
    /// program account.
    pub fn instruction(&self, token_program: &Pubkey32) -> Instruction {
        let mut data = swap_discriminator().to_vec();
        data.extend_from_slice(&self.amount.to_le_bytes());
        data.extend_from_slice(&self.other_amount_threshold.to_le_bytes());
        data.extend_from_slice(&self.sqrt_price_limit.to_le_bytes());
        data.push(u8::from(self.amount_specified_is_input));
        data.push(u8::from(self.a_to_b));
        let meta = |pubkey: Pubkey32, is_signer: bool, is_writable: bool| AccountMeta {
            pubkey,
            is_signer,
            is_writable,
        };
        Instruction {
            program_id: program_pubkey(WHIRLPOOL_PROGRAM),
            accounts: vec![
                meta(*token_program, false, false),
                meta(self.token_authority, true, false),
                meta(self.whirlpool, false, true),
                meta(self.token_owner_account_a, false, true),
                meta(self.token_vault_a, false, true),
                meta(self.token_owner_account_b, false, true),
                meta(self.token_vault_b, false, true),
                meta(self.tick_arrays[0], false, true),
                meta(self.tick_arrays[1], false, true),
                meta(self.tick_arrays[2], false, true),
                meta(self.oracle, false, false),
            ],
            data,
        }
    }
}

/// Final-balance guard: an SPL Token `Transfer` of `token_account` to itself
/// for `min_balance`, signed by `owner`. Asserts that `token_account` holds
/// at least `min_balance` at the point this instruction executes; see
/// [`FinalBalanceGuard::instruction`] for why a self-transfer enforces that.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FinalBalanceGuard {
    pub token_account: Pubkey32,
    pub owner: Pubkey32,
    pub min_balance: u64,
}

impl FinalBalanceGuard {
    /// Build the guard as one SPL Token `Transfer` instruction from
    /// `token_account` to itself for exactly `min_balance`, signed by
    /// `owner`. No custom on-chain program is required or deployed.
    ///
    /// Why a self-transfer proves a final-balance invariant: the SPL Token
    /// program checks `source.amount >= amount` before it does anything
    /// else, so if `token_account` holds less than `min_balance` at the
    /// point this instruction executes, it fails the whole instruction with
    /// `TokenError::InsufficientFunds` (custom program error 1) and the
    /// entire transaction — every earlier instruction in this plan — is
    /// rolled back atomically. A balance that already meets or exceeds
    /// `min_balance` makes the transfer a same-account no-op: it changes no
    /// state. The token program also checks that `owner` is the account's
    /// actual owner and has signed (`TokenError::OwnerMismatch`, custom
    /// error 4), and that `token_account` decodes as a real SPL Token
    /// account of this program. Placed last in the plan's instruction list,
    /// it asserts the FINAL balance, not a balance at some earlier point in
    /// the transaction. litesvm's mainnet feature set runs the p-token build
    /// of the token program (`replace_spl_token_with_p_token`), which agrees
    /// with spl-token on every outcome this module's harness asserts. This
    /// assumes `min_balance` is an absolute floor on the guarded account,
    /// which is only a meaningful final-balance check when that account is
    /// the route's dedicated start/end account.
    ///
    // ponytail: this enforces a balance-only invariant (>= min_balance); it
    // cannot express a richer post-condition. A custom guard program is the
    // deliberate upgrade path if a richer invariant is ever needed.
    pub fn instruction(&self, token_program: &Pubkey32) -> Instruction {
        let mut data = vec![3u8];
        data.extend_from_slice(&self.min_balance.to_le_bytes());
        Instruction {
            program_id: *token_program,
            accounts: vec![
                AccountMeta {
                    pubkey: self.token_account,
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: self.token_account,
                    is_signer: false,
                    is_writable: true,
                },
                AccountMeta {
                    pubkey: self.owner,
                    is_signer: true,
                    is_writable: false,
                },
            ],
            data,
        }
    }
}

/// Programs, whirlpools and mints a plan is permitted to reference.
#[derive(Clone, Debug, Default)]
pub struct ProgramAllowlist {
    pub programs: Vec<Pubkey32>,
    pub whirlpools: Vec<Pubkey32>,
    pub mints: Vec<Pubkey32>,
}

/// Why `SolanaPlan::validate` rejected a plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlanRejection {
    UnsupportedProgram { program: Pubkey32 },
    UnsupportedWhirlpool { whirlpool: Pubkey32 },
    UnsupportedMint { mint: Pubkey32 },
    AuthorityMismatch,
    GuardOwnerMismatch,
    GuardAccountMismatch,
    StartingAccountMismatch,
    RouteNotCyclic,
    InsufficientFinalBalance { required: u64, guaranteed: u64 },
    WritableSetViolation { account: Pubkey32 },
    ComputeBudgetOutOfBounds,
}

/// A deterministic `"sha256:" + hex` digest over a plan's canonical bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanDigest(pub String);

impl fmt::Display for PlanDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One intended atomic Solana transaction: a compute-budget prefix, two
/// same-mint Orca Whirlpool swap legs and a final-balance guard.
///
/// This is a research artifact only. `SolanaPlan` holds no keypair, no
/// signature and no serialized signed transaction anywhere in its fields or
/// methods; `instructions()` and `canonical_bytes()` only describe intent and
/// never execute, simulate or submit anything.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SolanaPlan {
    pub payer: Pubkey32,
    pub authority: Pubkey32,
    pub starting_mint: Pubkey32,
    pub starting_token_account: Pubkey32,
    pub token_program: Pubkey32,
    pub compute_budget: ComputeBudget,
    pub legs: [WhirlpoolSwapLeg; 2],
    pub guard: FinalBalanceGuard,
    pub recent_blockhash_placeholder: [u8; 32],
}

impl SolanaPlan {
    /// The plan's instructions in submission order: compute budget (2), swap
    /// legs (2), guard (1).
    pub fn instructions(&self) -> Vec<Instruction> {
        let mut instructions = Vec::with_capacity(5);
        instructions.extend(self.compute_budget.instructions());
        instructions.push(self.legs[0].instruction(&self.token_program));
        instructions.push(self.legs[1].instruction(&self.token_program));
        instructions.push(self.guard.instruction(&self.token_program));
        instructions
    }

    /// Pre-flight validation against an allowlist and the caller-supplied
    /// (input, output) mint pair for each leg. `leg_mints` is supplied by the
    /// caller because a plan alone does not decode Whirlpool state to
    /// determine swap direction; it only carries `a_to_b`.
    pub fn validate(
        &self,
        allowlist: &ProgramAllowlist,
        leg_mints: &[(Pubkey32, Pubkey32); 2],
    ) -> std::result::Result<(), PlanRejection> {
        for instruction in self.instructions() {
            if !allowlist.programs.contains(&instruction.program_id) {
                return Err(PlanRejection::UnsupportedProgram {
                    program: instruction.program_id,
                });
            }
        }
        for leg in &self.legs {
            if !allowlist.whirlpools.contains(&leg.whirlpool) {
                return Err(PlanRejection::UnsupportedWhirlpool {
                    whirlpool: leg.whirlpool,
                });
            }
        }
        for (input_mint, output_mint) in leg_mints {
            if !allowlist.mints.contains(input_mint) {
                return Err(PlanRejection::UnsupportedMint { mint: *input_mint });
            }
            if !allowlist.mints.contains(output_mint) {
                return Err(PlanRejection::UnsupportedMint { mint: *output_mint });
            }
        }
        for leg in &self.legs {
            if leg.token_authority != self.authority {
                return Err(PlanRejection::AuthorityMismatch);
            }
        }
        if self.guard.owner != self.authority {
            return Err(PlanRejection::GuardOwnerMismatch);
        }
        if self.guard.token_account != self.starting_token_account {
            return Err(PlanRejection::GuardAccountMismatch);
        }
        if self.starting_token_account != self.legs[0].input_account()
            || self.starting_token_account != self.legs[1].output_account()
        {
            return Err(PlanRejection::StartingAccountMismatch);
        }
        if leg_mints[0].0 != self.starting_mint
            || leg_mints[0].1 != leg_mints[1].0
            || leg_mints[1].1 != self.starting_mint
        {
            return Err(PlanRejection::RouteNotCyclic);
        }
        let guaranteed = if self.legs[1].amount_specified_is_input {
            self.legs[1].other_amount_threshold
        } else {
            self.legs[1].amount
        };
        if guaranteed < self.guard.min_balance {
            return Err(PlanRejection::InsufficientFinalBalance {
                required: self.guard.min_balance,
                guaranteed,
            });
        }
        for leg in &self.legs {
            let writable = leg.writable_accounts();
            for account in &leg.instruction(&self.token_program).accounts {
                if account.is_writable != writable.contains(&account.pubkey) {
                    return Err(PlanRejection::WritableSetViolation {
                        account: account.pubkey,
                    });
                }
            }
        }
        if !(MIN_COMPUTE_UNITS..=MAX_COMPUTE_UNITS).contains(&self.compute_budget.units) {
            return Err(PlanRejection::ComputeBudgetOutOfBounds);
        }
        Ok(())
    }

    /// Deterministic byte encoding of every instruction, in order:
    /// `program_id(32) ‖ u16 LE account count ‖ [pubkey(32) ‖ flags(1)]* ‖
    /// u32 LE data len ‖ data`. Flags bit 0 is signer, bit 1 is writable.
    /// Never uses serde/JSON, so it cannot silently change with field order.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for instruction in self.instructions() {
            bytes.extend_from_slice(instruction.program_id.as_bytes());
            bytes.extend_from_slice(&(instruction.accounts.len() as u16).to_le_bytes());
            for account in &instruction.accounts {
                bytes.extend_from_slice(account.pubkey.as_bytes());
                let mut flags = 0u8;
                if account.is_signer {
                    flags |= 0b0000_0001;
                }
                if account.is_writable {
                    flags |= 0b0000_0010;
                }
                bytes.push(flags);
            }
            bytes.extend_from_slice(&(instruction.data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&instruction.data);
        }
        bytes
    }

    /// `"sha256:" + hex(sha256(canonical_bytes()))`.
    pub fn digest(&self) -> PlanDigest {
        let hash = Sha256::digest(self.canonical_bytes());
        PlanDigest(format!("sha256:{}", hex::encode(hash)))
    }
}
