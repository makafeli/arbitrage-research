# Complete issue specifications

Version 0.2.0. Generated from the canonical backlog definition. All issues are planned; acceptance evidence and optional live authorization remain outstanding.

<!-- arb-ticket:EPIC-01 -->
## EPIC-01: Scope, provenance and operating assumptions

**Milestone:** M0 · **Accountable role:** Product Owner / Technical Lead · **Status:** planned

### Product outcome
Representative pool data is obtainable; registry/protocol scope and costs are understood; decisions have owners.

### Scope and child tickets
- [ ] ARB-001 — Ratify research scope, success measures and release gates
- [ ] ARB-002 — Qualify data providers and set a spending envelope
- [ ] ARB-003 — Verify initial chain, token and venue identities
- [ ] ARB-004 — Set deployment, retention and benchmark assumptions
- [ ] ARB-005 — Establish threat model, license provenance and paper boundaries
- [ ] ARB-006 — Create delivery conventions, repository governance and acceptance workflow

### Acceptance and evidence
- [ ] Representative pool data is obtainable; registry/protocol scope and costs are understood; decisions have owners.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: none. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 9-16 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-02 -->
## EPIC-02: Rust platform and first observation slice

**Milestone:** M1 · **Accountable role:** Technical Lead · **Status:** planned

### Product outcome
Clean research build, durable commands, restart-to-STOPPED and a versioned real observation decision trace are demonstrated.

### Scope and child tickets
- [ ] ARB-007 — Build the Rust workspace and reproducible research CI
- [ ] ARB-008 — Implement exact amounts, identities and evidence invariants
- [ ] ARB-009 — Implement immutable validated experiment configuration
- [ ] ARB-010 — Add PostgreSQL migrations, durable sessions and command journal
- [ ] ARB-011 — Implement worker lifecycle, cancellation fences and restart recovery
- [ ] ARB-012 — Serve the authenticated control API and schema-consistent errors
- [ ] ARB-013 — Add bounded scheduling, stage telemetry and chain isolation
- [ ] ARB-014 — Implement versioned capture manifests and fixture provenance
- [ ] ARB-015 — Deliver the first observation-to-control vertical slice

### Acceptance and evidence
- [ ] Clean research build, durable commands, restart-to-STOPPED and a versioned real observation decision trace are demonstrated.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-01. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 23-35 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-03 -->
## EPIC-03: Qualified Solana and Base market adapters

**Milestone:** M2 · **Accountable role:** Chain engineering leads · **Status:** planned

### Product outcome
Both initial pool families pass exact arithmetic and failure-state qualification; invalid state cannot pass eligibility.

### Scope and child tickets
- [ ] ARB-016 — Implement Base Uniswap V3 read-only ingestion
- [ ] ARB-017 — Implement Solana Orca Whirlpool read-only ingestion
- [ ] ARB-018 — Build coherent snapshots, freshness gates and rollback handling
- [ ] ARB-019 — Qualify exact Uniswap V3 quote arithmetic
- [ ] ARB-020 — Qualify exact Orca Whirlpool quote arithmetic
- [ ] ARB-021 — Enforce adapter capability readiness and registry activation
- [ ] ARB-022 — Implement bounded distinct-pool cyclic route discovery
- [ ] ARB-023 — Persist decision traces, rejections and opportunity deduplication
- [ ] ARB-024 — Run dual-chain adapter qualification and chaos gate

### Acceptance and evidence
- [ ] Both initial pool families pass exact arithmetic and failure-state qualification; invalid state cannot pass eligibility.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-02. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 31-47 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-04 -->
## EPIC-04: Executable-paper simulation, accounting and replay

**Milestone:** M3 · **Accountable role:** Engine Lead / QA · **Status:** planned

### Product outcome
One complete supported atomic route per chain fully simulates under declared virtual funding; evidence, cost and reservation gates pass. Otherwise only quote-research preview is releasable.

### Scope and child tickets
- [ ] ARB-025 — Implement complete cost ledger and reproducible valuation
- [ ] ARB-026 — Build virtual portfolios, principal and fee reservations
- [ ] ARB-027 — Implement deterministic offline replay and compatibility checks
- [ ] ARB-028 — Build research-only Base transaction plans and atomic guard artifact
- [ ] ARB-029 — Build research-only Solana transaction plans and final balance guard
- [ ] ARB-030 — Implement complete Base simulation and exact-plan evidence
- [ ] ARB-031 — Implement complete Solana simulation and exact-plan evidence
- [ ] ARB-032 — Model delay, inclusion and failure scenarios without false certainty
- [ ] ARB-033 — Enforce estimated-executable evidence eligibility
- [ ] ARB-034 — Implement fair comparison cohorts and holdout analysis
- [ ] ARB-035 — Pass executable-paper release gate and financial correctness review

### Acceptance and evidence
- [ ] One complete supported atomic route per chain fully simulates under declared virtual funding; evidence, cost and reservation gates pass. Otherwise only quote-research preview is releasable.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-02, EPIC-03. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 39-59 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-05 -->
## EPIC-05: Approved dashboard and research operations

**Milestone:** M4 · **Accountable role:** Product Owner / Technical Lead · **Status:** planned

### Product outcome
Controls, comparison, exports, accessibility and research deployment recovery pass; no signer or broadcast capability ships.

### Scope and child tickets
- [ ] ARB-036 — Implement the approved dashboard shell and design tokens
- [ ] ARB-037 — Connect dashboard queries, provenance and data-quality states
- [ ] ARB-038 — Implement experiment creation, immutable settings and capability feedback
- [ ] ARB-039 — Implement start, pause, resume and stop acknowledgement UX
- [ ] ARB-040 — Build run history, portfolio ledger and comparison views
- [ ] ARB-041 — Implement reproducible JSON/CSV exports and redaction
- [ ] ARB-042 — Instrument system health, alerts and actionable diagnostics
- [ ] ARB-043 — Pass dashboard accessibility, responsiveness and failure-state review
- [ ] ARB-044 — Ship the research deployment with backups and recovery proof

### Acceptance and evidence
- [ ] Controls, comparison, exports, accessibility and research deployment recovery pass; no signer or broadcast capability ships.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-02, EPIC-03, EPIC-04. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 25-35 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-06 -->
## EPIC-06: Observation campaign and feasibility decision

**Milestone:** M5 · **Accountable role:** Product Owner / Operator · **Status:** planned

### Product outcome
Adequate documented coverage, sampled replay/accounting review and a candid economics report support a recorded go/revise/stop decision.

### Scope and child tickets
- [ ] ARB-045 — Register the observation campaign and analysis protocol
- [ ] ARB-046 — Operate the campaign and keep a data-quality incident log
- [ ] ARB-047 — Audit replay samples and investigate accounting or model discrepancies
- [ ] ARB-048 — Produce the comparative economics and feasibility report
- [ ] ARB-049 — Run operational and research-security readiness review
- [ ] ARB-050 — Record the go, revise or stop decision and optional live prerequisites

### Acceptance and evidence
- [ ] Adequate documented coverage, sampled replay/accounting review and a candid economics report support a recorded go/revise/stop decision.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-03, EPIC-04, EPIC-05. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 14-24 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-07 -->
## EPIC-07: Optional reviewed bounded live execution

**Milestone:** M6 · **Accountable role:** Technical Lead / Operator · **Status:** planned

### Product outcome
Exact live configuration, independent review, signer/submission/reconciliation failure drills and operator decision are complete before activation.

### Scope and child tickets
- [ ] ARB-051 — Approve a concrete live execution design and limit specification
- [ ] ARB-052 — Implement isolated signer policies, epochs and durable disarm
- [ ] ARB-053 — Harden execution artifacts and enforce live atomic constraints
- [ ] ARB-054 — Implement live inventory, fee reservations and submission-time risk checks
- [ ] ARB-055 — Implement durable intent, signed-payload and dispatch-start journals
- [ ] ARB-056 — Integrate the approved chain-specific submission transport
- [ ] ARB-057 — Reconcile pending, unknown, failed and finalized live outcomes
- [ ] ARB-058 — Implement fencing, crash recovery and controlled takeover
- [ ] ARB-059 — Add explicit live readiness, arming and incident-control UX
- [ ] ARB-060 — Complete independent execution review and adversarial remediation
- [ ] ARB-061 — Prepare the bounded live pilot release and final approval packet

### Acceptance and evidence
- [ ] Exact live configuration, independent review, signer/submission/reconciliation failure drills and operator decision are complete before activation.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-06. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 44-73 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:EPIC-08 -->
## EPIC-08: Optional pilot, measured expansion and closeout

**Milestone:** M7 · **Accountable role:** Operator / Technical Lead · **Status:** planned

### Product outcome
Each pilot/expansion has its own qualifying evidence and required approval; actual outcomes reconcile and long-term maintenance or shutdown is documented.

### Scope and child tickets
- [ ] ARB-062 — Deploy and run one explicitly approved bounded live pilot
- [ ] ARB-063 — Evaluate the pilot and approve continuation, reduction or shutdown
- [ ] ARB-064 — Qualify a second venue per chain for research coverage
- [ ] ARB-065 — Add reviewed token universes and optional three-leg research routes
- [ ] ARB-066 — Qualify an additional chain through the adapter and operations contract
- [ ] ARB-067 — Assess and implement an optional atomic funding adapter
- [ ] ARB-068 — Establish maintenance, change control and safe project closeout

### Acceptance and evidence
- [ ] Each pilot/expansion has its own qualifying evidence and required approval; actual outcomes reconcile and long-term maintenance or shutdown is documented.
- [ ] Every required child ticket has actual acceptance evidence, or a documented scope decision explains the exclusion and resulting limitation.
- [ ] Requirement and capability claims match code, tests, observed results and release notes.
- [ ] Blocking correctness, control, recovery or security findings are resolved before release.

### Dependencies and scheduling
Prerequisite epics: EPIC-06. Ticket-level prerequisites are authoritative; stages may overlap.
Child estimates total 27-46 working person-days across responsible roles, excluding elapsed campaign/reviewer waits. Do not add epic estimates again to ticket estimates.
M1's integration ticket uses Base for the demonstration after both quote adapters are available; its shared M2 prerequisites are planned overlap, not a dependency cycle.

### Scope/approval boundary
Research-only work may proceed within approved scope. Optional production deployment, funding, keys and live activation require the explicit later operator gate. M7 research expansion can proceed after its own research decision without first trading live.

### References
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`
- `planning/BACKLOG.md`

---

<!-- arb-ticket:ARB-001 -->
## ARB-001: Ratify research scope, success measures and release gates

**Milestone:** M0 · **Epic:** EPIC-01 · **Priority:** P0 · **Responsible role:** Product Owner
**Estimate:** 1-2 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Turn the proposed platform into a bounded research commitment with a decision-maker for unresolved assumptions. Acceptance is useful learning, including a negative economic result.

### Scope
- Record the private single-operator scope, Rust backend, Solana/Base comparison and USDC-starting two-leg routes.
- Define research-preview, executable-paper, observation-campaign and optional-live releases with separate exit evidence.
- Assign decision roles and an owner/due milestone to each open question; no real-person assignment without consent.

### Acceptance criteria
- [ ] PRD and decision register agree on supported markets and exclusions.
- [ ] M3 requires one complete atomic route simulation per chain under declared virtual funding; missing capability permits only quote-research preview.
- [ ] Live activation requires a later concrete operator decision and never follows automatically from paper results.
- [ ] Initial observation success measures report coverage, reproducibility and total costs without an income target.

### Verification and review evidence
- Product Owner and Technical Lead review a requirements-to-milestones checklist.
- Publish dated decisions and unresolved assumptions with consequences for dependent tickets.

### Dependencies and release gate
- Prerequisites: none; this issue establishes prerequisites for later work.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F01, PRD-F09, PRD-F14.
- `docs/01-PRD.md`
- `docs/07-DELIVERY-PLAN.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-002 -->
## ARB-002: Qualify data providers and set a spending envelope

