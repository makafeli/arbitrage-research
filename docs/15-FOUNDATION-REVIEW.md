# Foundation acceptance review

Review date: **12 September 2026**. Scope: **ARB-001, ARB-005 and ARB-006**, the existing product documents, and the research foundation changes on `feat/research-foundations`.

This is an agent-assisted product and technical review. The Product Owner perspective checks the research commitment and release evidence; the Technical Lead/Security perspective checks the implementation boundaries, provenance and delivery workflow. These labels identify review responsibilities, not separate human approvers or an external security firm. The operator retains decisions about spending, deployment, live funding and activation. This review is not a security audit, profitability assessment, legal opinion or approval to trade.

## Acceptance recommendation

| Ticket | Review conclusion | Limit of the conclusion |
|---|---|---|
| ARB-001 — Research scope and gates | The documented acceptance criteria are satisfied; the owner/milestone register below completes the outstanding decision-accountability detail. | Provider qualification, experimental results and live approval remain separate work. This review ratifies the implementation baseline within the user's authorized build scope; it does not invent an operator budget or live decision. |
| ARB-005 — Threat model, provenance and paper boundary | The foundation requirements are established and the reviewed research source respects the capability boundary. | This completes establishment of the model and review requirements, not all future security controls, dependency review or deployment certification. Findings below continue to block their named downstream gates. |
| ARB-006 — Delivery governance | Its acceptance checks pass. Project definitions and importer support now include Role, Release gate and Dependency IDs, with regression coverage for initialization and preservation. | Native owner Project/Wiki publication remains explicitly pending. Offline source verification does not claim that these fields exist remotely; the ticket explicitly allows unavailable native administration to remain recorded as pending. |

The maintainer should attach the committed review and applicable CI evidence before changing GitHub issue state. This document does not close issues or supersede the original bodies in [backlog.json](../planning/backlog.json).

## Product Owner perspective: ARB-001

| Original criterion | Status and evidence |
|---|---|
| PRD and decision register agree on supported markets and exclusions | **PASS — documentation.** [PRD](01-PRD.md) sections 1–4 and [ADR register](09-DECISIONS-AND-OPEN-QUESTIONS.md) ADR-01–08 specify Rust services, one private operator, Base and Solana, same-chain two-leg cycles through distinct eligible pools, and USDC starting inventory. Base uses USDC/WETH with Uniswap V3 qualification; Solana uses USDC/wSOL with Orca qualification. A different starting asset requires a new configuration/session. Cross-chain routes, CEX trading, unrestricted tokens, customer funds and automatic live promotion remain excluded. |
| M3 requires one complete atomic route simulation on each chain with declared virtual funding | **PASS — release requirement.** PRD-F07, PRD section 7 and the [M3 delivery gate](07-DELIVERY-PLAN.md) explicitly require complete plans/guards and successful full-transaction simulation on both chains. Local quote mathematics, raw captures and deterministic decoding qualify only for a research preview. The new source does not establish this M3 capability. |
| Live activation requires a later concrete operator decision | **PASS — policy and source boundary.** PRD-F14, ADR-12 and delivery-plan acceptance policy require separate live configuration, limits, reviews and operator activation. Research configuration rejects LIVE and all execution/signing/broadcast toggles; the API rejects LIVE creation and DISARM. |
| Observation success reports coverage, reproducibility and total costs without an income target | **PASS — documentation.** PRD sections 1 and 7 and PRD-F05/F09 define usable coverage, replay consistency, calculation agreement, complete costs, uncertainty and explained exclusions. Zero opportunities or a negative economic result are useful outcomes. No winning-chain or income target is approved. |

The release sequence remains research preview, executable paper, observation campaign, and optional bounded live capability. A code milestone does not substitute for M5 observation evidence. Optional research expansion can be considered on its own qualification merits; any live pilot still requires M6 and operator authorization. Railway is now the user's preferred deployment platform; that preference does not supply a region, budget, account credentials or an existing deployment.

### Dated decision-accountability register

This supplements the open questions in [the ADR register](09-DECISIONS-AND-OPEN-QUESTIONS.md). Due milestones are decision gates, not invented calendar commitments. Role owners are not assignments to named people.

