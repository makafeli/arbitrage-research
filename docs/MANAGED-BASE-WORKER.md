# Managed Base source reconciliation

This is an opt-in research-worker integration under existing ARB-044 (#58),
ARB-018 (#32) and ARB-016 (#30). It does not authorize live execution, complete
paper settlement, claim a Railway deployment or accept those whole tasks.

## Purpose and activation boundary

Previously an explicitly source-bound research worker needed a separate writer to
advance its Base ingestion stream to the exact block in every pool capture. The
worker can now perform that bounded catch-up itself, using its existing database,
RPC transport, control journal and dedicated capture volume.

Keep the existing validated config, registry, operator and API-created session.
An **explicitly initialized, ACTIVE Base stream** with the exact same typed
registry, ABI, pool list and data origin must already exist. Its original seed is
the lower coverage boundary. Initialization is still an operator/deployment step;
this change does not create a seed, assume preceding coverage or skip to a tip.
Never use synthetic test registry/seed data as production qualification.

```sh
# Additional settings for the existing research-worker process.
export ARB_BASE_INGESTION_STREAM=the-existing-approved-stream
export ARB_BASE_MANAGED_INGESTION=true
```

`ARB_BASE_MANAGED_INGESTION` defaults to false. Only literal `true` or `false` is
accepted. Enabling it requires the explicit stream setting and a matching ACTIVE
source before the process claims its session. Missing, HALTED or different sources
fail rather than being recreated or rearmed. Solana is unsupported by this option.
The existing externally managed source mode is unchanged when this option is off.

Use **one source writer**: do not run a separate `base-ingest` against this stream
at the same time. Session leases protect session ownership; source cursor checks
prevent a concurrent writer from being silently overwritten. Shared source
writer failover is not implemented. One process still owns one configured
session and one capture volume. There is no automatic session discovery here.

## One acquisition cycle

1. The worker reads the persisted ACTIVE source cursor and verifies its immutable
   binding. It commits the existing collection-attempt record before RPC work.
2. Existing capture code acquires all configured pools against a single finalized
   Base block. It validates the complete original shared quote transcript.
3. `recover_logs_bounded` verifies the original checkpoint and the precise captured
   target against the node. A newer finalized tip does not shift the target. It
   fetches missing logs by exact block hash, checks ancestry and pool runtime code,
   and rechecks canonicality. Empty hash-specific log results are valid evidence of
   no returned logs, not proof of all historical market coverage. A finalized step
   longer than the bounded per-attempt range is not fetched or rejected in one
   attempt: the walk covers exactly the configured `max_blocks` blocks after the
   checkpoint and returns that reached height, with nothing skipped and no limit
   raised. A capture whose finalized anchor is farther away than this attempt
   reached is not admitted for research; the next capture resumes the walk from
   the returned checkpoint until the anchor is reached.
4. No pool artifacts are written until that recovery succeeds. The source batch
   and checkpoint are then committed atomically, using the expected original
   cursor. Capture admission, exact source associations and quote publication run
   only afterward through their existing epoch, lease and generation fences, and
   only once the source has walked all the way to that capture's own anchor block.
5. A stream at its seed with zero accepted batches does **not** qualify protected
   quote admission. START remains PENDING until an accepted source batch exists.
   An unchanged already accepted checkpoint can be reused only after rechecks;
   repeated polling does not manufacture new ingestion revisions.

A source commit is not a trade or quote commitment. A later capture-admission or
quote failure can leave a correctly advanced source, but cannot publish without
matching source references. A crash after source commit keeps the original
checkpoint and immutable logs; retained raw bundles never gain automatic admission
on restart. Historical source validity remains separate from market freshness.

## Shared budgets and evidence

The **same** transport performs pool reads and log recovery. Taking the quote
transcript does not reset request count, retained-response bytes, pacing or the
cumulative clock. Existing limits remain 5 seconds per request, 60 seconds per
transport, 4,096 requests and 64 MiB retained responses. Existing capture-volume
and 65-second evaluation-clock limits are unchanged. Managed recovery uses the
existing 16-block default range and bounded per-block/total log counts, walked
over consecutive captures instead of raised: a finalized step of N blocks takes
`ceil(N / 16)` consecutive capture attempts, each committing one accepted batch
of at most 16 blocks against the existing 4,096-batch retention, 2 MiB
per-batch size limit and 64 MiB per-stream payload cap (`MAX_STREAM_BYTES` in
`crates/arb-storage/src/ingestion.rs`). It does not increase limits to make a
late or large capture pass, and stream rotation past that retention remains a
separate explicit operator step.

At Base's observed pattern of roughly +180 finalized blocks every ~6 minutes,
each step costs about `ceil(180 / 16) = 12` batches. At that rate the
4,096-batch ceiling is reached in around `4096 / 12 ≈ 341` steps, or roughly
1.5 days of continuous stalling-then-jumping; the 64 MiB stream cap can be
reached sooner depending on log volume per batch. Reaching either ceiling is
not a silent stop: the worker faults with `MANAGED_SOURCE_PERSISTENCE_FAILED`.
There is no rotation command yet — starting a fresh stream past that ceiling
is a follow-up operator tool, not something this change delivers.

The bounded walk only keeps up with the finalized tip while each ~16-block
step completes within a capture cycle (roughly 5-35 seconds, driven by
`CAPTURE_INTERVAL` plus RPC latency): that requires the RPC round-trip per
call to stay well under ~350 ms. `source_lag_blocks` on the `capture-written`
log line (anchor block minus the committed batch's `through` block, 0 once
caught up) is the field to watch for whether the walk is converging or falling
further behind.

Each pool bundle's `rpc.json` remains the original quote acquisition transcript.
Recovery responses are not inserted into it or reconstructed as fake quote RPCs.
Accepted decoded log data is kept in the durable ingestion batch and explicitly
linked to that pool capture. This change does not add a raw recovery-transcript
archive or a complete independent provider qualification experiment.

A catch-up exceeding its limits is a source failure, not permission to skip a
range. This managed mode does not borrow the separate CLI's reconnect budget or
retry automatically. Provider/429, invalid input, resource and continuity failures
remain fixed typed failures; no resolved endpoint or provider message is persisted
as a public reason. Freshness, complete tick coverage and transaction simulation
remain separate original acceptance gates.

## STOP, process shutdown and faults

Dashboard commands are polled independently of synchronous capture/recovery.
STOP first records PENDING, then APPLIED with the normal admission fence. An
already running RPC cannot be recalled. Its source catch-up may complete while
STOPPED or PAUSED, but the old generation cannot admit new quotes. Readiness/feed
work is intentionally distinct from active research.

SIGINT/SIGTERM fences local acquisition/evaluation admission immediately. A bounded
in-flight request may finish; its prepared source batch is not committed after the
process has observed shutdown, and shutdown itself does not HALT a healthy source.
It does not fabricate a completed dashboard STOP command. Restart uses the saved
source, restores the existing worker recovery behavior and never auto-starts.

A terminal recovery fault marks the expected source HALTED, invalidates dependent
source history and faults the session. Previous decision payloads, accepted logs
and checkpoints remain unchanged. Restart refuses that source without requests.
Concurrent source changes are never overwritten; an unsuccessful optimistic
persistence attempt fails the worker instead of claiming a current source.

## Verification and release limits

The new tests use synthetic loopback HTTP inputs, actual worker subprocesses and
actual PostgreSQL. They verify exact-target catch-up behind a moving tip, changed
headers/reorgs and cancellation; publication after durable source persistence;
unchanged quote replay; no false readiness at an empty seed; STOP during blocked
log recovery; malformed recovery invalidation with no partial artifacts; restart
refusal for HALTED sources; SIGTERM preserving the prior ACTIVE checkpoint; and
two consecutive finalized steps beyond the bounded range, each walked over
consecutive captures while the session stays RUNNING and the worker process
never exits. Every research-purpose collection finishing while its step is
still short of the anchor is `ACQUISITION_FAILED`/`ACQUISITION_UNAVAILABLE`
(checked directly against the `collection_attempts` table, not only the log),
and a new research collection admits, with `source_caught_up:true` and
`source_lag_blocks:0`, once each step's walk reaches its anchor. The strict
opt-in setting also has a unit test.

The pre-existing control, raw-capture, source-binding, HTTP, ledger and restart
checks remain required. Exact local results, final source CI and review are
recorded on the existing issue and PR rather than treating this document as proof.

No research worker has been deployed by this source change. Railway service
creation, approved provider/seed initialization, persistent volume permissions,
operational backup/restore and end-to-end hosted control checks remain in #58.
Automatic virtual settlement and full simulation are not delivered by a working
source feed. Real trading remains disabled.
