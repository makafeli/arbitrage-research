# Solana worker boundary

This binary is an explicit capability placeholder. Running `cargo run -p solana-worker`
prints that the worker is unimplemented and exits with status **2**. It reads no
credentials and opens no RPC connection. It does not load the example configuration.

The planned Solana worker will own account/slot ingestion, snapshot consistency,
venue math and its bounded evaluation loop. A Solana account feed cannot be treated
as an EVM block stream. Supported instructions, compute budgets and simulation
context require their own adapter qualification.
