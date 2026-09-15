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

### EPIC-02: seven original tasks accepted

Configuration #23 and the durable journal #24 closed on 14 September; worker
lifecycle #25, authenticated API #26 and capture provenance #28 closed on
15 September after the reviewed PR #125 integration and native closeout checks.
Together with the earlier #20/#22 acceptances, seven of nine epic tasks are done.
[The acceptance record](docs/EPIC-02-ACCEPTANCE.md) preserves exact source, CI and
chronology. #27 real-stage telemetry/isolation and #29 recorded integration remain
open, including their original M2 prerequisites. The epic is not yet complete.

### EPIC-01 completed

EPIC-01 / #2 and ARB-003 / #16 are closed with verified original criteria,
merged source and dependency-aware closeout evidence. All six foundation
children are accepted. [The completion record](docs/EPIC-01-HANDOFF.md) links the
actual source, tests and native states. The next delivery unit is the existing
platform/control epic; no campaign or live capability is enabled by this closure.

### Initial identity evidence

The owner-supplied Base run34892533716 succeeded with27 read-only requests on
14 September 2026. The former access blocker is resolved for this sample.
The [reviewed identity inventory](docs/registries/initial-identities.json) combines
two Base and two Solana candidates with exact source/context provenance.
[EPIC-01 acceptance](docs/EPIC-01-HANDOFF.md) maps the original criteria and
remaining downstream limits; native #16/#2 state records actual closure.
This is an identity catalogue, not an enabled runtime registry or campaign.

### Historical implementation checkpoint

The 14 September continuation reconciled main at
`b24ed4c4a9d76aadbdef1522f2c6dc1858e2191a` before dispatch. That baseline includes
the capture audit/export work (#105/#109), descriptive comparisons (#106),
foundation evidence and four original closeouts (#110/#111), and initial
registry observations (#112). Seven of the original 68 tickets are accepted;
merged implementation slices do not automatically accept the other 61.

Five actual ChatGPT collaboration children ran concurrently with distinct
acknowledged branches/worktrees/private indices and then performed cross-reviews.
Their configuration, Base, Solana, financial and dashboard results are recorded
in [parallel boundary verification](docs/30-PARALLEL-BOUNDARY-VERIFICATION.md),
including precise PR/head/base/CI evidence and remaining requirements. Planner
capacity is separate from this observed execution. The native custom-role Codex
launcher/cloud-environment acceptance in #107 remains a different, unproven gate;
do not treat its historical zero-worker setup status as this session's result.

Recovery baseline: PR [#104](https://github.com/makafeli/arbitrage-research/pull/104),
`94d045cebcd0c4087302c2faefdf13913300c698` (13 September 2026). The quiesced backup,
verified isolated restore and final-descriptor binding are delivered; bounded #103
was closed after review, merge and successful main checks. [Main project CI
34772210835](https://github.com/makafeli/arbitrage-research/actions/runs/34772210835),
[recovery 34772210829](https://github.com/makafeli/arbitrage-research/actions/runs/34772210829)
and [delivery review 34772210834](https://github.com/makafeli/arbitrage-research/actions/runs/34772210834)
passed. The recovery suite has 41 offline cases plus a real PostgreSQL storage drill.
This does not accept original #58's production/application-aware recovery gates.

The Railway web/API/PostgreSQL foundation was already deployed through PR #95;
[deployment verification](docs/21-RAILWAY-DEPLOYMENT-VERIFICATION.md) identifies it.
PRs #94, #99 and #102 subsequently delivered evidence and authentication/export fixes.
No fresh Railway rollout/health probe is claimed for this source checkpoint.

The merged [local capture dependency audit](docs/23-CAPTURE-DEPENDENCY-AUDIT.md)
and [frozen-export binding](docs/25-EXPORT-CAPTURE-AUDIT.md) separate byte
integrity, expiry and declared completeness without rewriting historical results
or asserting replay/market eligibility. Original ARB-014/#28 and export/recovery
release gates remain open. Initial registry evidence includes two observed
Solana pool identities, but Base returned HTTP403 before state verification;
[the registry record](docs/27-INITIAL-REGISTRY-VERIFICATION.md) keeps observations,
code corrections and actual qualification distinct.

### Historical chain-time checkpoint

PR [#93](https://github.com/makafeli/arbitrage-research/pull/93) was merged on 13 September 2026 at `be061b7078e063c98f33b79384506c1b48961370`, adding optional captured-chain-time assessment and authenticated adapter diagnostics. Its exact final head `421a516d2172a3f27938754f8a82b71dd039c22f` and the merge commit both passed all four CI jobs; six retained screenshots were reviewed. The [main validation run](https://github.com/makafeli/arbitrage-research/actions/runs/34752684136) records the accepted baseline for this continuation: 294 Rust tests including 61 PostgreSQL tests, 52 Chromium scenarios and 39 Node tests.

The preceding PR [#92](https://github.com/makafeli/arbitrage-research/pull/92) adds common-anchor pool-set acquisition, original-batch replay and immutable manual cost assessments to the collection/export baseline from PR [#90](https://github.com/makafeli/arbitrage-research/pull/90). The platform connects same-network captured route evaluation, durable decision queries, offline economic replay and immutable virtual accounts to the dashboard. Base V3 and Solana Whirlpool calculations use exact amounts; unsupported state and inconsistent capture contexts fail closed. Both OBSERVE and PAPER workers recover stopped and require an explicit START. The paper ledger supports reservations and reconciled outcomes internally, but the worker does not automatically convert quotes into virtual fills.

[Common-anchor and cost verification](docs/19-ANCHORED-CAPTURE-AND-COST-VERIFICATION.md) records the reviewed source, original-input compatibility, exact manual-cost replay and visual evidence. The [collection/export record](docs/18-COLLECTION-AND-EXPORT-VERIFICATION.md) retains the preceding baseline. Match the latest record's tested commit to the branch before claiming acceptance.

The [chain-time and support cohort](docs/20-CHAIN-TIME-AND-SUPPORT-VERIFICATION.md) adds opt-in captured-time assessment and explicit operator capability diagnostics. The next usable-market milestone still requires qualified providers/pools, calibrated chain-lag evidence and durable rollback/gap invalidation. Complete atomic transaction simulation, explicit costs/inclusion assumptions and scenario-driven paper settlement remain necessary for executable paper research. A gross route quote or virtual opening balance establishes neither profitable arbitrage nor a simulated complete transaction.

## External prerequisites

The owner supplied an existing Base endpoint through ARB_BASE_RPC_URL; workflow34892533716 verified read-only access. The reviewed identity inventory addresses the original ARB-003 identity scope. Sustained provider quotas, additional paid spending approval, size-specific quote qualification and actual campaign readiness remain separate requirements. Railway is the selected deployment platform, confirmed by the user on 12 September 2026. The existing production project/environment is identified in docs/21-RAILWAY-DEPLOYMENT-VERIFICATION.md; do not ask to provision it again. This continuation has no authenticated Railway administrative connection and does not infer a new deployment from green GitHub CI. Development proceeds locally and in CI without purchasing services. Credentials belong in the deployment secret mechanism, never tickets, browser storage or committed configuration.

Actual observation campaigns require elapsed market data; comparative profitability claims require those results and complete costs. Independent review, release approval, funded live pilots and account operations retain their specified gates. No signing or broadcasting capability is included in the research build. Describing later live tickets does not approve a deployment or trade.

The native owner Project and native Wiki still require GitHub capabilities unavailable to the connected tool interface. Their definitions and source remain in the repository; the [setup status](docs/13-GITHUB-SETUP-STATUS.md) records this separately from the published issues and milestones.