**Milestone:** M0 · **Epic:** EPIC-01 · **Priority:** P0 · **Responsible role:** Operations engineer
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Adapter feasibility depends on access to historical and coherent live state. Establish what each provider actually supplies and what it costs before buying services.

### Scope
- Compare read-only Base RPC and Solana RPC/streaming candidates using representative pool/account queries.
- Document rate limits, retention, subscriptions, batch/context semantics, reconnect behavior and data terms.
- Record a proposed monthly ceiling and an approved cost owner before any paid purchase.

### Acceptance criteria
- [ ] At least one provider per chain yields the complete required state for the candidate pool family, or a blocking gap is recorded.
- [ ] Provider-specific gaps are distinguished from chain inactivity.
- [ ] Credentials use environment/secret references and never appear in fixtures, issue bodies or exports.
- [ ] No undocumented provider feature is required by the initial vertical slice.

### Verification and review evidence
- Attach redacted sample request/response provenance and measured request-latency distributions.
- Record official provider documentation URLs and retrieval date alongside the shortlist.

### Dependencies and release gate
- Prerequisites: ARB-001.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, NFR-02, NFR-05.
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-003 -->
## ARB-003: Verify initial chain, token and venue identities

**Milestone:** M0 · **Epic:** EPIC-01 · **Priority:** P0 · **Responsible role:** Chain engineers
**Estimate:** 2-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Tickers and plausible addresses are insufficient to establish a tradeable universe. Build an evidence-backed registry for a deliberately small supported pool set.

### Scope
- Verify Base Uniswap V3 and Solana Orca Whirlpool program/contract identities from official deployment records and chain observations.
- Identify USDC/WETH and USDC/wSOL pools, owners, decimals, fee parameters and liquidity suitability.
- Record source provenance, code/program versions and unsupported token behavior.

### Acceptance criteria
- [ ] Every candidate asset is keyed by network plus address/mint and every pool records exact asset identities.
- [ ] No fixture: identifier can pass a production registry validator.
- [ ] At least two distinct eligible pools per chain are sought; absence is recorded and blocks two-pool experiments rather than fabricating liquidity.
- [ ] Same-venue routes are permitted; Sushi/Raydium and additional token behaviors remain unqualified expansion scope.

### Verification and review evidence
- Cross-check registry entries against primary deployment documentation and read-only chain state.
- Review evidence with the opposite chain engineer or Technical Lead.

### Dependencies and release gate
- Prerequisites: ARB-001, ARB-002.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F02, PRD-F04, NFR-01.
- `docs/01-PRD.md`
- `docs/02-ARCHITECTURE.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-004 -->
## ARB-004: Set deployment, retention and benchmark assumptions

**Milestone:** M0 · **Epic:** EPIC-01 · **Priority:** P1 · **Responsible role:** Technical Lead / Operations
**Estimate:** 1-2 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Performance targets and storage plans have no meaning without an operating environment. Define a repeatable research host profile and capture policy.

### Scope
- Choose a documented Linux architecture, CPU/RAM/storage profile and hosted-provider connection topology.
- Budget raw capture, summaries, backups and replay manifests separately.
- Define preliminary queue/freshness budgets as hypotheses to measure in M1.

### Acceptance criteria
- [ ] The deployment profile can run independent Base/Solana workers, API, database and dashboard.
- [ ] Retention explains which historical claims cease to be reproducible after raw data expiry.
- [ ] No universal microsecond or throughput guarantee is asserted.
- [ ] Capacity and cost owners are documented; Redis/Kafka/Kubernetes are not introduced without a measured need.

### Verification and review evidence
- Publish a reproducible benchmark-environment manifest.
- Review a sample 24-hour storage estimate with transparent input rates and assumptions.

### Dependencies and release gate
- Prerequisites: ARB-001, ARB-002.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F13, NFR-02, NFR-03, NFR-08.
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-005 -->
## ARB-005: Establish threat model, license provenance and paper boundaries

**Milestone:** M0 · **Epic:** EPIC-01 · **Priority:** P0 · **Responsible role:** Security reviewer / Technical Lead
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The original untrusted script must not become a production dependency. Establish trust boundaries and a research package that cannot move funds.

### Scope
- Model provider data, imported fixtures, dependencies, operator browser, API and worker boundaries.
- Record provenance/license review requirements for third-party libraries and protocol code.
- Define a build/deployment boundary excluding private-key loading, transaction signing and broadcast from research artifacts.

### Acceptance criteria
- [ ] The uploaded script is excluded from trusted execution code.
- [ ] Provider data and imported files are validated as untrusted inputs.
- [ ] Paper and replay services cannot acquire signing or submission capabilities through an ordinary configuration toggle.
- [ ] Threat findings have severity, owner role and a milestone gate; unresolved findings are visible.

### Verification and review evidence
- Review dependency graph and process/capability diagrams.
- Provide an attack-surface checklist covering logs, exports, authentication and fixture import.

### Dependencies and release gate
- Prerequisites: ARB-001.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F16, NFR-06.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-006 -->
## ARB-006: Create delivery conventions, repository governance and acceptance workflow

**Milestone:** M0 · **Epic:** EPIC-01 · **Priority:** P1 · **Responsible role:** Technical Lead / Product Owner
**Estimate:** 1-2 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Make the design actionable without treating ticket creation as completed engineering. Establish durable issue IDs and evidence-based review conventions.

### Scope
- Publish repository README, contribution guide, issue/PR templates, wiki navigation and stable backlog markers.
- Define labels, milestones and project fields for priority, role, dependency and release gate.
- Document branch/CI protection proposals and who can accept a release.

### Acceptance criteria
- [ ] All eight epics and their work tickets have stable ARB/EPIC IDs and resolvable prerequisites.
- [ ] A PR template requests the concrete behavior, relevant requirement IDs and actual validation evidence.
- [ ] Project setup can be rerun without duplicating issues or board entries.
- [ ] Native GitHub features that cannot be configured by available permissions are explicitly marked pending; no nonexistent board/wiki is reported as complete.

### Verification and review evidence
- Validate backlog JSON and check all dependency IDs and cycles.
- Exercise the bootstrap in a dry-run or inspect an execution manifest without mutating unrelated repositories.

### Dependencies and release gate
- Prerequisites: ARB-001, ARB-005.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: NFR-06, NFR-08.
- `docs/07-DELIVERY-PLAN.md`
- `planning/BACKLOG.md`
- `planning/backlog.json`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-007 -->
## ARB-007: Build the Rust workspace and reproducible research CI

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Technical Lead
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A coherent workspace gives later adapters one trusted build baseline. Repository scaffolding is accepted only after compilation and required checks run in a named environment.

### Scope
- Create modular Cargo workspace applications/crates with documented ownership and dependency direction.
- Pin a supported toolchain, commit Cargo.lock and configure formatting, linting and meaningful tests.
- Separate research build targets from deferred signer and live submission packages.

### Acceptance criteria
- [ ] A clean checkout compiles and runs research checks on the documented Linux CI environment.
- [ ] The actual lockfile is generated by Cargo, not invented.
- [ ] Research dependency inspection proves no key backend or broadcast entry point is shipped.
- [ ] CI exposes failures rather than silently skipping unavailable required tools.

### Verification and review evidence
- Attach successful CI run and toolchain/dependency versions.
- Run formatting, clippy and workspace tests appropriate to implemented packages; record absent local toolchains honestly.

### Dependencies and release gate
- Prerequisites: ARB-004, ARB-005, ARB-006.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: NFR-01, NFR-06, NFR-08.
- `docs/02-ARCHITECTURE.md`
- `docs/03-REPOSITORY-STRUCTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-008 -->
## ARB-008: Implement exact amounts, identities and evidence invariants

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Engine engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Errors in units and evidence labels can make a false profit look valid. Create exact, validated domain types shared by every chain and API.

### Scope
- Implement chain/asset/pool/route identifiers, raw unsigned amounts, signed P&L and explicit decimals.
- Define immutable session modes, lifecycle enums and opportunity evidence types.
- Use checked integer/fixed-point operations with explicit overflow and rounding errors.

### Acceptance criteria
- [ ] JSON round-trips values above JavaScript's safe integer range as strings.
- [ ] Cross-network route continuity, ticker-only identity, invalid decimals and overflow are rejected.
- [ ] PAPER or REPLAY cannot emit REALIZED; local arithmetic alone cannot emit SIMULATED.
- [ ] Missing costs and unknown submission outcomes have explicit variants rather than numeric zero or failed defaults.

### Verification and review evidence
- Run boundary/property tests for amounts and route invariants.
- Validate shared examples and API schemas against the Rust serialization shape.

### Dependencies and release gate
- Prerequisites: ARB-007.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F01, PRD-F02, PRD-F06, NFR-01.
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `specs/opportunity.schema.json`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-009 -->
## ARB-009: Implement immutable validated experiment configuration

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Backend engineer
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A configuration must identify the exact experiment and fail safely before workers start. Implement a validated research configuration with clear capability errors.

### Scope
- Parse TOML into typed provider, network, asset, route, size, delay, fee and retention settings.
- Compute configuration version/digest and preserve defaults in the effective configuration.
- Reject unsupported combinations and fixture identities in production registries.

### Acceptance criteria
- [ ] A session's mode, network, starting asset and configuration version cannot mutate in place.
- [ ] Changing mode or starting asset requires a stopped prior session and a new session configuration.
- [ ] The shipped example remains inert: networks disabled, empty allowlists and no live permission.
- [ ] Errors identify the field and reason without exposing credential values.

### Verification and review evidence
- Run valid/invalid configuration fixtures for units, limits, capabilities and secret redaction.
- Verify canonical configuration hashes remain stable across equivalent serialization.

