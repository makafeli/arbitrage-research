# ARB-013 / #27: bounded asynchronous scheduler telemetry

The research worker already bounds Tokio to two I/O threads and one blocking
thread, with at most one acquisition/evaluation job admitted at a time. Those
limits are preserved, not rebuilt or claimed as a new CPU-worker implementation.
Different chains use separate worker processes; actual Railway resource quotas
and shared database/I/O contention remain deployment concerns.

## Optional process-local export

Set `ARB_STAGE_METRICS_STDERR=1` in an authorized research-worker environment to
emit `scheduler-stage-metrics` JSON lines. Unset or `0` disables the feature;
other values fail before worker activity and do not echo the supplied value.
No environment, deployment, credentials or already running worker is modified by
checking in this code. The feature is off by default.

Every second the control runtime attempts one snapshot. If the scheduler mutex
is busy, it skips the sample rather than waiting. A single-pass bounded snapshot
copies exactly two networks times six stages, with no payload or correlation-ID
copy. It records queued/in-flight counts, oldest queue age and cumulative typed
accepted/dispatched/completed/drop counters. Missing queue age remains null, not
zero. These are process-local scheduler records, not aggregate chain health.

A capacity-one channel transfers owned snapshots using try_send. One dedicated
writer thread performs JSON encoding and stderr I/O outside all scheduler/control
locks and outside Tokio's sole blocking-worker slot. At most one frame waits while
one is written. A full channel drops the metric sample, never research work. The
next published frame carries the cumulative missed sample count. Large counters
and milliseconds are decimal strings; labels have fixed chain/stage cardinality.
No provider URL, credential, work payload or unbounded correlation label is sent.

A disconnected output sink disables the optional publisher, without changing
session mode, admission, generation or durable command/collection records.
Publisher drop closes the channel. The control shutdown path does not join a
potentially stuck OS write: process exit ends the writer. This is bounded lossy
operational telemetry, not an audit log or a hard real-time delivery guarantee.
A stalled stderr reader can still affect other writes to stderr; this change
specifically removes metric consumer I/O from scheduling and command processing.
It does not solve all process logging, shared storage or system-wide contention.

## Verified behavior required

Targeted Rust tests cover invalid channel capacities, a thousand publications
against a non-draining consumer, consumer disconnection, exact twelve-row
cardinality, a retained snapshot during gate changes, and Base work proceeding
with a stalled Solana permit. A direct contention test holds the scheduler lock
while trying to publish. Worker tests run an actually blocked Write sink while
work admission/completion and control fencing continue, and check JSON semantics
and output-error disconnection. No test connects to RPC, a wallet or production.

Run `cargo test --locked -p arb-scheduler` and
`cargo test --locked -p research-worker --bin research-worker stage_metrics` with
the pinned toolchain; full workspace/database/HTTP tests remain mandatory for
integration. Test execution and source SHA are recorded in the PR, not inferred
from this runbook.

## Original ticket remains open

The implemented acquisition/evaluation path currently schedules Quote work. Empty
counters for Ingestion, Snapshot, Simulation, Persistence and API mean no work was
instrumented in this scheduler stage, not measured zero latency or full coverage.
Full stage-duration instrumentation, named-host p50/p95/p99 profiles, system-wide
slow analytics/database isolation evidence, resource calibration and original
ARB-011/#25 acceptance remain outstanding. No original scope or dependency is
removed. This independently implementable slice advances #27 while #16's Base
qualification work is blocked; it cannot close #27 or enable a campaign.

## Measured permit durations and bounded percentile estimates

The continuation adds three independent cumulative populations per network and
scheduler stage: queue residence for dispatched work, execution duration for
successfully finished permits, and execution duration for explicitly rejected
finishes. Execution is measured from dispatch until the supplied monotonic finish
clock. This is scheduler acceptance, not a successful trade or transaction.
Queue items rejected before dispatch do not enter a queue-wait population.
Abandoned permits retain the original abandonment counter but have no invented
completion time. A finish clock before dispatch rejects the result and increments
`unmeasurable_completions`, without recording zero-duration success or failure.
A dispatch clock before queue admission similarly rejects future-timestamp work.

Each population has 96 fixed counters indexed by the bit length of duration in
nanoseconds. Zero has its own bucket. Percentiles use nearest-rank selection and
are reported as conservative **upper bounds**, limited by the exact observed
maximum. For example, 100 measurements from 1 to 100 nanoseconds produce a p50
upper bound of 63 nanoseconds, not a claim that the exact median was 63. Minimum
and maximum are exact. The counter array covers Duration::MAX without converting
to floating point or narrowing its full nanosecond value. At sample-count
exhaustion the entire population freezes and sets `saturated=true`; it never
wraps or changes the denominator independently of the bins. No raw samples,
per-trace metric labels or unbounded allocation are retained.

The existing asynchronous output includes the method name, string-valued counts
and nanosecond bounds. Empty populations have zero samples and null timing values,
not zero latency. This adds bounded process-lifetime telemetry state even when
the optional output is disabled. It is not a rolling window or a persisted
cross-restart aggregate. The dedicated capacity-one output and lock-contention
behavior remain unchanged.

`cargo run --locked -p arb-scheduler --example timing_profile` explicitly produces
a synthetic microprofile for both networks and six stage labels, 200 completed
permits per stage/network. The workload is the same deterministic CPU placeholder
for every label; it measures scheduler boundaries, **not actual RPC, pricing,
simulation, storage or API performance**. The path-scoped timing workflow records
the actual GitHub runner identity, CPU count, architecture, toolchain, build mode
and checked-out source beside the output. Debug-build CI results are not Railway
capacity calibration or a production SLA. Applicable full workspace/PostgreSQL
and browser checks still run separately before integration.

This removes the missing permit-duration/percentile telemetry implementation but
not the remaining original ticket gates: #25 acceptance, real application-wide
stage instrumentation, deployed resource calibration and slow shared-database/
analytics isolation. No runtime configuration or deployed worker is changed.
