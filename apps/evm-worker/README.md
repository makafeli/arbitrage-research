# Evm worker: one-shot capture CLI

These commands acquire or inspect bounded read-only input bundles. They are **independent of dashboard session Start/Pause/Stop**. No RPC read is attempted without the `record` subcommand, a validated enabled configuration and a matching independently qualified registry.

```sh
cargo run -p evm-worker -- fixture \
  crates/arb-evm/tests/fixtures/registry.json \
  crates/arb-evm/tests/fixtures/rpc.json \
  /tmp/evm-manual-capture

cargo run -p evm-worker -- inspect /tmp/evm-manual-capture
```

The capture directory must not already exist. Fixture mode has no HTTP client and writes origin `manually-constructed`; these values cannot be reported as recorded market results. The command prints the manifest SHA-256 digest. Supply it as the optional third `inspect` argument to match the whole manifest against that previously recorded digest.

For real **state acquisition only**:

```sh
cargo run -p evm-worker -- record /path/to/research.toml /path/to/qualified-registry.json /path/to/new-capture
```

Set the RPC URL through the environment variable named by the configuration's `env:NAME` reference, using your local secret manager. The CLI checks the enabled network, pool/asset allowlists and `registry_qualification_digest` against the exact registry file bytes. It never prints the endpoint or provider error body. The config's effective JSON is stored without resolved secrets. Capture inputs remain unqualified for executable quotes, simulation and trading.

The HTTP reader has a 60-second total deadline, a 15-second per-request timeout, at most 4,096 requests and a 64 MiB bundle ceiling. Interrupting the command stops local acquisition; no transaction can have been emitted. Failed bundle writes are marked incomplete and rejected by readers. The global capture-directory quota and dashboard-controlled acquisition loop are separate integration work.

See [adapter documentation](../../crates/arb-evm/README.md) and [bundle format](../../crates/arb-capture/README.md).