### Dependencies and release gate
- Prerequisites: ARB-003, ARB-008.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F01, PRD-F02, NFR-08.
- `config/research.example.toml`
- `docs/01-PRD.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-010 -->
## ARB-010: Add PostgreSQL migrations, durable sessions and command journal

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Backend engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Operator intent must survive retries and restarts. Store configurations, sessions, revisions and command application outcomes durably before acknowledging control requests.

### Scope
- Implement reviewed migrations for configuration snapshots, sessions, commands and audit events.
- Persist command idempotency keys scoped to an authenticated operator/session.
- Separate desired command, PENDING acceptance and worker-applied acknowledgement.

### Acceptance criteria
- [ ] Duplicate requests return the same command and cannot create duplicate state transitions.
- [ ] Stale expected revisions produce an explicit conflict.
- [ ] Restart preserves commands and observed status without inferring APPLIED from API acceptance.
- [ ] Migration rollback/recovery instructions preserve audit history and make compatibility limitations explicit.

### Verification and review evidence
- Run database integration tests for concurrent duplicates and stale revisions.
- Interrupt the process between command persistence and worker acknowledgement; verify correct recovery.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-009.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F11, PRD-F13, NFR-04.
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-011 -->
## ARB-011: Implement worker lifecycle, cancellation fences and restart recovery

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Backend engineer / QA
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Start and stop must reflect actual worker behavior. Implement session-local control fences and state transitions that remain meaningful during slow or lost communication.

### Scope
- Implement RECOVERING, STOPPED, RUNNING, PAUSING, PAUSED, DRAINING and FAULTED transitions.
- Close evaluation/admission gates on pause or stop, cancel unsent generation-tagged work, and keep feeds/reconciliation active.
- Use durable worker acknowledgement and boot into RECOVERING then STOPPED.

### Acceptance criteria
- [ ] PENDING remains visible until the worker applies and acknowledges the local fence.
- [ ] Stop reaches DRAINING while prior emitted attempts are unresolved, then STOPPED; a paper run with no outstanding attempts can stop directly after application.
- [ ] Old-generation queued results cannot be admitted after the fence.
- [ ] Stop-all reports each session's command separately and makes no atomic global-stop claim.
- [ ] Restart never automatically starts or arms a LIVE session.

### Verification and review evidence
- Run deterministic lifecycle tests for delayed ACK, disconnected worker, duplicate commands, cancellation and recovery.
- Inject an unresolved attempt fixture and verify DRAINING cannot be mistaken for complete settlement.

### Dependencies and release gate
- Prerequisites: ARB-010.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F11, NFR-03, NFR-04.
- `docs/01-PRD.md`
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-012 -->
## ARB-012: Serve the authenticated control API and schema-consistent errors

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Backend engineer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The dashboard needs one reliable control contract. Implement research API operations with authentication, idempotency and command status semantics.

### Scope
- Implement health, session creation/list/detail, commands/status and opportunity queries from the OpenAPI contract.
- Add single-operator authentication, CSRF protection for cookie-authenticated mutation and request size/rate limits.
- Keep network credentials and internals out of browser responses.

### Acceptance criteria
- [ ] API requests and responses validate against the committed schema.
- [ ] Unauthenticated and cross-site mutation attempts fail without changing sessions.
- [ ] DISARM/live-only actions in a research session return explicit forbidden/capability errors.
- [ ] A command response never claims STOPPED solely because HTTP acceptance succeeded.
- [ ] Errors include stable codes and correlation IDs while redacting secrets.

### Verification and review evidence
- Run contract/integration cases for success, invalid input, revision conflict, idempotency and authentication failures.
- Verify origin/CSRF handling and sensitive response/log redaction.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-010, ARB-011.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F11, PRD-F12, PRD-F16.
- `specs/openapi.yaml`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-013 -->
## ARB-013: Add bounded scheduling, stage telemetry and chain isolation

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P1 · **Responsible role:** Systems engineer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Low latency requires bounded work and visible data age. Introduce explicit queues and CPU scheduling so a slow consumer cannot silently invalidate results.

### Scope
- Use Tokio for I/O and bounded CPU workers for quote/evaluation jobs.
- Define per-stage deadlines, queue limits, generation tags and drop/resync policies.
- Instrument ingestion, snapshot, quote, simulation, persistence and API stages with correlation IDs.

### Acceptance criteria
- [ ] No unbounded task creation or unbounded spawn_blocking path exists for incoming market work.
- [ ] Queue age and stale/drop reasons are exported by chain and stage.
- [ ] Slow dashboard/analytics reads cannot block worker evaluation or control acknowledgement.
- [ ] A blocked Solana worker does not stall the Base worker's control loop, and vice versa.

### Verification and review evidence
- Run overload tests with a deliberately slow consumer and show bounded memory/work queues.
- Capture an initial p50/p95/p99 stage profile on the named host without treating it as an SLA.

### Dependencies and release gate
- Prerequisites: ARB-007, ARB-008, ARB-011.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: NFR-02, NFR-03, NFR-05.
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-014 -->
## ARB-014: Implement versioned capture manifests and fixture provenance

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P1 · **Responsible role:** Data engineer
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A stored opportunity is not reproducible unless its inputs and versions can be located. Define the capture manifest before observations begin accumulating.

### Scope
- Persist normalized/raw references, provider provenance, chain state, ingestion ordering and timestamps.
- Hash capture objects and include schema, adapter, configuration and build versions.
- Tag synthetic, recorded-live and manually constructed fixture origins explicitly.

### Acceptance criteria
- [ ] Every decision can locate its declared input bundle or report why it cannot.
- [ ] Corrupt hashes, missing objects and incompatible versions are rejected or explicitly quarantined.
- [ ] Synthetic fixtures are never counted in market performance reports.
- [ ] Retention metadata marks when raw inputs expire without rewriting the historical result.

### Verification and review evidence
- Round-trip and corrupt a representative manifest to verify integrity and missing-input handling.
- Review a capture export for provenance completeness and secret leakage.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-009, ARB-010.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F08, PRD-F13, NFR-08.
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-015 -->
## ARB-015: Deliver the first observation-to-control vertical slice

**Milestone:** M1 · **Epic:** EPIC-02 · **Priority:** P0 · **Responsible role:** Technical Lead / QA
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Prove the architecture on one real recorded chain observation after the shared platform and both quote adapters are available. Base is the demonstration chain for this integrated trace; this is not an earlier Base-only release or a claim of better economics.

### Scope
- Connect verified pool capture, a coherent snapshot, exact route evaluation, persistence and API presentation.
- Show a recorded CANDIDATE decision with state age and excluded costs.
- Exercise stop acknowledgement, restart and input replay from the same decision trace.

### Acceptance criteria
- [ ] The demo identifies exact source/config/build versions and contains no invented market values.
- [ ] A negative or rejected route is accepted as a valid demonstration result.
- [ ] The operator sees command PENDING then APPLIED and actual worker state.
- [ ] A restart returns STOPPED; gaps and missing data remain visible.
- [ ] The integration gate records that both quote adapters are prerequisites; the Base demonstration does not by itself claim the dual-chain failure/qualification gate has passed.

### Verification and review evidence
- Attach a redacted trace/capture and runbook reproduction steps.
- Record measured stage latency and the stop/restart exercise outcome.

### Dependencies and release gate
- Prerequisites: ARB-012, ARB-013, ARB-014, ARB-016, ARB-018, ARB-022, ARB-023.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, PRD-F06, PRD-F11, PRD-F12.
- `docs/07-DELIVERY-PLAN.md`
- `docs/10-BUILD-HANDOFF.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-016 -->
## ARB-016: Implement Base Uniswap V3 read-only ingestion

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** EVM engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Acquire the complete inputs required by supported Base pools without mixing unrelated blocks. Decode real on-chain data using verified identities.

### Scope
- Implement initial pool discovery from the approved manifest, block/log subscriptions and required storage/tick reads.
- Pin deployment/ABI provenance and decode exact protocol units.
- Capture block number/hash, parent relationships, provider context and pool update evidence.

### Acceptance criteria
- [ ] All approved pool state needed for quoting is available and attributable to one coherent block reference.
- [ ] Unexpected contract identity, malformed event and missing tick data fail explicitly.
- [ ] Reconnect backfill is bounded and identifies data gaps.
- [ ] The adapter advertises decode/read capabilities only until quote and simulation qualification complete.

### Verification and review evidence
- Replay representative real capture fixtures and compare decoded values with official ABI/state queries.
- Inject missing logs, malformed data and provider reconnects.

### Dependencies and release gate
- Prerequisites: ARB-002, ARB-003, ARB-008, ARB-009, ARB-014.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F02, PRD-F03, NFR-08.
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-017 -->
## ARB-017: Implement Solana Orca Whirlpool read-only ingestion

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** Solana engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Whirlpool quotes require validated accounts and tick arrays with defensible context. Capture enough state to make completeness and consistency explicit.

### Scope
- Read/subscribe to approved Whirlpool, vault, mint and tick-array accounts with program ownership checks.
- Record slot/context, account write provenance and stream reconnect boundaries.
- Enumerate supported token programs/extensions and reject unsupported behavior.

### Acceptance criteria
- [ ] Pool and dependent accounts validate owner/program and configured asset identities.
- [ ] Missing arrays or inconsistent account context cannot be treated as a complete valid quote state.
- [ ] Provider semantics for account coherence are documented and unresolved coherence becomes a rejection reason.
- [ ] Account decode version and source provenance are stored with captures.

### Verification and review evidence
- Compare decoded representative account fixtures with the pinned official layout/reference implementation.
- Test owner mismatch, unavailable arrays, duplicate/out-of-order writes and reconnect gaps.

### Dependencies and release gate
- Prerequisites: ARB-002, ARB-003, ARB-008, ARB-009, ARB-014.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F02, PRD-F03, NFR-08.
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-018 -->
## ARB-018: Build coherent snapshots, freshness gates and rollback handling

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** Chain engineers
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A fresh quote built from inconsistent state is unreliable. Centralize state-completeness and chain-specific rollback rules before opportunity eligibility.

### Scope
- Build immutable per-chain snapshot IDs with completeness, coherence and age metadata.
- Invalidate affected quotes on Base reorgs and Solana rollback/context changes.
- Separate observation timestamps, chain state and comparison-window timestamps.

### Acceptance criteria
- [ ] Stale, incomplete, forked or inconsistent state cannot produce ESTIMATED_EXECUTABLE.
- [ ] A rollback invalidates dependent work and preserves an audit reason.
- [ ] Each chain's finality/context policy is explicit and versioned.
- [ ] No cross-chain shared atomic snapshot is implied by similar wall-clock times.

### Verification and review evidence
- Replay gaps, reorgs, out-of-order updates and mismatched dependent-account fixtures.
- Show a previously valid snapshot becomes ineligible without rewriting its captured provenance.

### Dependencies and release gate
- Prerequisites: ARB-016, ARB-017.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, PRD-F06, NFR-04.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-019 -->
## ARB-019: Qualify exact Uniswap V3 quote arithmetic

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** EVM engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Concentrated-liquidity rounding and tick traversal determine whether a spread exists. Implement exact-input quotes against pinned protocol behavior.

### Scope
- Implement supported tick traversal, liquidity transitions, fees and amount rounding using checked integers.
- Expose quoted output, included pool fees/price impact and explicit failure reasons.
- Bound tick traversal and reject incomplete coverage rather than extrapolating.

### Acceptance criteria
- [ ] Golden and protocol differential cases agree within the protocol's exact rounding rules.
- [ ] Zero liquidity, tick boundaries, maximum supported amounts and arithmetic overflow are tested.
- [ ] Fees already reflected in output are labeled as included, not charged again downstream.
- [ ] Unsupported V2/V4 or other pool models never silently route through this implementation.

### Verification and review evidence
- Publish a fixture corpus with pinned official implementation/source versions and expected outputs.
- Run boundary, randomized differential and incomplete-tick tests.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-016, ARB-018.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F04, NFR-01.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-020 -->
## ARB-020: Qualify exact Orca Whirlpool quote arithmetic

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** Solana engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Whirlpool tick arrays and token units require chain-specific correctness. Implement exact-input quotes for the explicitly supported account/token configuration.

### Scope
- Implement or bind audited/pinned reference arithmetic with documented provenance and compatible licenses.
- Handle tick-array transitions, fees, liquidity changes and prescribed rounding.
- Reject unsupported token extensions and incomplete traversal inputs.

### Acceptance criteria
- [ ] The adapter matches pinned official reference behavior for all qualification cases.
- [ ] Boundary amounts, tick crossings, empty liquidity and unavailable arrays produce expected results.
- [ ] The output declares included fees/impact and exact input/output asset units.
- [ ] Token behavior outside the supported matrix cannot be accidentally accepted by matching a ticker.

### Verification and review evidence
- Run golden/randomized differential cases against the pinned protocol implementation.
- Review overflow and truncation behavior at serialization/math boundaries.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-017, ARB-018.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F04, NFR-01.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-021 -->
## ARB-021: Enforce adapter capability readiness and registry activation

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** Technical Lead / Chain engineers
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
An adapter that decodes a pool is not necessarily ready to simulate or trade it. Make readiness explicit in the registry and service contract.

### Scope
- Implement decode, quote, build, full-simulate and submit capability declarations by supported pool/token combination.
- Tie activation to qualified registry versions and validation evidence.
- Expose unsupported capabilities in API/operator diagnostics.

### Acceptance criteria
- [ ] Configuration cannot enable a capability that the selected adapter has not qualified.
- [ ] Research builds never advertise submit capability.
- [ ] Adding another venue requires code/protocol validation, not only a TOML entry.
- [ ] A registry with no eligible routes starts observation safely and explains the lack of coverage.

### Verification and review evidence
- Test capability mismatch and changed token/program identity against activation validation.
- Review the generated supported-venue matrix.