| ID | Decision or unresolved assumption | Accountable role | Due gate and consequence |
|---|---|---|---|
| FQ-01 | Private single-operator research remains the implementation baseline. Commercial/multi-user use is excluded. | Product Owner; operator for a scope change | **M0 baseline accepted for implementation.** A commercial change requires a new access, isolation and custody design before expansion. |
| FQ-02 | Monthly infrastructure/data spending ceiling and provider subscriptions are unset. | Operator, advised by Operations | **M0 / ARB-002**, before any paid subscription or provider suitability claim. Local fixtures do not resolve this question. |
| FQ-03 | Railway is preferred; region, project/environment, service resources and measured RPC RTT remain to be selected. | Operations / Technical Lead; operator for costs | **M0 / ARB-004**, before deployment and performance baselines. Prepared configuration is not a running deployment. |
| FQ-04 | Real initial token/pool identities, current deployed code, liquidity and provider consistency need qualification. | Base and Solana protocol engineers | **M0 / ARB-003 and M2 / ARB-016–024**, before an adapter is declared qualified or acquired state supports executable estimates. |
| FQ-05 | Synthetic trade sizes, initial virtual balances, fee assumptions and comparable observation windows must be frozen per experiment. | Research Lead | **M3**, before a comparable executable-paper study. No real funds are allocated by this decision. |
| FQ-06 | Coverage, reproducibility, full cost accounting, holdout and uncertainty are the success measures; campaign coverage thresholds still need an experiment protocol. | Research Lead / Product Owner | **M5**, before the campaign and ARB-048 comparative feasibility report. Downtime cannot be recorded as zero opportunity. |
| FQ-07 | Real notional, loss and fee limits remain zero/unapproved. | Operator, advised by Risk/Security reviewer | **M6**, before live capability, funding or arming. Paper outcomes cannot settle this decision automatically. |
| FQ-08 | Wallet custody, signer policy, execution mechanism and independent review scope are unset. | Operator / Security reviewer | **M6**, before production keys, signing, deployment of execution artifacts or broadcast. |
| FQ-09 | “Arbitrage Research” is a working name; public branding is not a release requirement for private research. | Product Owner / operator | Before a branded external release. It does not block the current research implementation. |
| FQ-10 | No project-wide redistribution license has been selected in the repository root. Third-party notices retain their own terms. | Operator / Technical Lead, with legal advice if needed | Before licensing or distributing the project's own code/binaries under a chosen grant. Public repository visibility is not an assertion of a particular license. |
| FQ-11 | Production retention, aggregate capacity, backup location and restore targets need measured evidence. | Operations | **M1 planning; ARB-044 before research deployment acceptance.** The current worker assumes one writer per privately owned capture volume. |

## Technical Lead/Security perspective: ARB-005

### Source and capability review

The uploaded single-file artifact was read only as untrusted reference material. Its SHA-256 is `b26db4d71abbb883ffce1be182750f377df49c5eb35dd062ff042a5d526f114d` and its size is 226,277 bytes. It is not a workspace member, build input, imported module or vendored contract.

A scan of 63 Rust/TypeScript/JavaScript source files under `apps/` and `crates/` found neither an exact copy nor the uploaded artifact's distinctive `ArbitrageExecutor v3.3-live`, `ArbitrageExecutorProof` and `PRODUCTION KEEPER v3.3-live` markers. The manifests and reviewed entry points provide the stronger dependency evidence: the 15-member [Cargo workspace](../Cargo.toml) contains research/control/capture/replay components and no signer, Solidity executor, transaction submission app or import of the uploaded file. A textual scan alone is not a security proof and this review makes no certification of the uploaded artifact.

