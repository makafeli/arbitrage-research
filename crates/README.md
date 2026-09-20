# Rust foundation

Only `arb-domain` is implemented in this initial scaffold. It has no third-party
dependencies and no network, filesystem, signing or broadcast interface. The
adapter, engine, risk, simulation, execution, storage, control protocol, signing,
EVM, Solana and telemetry crates described in the architecture are planned. They
will be added with working vertical features, rather than empty placeholder crates.

The workspace pins Rust **1.90.0**, Axum **0.8.9** and Tokio **1.53.1** as an initial
build baseline. This is not a claim that the compiler is the newest release or that
the dependency graph has completed security review. Version availability was
checked against the [Rust release announcement](https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/),
[Axum documentation](https://docs.rs/axum/0.8.9/axum/) and
[Tokio documentation](https://docs.rs/tokio/1.53.1/tokio/). Compatible SDK versions
for chain adapters are deliberately not selected before the adapter spike.

The resolved `Cargo.lock` is committed. Use it for normal development and review changes to the dependency graph explicitly:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The [verified GitHub CI run](https://github.com/makafeli/arbitrage-research/actions/runs/34709107929) passed formatting, Clippy across all workspace targets and all 19 Rust tests on the pinned toolchain. The initial workspace lacked local Rust tooling; CI supplied the compiler and generated the reviewed lockfile. This verifies the foundation's source behavior, not protocol arithmetic, durable execution controls or a trading strategy.

`arb-evm::simulation` (ARB-030, half 1) holds the simulation manifest contract and the exact-plan evidence gate. No simulation runner exists yet; `CapabilityReport.full_transaction_simulation` is still `false`, and no record can carry `SIMULATED` from this code alone.

`arb-solana-harness` (ARB-029) runs `arb-solana`'s research-only `SolanaPlan` inside an offline, in-process `litesvm` VM. `tests/mainnet_route.rs` loads the REAL, unmodified Orca Whirlpool program ELF and REAL pool/vault/mint/tick-array account state pinned to one finalized mainnet slot (see `tests/fixtures/mainnet/accounts.json`'s `provenance`), then executes a real two-leg swap cycle across two SOL/USDC whirlpools that share both mints. It proves three things against that real program and real state: the cycle executes and the resulting balances match a chained quote; the final-balance guard reverts the whole route atomically (both token accounts and both pools' vaults byte-for-byte unchanged) when the outcome would be short; and the crate's offline math (`quote_two_leg_cycle_math`) matches the executed leg outputs exactly. What it does not prove: the authority's own funding is synthetic (this harness has no real funded wallet), the state is frozen to one slot rather than live, both pools are static-fee legacy whirlpools (no adaptive-fee/oracle pools), and nothing here signs, submits or deploys anything — it is a research artifact, not a trading strategy or a deployment.