### Dependencies and release gate
- Prerequisites: ARB-003, ARB-009, ARB-019, ARB-020.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F02, PRD-F04, PRD-F06.
- `docs/01-PRD.md`
- `docs/02-ARCHITECTURE.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-022 -->
## ARB-022: Implement bounded distinct-pool cyclic route discovery

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** Engine engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Discover only routes the implementation can evaluate correctly. Start with finite USDC cycles through two distinct supported pools.

### Scope
- Build chain-local directed asset/pool graphs from activated registry snapshots.
- Enumerate USDC→WETH→USDC and USDC→wSOL→USDC across distinct eligible pools and configured sizes.
- Apply queue/deadline bounds and deterministic route ordering.

### Acceptance criteria
- [ ] Every route closes to its starting asset and never crosses networks.
- [ ] The same pool cannot occupy both legs of a Phase 1 cycle.
- [ ] Unknown pool models, three-leg routes and unconfigured sizes are rejected.
- [ ] A cancelled or expired generation cannot publish an admissible new opportunity.
- [ ] No-route results retain diagnostics rather than inventing market opportunities.

### Verification and review evidence
- Use small graph fixtures with known valid, duplicate and invalid cycles.
- Run high-pool-count stress tests against route/queue bounds.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-009, ARB-019, ARB-020, ARB-021.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F04, NFR-02, NFR-03.
- `docs/01-PRD.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-023 -->
## ARB-023: Persist decision traces, rejections and opportunity deduplication

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P1 · **Responsible role:** Engine engineer / Data engineer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Repeated observations must not inflate opportunity counts or hide rejected cases. Persist a trace that explains each result and groups persistent opportunities transparently.

### Scope
- Record snapshot, route, amount, quote method, checks, rejection reason and correlation IDs.
- Define a versioned grouping key and persistence window while preserving raw observations.
- Separate observations, unique opportunities, eligible attempts and reconciled transactions in query schemas.

### Acceptance criteria
- [ ] Every displayed number can be traced to a calculation version and input state.
- [ ] Grouping never removes rejected observations from research denominators.
- [ ] Positive and negative quoted routes remain CANDIDATE until stronger evidence exists.
- [ ] Queries distinguish zero qualifying routes from provider downtime or missing captures.

### Verification and review evidence
- Replay repeated and changing-spread fixtures to verify grouping boundaries.
- Compare aggregate counts to raw records and inspect one end-to-end decision trace.

### Dependencies and release gate
- Prerequisites: ARB-010, ARB-014, ARB-018, ARB-022.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F04, PRD-F06, PRD-F12, PRD-F13.
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-024 -->
## ARB-024: Run dual-chain adapter qualification and chaos gate

**Milestone:** M2 · **Epic:** EPIC-03 · **Priority:** P0 · **Responsible role:** QA / Chain engineers
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Establish both adapters as reliable inputs before interpreting paper economics. Combine protocol agreement with real capture and failure behavior.

### Scope
- Run both qualification corpora and replay representative live captures.
- Exercise provider failures, gaps, overload, rollbacks and malformed account/contract data.
- Publish support matrix, open defects and measured freshness/latency budgets.

### Acceptance criteria
- [ ] Each enabled pool type passes golden/differential and boundary checks.
- [ ] Stale or incoherent state cannot pass executable eligibility in any supported path.
- [ ] Known gaps and excluded assets/pools are visible to the operator.
- [ ] M2 exits only when both chain adapters meet their documented support claims.

### Verification and review evidence
- Attach CI reports, corpus/build hashes and named-host latency distributions.
- Technical Lead signs off the qualification matrix; unresolved correctness defects block dependent release claims.

### Dependencies and release gate
- Prerequisites: ARB-013, ARB-015, ARB-019, ARB-020, ARB-021, ARB-023.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, NFR-01, NFR-02, NFR-04.
- `docs/07-DELIVERY-PLAN.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-025 -->
## ARB-025: Implement complete cost ledger and reproducible valuation

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Engine engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A gross spread is not net economics. Record transaction costs, assumptions and reference conversions without double counting or treating missing fees as zero.

### Scope
- Represent DEX fees/impact, network fees, Base L1-data components where applicable, Solana priority fees/tips, funding and failure scenarios.
- Store native-currency raw units with timestamped conversion provenance.
- Separate gross, transaction-net and fully allocated operating results.

### Acceptance criteria
- [ ] Quoted output's included pool fees and price impact are not subtracted again.
- [ ] Unknown execution/funding/valuation inputs block complete-cost eligibility.
- [ ] USDC-unit outcomes do not silently claim guaranteed USD value.
- [ ] Operating overhead is separately allocated and its method/version is disclosed.
- [ ] Negative outcomes and failed-attempt scenario costs remain in reports.

### Verification and review evidence
- Run hand-calculated cost fixtures for both chains, unavailable inputs and signed P&L.
- Verify aggregate ledger conservation and reference-conversion reproduction.

### Dependencies and release gate
- Prerequisites: ARB-008, ARB-023.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F05, PRD-F10, NFR-01.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-026 -->
## ARB-026: Build virtual portfolios, principal and fee reservations

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Engine engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Paper fills cannot all use the same balance. Implement exact virtual accounting with separate trade principal and native-fee budgets.

### Scope
- Create immutable initial balances per new paper run and a journal of reservations/releases/settlements.
- Reserve USDC principal and ETH/SOL fees independently for competing hypothetical attempts.
- Expose free, reserved and total balances without valuing inventory changes as arbitrage profit.

### Acceptance criteria
- [ ] Concurrent candidates cannot reserve more principal or fee balance than available.
- [ ] A fees-only balance does not satisfy trade principal requirements.
- [ ] Rejections, expiries and modeled failures release or spend reservations according to the explicit scenario.
- [ ] A portfolio reset creates a new run instead of rewriting prior results.
- [ ] Ledger replay reconstructs balances exactly.

### Verification and review evidence
- Run concurrent reservation and crash/restart ledger tests.
- Use a conflicting-opportunity fixture to prove mutually exclusive capital is respected.

### Dependencies and release gate
- Prerequisites: ARB-010, ARB-025.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F07, PRD-F10, PRD-F17.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-027 -->
## ARB-027: Implement deterministic offline replay and compatibility checks

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Engine engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Reproduce decisions using the state that was available at the time. Replay must remain deterministic and must not fill historical gaps from today's RPC.

### Scope
- Inject clock/event ordering and pin capture/config/adapter/build versions.
- Provide CLI replay selection by manifest/run and deterministic result export.
- Reject incompatible schemas and incomplete captures with explicit reasons.

### Acceptance criteria
- [ ] The same inputs produce the same raw amounts, checks and trace ordering.
- [ ] Offline replay makes no external network calls.
- [ ] Missing inputs block full reproducibility claims rather than being silently reconstructed.
- [ ] Local mathematical replay remains CANDIDATE regardless of a positive result.
- [ ] Replay logs identify any platform/version limitations affecting determinism.

### Verification and review evidence
- Run replay twice in isolated environments and compare canonical outputs.
- Deny network access and remove a capture object to verify correct behavior.

### Dependencies and release gate
- Prerequisites: ARB-014, ARB-019, ARB-020, ARB-023.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F08, NFR-08.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-028 -->
## ARB-028: Build research-only Base transaction plans and atomic guard artifact

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** EVM engineer
**Estimate:** 5-7 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Executable-paper evidence requires a complete intended atomic transaction, not two standalone quote calls. Build the Base plan and minimal local execution artifact.

### Scope
- Encode supported pool swaps, exact inputs, minimum outputs, deadline and route/pool allowlists.
- Implement final starting-asset balance/profit guard with explicit pre/post accounting and reentrancy/callback authorization.
- Create local-fork deployment fixtures and declared virtual funding/allowance setup.

### Acceptance criteria
- [ ] The complete route executes atomically in the test harness and reverts when required final-balance/route guards fail.
- [ ] Execution bytes and guard parameters produce a canonical plan digest.
- [ ] Spending-account principal and allowances are explicit; a funded fee account is insufficient.
- [ ] No production deployment, private key or real broadcast is included.
- [ ] Tests cover unauthorized callbacks, unsupported targets and insufficient final balance.

### Verification and review evidence
- Run contract/local-fork positive and rejection fixtures, including unprofitable synthetic states.
- Review artifact provenance and attack surfaces; this research artifact is not an independent security audit.

### Dependencies and release gate
- Prerequisites: ARB-019, ARB-021, ARB-025.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F06, PRD-F07, NFR-01.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-029 -->
## ARB-029: Build research-only Solana transaction plans and final balance guard

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Solana engineer
**Estimate:** 5-7 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A Solana quote is insufficient to prove the complete instruction sequence is valid. Build a full supported route with enforceable atomic output conditions.

### Scope
- Construct complete swap instructions, account metas, compute-budget instructions and required token accounts.
- Implement or qualify a minimal final-balance guard program where the selected execution design needs one.
- Document virtual account funding, owners, authority and simulation constraints.

### Acceptance criteria
- [ ] Both swap legs and the guard are part of one complete intended transaction plan.
- [ ] Account ownership, token program, authority, writable set, limits and program allowlists are validated.
- [ ] Plan bytes/digest include compute/fee/guard parameters and exact relevant accounts.
- [ ] Insufficient final balance or invalid account/program identity fails atomically in the harness.
- [ ] No production program deployment, private-key signing or broadcast is introduced.

### Verification and review evidence
- Run full-plan local validator/simulation success and rejection fixtures under declared virtual funding.
- Review compute/account-size bounds and guard bypass cases.

### Dependencies and release gate
- Prerequisites: ARB-020, ARB-021, ARB-025.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F06, PRD-F07, NFR-01.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-030 -->
## ARB-030: Implement complete Base simulation and exact-plan evidence

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** EVM engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Promote evidence only when the complete Base transaction successfully simulates against identified state. Preserve limits of fork/state-override assumptions.

### Scope
- Execute the exact recorded plan on an isolated block-pinned fork/read-only simulation environment.
- Capture state, funding/allowance overrides, artifact versions, logs, balance changes and fee estimates.
- Bind success to the exact plan digest and simulation state.

### Acceptance criteria
- [ ] Successful full execution may earn SIMULATED; quote math or failed simulation never does.
- [ ] Changed route/amount/guard/fee plan invalidates prior exact-plan evidence.
- [ ] Unavailable historical/fork state or missing required overrides remains a capability gap.
- [ ] Synthetic or overridden funding assumptions are visible and excluded from realized market claims.

### Verification and review evidence
- Run success, revert, missing allowance, stale state and digest-mismatch cases.
- Reproduce one complete simulation from the stored manifest without signing a production transaction.

### Dependencies and release gate
- Prerequisites: ARB-018, ARB-027, ARB-028.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F06, PRD-F07.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-031 -->
## ARB-031: Implement complete Solana simulation and exact-plan evidence

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Solana engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Promote evidence only from the complete Solana transaction under recorded account/state assumptions. RPC simulation and local fixtures have different limitations that must remain visible.

### Scope
- Implement read-only full-transaction simulation with pinned context where supported and an isolated local fixture path where needed.
- Record exact instructions, account versions, virtual funding, compute consumption, program logs and result.
- Tie evidence to plan digest and coherent-state policy.

### Acceptance criteria
- [ ] Successful complete simulation may earn SIMULATED; instruction fragments and mathematical replay cannot.
- [ ] Missing accounts, guard failure and unsupported token behavior produce explicit rejection evidence.
- [ ] Provider inability to simulate declared virtual funding is not hidden; the UI shows the capability limitation.
- [ ] No signature-capable key store or submission endpoint is introduced.

### Verification and review evidence
- Run success/rejection cases and changed-account/changed-plan invalidation cases.
- Reproduce one route simulation with all assumptions and relevant context retained.

### Dependencies and release gate
- Prerequisites: ARB-018, ARB-027, ARB-029.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F06, PRD-F07.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-032 -->
## ARB-032: Model delay, inclusion and failure scenarios without false certainty

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Research engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Immediate quote fills overstate paper results. Apply explicit delay/future-state scenarios and account for rejection or lost opportunity without inventing win probabilities.

### Scope
- Evaluate configured quote-to-attempt delays against later captured state when available.
- Define named inclusion/failure scenarios, expiry conditions and conservative external fee assumptions.
- Track censored/missing-future-state cases separately from failures or zero profit.

### Acceptance criteria
- [ ] Every modeled outcome names its delay/inclusion assumptions and available data window.
- [ ] No unsupported fill probability or confidence interval is presented as measured fact.
- [ ] Scenarios consume conflicting capital consistently and include rejected/expired attempts in denominators.
- [ ] Changing a scenario creates a new experiment version rather than rewriting the original result.

### Verification and review evidence
- Use fixtures where a quoted spread disappears, worsens or survives before the scenario delay.
- Inspect missing-future-state and simultaneous-capital-conflict results.

### Dependencies and release gate
- Prerequisites: ARB-025, ARB-026, ARB-027, ARB-030, ARB-031.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F05, PRD-F07, PRD-F09.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-033 -->
## ARB-033: Enforce estimated-executable evidence eligibility

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** Engine engineer / QA
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A simulated transaction is still an estimate. Centralize the complete eligibility predicate and explain every missing prerequisite.

### Scope
- Require successful exact-plan simulation, fresh/coherent/complete state and complete costs.
- Require principal/native-fee reservations, supported atomic guards, limits and a named delay/inclusion scenario.
- Return structured pass/fail reasons for each check and preserve the weaker evidence when eligibility fails.

### Acceptance criteria
- [ ] Removing any required predicate prevents ESTIMATED_EXECUTABLE.
- [ ] A stale simulation, changed plan or absent fee valuation fails eligibility.
- [ ] PAPER and REPLAY never progress to REALIZED.
- [ ] The same eligibility function serves API, exports and dashboard; display logic cannot upgrade evidence.

### Verification and review evidence
- Run a table of one-predicate-at-a-time negative cases plus a fully qualified case per chain.
- Validate the resulting objects against opportunity.schema.json.

### Dependencies and release gate
- Prerequisites: ARB-024, ARB-025, ARB-026, ARB-030, ARB-031, ARB-032.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F06, PRD-F07, PRD-F17.
- `specs/opportunity.schema.json`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-034 -->
## ARB-034: Implement fair comparison cohorts and holdout analysis

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P1 · **Responsible role:** Research engineer / Product Owner
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Different coverage and capital assumptions can make chain rankings misleading. Build comparison cohorts with explicit matched windows and a separate native opportunity view.

### Scope
- Match observation windows, starting/reference assets, capital, route sizes and declared scenario assumptions.
- Deduplicate persistent opportunities and enforce eligible sample/coverage rules.
- Reserve time-based holdout periods and separate exploratory tuning from evaluation.

### Acceptance criteria
- [ ] Comparisons show usable hours, gaps, excluded data, opportunity counts and scenario definitions.
- [ ] Insufficient overlapping coverage suppresses ranking and explains why.
- [ ] Chain-native opportunity sets are visible separately from matched experiments.
- [ ] No current 'busiest' or 'best' chain claim is inferred from unmatched counts or synthetic data.

### Verification and review evidence
- Test uneven uptime, no-opportunity periods, missing fee inputs and mismatched capital fixtures.
- Review a sample methodology report with the Product Owner.

### Dependencies and release gate
- Prerequisites: ARB-023, ARB-025, ARB-026, ARB-032, ARB-033.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F09, PRD-F10, PRD-F18.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-035 -->
## ARB-035: Pass executable-paper release gate and financial correctness review

**Milestone:** M3 · **Epic:** EPIC-04 · **Priority:** P0 · **Responsible role:** QA / Technical Lead
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Determine what the implementation may truthfully claim. A two-chain executable-paper release requires complete simulation capability, exact accounting and recoverable controls.

### Scope
- Run qualification suites and a representative end-to-end paper/replay scenario on both chains.
- Review evidence transitions, assumptions, accounting conservation and excluded costs.
- Publish a capability-based release decision with outstanding defects.

### Acceptance criteria
- [ ] At least one supported atomic route per chain fully simulates under declared virtual funding, with success and rejection evidence.
- [ ] Missing full simulation on either chain restricts release to clearly labeled quote-research preview.
- [ ] No signing, key loading or transaction broadcast path is included in the research release.
- [ ] Synthetic success fixtures prove capability only and never populate market-return reports.
- [ ] All blocking arithmetic, evidence or reservation defects are resolved before exit.

### Verification and review evidence
- Attach corpus/build/config hashes, actual CI results and an acceptance checklist.
- Technical Lead and Product Owner record supported claims and remaining limitations.

### Dependencies and release gate
- Prerequisites: ARB-024, ARB-026, ARB-027, ARB-030, ARB-031, ARB-033, ARB-034.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F05, PRD-F06, PRD-F07, PRD-F08, NFR-01.
- `docs/07-DELIVERY-PLAN.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-036 -->
## ARB-036: Implement the approved dashboard shell and design tokens

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** Frontend engineer / UI Designer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Turn the approved prototype into a maintainable application while preserving its readable research-focused hierarchy. The shell must make demo data unmistakable.

