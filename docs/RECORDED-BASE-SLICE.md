# ARB-015: bounded recorded Base integration

This is the remaining original EPIC-02 demonstration, not a new task or a live
trading release. #27 was already accepted on 15 September after PR127; this
continuation must not count that closure again. #29 and its original Base/Solana
ingestion, snapshot, mathematics and route prerequisites are still authoritative.
A completed experiment does not itself accept those requirements or the epic.

## Reused implementation, new orchestration

The driver starts the actual compiled `control-api`, `research-worker` and
`replay` programs with one disposable PostgreSQL database and the actual Vite/
React dashboard. It does not replace calculation, persistence, configuration,
control or replay with a Python implementation. The browser uses the real local
API and requires the exact recorded observation, configuration and capture hashes
in the visible decision detail. Only authentication is permitted to mutate state
from the browser. Startup and STOP actions go through the authenticated API.

Before capturing state, the existing operator observer rechecks the two Base
pool identities, factory, token decimal precision and observed code fingerprints
against the reviewed EPIC-01 inventory. Changed identities fail closed. The
isolated OBSERVE configuration starts from canonical USDC with an exact input of
1,000,000 minor units. Each pool has only its current bitmap word enabled. A quote
that needs another word must reject; the driver neither extrapolates missing
liquidity nor opens a wider window automatically. This is a CANDIDATE-only
experimental configuration, not an independently qualified production registry.

The real HTTPS worker creates RECORDED_LIVE capture manifests and raw transcripts.
A negative or rejected result is valid, but its input, source and missing-cost
information must remain genuine. The driver waits for actual durable readiness,
records START and STOP as PENDING then APPLIED, checks STOPPED and terminates the
worker before inspecting files. Captures from the same decision batch are replayed
by the existing replay executable with zero network requests. Result amounts,
reasons, calculation/configuration identity and capture references must match;
new replay observation IDs and presentation times are not claimed identical.

After the original fifteen-second lease has elapsed, a fresh worker must announce
its own recovery and return STOPPED with the same network/mode/configuration. A
previously stopped database row alone is insufficient. Actual API request timings
and the worker's existing stage metrics accompany the raw evidence. These are
small measurements on a named CI runner, not sustained-provider or Railway SLAs.
Full transaction simulation stays unimplemented, with no invented measurement.

## Deliberate execution boundary

Default invocation makes no network request and reads no provider credential:

```sh
python3 scripts/recorded_base_slice.py
python3 scripts/test_recorded_base_slice.py -v
```

The **Recorded Base observation slice** workflow runs offline tests for PRs and
ordinary pushes. Real collection occurs only on owner-dispatched main, or one
explicit owner-authored push on `feat/ARB-015-recorded-slice` with the exact commit
message `Run recorded Base slice once`. The latter is a one-shot integration
invocation, not a background collector. Test/review the source before issuing
that invocation. No normal merge, PR, fork or synchronization runs with the
provider secret. The secret is supplied only to the experiment step after
compilation and package installation are finished.

The harness admits only PostgreSQL at 127.0.0.1:5432/arb_recorded_slice, rejects
connection query/fragment overrides, and refuses occupied local application
ports. It cannot use the production Railway database. Outputs require a new
private directory. There is a four-minute process alarm, bounded API calls, a
six-attempt admission check, fixed provider methods/windows, existing capture
quotas/timeouts, and a fifteen-minute outer CI job limit including compilation.
A failed observed acquisition ends the driver rather than authorizing provider
rotation. The unchanged worker can begin a subsequent bounded request between
polls; termination prevents continued collection. No request already sent can be
recalled. A restarted stopped worker may perform read-only readiness acquisition.

All child processes are stopped in cleanup. Individual subprocesses receive only
their necessary secrets: provider access stays with the capture worker; replay,
browser and package build do not inherit it. The generated local operator secret
is never committed. Before any artifact is uploaded, regular-file/size bounds
and known secret/URL path/query scans must pass. Symlinks are refused. Failed
revalidation removes the old upload marker. This is an additional export guard,
not a universal secret detector or a guarantee about an untrusted provider.

Only `evidence/` is uploaded. Private temporary configuration and raw process
launch details remain outside it. Genuine captured market responses are retained
in the access-controlled run artifact, never silently relabelled as synthetic
fixtures or committed to source. Artifact retention is thirty days; hashes cannot
reconstruct expired bytes. Provider data-use rights remain the account owner's
responsibility. No signing, broadcasting, funds, paid account or production
resource operation is performed.

## Acceptance status

The source and offline tests are not an executed recorded experiment. The actual
run ID, checked source, hashes, browser output and stop/restart/replay outcomes
must be inspected before the demonstration can be accepted. Missing tick state,
changed contracts, unsuccessful acquisition, failed replay or failed browser
inspection remain explicit failures. Existing ARB-016/018/022/023 prerequisites
and both quote adapters must be accepted before #29 and EPIC-02 close. They are
not waived by this driver's successful exit status.
