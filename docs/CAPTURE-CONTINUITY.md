# Capture and decision continuity dependencies

Continuation of existing ARB-018 / #32, following PR139. This adds a database
publication boundary, not a complete market worker, simulation or trading mode.
Original #30/#31 dependencies and #32 acceptance criteria remain unchanged.

## Producer contract

A producer that has validated a capture against a specific finalized ingestion
batch inserts `capture_ingestion_dependencies` after ordinary capture admission
and before publishing any decision that uses that capture. The row names the
operator, session, capture ID and exact manifest digest, plus the source stream,
revision, payload digest and complete checkpoint. The checkpoint must equal that
batch's `through` header. The source gate retains a SHARE lock through commit.

The producer remains responsible for reading/verifying the raw capture manifest,
its normalized chain context, registry and pool scope. A database association is
an explicit dependency assertion, not proof that arbitrary file contents were
correctly decoded. This patch does not infer a binding from time, pool name or a
healthy stream and does not invent bindings for historical captures. The current
research worker creates these associations automatically when an explicit source is configured, as described below.

All associations are immutable. A capture already mentioned in stored decisions
cannot be linked retrospectively. Session locking orders link registration and
decision publication, including the first association, so historical untracked
results cannot silently become tracked. A producer should register the complete
capture set in one short transaction; a failed transaction leaves no partial set.
Raw SQL clients remain trusted internal components, never browser clients.

## Existing publication path

Once a session declares a required source or contains an association, the existing `decision_traces` insertion
path automatically checks every new QUOTED decision with a database trigger.
All one to eight distinct capture references must have matching manifest/snapshot
IDs and capture generation. Every source must pass the PR139 continuity gate.
Source locks use a deterministic order and stay held until publication commits.
An invalidated source or missing/mismatched link rejects the new quote as a whole.
The worker's existing generation, epoch, configuration and trace checks remain.
This does not replace them or grant additional execution eligibility.

REJECTED, NO_ROUTE and DATA_UNAVAILABLE diagnostic rows can still be persisted
after a halt. Sessions without a required source or associations keep their old recording behavior,
but are explicitly UNTRACKED for this new gate. They are not claimed protected.
Existing exact-retry/history reads do not rewrite or retract original evidence.

## Historical inspection and current consumption

`decision_ingestion_validity` reports NO_KNOWN_INVALIDATION, INVALIDATED, UNTRACKED
or UNVERIFIABLE separately from the stored decision. Terminal stream exclusion
immediately changes the projection without modifying original decision payloads,
capture digests, timestamps or economic outcomes. NO_KNOWN_INVALIDATION does not
mean fresh, complete tick coverage, current provider qualification or profitable.

An internal consumer calls `require_current_decision_ingestion(operator, session,
observation)` within its own short transaction. It rejects untracked/missing or
invalidated inputs and returns the original payload only while holding the source
locks. Finish the related local publication in the same transaction. Never hold
this transaction across a network request and never cache its return value as
long-lived permission. This function alone does not validate full trace semantics
or simulation/capital/cost/freshness eligibility.

The browser API/export schema is unchanged. A later explicit endpoint integration
must expose this separate status rather than mutate historical evidence labels.
No capture-file retention, reconnect, Solana rollback or live-execution claim is
made here. In particular this is not an automatic paper settlement implementation.

## Verification

`crates/arb-storage/tests/capture_continuity.rs` exercises actual PostgreSQL with
synthetic database fixtures: original/new decision behavior, complete bindings,
exact reference errors, operator isolation, immutable history, rolled-back links,
and transaction ordering for publication and subsequent consumption versus HALT.
These tests verify the database boundary, not a new live-provider experiment or
complete engine DTO validation. Full CI, migrations and recovery checks remain
required. Actual results and tested commit IDs are recorded in the PR and #32.

PostgreSQL lock/trigger semantics are documented at:
- https://www.postgresql.org/docs/17/explicit-locking.html
- https://www.postgresql.org/docs/17/trigger-datachanges.html

## Explicit research-worker producer integration

Set `ARB_BASE_INGESTION_STREAM` to an already initialized Base stream owned by the
same operator. This is an internal identifier, not a URL, secret or trading switch.
The application does not create a stream, issue extra RPC requests or deploy a
worker merely because this code is present. The configured pool order, complete
typed registry digest, ABI source revision and recorded/fixture origin must match
the ingestion binding. The digest uses the same serialized `Vec<PoolRegistry>` as
`base-ingest`, not the distinct outer runtime registry-document hash.

After recovery to STOPPED and before the first provider request, the worker persists
an immutable per-session source requirement. A later restart cannot omit it, change
the stream or substitute another registry/origin. New quotes are guarded even before
the first capture association. Existing untracked decision history cannot be promoted
by turning the requirement on retrospectively; create a new research session instead.

Each admitted complete capture batch must have one exact finalized block number,
hash, parent hash and timestamp, plus the authorized per-pool registry and capture
provenance. The adapter validates the original responses, and `write_bundle` commits
the corresponding manifest/object digests before admission. All dependencies are
then registered atomically under the same worker generation and source locks.
The source must already retain the exact matching batch checkpoint: neither the
nearest height nor a similarly timed or differently hashed block is substituted.
An ingestion lag therefore produces a typed refusal, not optimistic publication or
an unbounded catch-up attempt. The independent ingestion process still needs its
own scheduling and resource budget.

After a source HALT, new publication and current re-consumption fail while historical
payloads remain readable and their separate validity becomes INVALIDATED. The worker
retains a typed collection failure when source binding fails. This does not provide
automatic recovery from terminal HALT, source rotation, Solana rollback, live trading
or automatic virtual settlement.

New worker publication uses `DecisionTrace::validate_for_publication` with at most
eight references, matching the bounded acquisition contract. Existing schema-level
`validate`, sealing, reads and exact retries retain the legacy representation so
stored 9..64-reference non-route diagnostics do not become corrupt after deployment.
Zero-input missing-data diagnostics remain valid. The number of decisions per batch
is still independently bounded at 64; it is not the number of captures per decision.

Tests use actual PostgreSQL and actual research-worker processes with the existing
synthetic HTTP provider. They cover automatic matching, same-height/wrong-hash refusal,
post-HALT exclusion, sticky source configuration, atomic rejected batches and the
publication-reference boundary. CI run/commit results are recorded on the PR and #32;
source code or this list alone is not a passed test or provider qualification.
