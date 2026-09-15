# Recorded Base slice: verified result, 15 September 2026

Existing work: ARB-015 / #29, PR #129. The experiment succeeded; the original
upstream ingestion/snapshot/route acceptance gates and EPIC-02 remain open.
No new issue, provider subscription, deployment or trading activation is implied.

## Source and retained evidence

- Experiment source: `4da3bd455fb1bdaa6650a7659fb9de1141f29cad`.
- Tested base: `ecabdfab7a045398994f421f1ffa624b7bd7d27b`.
- Tested source tree: `f1006efb63be15df59fe7b34c1ddeceeae2c15c0`.
- [Actual recorded run 34977582214](https://github.com/makafeli/arbitrage-research/actions/runs/34977582214).
- Evidence artifact: `10399852844`, expires 15 October 2026.
- Evidence ZIP SHA-256: `a53929fc930168699bb9a3b9d096cc23d80429ccaa4c2fa63b5d3162d47973a7`.
- Source artifact: `10400405349`, ZIP SHA-256
  `365aabda01a904b5329da1ec0bf842878fb975f9ca9eb2fcb847d0b3639755e2`.

Both archive hashes and their file checksums were independently checked during
integration. The source archive reconstructs the complete tree above. Raw market
captures remain in the bounded evidence artifact, not committed into source.
Hashes alone cannot reproduce raw data after artifact expiry. A successful
HTTPS response is not an independent attestation of provider honesty.

## What actually ran

The existing Rust API, worker and replay executables ran against a disposable
PostgreSQL database. The actual Chromium dashboard used authenticated local HTTP,
without route mocks. The runner was GitHub Actions 1000014324, Linux x86_64,
four logical CPUs, debug build. Total harness duration was about 134.4 seconds,
excluding dependency installation and compilation.

Session: `d545f1e2-119e-44c6-8879-fcce9be94e38`, mode `OBSERVE`.
Configuration digest:
`sha256:684d94e75aca45443e24ba9ea96c93019034f875ae84875635951cfe895a7f91`.

The journal records one completed readiness acquisition and one completed research
acquisition, two pools each, with zero unresolved or failed attempts in that
recorded interval. Four immutable capture bundles are retained. Only the research
acquisition produced decisions: two opposite two-pool USDC/WETH/USDC routes.

| Research result | Input, USDC minor units | Output, USDC minor units | Gross delta, USDC minor units |
|---|---:|---:|---:|
| First stored route | 1000000 | 995903 | -4097 |
| Opposite route | 1000000 | 997100 | -2900 |

Both are negative `QUOTED` results displayed as `CANDIDATE`, not trades or profit.
Pool fees are included; external costs remain unknown. The acquisition/evaluation
ages were 39,506 and 39,509 ms. The configured research age envelope is 60 seconds;
these measurements are not evidence of useful low-latency trading. Historical
chain age remains explicitly unmeasured in this legacy calculation record.
The dashboard correctly showed degraded/unsupported execution eligibility and
unknown net amounts, not an executable opportunity.

The replay read the same capture bundles and returned two exactly matching ordered
decision projections with zero network requests. Selection of the shared source
batch is order-independent; ordered route legs, manifest/snapshot identifiers,
amounts, diagnostics and decision multiplicity are still compared exactly.

START and STOP each have one durable command ID, with separate PENDING and APPLIED
receipts. Their stored acceptance-to-application intervals were approximately
203 and 210 ms; the polling harness observed acknowledgement after about one
second. Those are different measurements, neither an SLA. STOP had an effective
fence and actual STOPPED state. A new worker recovered to STOPPED with the same
session/network/mode/configuration and no execution authorization. Both screenshots
and the browser evidence JSON were inspected; the only browser mutation was login.

## Throughput correction and remaining limits

The driver explicitly sets `ARB_RPC_MIN_INTERVAL_MS=75` for its worker. Request
spacing consumes the existing 60-second capture deadline and does not reset it.
The unchanged default for other invocations is unpaced unless configured. This is
per-transport spacing, not an account-wide distributed compute-unit limiter.
Different clients/processes may still share the provider's account budget.

HTTP 429, access refusal and provider-server errors are distinguishable in the
transport's fixed redacted errors. No automatic retry, Retry-After handling,
provider fallback or account upgrade has been added by this implementation.
The collection journal can still aggregate transport failures under its broader
provider-unavailable category. The earlier failed run is not retroactively proven
to be throttling just because this paced run succeeded. The owner's rate-limit
notification is supporting context, not a captured 429 response for that run.

A future recovery policy must retain cumulative call/byte/time limits and coherent
block identity; waiting must never turn incomplete or stale state into valid input.
There is no reason to repeat the successful provider experiment merely to attach
this result record. Reuse its exact source, captures and checksums.

## Verification and acceptance boundary

All six PR workflows passed on the experiment source, including full project
34977587385, platform contracts, delivery, recovery, Rust preparation and the
offline recorded-slice tests. The separate push run above executed the real
recorded job, rather than skipping it. Integration also ran 92 Python tests
(19 slice, 12 operator wrapper, 23 observer, 20 delivery, 5 workflow, 13 identity),
project/specification checks and browser-script syntax locally. Rust/browser
execution was in CI, not claimed locally. The saved decisions and replay were
independently compared again, without another provider request.

This result satisfies the experiment portion of #29, not all original prerequisites.
#30 (Base ingestion), #32 (coherent snapshots), #36 (route discovery) and #37
(decision persistence) retain their original dependencies, including Solana input
and arithmetic qualification. Streaming, reconnect/backfill, persistent rollback
invalidation and current protocol qualification cannot be inferred from one Base
trace. No ticket or epic closes through this evidence record. The integrating
review is by the implementing assistant, not an independent security audit.

## Preserve future failure evidence without another request

The follow-up saves an already-fetched collection-attempt journal as
`collection-failure.json` before raising the existing terminal failure. It makes
no additional API/provider request and introduces no retry. At most six records
are accepted, original ordering and unknown/null fields are retained, and an
existing evidence file cannot be overwritten. Malformed or over-limit input
fails before writing. The existing bounded-file and credential-scan gate remains
mandatory before any artifact upload. A write failure remains a failure; a saved
diagnostic never becomes success or an accepted epic.

Seven new offline tests cover each terminal outcome, preserved context, no calls,
limits, malformed input, exclusive output and export refusal on credential-like
content. These tests do not retroactively add missing diagnostics to old runs or
prove that a historical provider failure was HTTP 429. They prevent loss of the
existing journal when a later collection fails. The successful recorded result
above keeps its original source and date; this correction does not rerun it.
