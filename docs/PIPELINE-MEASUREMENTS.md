# Actual process measurements within ARB-013

This extends the existing bounded scheduler telemetry without changing research
admission, origin, configuration, worker ownership or production settings. It is
not an acceptance of the whole epic or permission to run an experiment.

## Measured boundaries

The worker retains four fixed component populations for its immutable network:

- `ingestion_rpc`: each actual read-only transport call, including its response
  handling, measured with the monotonic process clock. Transport errors remain
  separate from completed calls.
- `snapshot_decode_excluding_rpc`: the complete serial acquisition/validation
  interval minus the summed actual transport-call intervals. This includes the
  residual decoding/context-validation and instrumentation overhead, not just
  pure protocol arithmetic. An invalid subtraction is unknown, never zero.
- `route_evaluation`: the actual engine call over captured inputs. Function
  completion does not mean profitable, executable or admitted. The existing
  generation/epoch/deadline and post-evaluation permit checks still decide admission.
- `capture_and_journal_persistence`: actual filesystem capture materialization and
  collection journal/admission futures. Different operations intentionally share
  this aggregate; it is not a pure SQL or single-file latency benchmark.

Each component uses separate completed/failed histograms and an unfinished count.
Polled persistence futures dropped before completion increment unfinished rather
than inventing an elapsed duration. An abruptly killed process cannot emit a final
metric: its durable journal remains the authority for unresolved work. Measurement
loss must not be interpreted as completion or absence of work.

There are eight fixed histograms in total, reusing the existing integer 96-bin
implementation with exact minima/maxima and conservative percentile upper bounds.
The counters occupy fixed memory even when output is disabled. Only the latest
collection UUID per component is retained, not an expanding set of trace labels.
A contended or poisoned metric mutex skips measurement rather than blocking the
worker or changing its durable state. The cumulative missed-sample counter is
separate from dropped work and resets on process restart.

The optional existing capacity-one writer adds `pipeline_at_encoding` to its JSON
record. That snapshot is taken when encoding, not when the scheduler frame was
sampled. Component last-collection IDs can differ: this is not one atomic
cross-component trace. Neither snapshot retains its lock during JSON encoding
or output. A slow consumer cannot cause unbounded buffering. Output remains off
by default and no deployed environment is changed by these source files.

## API and unsupported simulation

The API supplies `Server-Timing: api;dur=<milliseconds>` with three decimal
places and its existing `x-request-id`. The duration covers authorization through
handler/error-response production, including timeouts, not client transfer time
or response-body consumption. Header values are bounded generated numbers without
request paths, credentials, arbitrary labels or an inferred chain assignment.
Authentication, CSRF, rate/admission limits and command PENDING semantics stay intact.

The current worker does not perform full atomic transaction simulation. Its
telemetry reports `simulation.status=NOT_IMPLEMENTED` and a null duration, not a
zero latency or a successful simulation. API work is explicitly a separate process.
These omissions do not silently satisfy later simulation or market-data gates.

## Evidence to execute and inspect

`pipeline_metrics` tests cover distinct outcomes/networks, contention, fixed state
across 10,000 collection IDs, dropped asynchronous persistence and unchanged return
values. The API test uses the actual router with disposable PostgreSQL connectivity
and checks timing/correlation headers for denied, login and authorized responses.

The existing `controlled_capture` process tests enable telemetry in their two-pool
and stale-state cases. They require nonempty measurements for all four implemented
components from the actual worker executable, real PostgreSQL and loopback HTTP.
Original assertions for exact synthetic results, immutable provenance, raw captures,
STOP acknowledgement and absent real settlement remain. The other process tests for
blocked RPC, failed acquisition and generation fences are not removed or disabled.

The ordinary read-only `Worker pipeline observations` CI workflow records the
actual source/toolchain/runner and both measured profiles, plus logs even on failure.
It compiles committed source; it does not rewrite or publish source, use repository
write permissions, read provider credentials, or contact a blockchain. Inputs are
explicitly manually constructed loopback responses, not real market measurements.
Debug CI timing is not Railway capacity or a service-level guarantee. Passing
results and original-scope review must be inspected before integration; this document
by itself is not execution evidence. #29's recorded market trace and its original
M2 prerequisites remain independent requirements for whole-epic acceptance.