| Boundary reviewed | Implemented restriction | Remaining deployment or research boundary |
|---|---|---|
| Browser → API | Server-side operator cookie, CSRF and exact-origin checks, redacted structured errors, bounded rate/concurrency/deadline; safe configuration summaries only. | HTTPS proxy configuration and actual remote deployment must be verified. Demo data remains separately labelled and never becomes a connected fallback. |
| API → PostgreSQL | Typed session/command DTOs, immutable configuration references, operator-scoped queries, durable idempotency and revision checks. Receipt acceptance is distinct from worker application. | Real PostgreSQL integration cases are mandatory in CI; this environment cannot supply a local database execution result. |
| Worker → RPC | [ReadMethod](../crates/arb-adapter-api/src/lib.rs) is a closed enum containing `eth_chainId`, `eth_getBlockByNumber`, `eth_getCode`, `eth_call`, `getGenesisHash` and `getMultipleAccounts`. No broadcast method or free-form method string is exposed by the worker transport. | A generic HTTP library still exists underneath the restricted adapter. Egress restrictions/read-method gateways remain deployment controls against a compromised or modified process. |
| Provider result → decoder | Bounded HTTP reads, timeout/request quotas, response identity checks, configured chain/program/asset validation, strict account/ABI decoding, explicit quality flags and provenance. | Manually constructed fixtures do not prove current deployment identity, live provider consistency or complete quote state. |
| Imported files → capture/replay | Versioned typed manifests, object sizes/hashes, path and regular-file checks, incomplete-publication marker, retention checks and immutable configuration validation. | The filesystem must be privately owned; these checks do not provide a hostile shared-filesystem sandbox. Integrity hashes do not authenticate a provider. |
| Controlled capture → durable admission | Completed bundles are synced before database admission. Admission checks the original work generation, current lease/epoch and closed/open gate; late fenced captures remain unadmitted. Capture records are raw observations, not opportunities or financial outcomes. | A STOP acknowledgement closes admission, not an already-started RPC read. Read-only feed capture may continue while research admission is stopped. |
| Paper/replay → execution | Research config rejects execution/signing/broadcast toggles and LIVE. Paper/replay crates have no private-key loading or transaction submission interface. The existing source has no signing service or production execution artifact. | Adding live capability requires new code, a separate package/deployment boundary, explicit tests and M6 review. An ordinary config toggle cannot supply it. |

The reviewed controlled worker uses a separate bounded blocking capture job while its asynchronous control loop processes commands. `admit_capture_manifest` checks both generation and worker epoch before storage; storage repeats ownership/generation checks under its session lock. On cancellation or a database error, the coordinator's admission gate remains closed until explicit recovery. Inspection found no branch that intentionally admits a stale completed capture after an applied fence. The process/STOP integration test still requires its PostgreSQL-backed CI result; source inspection is not a measured stop-latency claim.

### Original criterion checklist

| Original criterion | Status and evidence |
|---|---|
| Uploaded script excluded from trusted execution | **PASS — manifests and source inspection**, with the fingerprint and scan scope above. |
| Provider data/imported files validated as untrusted inputs | **PASS — foundation boundary.** Typed read-method/response checks, chain-specific decoding, immutable configuration parsing and capture/replay validators are implemented. State qualification remains explicit and incomplete where evidence is missing. This is not completion of every later adapter/chaos gate. |
| Paper/replay cannot gain signing or submission through an ordinary toggle | **PASS — source and configuration boundary.** No signer/submission component exists in the research graph; LIVE and enabling execution flags are rejected. |
| Findings have severity, role owner and milestone gate | **PASS — register below.** Fixed findings and unresolved downstream restrictions are recorded separately. |

### Dependency and license provenance

[The Orca reference record](../third-party/ORCA-CORE-1.0.4.md) explains the historical static-fee dependency. This review independently compared the local release archives, their manifests, `Cargo.lock`, and [the recorded provenance](../third-party/orca-1.0.4-provenance.json):

| Package | Exact version | Manifest license | Verified archive SHA-256 |
|---|---|---|---|
| `orca_whirlpools_core` | `1.0.4` | `Apache-2.0` | `b884e3d6f39337baeb00c096b9d2d30682aeff8ead301303d039f84dcac24bec` |
| `orca_whirlpools_macros` | `1.0.4` | `Apache-2.0` | `3db647505a145cbb680f08725e0112b1691d984334cd1abe0d9f5c5a6e2ec91e` |

Both dependencies are pinned exactly; the core default features are disabled. The upstream publishing metadata references an unavailable commit and marks the core tree dirty, so the reproducible identity is the checked release archive, not an assertion of a clean build from that commit. The earlier provenance investigation also records source blobs matching an accessible Apache reference commit. That investigation is linked rather than represented as a fresh whole-upstream audit here.

The wrapper's current covered behavior is historical static-fee, fixed-tick-array, legacy-SPL **CANDIDATE mathematics**. It does not establish compatibility with every currently deployed Whirlpool program or adaptive-fee pool. The included [Apache license text](../third-party/LICENSE-APACHE-2.0.txt) and attribution must remain with applicable redistributed material; the project has not chosen a license for all of its own code.