### Scope
- Implement React/TypeScript shell, dark/light tokens, sidebar and responsive layout using design/dashboard-wireframe.html as the baseline.
- Provide Overview, Opportunities, Experiments, Runs, Strategies and System routes/views.
- Keep mode, backend connection state, data age and session controls visible.

### Acceptance criteria
- [ ] Both chain panels retain equal visual weight and the lime/navy design reference is recognizable.
- [ ] Every synthetic/demo dataset is explicitly labeled and cannot be mistaken for captured market performance.
- [ ] A chain filter only filters the view; it never changes worker scope.
- [ ] No wallet-connect, private-key input or live-arm shortcut appears in the research UI.

### Verification and review evidence
- Compare actual browser screenshots at desktop and narrow widths against the reference.
- Run TypeScript/build checks and record any unavailable browser verification honestly.

### Dependencies and release gate
- Prerequisites: ARB-006, ARB-008.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: NFR-07, PRD-F12.
- `design/dashboard-wireframe.html`
- `docs/05-UX-DESIGN.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-037 -->
## ARB-037: Connect dashboard queries, provenance and data-quality states

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** Frontend engineer / Backend engineer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Replace demo-only presentation with clearly sourced backend data and honest loading/offline states. Every result should reveal its evidence and freshness.

### Scope
- Integrate typed API queries and controlled refresh/subscription behavior.
- Display evidence badges, state references, age, provider health and coverage gaps.
- Build an opportunity inspector with route, cost breakdown, checks and provenance.

### Acceptance criteria
- [ ] Loading, empty, zero-opportunity, missing-data and disconnected states are distinct.
- [ ] An unavailable cost displays unknown with a reason rather than zero.
- [ ] A failed simulation is shown as a rejection and never receives a success evidence badge.
- [ ] Disconnect freezes/labels the last known snapshot and does not continue animating invented activity.

### Verification and review evidence
- Run frontend integration cases with valid, stale, incomplete, rejected and disconnected API fixtures.
- Inspect one real recorded decision end-to-end against its API trace.

### Dependencies and release gate
- Prerequisites: ARB-012, ARB-023, ARB-036.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, PRD-F06, PRD-F12.
- `docs/05-UX-DESIGN.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-038 -->
## ARB-038: Implement experiment creation, immutable settings and capability feedback

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** Frontend engineer / Product Owner
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Operators must know what experiment they are starting and what the current adapters can support. Provide configuration review before creating a research session.

### Scope
- Build OBSERVE/PAPER/REPLAY setup with chain, registry, starting asset, sizes, virtual balances and scenario selection.
- Show effective defaults, configuration digest and capability validation results.
- Create a new session for changed modes or starting assets.

### Acceptance criteria
- [ ] Unsupported pool models/token behavior cannot be selected as if supported.
- [ ] The review screen distinguishes virtual principal from native fee reserves and displays assumptions.
- [ ] Editing a completed/running experiment cannot mutate its recorded configuration.
- [ ] No research form can enable LIVE by changing a hidden field or URL parameter.

### Verification and review evidence
- Run keyboard-driven valid and invalid creation flows with API validation errors.
- Verify immutable historical settings after creating a revised experiment.

### Dependencies and release gate
- Prerequisites: ARB-009, ARB-012, ARB-021, ARB-036.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F01, PRD-F02, PRD-F07.
- `docs/05-UX-DESIGN.md`
- `docs/01-PRD.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-039 -->
## ARB-039: Implement start, pause, resume and stop acknowledgement UX

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P0 · **Responsible role:** Frontend engineer / QA
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The control must tell the operator whether a command is accepted or actually applied. Preserve truthful behavior through delayed worker responses and unresolved attempts.

### Scope
- Present per-session desired command, command status, observed state and last worker acknowledgement.
- Implement stop-all as a visible per-session fanout with partial outcomes.
- Show PAUSING/PENDING, applied fence, DRAINING and STOPPED meanings with accessible status updates.

### Acceptance criteria
- [ ] API acceptance never immediately renders a completed stop without worker evidence.
- [ ] Pause gates new work while feeds/reconciliation continue; resume requires a valid paused session.
- [ ] DRAINING explains that previously emitted attempts remain unresolved and cannot be recalled.
- [ ] Delayed/lost ACK and partial stop-all failures remain visible with safe retry/idempotency behavior.
- [ ] A local UI/demo control cannot claim to stop a backend worker.

### Verification and review evidence
- Run control integration tests with delayed ACK, duplicate clicks, disconnect and unresolved-attempt fixtures.
- Verify keyboard focus, live status announcements and primary stop access on narrow screens.

### Dependencies and release gate
- Prerequisites: ARB-011, ARB-012, ARB-036, ARB-037.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F11, PRD-F12, NFR-07.
- `docs/05-UX-DESIGN.md`
- `docs/01-PRD.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-040 -->
## ARB-040: Build run history, portfolio ledger and comparison views

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** Frontend engineer / Research engineer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Give the operator a way to explain results and compare experiments without collapsing evidence levels or accounting categories.

### Scope
- Show immutable run metadata, virtual balances/reservations and journaled balance movements.
- Present gross, transaction-net and fully allocated results separately.
- Build matched-chain comparison views with coverage, scenarios, holdout labels and excluded data.

### Acceptance criteria
- [ ] Paper outcomes remain visibly hypothetical and never appear in a realized-profit total.
- [ ] Inventory valuation movement, transfers and arbitrage outcomes are separate.
- [ ] Insufficient comparable data produces an explanation instead of a winner badge.
- [ ] Expired raw captures show a reproducibility limitation in run history.

### Verification and review evidence
- Verify totals against ledger/API fixtures including losses and missing conversion rates.
- Conduct an operator walkthrough explaining why one route was rejected and one comparison was withheld.

### Dependencies and release gate
- Prerequisites: ARB-026, ARB-034, ARB-037, ARB-038.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F09, PRD-F10, PRD-F13.
- `docs/05-UX-DESIGN.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-041 -->
## ARB-041: Implement reproducible JSON/CSV exports and redaction

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** Data engineer / Frontend engineer
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Research must remain inspectable outside the dashboard. Export amounts and methodology without losing precision or leaking credentials.

### Scope
- Export run metadata, summaries, decision records, ledger entries and capture references.
- Include raw integer amounts, decimals, timezone, evidence labels, versions and assumptions.
- Escape spreadsheet-formula-leading cells and redact provider/authentication secrets.

### Acceptance criteria
- [ ] Large amounts round-trip without numeric precision loss.
- [ ] Exports disclose unavailable inputs, excluded observations and hypothetical outcome labels.
- [ ] CSV output cannot execute injected formulas when opened in common spreadsheet software.
- [ ] Expired or missing capture dependencies are identified rather than omitted silently.

### Verification and review evidence
- Round-trip JSON/CSV fixtures and compare raw amounts and row counts with the source query.
- Test formula injection, quotes/newlines and credential redaction cases.

### Dependencies and release gate
- Prerequisites: ARB-023, ARB-027, ARB-034, ARB-040.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F13, PRD-F16, PRD-F18.
- `docs/01-PRD.md`
- `docs/08-DATA-AND-API-CONTRACTS.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-042 -->
## ARB-042: Instrument system health, alerts and actionable diagnostics

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** Operations engineer / Frontend engineer
**Estimate:** 3-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The operator needs to distinguish a quiet market from broken infrastructure. Expose service health, backlog and data quality with actionable alert conditions.

