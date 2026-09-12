# Implementation status and capability boundary

Updated 12 September 2026 during the parallel implementation. The [goal](../GOAL.md) records the continuing delivery objective; [implementation progress](../planning/implementation-progress.json) maps all 68 tasks to native issues and acceptance dependencies. Requirements elsewhere describe the target product, and must not be read as completed capabilities.

| Component | Implemented source behavior | Remaining boundary |
|---|---|---|
| Domain/configuration | Exact integer amounts, signed P&L, identities/routes, declared evidence validation, immutable canonical configuration, secret references | Declared consistency is not proof that provider state or settlement is true; production registry qualification remains required |
| PostgreSQL/control | Immutable sessions/configurations, scoped idempotency, revision conflicts, append-only audit, actual attempt IDs, worker leases/generations, cancellation-safe local gates, recovery validation | Database/process integration passed in the linked verification record; no deployed backup/restore proof, signing lease or live transport |
| API | Authenticated sessions/commands, persisted decisions/opportunities/groups/coverage, immutable virtual-run creation and ledger reads; Origin/CSRF and bounded requests | Single operator; browser-auth state is not durable across process restart; no settlement or signing endpoint |
| Dashboard | Approved demo plus Connected sessions, decision inspection and paper-ledger views with explicit origin and exact integer amounts | Review and browser evidence for the new views are recorded separately; market collection and report completeness remain unqualified |
| Capture | Base hash-pinned state and tick reads, Orca account/tick-array decoding, bounded read-only RPC, immutable manifests, content hashes and provenance | Manually constructed fixtures do not qualify production providers; streaming, rollback invalidation and full range qualification remain incomplete |
| Research worker | OBSERVE/PAPER recovery, independent control polling, batches of up to eight pools, generation-fenced capture and decision admission, bounded scheduler integration | Sequential per-network research collection; no paper settlement, full transaction simulation or host-wide resource guarantee |
| Scheduler | Bounded per-chain queues/permits, shared cap, fair dispatch, stale/stop rejection and telemetry counters | Not yet the complete measured CPU worker pool and metrics backend |
| Protocol mathematics | Base V3 exact integer traversal with MIT SDK reference comparison; pinned Apache-licensed historical Orca static-fee math | Current deployed-protocol equivalence, real provider/range qualification and unsupported token/fee models remain gated |
| Paper accounting | Immutable PostgreSQL runs, exact virtual principal/native-fee balances, reservations, idempotent journal and restart reconstruction | Settlement commands are internal research primitives; no automatic quote-to-fill pipeline or complete atomic simulation |
| Replay | Offline capture re-decoding and bounded same-engine research evaluation with explicit historical age and frozen configuration | No paper-outcome replay, seeded inclusion simulation or comparable market profitability report |
| Railway | Container definitions, same-origin Caddy proxy, private API, IaC foundation and deployment/volume runbook | No authenticated Railway plan/deployment, host measurements, provider credentials or backup-restore proof |

## Operator controls

Session mode is immutable. New sessions begin RECOVERING; a worker reconciles durable state before STOPPED. API acceptance yields PENDING, and only the worker's committed result yields APPLIED. STOP fences admission and may leave DRAINING while old attempts are unresolved. Duplicate resolution is idempotent by attempt ID. Cancelled database operations leave the local gate closed until recovery; a lost readiness condition faults the worker instead of silently reopening it.

The observation worker may continue collecting bounded raw input while stopped or paused to establish readiness. Such files are unadmitted raw evidence unless the current running generation admits them durably. Already started network requests cannot be recalled. Stopping Railway infrastructure is not an APPLIED operator STOP. Independent capture CLI processes are not controlled by the dashboard.

## Validation and claims

Local Rust validation uses the extracted Rust 1.91.1 toolchain; CI remains pinned to Rust 1.90.0. The local environment cannot run PostgreSQL or the matching Chromium binary. Mandatory PostgreSQL suites and browser tests therefore run in GitHub Actions. Local commands that filter database tests report that exclusion explicitly; absence of a database must never be counted as a passing integration test.

The first-round checkpoint is in the [foundation verification record](14-IMPLEMENTATION-VERIFICATION.md), including 139 passing Rust tests with actual PostgreSQL and a UI-client/API/database check. The [research integration record](17-RESEARCH-INTEGRATION-VERIFICATION.md) tracks the second round separately; authored tests are not evidence of a passing run. Peer reviews found and corrected cancellation, duplicate resolution, readiness-loss and persisted-state consistency defects. These reviews are engineering evidence and are not an independent security audit.

## Gates still ahead

The first complete observation milestone requires qualified production pool identities, coherent state, exact route evaluation, a persisted CANDIDATE visible in the dashboard, durable control and replay evidence. The captured decision pipeline is progress toward that milestone; synthetic and manually constructed integration cases do not satisfy production qualification. M3 additionally requires a full intended atomic transaction simulation per chain under declared virtual funding, correct durable accounting and inclusion scenarios.

No research process signs or broadcasts transactions. The uploaded untrusted script is excluded. Live keys, deployment of execution contracts/programs, funding, real trading, independent review and pilot approval retain their separate backlog gates. No source file, passing arithmetic test or synthetic screenshot establishes profitability.
