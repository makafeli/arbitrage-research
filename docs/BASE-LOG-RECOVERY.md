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
upper bound, then walks every intervening header in order to verify ancestry.
Logs for the whole step are requested with **one ranged `eth_getLogs` call**
(`fromBlock` = checkpoint + 1, `toBlock` = target, `address` = the pool list),
not one call per block. Factory and pool runtime code are checked only at the
step's two bounds — the checkpoint block itself (`from`) and the target block
(`through`) — each pinned by blockHash with `requireCanonical: true`, not at
every intervening block; see "Residual risk of the ranged fetch" below for
what this does and does not prove. The target and starting checkpoint are
rechecked after the complete range.

Defaults are 32 blocks, 256 logs per block and 2,048 logs per attempt. Hard
maximums are 32 blocks, 512 logs per block, 4,096 logs total and the existing
eight-pool limit (issue #180, 2026-09-19: the default recovery range doubled
from 16 to 32 blocks and now equals the hard maximum; the default log-per-
attempt budget doubled from 1,024 to 2,048 in step, keeping ~64 logs/block of
headroom — the hard maximum of 4,096 is unchanged). With B recovered blocks
and P pools, a successful nonempty attempt uses `6 + 2 * (P + 1) + B + 1` RPC
calls — 6 fixed calls (chain id, checkpoint header, finalized header, target
confirmation, final target recheck, final checkpoint recheck), two bound code
checks of `P + 1` calls each, one ancestry header per recovered block, and one
ranged `eth_getLogs` — at most 57 at P = 8, B = 32. Existing HTTP
response/cumulative byte, request and 60-second capture limits still apply.
Configured request pacing is inherited; this component has no retries,
endpoint changes or quota reset. Cancellation is checked before and after
every request. An already-sent request cannot be recalled and remains bounded
by the existing transport timeout.

Each complete block retains an explicitly empty or nonempty provider result.
The ranged response's provider-advertised order is trusted instead of being
re-sorted: **provider order is trusted; a non-increasing `logIndex` within a
block is `ConflictingLog`.** This is a deliberate tightening introduced with
the ranged fetch (issue #180, 2026-09-19), not a pre-existing invariant — the
earlier per-block, blockHash-filtered request had no cross-log ordering within
a single block's result to violate. Duplicates, changed transaction/index
bindings, removed logs, foreign pools and mismatched block references still
fail the whole range. Noncontiguous log indexes are valid because other
contracts may occupy them.

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

### Residual risk of the ranged fetch

A **present** log is verified: it is attributed to its own block by matching
`blockNumber`/`blockHash` against that block's independently fetched,
ancestry-checked header. An **absent** log is not independently verifiable by
the request itself — a stale or split-view provider can return an "explicitly
empty" range for blocks that later have logs committed on the canonical chain,
and there is no per-block blockHash-pinned call left to catch that on its own
(issue #180, 2026-09-19: this is the ranged-fetch replacement for the earlier
per-block, blockHash-bound empty-result caveat below).

What bounds this: the target is always the `finalized` header, never a
provisional tip, and the by-number rechecks of both the target and the
starting checkpoint after the whole range (the final-target and
final-checkpoint recheck calls in `crates/arb-evm/src/backfill.rs`) catch a
reorg or a provider swap that would otherwise let a stale empty range through
undetected. Absent-log correctness is bounded by finality plus these rechecks,
not proven per block the way a present log is.

**Optional follow-up, not implemented here:** a blockHash-pinned `eth_getLogs`
call for each block the ranged response returned no logs for, to
independently confirm "no logs" per block instead of relying on the bounds
above. Left as a follow-up because it reintroduces one call per empty block,
undoing part of the ranged fetch's call-count reduction; owner decision
pending.

An empty ranged result is not a missing block, but is still a single
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

## Acquisition RPC failure logging

When the adapter call inside `AcquisitionRpc::call`
(`apps/research-worker/src/main.rs`) fails, the worker prints one
`acquisition-rpc-failed` JSON line carrying the collection's correlation ID,
the `ReadMethod` (serialized under its serde variant name, e.g. `EthCall`),
the adapter's fixed error `label`, and the `CollectionReason` that label maps
to, serialized in its wire form: `ACQUISITION_DEADLINE`, `RESOURCE_LIMIT`, or
`PROVIDER_UNAVAILABLE`. `correlation` equals the `collection_attempt_id` on
the `collection-finished` line of the same attempt — that shared value is the
join key between the two log lines. `label` is always one of the fixed
strings `HttpReadRpc::call` returns from `crates/arb-adapter-api/src/lib.rs`:
`RPC request quota exhausted`, `capture RPC deadline exceeded`, `RPC transport
failed (endpoint redacted)`, `RPC HTTP error: rate limited (details
redacted)`, `RPC HTTP error: access refused (details redacted)`, `RPC HTTP
error: provider server failure (details redacted)`, `RPC HTTP error (details
redacted)`, `RPC response read failed`, `RPC response or capture exceeds byte
quota`, `RPC response is not UTF-8`, `malformed JSON-RPC response`, and
`JSON-RPC identity mismatch or error (details redacted)`. Because
`AdapterError` wraps only a `&'static str` chosen from this fixed set, the
label is the only provider detail the worker ever logs — it can never carry
an endpoint, a header or a response body.

Sample line (serde_json emits object keys alphabetically):

```json
{"correlation":"3fa2b6d1-3c22-4a51-9e0b-7e5a2f9b6a10","event":"acquisition-rpc-failed","label":"RPC HTTP error: rate limited (details redacted)","method":"EthGetLogs","reason":"PROVIDER_UNAVAILABLE"}
```

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
- Hash-specific log filtering and the empty-result ambiguity (the per-block
  design superseded by the ranged `eth_getLogs` fetch, issue #180,
  2026-09-19; kept for the empty-result-ambiguity background, which still
  applies to a ranged response — see "Residual risk of the ranged fetch"
  above): https://eips.ethereum.org/EIPS/eip-234
- Canonical hash-pinned code reads (still the mechanism behind the two bound
  code checks): https://eips.ethereum.org/EIPS/eip-1898
- Cancun/Ecotone immutable contract code (the basis for checking code
  identity only at the step's two bounds instead of every block, issue #180,
  2026-09-19): https://eips.ethereum.org/EIPS/eip-6780
- Connection-scoped notifications, skipped heads and removed logs:
  https://geth.ethereum.org/docs/interacting-with-geth/rpc/pubsub
