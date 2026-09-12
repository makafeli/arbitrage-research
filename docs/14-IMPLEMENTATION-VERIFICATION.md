# Implementation verification record

Date: 12 September 2026. Scope: the parallel implementation on `feat/research-foundations`. GitHub CI and the tested commit are recorded below when available. The earlier validation document records the original scaffold and is retained as historical evidence.

## Checks already executed

- Local environment: Rust 1.91.1 extracted from verified Ubuntu package artifacts; no global toolchain replacement. Repository and CI target remain Rust 1.90.0.
- Initial unchanged main: 19 Rust tests, formatting and Clippy passed locally before implementation.
- Intermediate integrated cohort: all-target workspace compilation passed; 95 tests excluding database suites passed before later additions. The final counts supersede that intermediate count when recorded.
- Frontend: production build, application/test type checks and 12 Node tests passed after Connected mode and neutral mode-aware metadata were implemented.
- Contract fixtures: all 11 OpenAPI operation IDs and local references resolved; valid fixtures passed and all 12 specified negative evidence/amount examples were rejected. This custom checker is not a complete OpenAPI certification.
- Structural validation: 15 Cargo workspace members, 8 epics, 68 tasks, acyclic dependencies, local document links and implementation-progress acceptance rules passed.
- GitHub bootstrap: 12 regression tests passed before the supplemental project-field enhancement; rerun evidence is required after that change.

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

Pending publication and execution for this implementation commit. No database, browser or container pass is claimed until the run below has completed successfully.