For every new/updated third-party protocol implementation, the Technical Lead must record the exact package/repository version, archive/commit digest, source of copied or adapted code, license and applicable notices, enabled features, build compatibility, and the covered protocol/deployment assumptions. Lockfile changes require review of changed direct and transitive dependencies and security advisories. Current deployment equivalence needs protocol fixtures, independent expected values and full-transaction comparisons where required. A package version pin and passing compiler do not complete those reviews. A complete transitive license/advisory inventory remains a release task; this focused review does not claim one.

### Attack-surface checklist and findings

| Surface | Review result and next gate |
|---|---|
| Logs and errors | API errors contain stable codes/request IDs rather than supplied secrets or SQL internals. Config/parser errors avoid echoing raw configuration. RPC transport errors redact endpoints and response errors. Worker captures/logging distinguish raw records from financial outcomes. Review new fields before exporting them. |
| Exports and fixtures | Captures contain normalized configuration references, provider aliases and bounded recorded inputs; no endpoint/authorization header is intentionally retained. Synthetic/manually constructed/recorded origins remain distinct. Replay validates hashes and input identity; a capture does not prove provider honesty. |
| Authentication and controls | Cookie, CSRF, origin, rate, deadline and capacity tests cover the API boundary. PENDING/APPLIED semantics are preserved. Operator scope is enforced in database methods. Network/TLS exposure remains a deployment review. |
| Runtime authority | Config cannot enable transaction signing or broadcast. Generic host access, underlying HTTP transport and administrator capabilities are outside that source-level guarantee. No wallet keys belong in these processes. |
| Filesystem and retention | Flat object names, quotas, file-type checks and incomplete markers protect the intended private capture directory. One controlled writer per volume is an explicit assumption; restore and global operations evidence remain downstream gates. |

| Finding | Severity | State / accountable role | Gate and consequence |
|---|---|---|---|
| FR-01: RUNNING worker could reopen admission when readiness became false | High | **Source fixed** by Control/Storage engineer; readiness now faults durably and cannot reopen merely on a later ready poll. Regression added. | Actual database CI evidence required before control acceptance. No automatic resume after lost readiness. |
| FR-02: Read projections could bypass lifecycle-versus-column validation | High | **Source fixed** by Storage engineer; session reads/replays now validate persisted consistency. Regression added. | Database CI must demonstrate corrupt rows fail rather than report a trustworthy observed state. |
| FR-03: Recovery counter could disagree with actual outstanding attempt records | High | **Source fixed** by Storage engineer; recovery compares the records before STOPPED. Regression added. | Database CI required; unexplained attempts cannot be cleared by a counter-only restore. |
| FR-04: Current provider/registry and deployed-program equivalence are unqualified | High | **Open** — protocol engineers / Operations | ARB-002/003 and M2 qualification gates. No current-market executable or comparative result may be claimed from the constructed fixtures or historical math package. |
| FR-05: Complete atomic simulation per chain is absent | High | **Open** — Base/Solana execution engineers | ARB-028 onward and M3. Release only the capabilities actually implemented; no executable-paper comparison without both complete simulations and economic checks. |
| FR-06: Deployed network/TLS, backup restoration and operations controls lack deployment evidence | High | **Open** — Operations / Security reviewer | ARB-044 and ARB-049 before research deployment acceptance; deployment configuration alone is insufficient. Live controls require additional M6 review. |
| FR-07: Full transitive dependency/advisory and distribution-license review is incomplete | Medium | **Open** — Technical Lead; operator for the project's license | Before distributing a release or accepting its dependency/security gate. The focused Orca record is not a whole dependency audit. |
| FR-08: Capture aggregate quota assumes one controlled writer per private volume | Medium | **Documented restriction** — Operations / Data engineer | Preserve one writer/owned volume in initial deployment. Add shared reservation or separate volumes before multiple writers use one quota. |
| FR-09: Native owner Project, Wiki publication and administrative enforcement are not confirmed by this review | Low | **Pending** — Repository administrator | Follow the [setup status](13-GITHUB-SETUP-STATUS.md). Wiki source, workflow proposals and a Project definition do not establish native publication or enforced rules. |

## Delivery governance perspective: ARB-006

