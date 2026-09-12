# Implementation status and capability boundary

Updated 12 September 2026 during the parallel implementation. The [goal](../GOAL.md) records the continuing delivery objective; [implementation progress](../planning/implementation-progress.json) maps all 68 tasks to native issues and acceptance dependencies. Requirements elsewhere describe the target product, and must not be read as completed capabilities.

| Component | Implemented source behavior | Remaining boundary |
|---|---|---|
| Domain/configuration | Exact integer amounts, signed P&L, identities/routes, declared evidence validation, immutable canonical configuration, secret references | Declared consistency is not proof that provider state or settlement is true; production registry qualification remains required |
| PostgreSQL/control | Immutable sessions/configurations, scoped idempotency, revision conflicts, append-only audit, actual attempt IDs, worker leases/generations, cancellation-safe local gates, recovery validation | Database/process integration requires the recorded CI result; no signing lease or live transport |
| API | Authenticated `/v1` sessions/commands/capabilities, secure cookies, Origin/CSRF, rate/body/concurrency/deadline limits, redacted errors | No opportunity production/query integration, multi-operator service or durable browser-auth store |
| Dashboard | Preserved approved demo plus explicit Connected mode, real API client, auth, creation, per-session receipts, stale/error/retry states and evidence inspector | Paper ledger/report views and real captured opportunities remain integration work; exact browser evidence belongs to CI |
| Capture | Base hash-pinned state and tick reads, Orca account/tick-array decoding, bounded read-only RPC, immutable manifests, content hashes and provenance | Manually constructed fixtures do not qualify production providers; streaming, rollback invalidation and full range qualification remain incomplete |
| Observation worker | OBSERVE-only process, independent control polling, one bounded capture, generation-fenced durable capture admission and fail-closed recovery | Raw captures are not opportunities, paper fills or full transaction simulations |
| Scheduler | Bounded per-chain queues/permits, shared cap, fair dispatch, stale/stop rejection and telemetry counters | Not yet the complete measured CPU worker pool and metrics backend |
| Orca mathematics | Pinned Apache-licensed historical static-fee exact-input primitive and two-leg fixture composition | Adaptive fees/unsupported arrays rejected; current program equivalence, complete captured-state integration and Base exact math remain unqualified |
| Paper core | Exact costs, timestamped valuation, separate native-fee/principal reservations, idempotent hypothetical outcomes, paired journal entries and deterministic journal replay | In-memory accounting only; PostgreSQL durability, complete simulation and dashboard integration remain incomplete |
| Replay | Offline re-decoding of capture transcripts, exact normalized snapshot comparison and configuration/provenance validation | No complete route/economic replay, seeded inclusion simulation or comparable market report |
| Railway | Container definitions, same-origin Caddy proxy, private API, IaC foundation and deployment/volume runbook | No authenticated Railway plan/deployment, host measurements, provider credentials or backup-restore proof |

## Operator controls

Session mode is immutable. New sessions begin RECOVERING; a worker reconciles durable state before STOPPED. API acceptance yields PENDING, and only the worker's committed result yields APPLIED. STOP fences admission and may leave DRAINING while old attempts are unresolved. Duplicate resolution is idempotent by attempt ID. Cancelled database operations leave the local gate closed until recovery; a lost readiness condition faults the worker instead of silently reopening it.

The observation worker may continue collecting bounded raw input while stopped or paused to establish readiness. Such files are unadmitted raw evidence unless the current running generation admits them durably. Already started network requests cannot be recalled. Stopping Railway infrastructure is not an APPLIED operator STOP. Independent capture CLI processes are not controlled by the dashboard.

## Validation and claims

Local Rust validation uses the extracted Rust 1.91.1 toolchain; CI remains pinned to Rust 1.90.0. The local environment cannot run PostgreSQL or the matching Chromium binary. Mandatory PostgreSQL suites and browser tests therefore run in GitHub Actions. Local commands that filter database tests report that exclusion explicitly; absence of a database must never be counted as a passing integration test.

The exact tested commit, check totals and limitations belong in the implementation verification record. Peer reviews found and corrected cancellation, duplicate resolution, readiness-loss and persisted-state consistency defects. These reviews are engineering evidence and are not an independent security audit.

## Gates still ahead

The first complete observation milestone requires qualified production pool identities, coherent state, exact route evaluation, a persisted CANDIDATE visible in the dashboard, durable control and replay evidence. The current capture and control implementation is progress toward that milestone. M3 additionally requires a full intended atomic transaction simulation per chain under declared virtual funding, correct durable accounting and inclusion scenarios.

No research process signs or broadcasts transactions. The uploaded untrusted script is excluded. Live keys, deployment of execution contracts/programs, funding, real trading, independent review and pilot approval retain their separate backlog gates. No source file, passing arithmetic test or synthetic screenshot establishes profitability.
