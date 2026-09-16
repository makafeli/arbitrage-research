# Explicit Base block/log filter following

This is the continuation of ARB-016 / #30 after PR132/133. It adds an explicitly
started `base-ingest --follow` mode, not a deployed service or automatic trade.
The ordinary `--run`, inert default, migration and initialization behavior remain
unchanged. Existing registry, database and provider settings are reused.

## Operation and authority

After verifying a stored ACTIVE cursor and its original registry/origin binding,
`--follow` creates one node block filter and one address-scoped log filter. It polls
both with `eth_getFilterChanges`. Those are HTTP node filters, not WebSocket push
subscriptions. The implementation follows the Ethereum JSON-RPC filter contract:
<https://ethereum.org/developers/docs/apis/json-rpc/#eth_newfilter>.

Every iteration then performs the existing complete finalized-hash recovery from
the stored PostgreSQL cursor. **Even an empty notification array causes this
reconciliation.** Lost notifications, a delayed finality update or process restart
cannot make the process jump to the head advertised by a notification. Notification
hashes and decoded speculative/removed logs are only hints: they never enter the
accepted event journal or become quote state. Removed hints do not certify a
rollback of finalized history; existing canonical checks and halt behavior remain.

The process emits one `FILTER_HINTS` JSON record per successful notification poll,
with bounded counts and `authoritative: false`, followed by the ordinary committed
batch or no-new-finalized-blocks status. Hints are not a market coverage counter.
The caller still must not interpret ACTIVE as proof of a running process.

```sh
# The existing explicit migration/initialization is required only once.
cargo run --locked -p evm-worker --bin base-ingest -- --follow
```

This uses `ARB_INGEST_MAX_POLLS` (1 by default; at most 100) and the existing
`ARB_INGEST_POLL_MS` interval. Remote access still requires explicit RPC pacing.
No endless daemon, provider account, endpoint conversion or fallback is created.
A provider must support these four additional node filter methods; successful
historical eth_call access alone does not prove filter support. A real provider
follow experiment has not been run as part of offline development.

## Bounds, failures and cleanup

There are at most two known node filter IDs, each at most 32 bytes encoded as a
canonical hexadecimal quantity, and at most 512 block hints plus 512 log hints
per poll. Logs are decoded against the original approved pool set with the
existing strict event decoder. Wrong chains, duplicate IDs/pools, malformed or
foreign notifications and exceeded counts fail without accepting a partial poll.
A failed/cancelled pair cannot be polled again implicitly.

Creation, notification reads and full recovery consume the same existing transport
request/byte/60-second budget for that iteration. The first iteration includes
filter allocation; subsequent iterations reuse the IDs. No transport quota is
increased. Closing sends at most two explicit uninstall requests, even on a normal
error/cancellation exit. Final cleanup has an independent budget of two calls with
one-second request timeouts, so it cannot retry collection or extend its validity.
Unknown allocations after a lost creation response and abrupt process death may
remain at the node until the provider expires them. Drop does not hide network I/O.

A node returning false to uninstall indicates an already-absent filter and is a
valid response. Malformed cleanup responses and cleanup transport failures are
errors; a cleanup failure after a completed atomic batch does not undo that batch.
The original acquisition error takes precedence when both collection and cleanup
fail. No automatic filter recreation, provider retry, rearm of HALTED streams or
infinite catch-up is attempted. An expired filter remains an explicit failure
requiring diagnosis, not permission to skip the gap.

Existing SIGINT/SIGTERM cancellation, immutable binding, atomic cursor persistence,
16-block default recovery range, 4096-batch/64-MiB retention and STOPPED/error
semantics are preserved. The dashboard session STOP still does not control this
separately launched foreground process; use its process controls.

## Verification boundary

Tests exercise exact filter parameters, partial allocation cleanup, malformed and
foreign replies, removed hints, bounded calls/results, failure fencing and explicit
cleanup. Real local HTTP tests verify all four wire methods and transcript playback.
Actual executable/PostgreSQL tests cover quiet notifications with finalized advance,
filter reuse, process restart, rejection without cursor advance and refusal before
allocation for a missing/halted stream. Inputs are synthetic and labelled as such.

These tests do not prove actual provider completeness, current deployment, low-latency
quotes, automatic reconnect, derived snapshot invalidation or paper settlement.
#30 and the original #29 dependency gates remain open until all original criteria
are met. The source adds no API endpoint, migration, dependency version or permission.
