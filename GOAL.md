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

## Current delivery checkpoint

PR [#92](https://github.com/makafeli/arbitrage-research/pull/92) adds common-anchor pool-set acquisition, original-batch replay and immutable manual cost assessments to the collection/export baseline from PR [#90](https://github.com/makafeli/arbitrage-research/pull/90). The platform connects same-network captured route evaluation, durable decision queries, offline economic replay and immutable virtual accounts to the dashboard. Base V3 and Solana Whirlpool calculations use exact amounts; unsupported state and inconsistent capture contexts fail closed. Both OBSERVE and PAPER workers recover stopped and require an explicit START. The paper ledger supports reservations and reconciled outcomes internally, but the worker does not automatically convert quotes into virtual fills.

[Common-anchor and cost verification](docs/19-ANCHORED-CAPTURE-AND-COST-VERIFICATION.md) records the reviewed source, original-input compatibility, exact manual-cost reproduction and visual evidence. The [collection/export record](docs/18-COLLECTION-AND-EXPORT-VERIFICATION.md) retains the preceding baseline. Match the latest record's tested commit to the branch before claiming acceptance.

The next usable-market milestone still requires qualified providers/pools, chain-lag evidence and durable rollback/gap invalidation. Complete atomic transaction simulation, explicit costs/inclusion assumptions and scenario-driven paper settlement remain necessary for executable paper research. A gross route quote or virtual opening balance establishes neither profitable arbitrage nor a simulated complete transaction.

## External prerequisites

Provider accounts, spending limits, actual pool qualification and RPC spending limits have not been supplied. Railway is the selected deployment platform, confirmed by the user on 12 September 2026; project/environment identities and credentials are not connected here. Development proceeds locally and in CI without purchasing services. Credentials belong in the deployment secret mechanism, never tickets, browser storage or committed configuration.

Actual observation campaigns require elapsed market data; comparative profitability claims require those results and complete costs. Independent review, release approval, funded live pilots and account operations retain their specified gates. No signing or broadcasting capability is included in the research build. Describing later live tickets does not approve a deployment or trade.

The native owner Project and native Wiki still require GitHub capabilities unavailable to the connected tool interface. Their definitions and source remain in the repository; the [setup status](docs/13-GITHUB-SETUP-STATUS.md) records this separately from the published issues and milestones.
