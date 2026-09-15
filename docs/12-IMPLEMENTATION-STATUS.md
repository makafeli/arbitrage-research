# Implementation status and capability boundary

Updated 15 September 2026 against main `ff61b29e75fe4f368eab521a1c9dfdd8c2d90cb8`. The [goal](../GOAL.md) records the continuing delivery objective; [implementation progress](../planning/implementation-progress.json) maps all 68 tasks to native issues and acceptance dependencies. [Platform acceptance](EPIC-02-ACCEPTANCE.md), [pipeline measurements](PIPELINE-MEASUREMENTS.md) and [repository reconciliation](REPOSITORY-HYGIENE.md) identify the current source/test/merge evidence. The [parallel boundary verification](30-PARALLEL-BOUNDARY-VERIFICATION.md) retains the earlier work-wave evidence. Requirements elsewhere describe the target product, and must not be read as completed capabilities.

| Component | Implemented source behavior | Remaining boundary |
|---|---|---|
| Domain/configuration | Accepted exact domain/configuration contracts; immutable mode/network/starting-asset bindings, canonical hashes and secret-redacted validation; both input and effective snapshots bounded for reloadability | Initial identities are accepted through EPIC-01, but identity catalogues are deliberately not runtime registries; complete quote/execution readiness and settlement truth remain separate |
| PostgreSQL/control | Accepted journal and lifecycle contracts: immutable sessions/configurations, scoped idempotency, revision conflicts, append-only audit, worker leases/generation fences and process-loss recovery to STOPPED | Real disposable PostgreSQL/subprocess tests do not certify production backup/restore, a signing lease or live transport |
| API | Accepted authenticated control/error contract; bounded reads reserve control admission, with Origin/CSRF, scoped idempotency and generated request timing/correlation headers; persisted research/ledger queries | Single operator; HTTP acceptance remains PENDING until worker ACK; read budgets are not a guarantee against every shared-database/host failure; no settlement or signing endpoint |
| Dashboard | Approved demo plus Connected sessions, decision inspection, paper-ledger views, validated opportunity evidence and source-bound local capture-audit import with exact integer amounts | A local imported audit is a caller-supplied filesystem observation, not proof of market/replay eligibility; original shell #50 is accepted, while comprehensive accessibility certification and qualified recorded capture-to-browser acceptance remain separate |
| Capture | Accepted versioned manifest/provenance contract and export-bound dependency audit; Base hash-pinned state/tick reads with final-header comparison and Orca account/context decoding are implemented | Initial identities and Base access are demonstrated, not full runtime/provider/quote-range qualification; streaming, rollback invalidation and the genuine recorded decision trace remain incomplete |
| Research worker | OBSERVE/PAPER recovery, independent control polling, common-anchor batches of up to eight pools, generation-fenced capture and decision admission, bounded scheduler integration | Sequential per-network research collection; no paper settlement, full transaction simulation or host-wide resource guarantee |
| Scheduler | Bounded per-chain queues/permits, fixed CPU admission, fair dispatch and typed stale/drop reasons; 96-bin queue/finish histograms and optional bounded export; actual RPC, decode, evaluation and persistence measurements plus API timing | PR #127's simultaneous dual-worker experiment is not yet passing; full simulation is explicitly NOT_IMPLEMENTED; observed CI/loopback profiles are not production capacity or an SLA |
| Protocol mathematics | Base V3 exact integer traversal with MIT SDK reference comparison; pinned Apache-licensed historical Orca static-fee math | Current deployed-protocol equivalence, real provider/range qualification and unsupported token/fee models remain gated |
| Paper accounting | Immutable PostgreSQL runs, exact virtual principal/native-fee balances, reservations, idempotent journal and restart reconstruction | Settlement commands are internal research primitives; no automatic quote-to-fill pipeline or complete atomic simulation |
| Research diagnostics/export | Durable pre-I/O attempts, typed failures, bounded frozen session JSON/CSV snapshots and redacted accounting; source-bound local raw-availability/expiry audit; descriptive comparison and structural holdout checks | Attempts do not prove schedule completeness; capture audits do not establish successful replay; asset decimals, full alerts, calibrated comparison and campaign acceptance remain open |
| Hypothetical cost assessments | Immutable manually declared scenarios bound to stored QUOTED decisions, exact native valuation, replayed reports, scoped API/history and export | No automatic fee acquisition, complete transaction simulation, paper settlement, refundable-rent modeling or measured economic result |
| Replay | Legacy and full-batch-v2 capture re-decoding and bounded same-engine research evaluation with explicit historical age and frozen configuration | No paper-outcome replay, seeded inclusion simulation or comparable market profitability report |
| Recovery | Quiesced database/capture backup, verified isolated restore, descriptor binding, 41 offline regressions and a real PostgreSQL storage drill | Production scheduling, encrypted offsite retention, application-aware paper/replay reconstruction remain open; actual disposable worker process-loss/restart-to-STOPPED proof is already part of accepted #24/#25 |
| Railway | Existing web/private API/PostgreSQL foundation deployed and documented in [deployment verification](21-RAILWAY-DEPLOYMENT-VERIFICATION.md); containers, same-origin proxy and CI gate | No fresh deployment/health verification for the latest source; qualified workers, provider credentials, measured capacity and full operational recovery acceptance remain open |