### Scope
- Build System view metrics for providers, freshness, queue age/depth, drops, worker heartbeat and storage.
- Define alert thresholds after measured baselines and show alert delivery failures.
- Link diagnostics to redacted correlated traces and remediation runbooks.

### Acceptance criteria
- [ ] No secret or signed payload is included in telemetry.
- [ ] Alert conditions distinguish service outage, state incoherence and zero opportunity.
- [ ] A blocked analytics query does not block worker control or data ingestion.
- [ ] Every operational alert names the affected chain/session, last evidence and next safe investigation step.

### Verification and review evidence
- Inject provider outage, stale snapshots, queue overload and alert transport failure.
- Verify metrics/trace labels have bounded cardinality and useful correlation.

### Dependencies and release gate
- Prerequisites: ARB-013, ARB-024, ARB-037.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, PRD-F12, NFR-05.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/02-ARCHITECTURE.md`
- `docs/05-UX-DESIGN.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-043 -->
## ARB-043: Pass dashboard accessibility, responsiveness and failure-state review

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P1 · **Responsible role:** UI/UX Designer / QA
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The approved appearance must remain usable when data or controls are under stress. Validate actual browser behavior rather than only static markup.

### Scope
- Review keyboard navigation, focus management, dialogs, color contrast and status announcements.
- Inspect desktop/narrow layouts, long addresses, large amounts and dense cost tables.
- Test empty, loading, offline, pending, rejected and partial-control states.

### Acceptance criteria
- [ ] Core monitoring and stop workflows work without a mouse and without relying only on color.
- [ ] Dialog focus opens/closes predictably and returns to its trigger.
- [ ] Narrow layouts preserve mode, stale-data indication and control access.
- [ ] Actual browser/test versions, screenshots and unresolved accessibility findings are recorded.

### Verification and review evidence
- Run automated accessibility checks plus manual keyboard/screen-reader-oriented review.
- Capture representative browser screenshots and a checklist of fixes; static checks alone do not satisfy this ticket.

### Dependencies and release gate
- Prerequisites: ARB-037, ARB-038, ARB-039, ARB-040, ARB-041, ARB-042.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: NFR-07, PRD-F11, PRD-F12.
- `docs/05-UX-DESIGN.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-044 -->
## ARB-044: Ship the research deployment with backups and recovery proof

**Milestone:** M4 · **Epic:** EPIC-05 · **Priority:** P0 · **Responsible role:** Operations engineer / QA
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Package a usable research release that can be restored after failure and cannot move funds. Verify deployment behavior in the documented environment.

### Scope
- Provide local/private deployment configuration, health checks and operator setup instructions.
- Implement database/capture backup, restore verification and retention jobs.
- Run graceful shutdown, abrupt restart and degraded-provider exercises.

### Acceptance criteria
- [ ] A clean environment can start the inert research deployment using documented steps.
- [ ] Restore reconstructs configurations, commands, ledger and replay references with known retention limitations.
- [ ] Restart returns workers to STOPPED and preserves unapplied commands for explicit handling.
- [ ] The delivered process graph has no signer/broadcast capability and no public unauthenticated controls.
- [ ] Release notes list actual test results, supported venues and remaining capability gaps.

### Verification and review evidence
- Perform a restore drill into an isolated environment and compare manifests/record counts.
- Record research release artifact hashes and the end-to-end recovery/stop evidence.

### Dependencies and release gate
- Prerequisites: ARB-035, ARB-039, ARB-041, ARB-042, ARB-043.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F13, PRD-F16, NFR-04, NFR-06.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/07-DELIVERY-PLAN.md`
- `docs/02-ARCHITECTURE.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-045 -->
## ARB-045: Register the observation campaign and analysis protocol

**Milestone:** M5 · **Epic:** EPIC-06 · **Priority:** P0 · **Responsible role:** Product Owner / Research engineer
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Predefine how the experiment will be assessed so strategy tuning does not quietly rewrite the success criteria. Calendar duration alone is insufficient.

### Scope
- Record configured chains/pools, capital, sizes, scenarios, cost allocation and comparison windows.
- Define usable coverage requirements, holdout periods, exclusion policy and stopping conditions.
- Plan an initial roughly 30-calendar-day campaign subject to adequate coverage and costs.

### Acceptance criteria
- [ ] The protocol is versioned before collecting the primary evaluation window.
- [ ] Exploratory tuning and held-out evaluation periods are labeled separately.
- [ ] Coverage shortfalls extend or invalidate comparisons instead of becoming zero-opportunity evidence.
- [ ] No campaign automatically escalates from paper to live.

### Verification and review evidence
- Review the protocol with Product Owner, Technical Lead and operator.
- Publish a dry-run report proving all required data/denominators are available.

### Dependencies and release gate
- Prerequisites: ARB-034, ARB-044.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F09, PRD-F18.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-046 -->
## ARB-046: Operate the campaign and keep a data-quality incident log

**Milestone:** M5 · **Epic:** EPIC-06 · **Priority:** P0 · **Responsible role:** Operations engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A credible economics report depends on knowing where observations were usable. Run the protocol and document incidents and exclusions as they occur.

### Scope
- Monitor both chains, provider spending, storage, freshness and replay-sample availability.
- Record outages, reconnect gaps, configuration changes and market-universe changes.
- Keep daily/periodic coverage summaries and link remediation tickets.

### Acceptance criteria
- [ ] Each campaign day identifies usable coverage per chain and reasons for exclusions.
- [ ] Configuration changes split experiment versions and do not contaminate holdout data.
- [ ] Synthetic tests stay segregated from campaign records.
- [ ] Coverage/cost stopping conditions are enforced and operator-visible.

### Verification and review evidence
- Sample raw captures against summary counts throughout the campaign.
- Review the incident log and coverage report before analysis acceptance.

### Dependencies and release gate
- Prerequisites: ARB-045.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F03, PRD-F13, NFR-05.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Estimate covers setup and periodic operator effort; the observation window is separate elapsed time, normally 4–6 weeks and longer if coverage is insufficient.

---

<!-- arb-ticket:ARB-047 -->
## ARB-047: Audit replay samples and investigate accounting or model discrepancies

**Milestone:** M5 · **Epic:** EPIC-06 · **Priority:** P0 · **Responsible role:** QA / Research engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Look for errors that a positive paper result could conceal. Reproduce stratified samples and reconcile reported economic components before drawing conclusions.

### Scope
- Sample accepted, rejected, expired, negative and missing-data cases across both chains and varied conditions.
- Recompute quotes/costs from retained captures and verify ledger/aggregate consistency.
- Track discrepancies by severity, affected data range and remediation.

### Acceptance criteria
- [ ] Sampling methodology and sample count are disclosed.
- [ ] Unresolved correctness discrepancies exclude affected results from decision-quality reports.
- [ ] Missing captures are reported as unavailable evidence, not assumed reproduction successes.
- [ ] Corrections produce new versioned analysis outputs with links to prior results.

### Verification and review evidence
- Publish replay hashes, sampled case references and difference summaries.
- Have a reviewer other than the primary calculation author inspect representative cases where feasible.

### Dependencies and release gate
- Prerequisites: ARB-046.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F05, PRD-F08, PRD-F10, NFR-01.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-048 -->
## ARB-048: Produce the comparative economics and feasibility report

**Milestone:** M5 · **Epic:** EPIC-06 · **Priority:** P0 · **Responsible role:** Research engineer / Product Owner
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Answer which tested configuration merits further work using recorded evidence and total costs. A finding of no credible advantage is a successful research outcome.

### Scope
- Report matched and native opportunity views, net outcomes, delay sensitivity, coverage and operating costs.
- Discuss competition/inclusion uncertainty and limits of the paper model.
- Compare continuing, narrowing, expanding observation or stopping the project.

### Acceptance criteria
- [ ] The report identifies the exact protocol/data/config/version window and excludes synthetic fixtures.
- [ ] No global busiest-chain claim is inferred from this limited market universe.
- [ ] Estimated profits are labeled hypothetical and losses/failed scenarios are retained.
- [ ] The recommendation names uncertainties that would change the conclusion and links reproducible exports.

### Verification and review evidence
- Reconcile tables against exported records and disclose aggregation methods.
- Product Owner reviews whether the report answers the agreed research questions.

### Dependencies and release gate
- Prerequisites: ARB-046, ARB-047.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F05, PRD-F09, PRD-F18.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/07-DELIVERY-PLAN.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-049 -->
## ARB-049: Run operational and research-security readiness review

**Milestone:** M5 · **Epic:** EPIC-06 · **Priority:** P0 · **Responsible role:** Technical Lead / Security reviewer
**Estimate:** 2-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Assess the sustained deployment and recovery evidence before proposing more responsibility. Fix weaknesses surfaced during the observation campaign.

### Scope
- Review authentication, secrets, backups, incident handling, dependency posture and retention.
- Repeat only the recovery/failure drills affected by actual changes or unresolved incidents.
- Publish remaining limitations and an owner for each remediation.

### Acceptance criteria
- [ ] No unresolved research-critical security or accounting defect is hidden by release status.
- [ ] Provider credentials remain absent from captures, logs and exports.
- [ ] Restore and stop/restart procedures match current deployment behavior.
- [ ] Review scope explicitly distinguishes research readiness from independent live-execution approval.

### Verification and review evidence
- Attach current security/recovery findings and evidence references.
- Verify remediation on realistic failure cases rather than checklist assertions alone.

### Dependencies and release gate
- Prerequisites: ARB-044, ARB-047.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F16, NFR-04, NFR-06.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-050 -->
## ARB-050: Record the go, revise or stop decision and optional live prerequisites

**Milestone:** M5 · **Epic:** EPIC-06 · **Priority:** P0 · **Responsible role:** Operator / Product Owner
**Estimate:** 1-2 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Keep the decision to risk real capital separate from technical progress. Present concrete evidence and choices for the operator.

### Scope
- Record whether to stop, extend research, narrow scope or consider one bounded live strategy.
- For live consideration, identify capital/loss/fee ceilings, custody, venue permissions, review budget and target chain.
- List evidence gaps and prerequisite approvals without enabling any execution path.

### Acceptance criteria
- [ ] The chosen action and rationale cite the feasibility and readiness reports.
- [ ] A positive paper result alone cannot satisfy the live gate.
- [ ] Unanswered custody, limits, review or funding questions keep live work unarmed.
- [ ] A stop decision includes export/retention and infrastructure shutdown instructions.

### Verification and review evidence
- Operator records an explicit dated decision; agents may prepare but cannot substitute their approval.
- Check downstream live tickets remain gated to the approved scope.

### Dependencies and release gate
- Prerequisites: ARB-048, ARB-049.
- Research work only. Signing, production execution deployment, funding and live activation are outside this milestone.

### References
- Requirements: PRD-F14, PRD-F17.
- `docs/07-DELIVERY-PLAN.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-051 -->
## ARB-051: Approve a concrete live execution design and limit specification

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Architect / Operator
**Estimate:** 2-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Define a narrow execution responsibility before introducing signing. Translate the operator's go decision into an exact design and enforceable permissions.

### Scope
- Select one initial reviewed chain/strategy, funding source, signer custody and submission mechanism.
- Specify per-trade principal, inventory, fee, daily loss and pending-attempt limits with valuation/finality rules.
- Document spending account/executor balances, allowances, allowed targets/programs and recovery ownership.

