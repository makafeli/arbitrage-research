# Implementation verification record

Date: 12 September 2026. Scope: the parallel implementation on `feat/research-foundations`. GitHub CI and the tested commit are recorded below when available. The earlier validation document records the original scaffold and is retained as historical evidence.

## Checks already executed

- Local environment: Rust 1.91.1 extracted from verified Ubuntu package artifacts; no global toolchain replacement. Repository and CI target remain Rust 1.90.0.
- Initial unchanged main: 19 Rust tests, formatting and Clippy passed locally before implementation.
- Final local cohort: all-target Clippy and formatting passed; 116 non-PostgreSQL Rust tests passed with zero failures or ignored tests. The storage/control database suites were explicitly excluded and two other database-dependent tests explicitly filtered. Remote full-suite evidence below supersedes these local exclusions.
- Frontend: production build, application/test type checks and 13 Node tests passed after Connected mode and neutral mode-aware metadata were implemented.
- Contract fixtures: all 11 OpenAPI operation IDs and local references resolved; valid fixtures passed and all 12 specified negative evidence/amount examples were rejected. This custom checker is not a complete OpenAPI certification.
- Structural validation: 15 Cargo workspace members, 8 epics, 68 tasks, acyclic dependencies, local document links and implementation-progress acceptance rules passed.
- GitHub bootstrap: 19 regression tests passed after the supplemental role/release-gate/dependency Project-field enhancement.

## Required remote checks

GitHub Actions provides a real isolated PostgreSQL 17.11 service. The full Rust suite must execute database/control/worker-process integration tests and the real UI API client smoke test. PostgreSQL could not run locally under the authoring container's UID restrictions; local database exclusions are explicit and do not establish durability.

The browser suite contains 20 scenarios: 10 preserved demo scenarios and 10 Connected scenarios. Local Chromium was unavailable and one bounded download attempt timed out. CI must execute them and preserve traces/screenshots. Frontend unit mocks alone do not prove browser cookie enforcement or layout.

CI also builds the prepared API, observation-worker and web images, and validates Caddy configuration. Those checks do not establish an actual Railway deployment, provider access, volume permissions or backup recovery. The Railway IaC file is authored from the official reference and statically reviewed; an authenticated Railway plan has not run.

## Peer-review findings addressed

- Duplicate research-attempt resolution now uses durable attempt IDs and cannot clear a second outstanding attempt.
- Cancelled database mutations leave local admission fenced until recovery; a pre-commit callback cannot open the gate.
- Loss of readiness after RUNNING faults the worker; a later positive poll cannot silently reopen it.
- Read projections validate lifecycle data against stored status columns, and recovery reconciles actual outstanding attempt rows.
- Capture verification distinguishes protocol source revision, original build identity and replay build identity; recorded configuration and registry constraints are revalidated.
- Demo/Connected browser metadata is explicit. Static proxy review found no Origin, cookie, route-prefix or CSP mismatch; end-to-end deployment proof remains separate.

These are author/peer engineering checks. They are not an independent security certification, production protocol qualification or evidence of profitable arbitrage.

## Final CI result

Initial implementation commit `9e9845f95bd5a5a199f695717efa52fa7c074c51` is under review in [PR #83](https://github.com/makafeli/arbitrage-research/pull/83). [Initial CI run](https://github.com/makafeli/arbitrage-research/actions/runs/34711983642) passed specification and container jobs. All 10 demo browser scenarios passed; the 10 Connected scenarios failed and are being corrected. The expanded Rust suite exposed a nonterminating Orca boundary case. The browser failure was traced to a native `fetch` receiver mismatch; the transport wrapper is corrected and a regression test passes. The math wrapper now bounds the SDK swap to captured tick coverage, rejects partial consumption and validates liquidity transitions. All 8 math regressions now pass in 0.02 seconds, including the retained formerly hanging case, both directions and malformed liquidity. All 4 strict capture replay regressions also pass. A corrected commit must pass the full remote suite before merge; no full-suite/database pass is claimed from the initial run.

## Verified remote backend and container checkpoint

Commit `aadca0c20c373a71b9b2c81eb26d96e93c06aed6` passed the **Rust**, **specifications** and **containers** jobs in [run 34712579037](https://github.com/makafeli/arbitrage-research/actions/runs/34712579037).

- Rust **1.90.0** formatting and all-target Clippy with warnings denied passed.
- The complete workspace ran **139 Rust tests: zero failures, ignored tests or filtered tests**. PostgreSQL **17.11** actually ran, including 12 control cases, 9 storage cases, the API restart/idempotency contract, and the real observation-worker process test.
- The worker-process test acknowledged STOP while its RPC request was still blocked, retained the late capture as unadmitted evidence, then required a fresh START before admission. This is a controlled integration result, not a production latency SLA.
- The UI's actual API client interoperated with a running Rust API and a second isolated PostgreSQL database: authentication, CSRF session, capabilities, empty records, configuration rejection and logout all passed. That Node-client check does not independently prove browser TLS/cookie enforcement.
- The API, observation-worker and web Docker images built successfully; Caddy configuration validation passed.
- Project/schema/fixture checks and all **19 GitHub importer regression tests** passed.
- Frontend build/type checking and **13 Node tests** passed. **19 of 20 browser scenarios** passed. The remaining exact-text lookup combined the unknown-cost value with the asset label; the value is now a separate element and the original assertion is retained.

Visual review of the saved Connected screenshots at 320, 390 and 1440 pixels identified word wrapping inside narrow action buttons. The follow-up presentation fix uses a mobile two-column action layout and keeps labels intact. Browser layout assertions accompany that correction. The screenshot data is controlled browser-fixture data, not acquired market evidence.

The backend source has not changed after this successful backend checkpoint. Final browser evidence and the current merge gate are available in [PR #83 checks](https://github.com/makafeli/arbitrage-research/pull/83/checks); all jobs for its final head must pass before merge. A passing CI job remains distinct from Railway deployment, provider qualification, full economic simulation and external review.
