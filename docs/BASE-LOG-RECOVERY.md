# Base event decoding and bounded log recovery (ARB-016 / #30)

This additive component extends the existing read-only Base adapter. It neither
starts a collector nor completes the original ingestion ticket by itself.
The successful observation/control/replay experiment remains recorded in
`RECORDED-BASE-SLICE-RESULT.md`; no new provider observation is part of these tests.

## Data path and bounds

`arb_evm::events` decodes all nine pool-event layouts in the existing pinned
Uniswap V3 ABI. It validates the emitting pool, complete block/transaction/log
identity, removal flag, topic counts, ABI lengths, address/integer padding and
protocol tick/price ranges. Amounts, including signed 256-bit swap deltas, remain
exact strings. Zero-value collection/burn events are not discarded. Unknown or
malformed events are errors, never empty market data.

Subscription notification decoders require the matching active subscription ID.
Head classification distinguishes a duplicate, direct extension, skipped heights
and conflicting/out-of-order ancestry. Neither a head nor a log notification
proves finality. Removed logs remain explicitly marked for invalidation.

`arb_evm::backfill::recover_logs` takes approved pool registries, an immutable
starting checkpoint, explicit limits, the existing `ReadRpc` and a cancellation
predicate. It checks Base chain identity and the checkpoint, reads a finalized
upper bound, then walks every intervening header in order. Logs are requested by
**blockHash**, without fromBlock/toBlock fallback. Factory and pool runtime hashes
are checked at every recovered block with `requireCanonical: true`. The target
and starting checkpoint are rechecked after the complete range.

Defaults are 16 blocks, 256 logs per block and 1,024 logs per attempt. Hard maximums
are 32 blocks, 512 logs per block, 4,096 logs total and the existing eight-pool
limit. With B recovered blocks and P pools, a successful nonempty attempt uses
`5 + B * (P + 3)` RPC calls, at most 357. Existing HTTP response/cumulative byte,
request and 60-second capture limits still apply. Configured request pacing is
inherited; this component has no retries, endpoint changes or quota reset.
Cancellation is checked before and after every request. An already-sent request
cannot be recalled and remains bounded by the existing transport timeout.

Each complete block retains an explicitly empty or nonempty provider result.
Logs are sorted by log index; duplicates, changed transaction/index bindings,
removed logs, foreign pools and mismatched block references fail the whole range.
Noncontiguous log indexes are valid because other contracts may occupy them.

A failed or over-limit recovery returns a typed gap reason, the unchanged starting
checkpoint, requested end and failing height where known. No partial prefix is
returned as success, and the function never advances an external cursor. The
caller must persist the entire result before committing `through` as its new
checkpoint. Versioned output retains Base identity, exact registry digest, pinned
ABI revision and complete per-block event metadata.

`arb_evm::backfill::recover_logs_bounded` runs the same chain-identity, ancestry,
code-identity, log-bound and recheck logic as `recover_logs_through` against an
exact requested target, but never rejects a range longer than `limits.max_blocks`
outright. When the requested header is farther away than one attempt may walk, it
fetches nothing beyond the cap: it walks exactly the first `max_blocks` blocks
after the checkpoint and returns that reached height as `through`, with no block
skipped and no limit raised. A caller that has not yet reached the requested
header repeats the call from the returned checkpoint on a later attempt until the
requested header is reached, at which point it behaves exactly like
`recover_logs_through`. `recover_logs` and `recover_logs_through` are unchanged.

## Evidence and capability boundaries

An empty blockHash-bound result is not a missing block, but is still a single
provider's assertion. These reads do **not** prove that a malicious or silently
truncating provider returned every log; receipt-root verification is not present.
The checkpoint is caller-supplied, not authenticated storage. A reorganization
returns an explicit error requiring invalidation/reconciliation; it does not
silently discard previously persisted observations or select a new checkpoint.

All results require a full pool/tick snapshot. Events are not substituted for
quote inputs, and they do not change quote, simulation or executable-capability
flags. No wallet, transaction signing, broadcasting or paid-provider operation is
introduced. The `EthGetLogs` enum variant adds one read-only wire method; prior
capture request order, transport limits, and `Cargo.lock` are unchanged.

This is a library component with tested real loopback HTTP and transcript replay.
A continuously running WebSocket/IPC subscription transport, reconnect scheduling,
durable cursor/rollback integration with the worker, representative real event
captures and complete quote-state qualification remain original #30 work. Do not
close #30, #29 or EPIC-02 on these component tests alone. No new task is required.

## Verification

Run with the repository's pinned Rust toolchain:

```sh
cargo test --locked -p arb-evm -p arb-adapter-api
cargo clippy --locked -p arb-evm -p arb-adapter-api --all-targets -- -D warnings
cargo check --workspace --all-targets --locked
cargo fmt --all -- --check
```

New cases exercise all event layouts, exact large/signed values, malformed topics,
ABI padding, pending metadata, subscription binding, removed/reordered events,
gap/ancestry detection, historical code changes, cancellation before/after I/O,
late reorganization, explicit bounds, empty results and unchanged checkpoints.
An actual local HTTP server verifies the new wire method, exact request, retained
response and zero-network transcript replay. All input is clearly synthetic.
Full GitHub CI and original-criterion review remain necessary before integration.

## Primary protocol references

The implementation is original Rust parsing/recovery code, not copied Solidity.
The existing pinned revision supplies the ABI shape and provenance:

- Uniswap ABI: https://github.com/Uniswap/v3-core/blob/d0831dc6b8a318df3872b6d68f6de135c9f3ec29/contracts/interfaces/pool/IUniswapV3PoolEvents.sol
  (Git blob `9d915dde934fc7a0430195f29bb2172c47783a3d`).
- Hash-specific log filtering and the empty-result ambiguity:
  https://eips.ethereum.org/EIPS/eip-234
- Canonical hash-pinned code reads: https://eips.ethereum.org/EIPS/eip-1898
- Connection-scoped notifications, skipped heads and removed logs:
  https://geth.ethereum.org/docs/interacting-with-geth/rpc/pubsub