| Original criterion | Status and evidence |
|---|---|
| Eight epics/work tickets have stable IDs and resolvable prerequisites | **PASS.** `validate_project.py` found 8 epics and 68 tickets, resolved IDs, an acyclic graph, valid links and the current 15-member workspace. [Canonical backlog](../planning/backlog.json) and [published issue mapping](../planning/GITHUB-ISSUES.md) preserve stable identifiers. |
| PR template requests concrete behavior, requirements and actual evidence | **PASS.** [PR template](../.github/PULL_REQUEST_TEMPLATE.md) requests the trigger/result, ARB/requirement IDs, checks actually run and remaining acceptance gaps. [Contributing guide](../CONTRIBUTING.md) defines branch/review conventions and distinguishes proposed enforcement from actual repository settings. |
| Setup can rerun without duplicate issues/Project items | **PASS — implementation and regression evidence.** The importer reconciles stable managed markers and existing Project item IDs, preserves operator fields, tracks uncertain writes and refuses ambiguous retries. All 19 importer regression tests passed in this review, including seven new Project-metadata cases. This is not an exactly-once distributed-transaction guarantee after all state is lost. |
| Unsupported native GitHub administration remains explicitly pending | **PASS.** [Setup status](13-GITHUB-SETUP-STATUS.md), [Project definition](../planning/github-project.json) and [automation guide](../scripts/README.md) distinguish prepared files from native publication and owner permissions. No new remote-feature verification was performed by this foundation review. |

The Project definition now includes **Delivery status**, **Priority**, **Stage**, **Role**, **Release gate** and **Dependency IDs**. The importer initializes missing fields on new or existing items and preserves operator values, including intentionally empty text. It queries both text and single-select value types, checks native field data types, and rejects missing select options with an actionable error before preparing item-field writes. Native blocked-by relationships remain the canonical relationship representation; Dependency IDs is a readable display of stable IDs.

Role uses the first existing role in each canonical responsible-role string, producing 23 options; all collaborating roles remain in the issue body. This is a project-display convention, not a person assignment. The 54 combined responsibility strings would exceed GitHub's documented 50-option single-select limit. Release gate uses the three canonical task gate codes and eight milestone exit-gate labels; full criteria remain in issue bodies. These compact labels also avoid commas being misinterpreted by the CLI's option-list argument. The mapping is versioned in [github-project.json](../planning/github-project.json). [GitHub single-select limits](https://docs.github.com/en/issues/planning-and-tracking-with-projects/understanding-fields/about-single-select-fields)

The item-field query uses GitHub's documented `ProjectV2FieldCommon` fragment for both text and single-select field references. The mutation supplies the matching text or single-select value shape. This fixes a source-level gap in the earlier unexecuted owner-Project path; actual owner-authenticated publication and rerun verification remain pending. [GitHub Project API guide](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects)

The release accepter is the operator/maintainer, supported by the relevant technical/research role. For live work, the operator's concrete authorization and independent execution-security review are additional gates. A solo-maintainer or agent review must identify itself; a workflow file, generated ticket, passing unit test or elapsed date cannot accept a release by itself.

## Checks performed for this review

| Check | Observed result on 12 September 2026 |
|---|---|
| `python scripts/validate_project.py` | Passed: 8 epics, 68 tasks, resolved acyclic dependencies, valid document links and 15 workspace members. |
| `python scripts/github_bootstrap.py --repo makafeli/arbitrage-research --dry-run` | Passed offline: intended issue/label/milestone/relationship/Project/view/Wiki components shown; no authentication, remote inspection or mutation. |
| `python scripts/test_github_bootstrap.py` | 19 tests passed. Test-owner/test-repo references in output are isolated fixtures. |
| Uploaded-artifact fingerprint, manifest and source scans | No uploaded artifact dependency/copy/distinctive marker in the reviewed execution source; read-only method and no-signer boundaries inspected. |
| Orca pinned archives and package manifests | Both archive SHA-256 values matched the lockfile/provenance record; both exact package manifests declared Apache-2.0. |
| API verification already reported by the build environment | 17 non-database API tests passed, including virtual-time deadline/capacity tests. The database integration case now requires `TEST_DATABASE_URL` and fails if absent; local unit-only runs must explicitly filter it. |
| Database, browser-service, controlled-worker process and container evidence | Must come from the current branch CI/deployment checks. This document does not convert locally unavailable checks into passing results. |

The broader [validation record](11-PACKAGE-VALIDATION.md) and [implementation status](12-IMPLEMENTATION-STATUS.md) should link the final source revision's CI results. External-data tickets, current-deployment qualification, full simulation, campaign economics and live milestones remain open until their own evidence exists.
