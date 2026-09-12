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
