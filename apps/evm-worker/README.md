# EVM worker boundary

This binary is an explicit capability placeholder. Running `cargo run -p evm-worker`
prints that the worker is unimplemented and exits with status **2**. It reads no
credentials and opens no RPC connection. It does not load the example configuration.

The planned Base worker will own ingestion, coherent state, pool math and its
bounded evaluation loop. Paper execution must be available and measured before
any separately reviewed signer or live transport is added. See the architecture
and delivery backlog for the implementation sequence.
