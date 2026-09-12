# Delivery goal

Build the Rust research platform described by the [PRD](docs/01-PRD.md), beginning with Base and Solana observation, reproducible replay and paper experiments, and connect the approved dashboard to authenticated services. Deliver the [68 specified implementation tickets](planning/TICKETS.md) with verifiable acceptance evidence.

The user authorized parallel implementation and GitHub delivery on 12 September 2026. This document persists the goal and the work state. It does not activate an unattended `/goal` service or imply that work continues after a tool session ends.

## Parallel ownership

| Workstream | Owned components | Initial tickets |
|---|---|---|
| Domain and configuration | Exact amounts, identities, evidence, immutable configuration | ARB-008, ARB-009 |
| Storage and control | PostgreSQL migrations, scoped sessions, idempotency, worker fences | ARB-010, ARB-011 |
| Control API | Authentication, CSRF, session/command endpoints, capabilities | ARB-012 |
| Acquisition | Read-only Base/Solana adapters, capture integrity and provenance | ARB-014, ARB-016, ARB-017, ARB-018 |
| Dashboard | Explicit demo/connected modes, server data, command acknowledgements | ARB-037, ARB-038, ARB-039 |
| Integration | Workspace, CI, cross-component review and acceptance evidence | ARB-007, subsequent integration gates |

Root integration owns shared manifests, lockfiles, CI, publication and final acceptance. Component authors coordinate interfaces without sharing the git index. Dependency ordering governs integration even when implementation proceeds in parallel.

## Acceptance and resumption

[Implementation progress](planning/implementation-progress.json) maps every ticket to its native issue, dependencies, evidence and remaining acceptance. [Capability status](docs/12-IMPLEMENTATION-STATUS.md) describes what the checked-in application actually does. The original backlog remains the acceptance contract.

1. Read this file, implementation progress and the latest CI result before resuming.
2. Implement independent components in parallel; integrate only compatible, reviewed interfaces.
3. Run the checks that establish each claimed behavior. PostgreSQL concurrency and restart tests require a real PostgreSQL service; omitted prerequisites are not passing tests.
4. Attach the tested commit and relevant evidence to the ticket. Close it only when its complete acceptance criteria and applicable dependency gates are satisfied.
5. Record incomplete criteria and external prerequisites explicitly. Continue independent work while awaiting them.

The first usable milestone is a captured, exact, reproducible observation route that appears in the connected dashboard and has a durable worker stop acknowledgement. Full atomic transaction simulation and virtual accounting are additional gates for executable paper research. A quote or metadata snapshot alone is not that milestone.

## External prerequisites

Provider accounts, spending limits, actual pool qualification and RPC spending limits have not been supplied. Railway is the selected deployment platform, confirmed by the user on 12 September 2026; project/environment identities and credentials are not connected here. Development proceeds locally and in CI without purchasing services. Credentials belong in the deployment secret mechanism, never tickets, browser storage or committed configuration.

Actual observation campaigns require elapsed market data; comparative profitability claims require those results and complete costs. Independent review, release approval, funded live pilots and account operations retain their specified gates. No signing or broadcasting capability is included in the research build. Describing later live tickets does not approve a deployment or trade.

The native owner Project and native Wiki still require GitHub capabilities unavailable to the connected tool interface. Their definitions and source remain in the repository; the [setup status](docs/13-GITHUB-SETUP-STATUS.md) records this separately from the published issues and milestones.
