# Restartable finalized Base event ingestion

This continuation of ARB-016 / #30 connects the existing canonical-hash recovery
component to an explicit polling executable and a PostgreSQL journal. It does
not start a deployed worker, expose new control API endpoints, generate quotes,
place paper fills, sign transactions or complete the original ticket by itself.

## Implemented boundary

`base-ingest` repeatedly invokes the existing `recover_logs` against the saved
checkpoint. Every accepted recovery is one atomic database transaction: the
complete decoded batch is inserted and the checkpoint is advanced together.
There is no checkpoint update before event persistence. Original registry digest,
ABI source commit, network, pool set, origin classification and initial checkpoint
are immutable. A competing writer must match the expected revision and context.
The same already-committed payload can be retried idempotently while the stream
remains active; a changed payload or a subsequent halt cannot be hidden by a retry.

A separate process resumes from PostgreSQL rather than assuming the chain tip
or an in-memory cursor. A crash before COMMIT leaves the old cursor; a lost response
after COMMIT is resolved by reading the database again. Database failures are not
permission to discard batches or recreate a stream. Stored event batches are
append-only, including explicitly empty blocks. A no-new-block poll adds neither
a fake coverage interval nor an empty receipt.

A changed checkpoint, broken ancestry, invalid log, provider refusal or range
limit stops the invocation and attempts to persist a typed HALTED state without
advancing the cursor. A halted stream cannot be restarted automatically. Its
old batches remain historical evidence, **not valid current canonical inputs**.
This conservative halt is not automatic rollback of dependent decisions: that
integration remains an original ARB-018 requirement. Recovery after a halt needs
an explicit reviewed continuation policy; do not hide the gap by silently seeding
a replacement at the latest block. Process interruption before a complete
recovery leaves an active cursor unchanged and can be resumed explicitly.

## Optional HTTP filter following

`--follow` adds bounded node block/log filters around this same durable recovery.
It reconciles finalized state even when no notification is returned. The ordinary
`--run` path is unchanged. See [filter following](BASE-FILTER-FOLLOW.md) for exact
semantics, resource cleanup and remaining reconnect/qualification limitations.

## Explicit local operation

The default and `--check`/`--help` do not read configuration or start services:

```sh
cargo run --locked -p evm-worker --bin base-ingest -- --check
```

A local operator provides these through the existing secret/environment mechanism.
Do not put credentials in source, arguments, logs, issues or chat.

| Environment name | Purpose |
|---|---|
| `ARB_INGEST_DATABASE_URL` | Existing permitted PostgreSQL test database connection. There is no fallback to a production URL. |
| `ARB_INGEST_OPERATOR_ID` | Operator-scoped identifier, at most 128 ASCII identity characters. |
| `ARB_INGEST_STREAM_ID` | Persistent stream name within that operator. |
| `ARB_INGEST_REGISTRY_FILE` | A reviewed array of the existing `PoolRegistry` configuration records, maximum 64 KiB and eight pools. The identity-only inventory is not this runtime configuration. |
| `ARB_BASE_RPC_URL` | Existing Base HTTPS endpoint, or loopback only for explicit synthetic validation. |
| `ARB_RPC_MIN_INTERVAL_MS` | An explicit canonical value 75–1000 is required for HTTPS. Loopback tests may use zero. This is per-transport spacing, not an account-wide CU quota. |
| `ARB_INGEST_MAX_POLLS` | 1 by default; explicitly 1–100. No unlimited or background daemon mode. |
| `ARB_INGEST_POLL_MS` | 2000 by default; explicitly 1000–60000. |

Schema migration, initialization, running and status are separate actions:

```sh
cargo run --locked -p evm-worker --bin base-ingest -- --migrate
cargo run --locked -p evm-worker --bin base-ingest -- --initialize
cargo run --locked -p evm-worker --bin base-ingest -- --run
cargo run --locked -p evm-worker --bin base-ingest -- --status
```