## Accepted original scope and current unfinished work

Fourteen of the 68 original implementation tasks are accepted. EPIC-01 is closed;
EPIC-02 has seven accepted children, with #27 and #29 still open. The dashboard
shell #50 is also accepted. Other implemented features keep their own original
acceptance gates; these figures are task states, not a product-completion percentage.

Main already contains PR #125 (platform contracts/read isolation) and PR #126
(actual pipeline telemetry and status synchronization). PR #127 adds the remaining
simultaneous Base/Solana isolation tests but currently fails before process
execution because its test configuration has fewer than the required distinct
pools/assets. This diagnostic is not a waiver of those validation requirements.
#29 still requires the original genuine recorded trace and M2 predecessors.

## Initial identities and access

The owner-triggered Base workflow34892533716 succeeded. The historical HTTP403
observations below remain provenance, not a current claim that Base is inaccessible.
The [reviewed inventory](registries/initial-identities.json) identifies two actual
pools on each original chain and is deliberately rejected as runtime configuration.
See [EPIC-01 criterion review](EPIC-01-HANDOFF.md) for the identity acceptance scope,
raw-evidence limitations, authority risks and unchanged downstream execution gates.

## Operator controls

Session mode is immutable. New sessions begin RECOVERING; a worker reconciles durable state before STOPPED. API acceptance yields PENDING, and only the worker's committed result yields APPLIED. STOP fences admission and may leave DRAINING while old attempts are unresolved. Duplicate resolution is idempotent by attempt ID. Cancelled database operations leave the local gate closed until recovery; a lost readiness condition faults the worker instead of silently reopening it.

The observation worker may continue collecting bounded raw input while stopped or paused to establish readiness. Such files are unadmitted raw evidence unless the current running generation admits them durably. Already started network requests cannot be recalled. Stopping Railway infrastructure is not an APPLIED operator STOP. Independent capture CLI processes are not controlled by the dashboard.

## Validation and claims

The historical 14 September boundary fixes were locally tested with Rust 1.90.0, matching pinned CI. Earlier records retain their own toolchain and environment limitations. Local focused suites do not stand in for the mandatory real PostgreSQL/HTTP and Chromium checks, which were run in GitHub Actions for the published source. A missing or filtered prerequisite is never a passing integration test. Each record identifies its exact head and base; later merges require combined validation.

The first-round checkpoint is in the [foundation verification record](14-IMPLEMENTATION-VERIFICATION.md), including 139 passing Rust tests with actual PostgreSQL and a UI-client/API/database check. The [research integration record](17-RESEARCH-INTEGRATION-VERIFICATION.md) records the accepted second round. The [collection/export record](18-COLLECTION-AND-EXPORT-VERIFICATION.md) tracks subsequent changes and their tested commit separately; authored tests are not evidence of a passing run. Peer reviews found and corrected cancellation, duplicate resolution, readiness-loss and persisted-state consistency defects. These reviews are engineering evidence and are not an independent security audit.

## Gates still ahead

The first complete observation milestone requires qualified production pool identities, coherent state, exact route evaluation, a persisted CANDIDATE visible in the dashboard, durable control and replay evidence. The captured decision pipeline is progress toward that milestone; synthetic and manually constructed integration cases do not satisfy production qualification. M3 additionally requires a full intended atomic transaction simulation per chain under declared virtual funding, correct durable accounting and inclusion scenarios.

No research process signs or broadcasts transactions. The uploaded untrusted script is excluded. Live keys, deployment of execution contracts/programs, funding, real trading, independent review and pilot approval retain their separate backlog gates. No source file, passing arithmetic test or synthetic screenshot establishes profitability.

The [common-anchor and cost-assessment record](19-ANCHORED-CAPTURE-AND-COST-VERIFICATION.md) describes its historical implementation cohort, backward replay compatibility, original-input retention, manual-cost provenance and its remaining acceptance gates.

The [chain-time and support cohort](20-CHAIN-TIME-AND-SUPPORT-VERIFICATION.md) adds optional immutable finalized-chain-time policy, versioned historical reports and authenticated startup adapter diagnostics. Legacy behavior remains explicit when no policy is configured. Full rollback history, provider qualification, calibrated limits and actual operational alerts remain open.
