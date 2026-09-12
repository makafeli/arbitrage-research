# Implementation verification record

**Verified research foundation — 12 September 2026.** Runtime commit `c77af2e1b8156c9bac9cf290d0a950a991f357e7` passed all four jobs in [GitHub Actions run 34712934988](https://github.com/makafeli/arbitrage-research/actions/runs/34712934988). The implementation is reviewed in [PR #83](https://github.com/makafeli/arbitrage-research/pull/83); its checks identify the final documentation head as well. Requirements describe the intended product; [the capability matrix](12-IMPLEMENTATION-STATUS.md) defines what this implementation actually provides.

## Executed checks

| Check | Observed result |
|---|---|
| Rust toolchain and static checks | Exact Rust 1.90.0; formatting and all-target Clippy with warnings denied passed. |
| Complete Rust workspace | **139 passed; zero failed, ignored or filtered tests.** Includes actual PostgreSQL 17.11 execution, not a database-presence skip. |
| Durable storage/control | 9 storage and 12 control integration cases passed, covering idempotency, concurrency, corruption rejection, cancellation, recovery, fencing and unresolved attempts. |
| API/database restart contract | Actual PostgreSQL integration passed; command intent deduplicates and durable session state survives API restart. Browser authentication intentionally does not survive process restart. |
| Observation-worker process | A real child worker and loopback HTTP server passed the blocked-RPC STOP case: acknowledgement while the read remains blocked, late capture unadmitted, fresh START required before a subsequent admission. This controlled timing result is not a production latency SLA. |
| UI client/API/database interoperability | Actual frontend client against the Rust API and a separate isolated PostgreSQL database passed auth, CSRF session, capabilities, empty records, rejected configuration and logout. |
| Frontend | Production build, application/test TypeScript checks and **13 Node tests** passed. |
| Chromium browser scenarios | **20 passed**: 10 original demo scenarios and 10 Connected scenarios, including delayed acknowledgement, uncertain retries, partial stop-all rejection, stale data, evidence inspection, keyboard focus, creation and responsive layout. |
| Containers | API, observation-worker and web Docker builds passed. Caddy configuration validation passed. |
| Delivery tooling | **19 GitHub importer regression tests**, offline setup dry-run, and project validation passed: 8 epics, 68 tasks, acyclic resolved dependencies, valid document links and 15 workspace members. |
| Contract fixtures | All 11 operation IDs/local references resolved; valid fixtures passed and all 12 specified negative evidence/amount examples were rejected. The custom checker is not a complete OpenAPI certification. |

Local verification used Rust 1.91.1 extracted from verified Ubuntu packages without replacing a global toolchain. The final local cohort passed 116 non-database Rust tests, formatting and Clippy. Local database suites were explicitly excluded; the remote full-suite result above supplies the previously unavailable PostgreSQL proof. The original unchanged main baseline passed 19 Rust tests before implementation.

## Browser and visual review

The final dashboard artifact is [dashboard-validation, artifact 10303861337](https://github.com/makafeli/arbitrage-research/actions/runs/34712934988/artifacts/10303861337). Its archive SHA-256 was verified before extraction:

`6b85f3dce5b479055e0fa9990ee1e749380df984d3c87f4ab35567621533182a`

Connected screenshots were visually inspected at 320, 390 and 1440 pixels across the correction cycle. Narrow-screen session actions now use two columns and preserve whole labels. Final browser geometry checks verify single-line labels, no clipping and touch targets of at least 44 by 44 pixels. The original navy/lime visual direction, distinct Demo/Connected provenance and visible control states are retained. Screenshot data comes from controlled browser fixtures, not acquired market data. CI artifacts have a 14-day retention period; the tests reproduce the views after expiry.

Browser scenarios use a controlled API stub. The separate real client/API/database smoke test proves wire interoperability, but neither establishes a deployed Railway HTTPS/proxy environment or production browser-cookie enforcement through that environment.

## Review findings corrected

- Duplicate attempt resolution uses durable IDs and cannot clear another outstanding attempt.
- Mutating control operations close admission before awaiting PostgreSQL. A cancelled operation cannot leave admission open; recovery is required.
- Lost readiness after RUNNING records FAULTED; a later healthy poll cannot silently reopen the worker.
- Read projections validate lifecycle data against stored columns; recovery compares actual outstanding attempt rows.
- Capture replay distinguishes protocol source, original build and current replay identity and revalidates recorded configuration/registry authorization.
- Chromium exposed an incorrectly bound native `fetch` receiver; the default transport now calls through the global receiver and has a regression test.
- The unknown-cost value is a separate element from its asset label, preserving exact text and denomination. The original strict assertion remains.
- Visual inspection found narrow action labels breaking within words. Responsive layout and rendered-label tests now cover that defect.
- Historical Orca high-level quoting could loop at an exhausted finite tick window. The wrapper uses the captured price boundary, rejects partial input and validates liquidity changes before computation. All eight math regressions pass, including the retained formerly hanging case, both directions at range edges and malformed liquidity. Independent agent review found no blocker in that termination fix.

The initial [run 34711983642](https://github.com/makafeli/arbitrage-research/actions/runs/34711983642) exposed the fetch/math defects. [Run 34712579037](https://github.com/makafeli/arbitrage-research/actions/runs/34712579037) passed all 139 Rust tests, specification and container jobs, and 19/20 browser scenarios before the final presentation correction. These earlier failures remain recorded; they are superseded by the fully passing runtime checkpoint above.

## Limits of acceptance

This is an agent-assisted engineering and foundation review, not an independent security certification or profitability result. [Foundation acceptance](15-FOUNDATION-REVIEW.md) covers ARB-001/005/006 within their original establishment scope. Passing implementations do not bypass another ticket's unresolved dependencies.

No actual Railway plan or deployment ran. Region/provider RTT, volume permissions, immutable release-image digests, backup restoration and deployed TLS/proxy behavior remain acceptance work. Both initial network configurations remain disabled until their actual providers, assets and pools are qualified. The [provider checkpoint](16-PROVIDER-QUALIFICATION.md) reports transport failures without inventing provider results.

Full atomic transaction simulation, durable economic paper execution, complete route replay and comparative market campaigns remain incomplete. The restricted historical Orca primitive does not qualify the current deployed program or adaptive-fee pools. Research artifacts contain no signing or transaction broadcasting capability; nothing in these checks approves funding or live activation.
