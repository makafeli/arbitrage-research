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

No `Cargo.lock` is fabricated. Generate it using this toolchain, review the resolved
graph and commit it before claiming reproducible builds:

```sh
cargo generate-lockfile
cargo fmt --all
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

At scaffold authoring, neither `cargo` nor `rustc` was available in the environment.
TOML manifests and source paths were inspected, but compilation, rustfmt and Rust
tests were not executed. Successful CI is a required next validation gate. A direct
dependency pin does not pin transitive dependencies; the committed lockfile remains
necessary.