### Acceptance criteria
- [ ] All live limits have explicit units, time/reset semantics and fail-closed behavior.
- [ ] Fees and trading principal are separately funded at the actual authorized spending locations.
- [ ] Approved scope excludes automatic multi-chain expansion or flash-loan activation.
- [ ] The operator approves the exact proposal; this ticket's creation or engineering completion does not fund or arm it.

### Verification and review evidence
- Threat-model the complete funds and authority flow with an independent reviewer.
- Publish an approved configuration template containing no secrets or funded addresses unless intentionally chosen by the operator.

### Dependencies and release gate
- Prerequisites: ARB-050.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F17, NFR-06.
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-052 -->
## ARB-052: Implement isolated signer policies, epochs and durable disarm

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Security engineer
**Estimate:** 5-8 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A signer must enforce narrow transaction permissions independently of the dashboard. Disarm must revoke future signing authority durably, not merely hide a button.

### Scope
- Separate signer process/credentials and authenticate caller/session/configuration/epoch.
- Validate decoded transaction targets, chain, values, guards, limits and permitted signing envelopes.
- Persist epoch revocation before acknowledging DISARM; quarantine late signing responses after worker fences.

### Acceptance criteria
- [ ] The signer refuses arbitrary payload signing and out-of-policy targets/amounts.
- [ ] DISARM remains PENDING while the signer cannot durably acknowledge revocation.
- [ ] APPLIED pause/stop is distinguished from signer revocation; already admitted operations may return bytes which are quarantined.
- [ ] Restart cannot restore a revoked epoch or automatically arm a session.
- [ ] Keys never enter browser, API logs, fixtures or ordinary exports.

### Verification and review evidence
- Run policy-bypass, replayed-request, revoked-epoch, unavailable-signer and late-response tests.
- Review IPC authentication and custody implementation with the independent reviewer.

### Dependencies and release gate
- Prerequisites: ARB-051.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F16, PRD-F17.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-053 -->
## ARB-053: Harden execution artifacts and enforce live atomic constraints

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Chain engineer / Security reviewer
**Estimate:** 6-10 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Research simulation artifacts need production-specific hardening and review before they can hold approvals or enforce real transaction limits.

### Scope
- Harden only the selected chain's executor/guard and route encoders for the approved live scope.
- Review callback/account/program authority, token behavior, allowances, reentrancy and asset accounting.
- Produce reproducible builds, deployment manifest and verification instructions without deploying until approved.

### Acceptance criteria
- [ ] Final-balance and minimum-output guards prevent violating approved transaction-level conditions.
- [ ] Unauthorized targets, callbacks, accounts and unsupported token behavior fail atomically.
- [ ] Upgrade/admin controls and any withdrawal/recovery authority are explicit and reviewed.
- [ ] Artifacts are reproducible and traceable to source/build hashes.
- [ ] No statement equates a transaction-level guard with guaranteed account-level profit after external fees.

### Verification and review evidence
- Run adversarial fork/local-validator tests, invariant/fuzz suites and malformed-route cases.
- Provide artifacts and threat assumptions for independent review before deployment/funding.

### Dependencies and release gate
- Prerequisites: ARB-028, ARB-029, ARB-051.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F17, NFR-01, NFR-06.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/02-ARCHITECTURE.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-054 -->
## ARB-054: Implement live inventory, fee reservations and submission-time risk checks

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Engine engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Live balance and limit checks must be performed against the account that actually spends funds. Prevent overlapping attempts from consuming the same principal or fee budget.

### Scope
- Reconcile actual chain balances, allowances/authority and reserved funds by account/executor.
- Check live limits at submission admission using durable state and fresh fee bounds.
- Block submissions on unavailable balances, unsettled discrepancies or exhausted caps.

### Acceptance criteria
- [ ] Available fee balance alone cannot authorize a principal-consuming route.
- [ ] Concurrent attempts cannot exceed principal, native-fee or pending-attempt limits.
- [ ] Loss/fee limits use the approved accounting/finality policy and cannot be reset by a worker restart.
- [ ] Changing limits requires a new approved configuration/revision and is audited.
- [ ] State unavailability fails closed for new live admissions while reconciliation continues.

### Verification and review evidence
- Run concurrency, stale-balance, allowance-loss, fee-spike and crash-recovery cases.
- Verify ledger invariants against a local-chain/fork account model.

### Dependencies and release gate
- Prerequisites: ARB-026, ARB-051.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F15, PRD-F17.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-055 -->
## ARB-055: Implement durable intent, signed-payload and dispatch-start journals

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Backend engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
A network timeout does not prove a transaction was not sent. Persist enough state before each send to recover unknown outcomes without duplicating trades.

### Scope
- Journal intent/reservation and immutable transaction plan before signing.
- Persist signed bytes/hash/reference and a synchronous dispatch-start UNKNOWN record before every network send.
- Define recovery for each crash boundary and protected handling of signed payloads.

### Acceptance criteria
- [ ] No network dispatch occurs before its durable dispatch-start record.
- [ ] Signed bytes are treated as sensitive executable authority and excluded from general logs/exports.
- [ ] A timeout or crash after dispatch-start remains UNKNOWN until reconciled.
- [ ] Retries reuse a reviewed policy and never blindly re-sign/re-submit a replacement trade.
- [ ] Analytics latency cannot bypass or replace the critical durable journal.

### Verification and review evidence
- Fault-inject every boundary from reservation through returned network response.
- Prove recovery enumerates every possibly emitted attempt, including transport exceptions and missing responses.

### Dependencies and release gate
- Prerequisites: ARB-010, ARB-052, ARB-054.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F15, NFR-04.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-056 -->
## ARB-056: Integrate the approved chain-specific submission transport

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Chain engineer
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Submit only approved exact plans through one explicit transport and preserve uncertain outcomes. Transport acceptance is not inclusion or profit.

### Scope
- Integrate the selected Base or Solana submission path with configured fee/tip bounds and expiry policy.
- Bind admission to current command fence, signer epoch, plan digest and risk reservations.
- Record transport responses, attempts and externally assigned identifiers without trusting them as final settlement.

### Acceptance criteria
- [ ] No send begins after the local fence is applied, except an already in-flight network call whose outcome remains tracked.
- [ ] Rejected/timeout/ambiguous responses map to explicit attempt states.
- [ ] Nonce or blockhash validity and replacement policy are chain-specific and tested.
- [ ] Submission responses cannot emit REALIZED evidence.
- [ ] A second chain/transport remains disabled until separately qualified.

### Verification and review evidence
- Use deterministic transport faults and isolated chain environments to exercise every response class.
- Measure decision-to-dispatch timing separately from inclusion/settlement delays.

### Dependencies and release gate
- Prerequisites: ARB-053, ARB-055.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F15, PRD-F17, NFR-02.
- `docs/02-ARCHITECTURE.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-057 -->
## ARB-057: Reconcile pending, unknown, failed and finalized live outcomes

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Chain engineer / Backend engineer
**Estimate:** 5-8 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Turn chain evidence into reliable account outcomes without mistaking a missing response for failure. Settlement and rollback rules must be explicit per supported chain.

### Scope
- Track emitted attempts through pending, unknown, inclusion, failure, expiry and configured finality.
- Reconcile actual balance deltas, gas/fees/tips and reservations into the ledger.
- Handle Base reorgs or Solana commitment/rollback changes under the approved policy.

### Acceptance criteria
- [ ] REALIZED is issued only after configured settlement and balance/cost reconciliation; losses remain valid outcomes.
- [ ] Unknown attempts retain reservations according to the reviewed policy until resolved.
- [ ] Unexplained balance discrepancies halt new submissions for the affected account.
- [ ] Restart reconstructs all outstanding attempts before permitting new admission.
- [ ] The UI/export exposes settlement policy and any later correction/rollback.

### Verification and review evidence
- Run timeout-then-included, expired, failed, reorged and discrepant-balance scenarios.
- Reconcile local/fork ledger totals against independent chain queries.

### Dependencies and release gate
- Prerequisites: ARB-055, ARB-056.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F10, PRD-F15, NFR-04.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-058 -->
## ARB-058: Implement fencing, crash recovery and controlled takeover

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Systems engineer / QA
**Estimate:** 4-6 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Lease expiry cannot invalidate a signed transaction. Prevent simultaneous live authority and make takeover a reconciled, controlled operation.

### Scope
- Implement worker ownership/fencing tokens and a documented old-process termination/isolation procedure.
- On restart or takeover, revoke/verify signer authority and reconcile outstanding attempts before rearming.
- Preserve stop/pause/disarm semantics through unreachable components.

### Acceptance criteria
- [ ] No automatic live takeover occurs based only on lease timeout.
- [ ] Previously signed bytes and in-flight sends remain tracked even after signer epoch revocation.
- [ ] A new worker cannot admit live work before prior ownership is fenced and unresolved attempts are reconciled per policy.
- [ ] Boot remains unarmed STOPPED after RECOVERING; manual approved arming is required.
- [ ] Stop-all exposes partial progress without claiming globally instantaneous revocation.

### Verification and review evidence
- Exercise split-brain, long process pause, signer outage, network partition and crash during send.
- Run the recovery procedure with a reviewer acting as the operator.

### Dependencies and release gate
- Prerequisites: ARB-011, ARB-052, ARB-055, ARB-057.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F11, PRD-F15, NFR-04.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/02-ARCHITECTURE.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-059 -->
## ARB-059: Add explicit live readiness, arming and incident-control UX

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Frontend engineer / Security engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The future live interface must present the exact responsibility being accepted. Add gated readiness and arming only after backend enforcement exists.

### Scope
- Show selected chain, signer identity reference, executor version, route permissions, capital/fee/loss limits and configuration hash.
- Require an authenticated explicit operator arm action with current readiness evidence.
- Display stop, disarm, pending/unknown attempts, settlement policy and reconciliation alerts.

### Acceptance criteria
- [ ] A PAPER session cannot mutate into LIVE; arming creates/uses a separate approved live session.
- [ ] Missing review, signer acknowledgement, balances or limits blocks arming with specific reasons.
- [ ] Stop and DISARM are distinct and their acknowledgements accurately reflect worker/signer application.
- [ ] The UI never promises recall of emitted transactions or guaranteed net profit.
- [ ] Authentication/CSRF and stale-revision checks cover every live mutation.

### Verification and review evidence
- Run security and UI integration cases for stale approval, changed config, signer outage and partial disarm.
- Complete keyboard/narrow-screen review of incident controls.

### Dependencies and release gate
- Prerequisites: ARB-039, ARB-051, ARB-052, ARB-054, ARB-057, ARB-058.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F15, PRD-F17, NFR-07.
- `docs/05-UX-DESIGN.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/01-PRD.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-060 -->
## ARB-060: Complete independent execution review and adversarial remediation

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Independent reviewer / Implementers
**Estimate:** 5-10 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Obtain qualified review outside implementation authorship before any funded execution. Automated agents and passing tests do not substitute for this gate.

### Scope
- Review signer, artifacts, allowances, authority, journal, transport, accounting, fencing and UI activation paths.
- Classify findings and remediate/retest affected behavior.
- Publish a scoped report with exact source/artifact versions and residual risks.

### Acceptance criteria
- [ ] No unresolved critical/high finding affecting approved funds/authority remains at the live gate.
- [ ] Review covers the exact intended deployment/configuration rather than an obsolete prototype.
- [ ] Regression evidence addresses each fixed finding.
- [ ] Review limitations and retained risks are visible to the operator.
- [ ] Independent reviewer identity/scope is real and cannot be claimed by agent self-review.

### Verification and review evidence
- Attach review deliverables, remediation commits and focused regression results.
- Re-run affected adversarial scenarios; expand tests when findings justify it.

### Dependencies and release gate
- Prerequisites: ARB-052, ARB-053, ARB-054, ARB-055, ARB-056, ARB-057, ARB-058, ARB-059.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F16, PRD-F17, NFR-06.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Estimate is internal review/remediation coordination effort. External review pricing, reviewer availability and elapsed time are separate; findings can increase implementation effort.

---