Only run migration against an authorized database after reviewing the additive
migration. Nothing in a normal `--run` or `--status` automatically applies schema
changes. The old `evm-worker` default executable remains unchanged.

Initialization verifies chain, finalized header, approved factory/pool code and
canonical recheck, then explicitly starts coverage **after** that seed. No history
at or before initialization is claimed. Initializing an existing stream refuses
before any provider request. Later starts require the same registry and input
origin. Status reads need the database/operator/stream only and make no RPC call.

Ctrl+C and, on Unix, SIGTERM cancel admission, wait for the bounded current RPC
to return, and reject its result before publishing a batch. Unix signal handlers
are installed before provider work; registration failure refuses startup. A stop
between polls is checked in the existing short waits, not only at the next poll.
SIGKILL or a host crash cannot be handled; the atomic database checkpoint remains
the restart boundary. A complete database COMMIT already in flight
cannot be recalled; an uncertain response requires rereading the durable cursor.
This standalone ingestion process is not a research control-API session, and its
ACTIVE state means the stream is resumable, not that a process is currently running.
Dashboard STOP does not control this explicitly started standalone process.

## Resource, provenance and deployment limits

The runtime uses two Tokio I/O threads and at most one blocking recovery task.
Each poll retains the original 60-second transport budget, five-second request
timeout and fixed request/byte limits. Recovery still rejects a gap larger than
16 blocks by default; it does not skip history to fit a window. Persistent limits
are 4096 committed batches and 64 MiB serialized payload per stream, with 2 MiB
per batch. Diagnostic pages are 1–16 batches. New streams are an explicit operator
operation, not automatic retention-limit rotation. Memory/storage limits are
application admission limits, not protection against an unavailable database.

Decoded projections are persisted; raw private-provider messages and endpoints
are not. Origin distinguishes HTTPS remote data from loopback fixtures but is not
an independent attestation of provider truth. Full ABI, runtime and ancestry
validation remains in `arb-evm`; storage revalidates the durable envelope/context.
The event journal is not a raw-capture archive or receipt-root completeness proof.
No network request is sent by compiling, ordinary CI, default invocation or merge.

These changes are a tested restartable finalized polling path. They are **not** a
WebSocket subscription client, live quote feed, automatic reconnect policy,
rollback invalidation of existing decisions, current hosted deployment, or genuine
market-event qualification. #30 and #29 keep their original remaining criteria.

## Verification

`crates/arb-storage/tests/ingestion.rs` uses required disposable PostgreSQL to
exercise atomic commit/restart, idempotency, concurrent writers, stale halt
refusal, immutable initialization, malformed/gapped input, operator boundaries,
large exact heights, database triggers and cancellation. An injected database
failure *after* batch insertion and cursor update proves both are rolled back.

`apps/evm-worker/tests/ingestion.rs` runs the real executable against a bounded
loopback RPC server and PostgreSQL: initialization, two separate process runs,
exact persisted events, repeated no-change polls, changed-registry refusal,
canonical-history halt and actual SIGINT during a held network request. The Unix
shutdown cases also send actual SIGTERM during a held RPC and between polls. They
require the same explicit CANCELLED outcome, no new provider admission or partial
batch, unchanged committed data, and a successful explicit process restart. Both
SIGTERM regressions failed against the preceding source, which exited by signal
rather than through its cancellation fence. Fixtures remain MANUALLY_CONSTRUCTED;
no test claims genuine market capture or a production stop-latency guarantee.

```sh
cargo test --locked -p arb-storage --test ingestion
cargo test --locked -p evm-worker --test ingestion
cargo clippy --locked -p arb-storage -p evm-worker --all-targets -- -D warnings
```

See the PR for exact source and executed results. Full project CI, existing
PostgreSQL/recovery/browser tests and source review remain integration gates.

Primary contracts: [PostgreSQL transactions](https://www.postgresql.org/docs/17/tutorial-transactions.html)
and [EIP-234 block-hash log filtering](https://eips.ethereum.org/EIPS/eip-234).
