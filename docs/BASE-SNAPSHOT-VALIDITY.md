# Durable Base ingestion snapshot invalidation

This is a bounded continuation of ARB-018 / issue 32. It does not accept the
whole ticket, enable a worker, or qualify a complete quote. It adds a durable
continuity gate to the existing Base ingestion journal from ARB-016.

## What changes automatically

Migration 0007 adds one append-only invalidation record per terminally halted
stream. The existing `Store::halt_ingestion` path requires no API or caller change:
the database trigger writes the invalidation in the same transaction as HALT.
If either operation fails or rolls back, neither becomes visible. An invalidation
captures the fixed reason, exact revision, immutable binding, retained checkpoint,
detection timestamp and versioned `base-finalized-stream-v1` policy.

The conservative rule fences ALL batches of the same operator and stream after
any terminal continuity, provider, input or resource failure. This does not claim
that every historical block was reorganized or every historical calculation was
incorrect. It means the stream can no longer establish eligibility for NEW work.
The existing no-rearm rule remains. A successful transient reconnect that does
not terminally halt the stream does not invent an invalidation.

Existing halted Base streams receive `MIGRATED_HALT` records on upgrade, preserving
the original `updated_at` as detection time. `recorded_at` identifies when the
migration registered that already-known condition. New failures use
`STREAM_TRANSITION`. Original batch payloads, hashes, checkpoints and capture
provenance are not rewritten. Raw diagnostic/replay reads remain available.

## Current projection and internal gate

`ingestion_snapshot_validity` identifies each retained batch by operator, stream,
revision and digest. It returns `NO_KNOWN_INVALIDATION`, `INVALIDATED`, or
`UNVERIFIABLE`. Absence of a known invalidation is NOT complete/fresh/qualified
state, running-worker health, simulation evidence or trading permission.

`require_current_ingestion_snapshot(operator, stream, revision, digest)` returns
exact retained batch JSON only while the stream remains ACTIVE, the digest
matches and no invalidation exists. It takes a `FOR SHARE` lock on the stream,
conflicting with HALT/advancement until the caller's transaction completes.
Missing identities, wrong digests, unsupported networks and halted streams fail
with fixed SQL errors. The function has invoker rights, no dynamic SQL, and a
two-second lock timeout. The caller must also bound the whole transaction.

A consumer must perform this gate and its dependent database write in the SAME
short transaction. A view read or a function call in autocommit is only a snapshot
of that moment. Never hold the lock across provider requests or user interaction.
A returned JSON object is not a lease that remains valid after transaction end.
No existing engine/API query is silently promoted to using this gate: wiring
independent capture/quote associations and checking downstream publication remain
subsequent original ARB-018 integration work.

PostgreSQL references:
- https://www.postgresql.org/docs/17/explicit-locking.html
- https://www.postgresql.org/docs/17/applevel-consistency.html

## Verification and release limits

The new `arb-storage` integration suite exercises actual PostgreSQL triggers,
retained payload equality, all terminal reasons, operator/stream isolation,
append-only audit behavior, rollback atomicity, exact references, held-lock
ordering and upgrade from the pre-invalidation schema. CI runs it through the
existing locked workspace tests. Authored tests are not passing evidence; the
PR records actual completed runs and any remaining findings.

This delivery is Base-only. Solana rollback/context policy, independent capture
and quote dependencies, rolling snapshot reconstruction, provider-calibrated
freshness, live market qualification and dashboard presentation remain open.
Historical finalized block age must not be confused with HTTP latency. The gate
cannot make old data fresh, restore missing ticks, settle paper trades or enable
real orders. Database owners can alter schema/disable triggers; this is not a
malicious-administrator or offsite audit-tampering defense.

The migration is additive. Deployment uses the existing migration process; do
not manually erase audit history to downgrade. The new view is not included in
existing full-session exports; it is a separately scoped internal projection.
Existing restore tooling must retain the new table, and its CI drill remains
required before integration. No credentials, invitation, provider budget,
production worker configuration or existing acceptance criteria are changed.