<!-- arb-ticket:ARB-061 -->
## ARB-061: Prepare the bounded live pilot release and final approval packet

**Milestone:** M6 · **Epic:** EPIC-07 · **Priority:** P0 · **Responsible role:** Technical Lead / Operator
**Estimate:** 2-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Make the final live decision concrete and reviewable. Assemble exact artifacts, configuration, limits, funding steps and recovery proof without activating prematurely.

### Scope
- Produce reproducible release/deployment hashes and approved configuration for one limited pilot.
- Provide transaction review, funding/allowance steps, stop/disarm/recovery and incident escalation procedures.
- Record independent review status, dry-run evidence and explicit operator acceptance conditions.

### Acceptance criteria
- [ ] Every deployed artifact and policy can be matched to reviewed source and build output.
- [ ] An operator can identify principal, fee reserve, maximum intended exposure and withdrawal authority before funding.
- [ ] No deployment/funding/arm step is performed merely by merging this ticket.
- [ ] The final activation packet requires explicit operator authorization for its exact version and limits.
- [ ] Any unready gate leaves the pilot unavailable.

### Verification and review evidence
- Conduct a tabletop drill and an unfunded isolated execution rehearsal.
- Technical Lead verifies the packet and the operator records a go/no-go decision.

### Dependencies and release gate
- Prerequisites: ARB-050, ARB-060.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F15, PRD-F17.
- `docs/07-DELIVERY-PLAN.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-062 -->
## ARB-062: Deploy and run one explicitly approved bounded live pilot

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P0 · **Responsible role:** Operator / Operations engineer
**Estimate:** 3-5 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Test the reviewed execution path under narrowly bounded real conditions only after the final operator approval. The pilot prioritizes reconciled behavior over trading volume.

### Scope
- Deploy/verify only the approved artifact and configuration, then fund the declared principal/fee locations under approved limits.
- Arm manually and observe a limited window/attempt budget with immediate incident control.
- Retain every submission, unknown outcome, reconciliation and operator action.

### Acceptance criteria
- [ ] Deployment, funding and arming match the exact written operator approval.
- [ ] Limits are enforced and any discrepancy/unresolved critical incident halts new submissions.
- [ ] All emitted attempts reconcile or remain explicitly unresolved; none disappear from the report.
- [ ] The pilot does not automatically scale capital, add routes or enable the second chain.
- [ ] Stop/disarm outcomes and residual pending exposure are verified at pilot end.

### Verification and review evidence
- Compare actual chain transactions/balance changes with the durable journal and ledger.
- Run the approved closeout checklist and preserve incident evidence.

### Dependencies and release gate
- Prerequisites: ARB-061.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F15, PRD-F17.
- `docs/07-DELIVERY-PLAN.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-063 -->
## ARB-063: Evaluate the pilot and approve continuation, reduction or shutdown

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P0 · **Responsible role:** Product Owner / Operator
**Estimate:** 2-3 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Use actual reconciled outcomes to assess the gap between paper assumptions and reality. Continued operation is a separate decision from completing a pilot.

### Scope
- Compare realized fees, inclusion, failures, latency and P&L against the pre-registered paper scenarios.
- Review incidents, unresolved outcomes, operational burden and fully allocated costs.
- Propose continue, revise, reduce, stop or separately approve expansion.

### Acceptance criteria
- [ ] Only reconciled settled transactions contribute to realized totals and the finality policy is disclosed.
- [ ] Unresolved attempts and accounting differences remain explicit blockers where material.
- [ ] No extrapolated guaranteed income is reported from a small sample.
- [ ] Any increase in exposure or supported scope requires updated limits/review and operator approval.

### Verification and review evidence
- Reconcile the report against chain evidence and exports.
- Operator records the next decision with artifact/configuration versions and bounded scope.

### Dependencies and release gate
- Prerequisites: ARB-062.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F09, PRD-F10, PRD-F17.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/07-DELIVERY-PLAN.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-064 -->
## ARB-064: Qualify a second venue per chain for research coverage

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P1 · **Responsible role:** Chain engineers
**Estimate:** 5-8 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Broaden observable opportunities only after the first experiment is reliable. Candidate Sushi V2/Base and Raydium CPMM/Solana integrations require deployment and liquidity verification.

### Scope
- Verify official program/contract identities, liquidity, token behavior and data availability for one candidate at a time.
- Implement pool-specific decode/quote/capture and capability qualification.
- Add cross-venue two-leg routes with separate experiment versions and fair comparison coverage.

### Acceptance criteria
- [ ] The expansion review records an actual qualified venue/pool set or rejects an unsuitable candidate.
- [ ] Exact arithmetic passes golden/differential fixtures before activation.
- [ ] Quote capability does not imply full simulation or live readiness.
- [ ] Research expansion remains signing-free; live support needs the relevant M6 review and operator approval.

### Verification and review evidence
- Run the new venue's full adapter corpus and cross-venue route fixtures.
- Measure whether additional coverage changes conclusions without mixing unmatched time windows.

### Dependencies and release gate
- Prerequisites: ARB-035, ARB-048, ARB-050.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F02, PRD-F04, PRD-F06.
- `docs/01-PRD.md`
- `docs/02-ARCHITECTURE.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`

### Scope boundaries
This is optional research expansion and does not depend on running a live pilot. Either named candidate may be replaced or rejected after current primary-source/deployment verification.

---

<!-- arb-ticket:ARB-065 -->
## ARB-065: Add reviewed token universes and optional three-leg research routes

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P2 · **Responsible role:** Engine engineer / Chain engineers
**Estimate:** 4-7 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Additional coins require qualified token behavior, liquidity and computational bounds. Expand the route engine deliberately instead of allowing arbitrary ticker lists.

### Scope
- Introduce a reviewed token registry expansion with decimals, program/contract behavior and risk exclusions.
- Implement bounded three-leg discovery only for supported pool types and explicitly configured experiments.
- Extend paper balance, cost and replay tests for varied decimals and starting assets.

### Acceptance criteria
- [ ] Unsupported rebasing, transfer-fee or extension behavior is rejected unless specifically implemented and qualified.
- [ ] Every new route closes in the configured starting asset and stays within one chain.
- [ ] Search complexity, queue limits and freshness budgets remain bounded.
- [ ] New starting assets require new sessions and separate comparison/valuation assumptions.
- [ ] No expanded route receives executable evidence without its own exact full-plan simulation and eligibility checks.

### Verification and review evidence
- Run multi-decimal/token-behavior fixtures and bounded graph stress tests.
- Review experiment comparability and additional risk assumptions before activation.

### Dependencies and release gate
- Prerequisites: ARB-064.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F01, PRD-F02, PRD-F04, NFR-03.
- `docs/01-PRD.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/02-ARCHITECTURE.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.

---

<!-- arb-ticket:ARB-066 -->
## ARB-066: Qualify an additional chain through the adapter and operations contract

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P2 · **Responsible role:** Architect / Chain engineer
**Estimate:** 6-10 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The modular design can support more chains, but a shared interface does not erase chain-specific correctness or operations work. Select one new chain based on evidence and cost.

### Scope
- Evaluate data access, liquidity, transaction/fee model, finality, language/SDK fit and operating budget.
- Implement the selected chain's read/quote/capture/replay capabilities and lifecycle isolation.
- Qualify full simulation separately and document any unsupported execution capability.

### Acceptance criteria
- [ ] Selection cites current primary sources and an explicit experiment hypothesis, not an unverified popularity ranking.
- [ ] The chain meets the same identity, arithmetic, freshness, evidence and recovery gates as Base/Solana.
- [ ] The dashboard can show the chain without pretending cross-chain atomicity.
- [ ] Paper/live capability levels remain separate and initial activation remains research-only.

### Verification and review evidence
- Run a complete adapter qualification corpus and one end-to-end observation/replay slice.
- Record measured resource cost and operational/finality limitations.

### Dependencies and release gate
- Prerequisites: ARB-048, ARB-050, ARB-064.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F02, PRD-F03, PRD-F06, NFR-04.
- `docs/02-ARCHITECTURE.md`
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Bridging and cross-chain arbitrage remain out of scope. This ticket is a bounded feasibility/initial adapter slice; complex chains may require a separately estimated implementation epic.

---

<!-- arb-ticket:ARB-067 -->
## ARB-067: Assess and implement an optional atomic funding adapter

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P2 · **Responsible role:** EVM or Solana engineer / Security reviewer
**Estimate:** 5-9 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
Flash loans or other atomic funding add lender-specific authority and repayment conditions. Add one only if measured strategy economics justify the complexity.

### Scope
- Verify a real lender/deployment, asset liquidity, fees and callback/repayment semantics from primary sources and chain state.
- Model funding fees and exact repayment into the complete transaction and final-balance guard.
- Implement isolated fixtures and submit changes for fresh independent review before any live use.

### Acceptance criteria
- [ ] Repayment is enforced atomically and unauthorized lender callbacks/targets are rejected.
- [ ] Fees and capital assumptions are reflected once in economics and evidence eligibility.
- [ ] Insufficient liquidity or unavailable lender support prevents activation.
- [ ] The new adapter does not inherit prior live approval; updated artifacts, limits and operator approval are required.
- [ ] Prefunded paper and existing approved operation continue to function without this adapter.

### Verification and review evidence
- Run repayment-failure, callback-spoof, fee-change and insufficient-liquidity tests.
- Attach an incremental independent review and an updated economic justification.

### Dependencies and release gate
- Prerequisites: ARB-051, ARB-053, ARB-060, ARB-063.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F14, PRD-F17.
- `docs/04-TRADING-AND-PAPER-MODEL.md`
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/07-DELIVERY-PLAN.md`

### Scope boundaries
Optional later scope. Flash loans are not required for initial paper/live operation, and this estimate excludes substantial audit remediation or new lender-program development.

---

<!-- arb-ticket:ARB-068 -->
## ARB-068: Establish maintenance, change control and safe project closeout

**Milestone:** M7 · **Epic:** EPIC-08 · **Priority:** P1 · **Responsible role:** Technical Lead / Operations / Operator
**Estimate:** 2-4 working person-days; proposed, not a delivery commitment. **Status:** planned; any scaffold still requires the acceptance evidence below.

### Problem and intended outcome
The project needs an end state whether it becomes useful software or a stopped experiment. Preserve evidence and prevent stale credentials or permissions from outliving operations.

### Scope
- Define dependency/provider/program change review, registry requalification and periodic restore checks.
- Document research shutdown and, if live was approved, stop/disarm/reconcile/withdraw/revoke procedures with operator authority.
- Archive reports, manifests and decisions under the retention policy and cancel approved unused infrastructure subscriptions.

### Acceptance criteria
- [ ] Protocol/program changes invalidate affected qualification until reviewed.
- [ ] Closing a research deployment exports evidence and removes credentials without claiming historical data is fully reproducible after expiry.
- [ ] Closing live operation cannot treat unknown submissions as settled or skip operator-authorized custody/allowance handling.
- [ ] Repository documentation states support status, unresolved issues and how to reproduce retained results.
- [ ] Recurring work has an owner/cadence; scheduled external actions are created only when explicitly authorized.

### Verification and review evidence
- Run a tabletop closeout from both research-only and unresolved-live-attempt scenarios.
- Verify archival integrity and current restore instructions.

### Dependencies and release gate
- Prerequisites: ARB-044, ARB-050.
- This ticket is a specification, not authorization to trade. Work requiring production deployment, keys, capital or live activation waits for the recorded operator gate and its exact approved scope.

### References
- Requirements: PRD-F13, PRD-F16, NFR-04, NFR-06, NFR-08.
- `docs/06-SECURITY-OPERATIONS-AND-TESTING.md`
- `docs/07-DELIVERY-PLAN.md`
- `docs/09-DECISIONS-AND-OPEN-QUESTIONS.md`

### Scope boundaries
Do not silently expand supported chains, venues, assets or evidence claims. Record any new scope as a separately reviewed ticket.
