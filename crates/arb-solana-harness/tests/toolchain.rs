//! Proves the pinned litesvm toolchain runs under the workspace's Rust
//! version. Replaced by the plan harness in the ARB-029 harness increment.
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn litesvm_boots_and_funds_an_ephemeral_account() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000)
        .expect("airdrop into the in-memory bank");
    assert_eq!(svm.get_balance(&payer.pubkey()), Some(1_000_000));
}
