# Delivery plan and team ownership

Version: 0.3. Research integration now includes capture/registry, exact candidate math, durable decisions/virtual accounts and connected dashboard source. The [delivery backlog](../planning/backlog.json) and milestone table remain TARGET acceptance, not blanket completion or a commercial quotation. [Integration verification](17-RESEARCH-INTEGRATION-VERIFICATION.md) records pending CI and exact accepted evidence. Multi-agent implementation/review does not constitute an independent audit or a staffed delivery company.

## 1. Delivery approach

Deliver a narrow, working observation-to-paper loop before expanding venues or enabling signing. Develop the shared Rust types and evidence model once, then implement chain-specific data and execution adapters. Integrate continuously so the operator sees actual observations, data gaps and rejection reasons early.

The first release contains Solana and Base workers, a shared paper/replay engine, persistence and a usable dashboard. Phase 1 uses USDC-starting cycles across distinct eligible pools on Uniswap V3 (Base) and Orca (Solana); cross-venue expansion follows adapter gates. Live execution is a separate release; flash loans, additional chains and CEX strategies have separate scope decisions.

## 2. Milestones and dependencies

| Milestone / epic | Deliverable | Exit gate | Dependency |
|---|---|---|---|
| M0 / EPIC-01 | Agreed PRD, data-provider shortlist, initial venue/token registry, deployment and performance assumptions. | Representative pool data can be obtained; protocol coverage and costs are understood; open decisions have owners. | Operator budget and access details. |
| M1 / EPIC-02 | Rust workspace, configuration validation, health API, persistence, one end-to-end chain observation. | Captured state produces a versioned decision trace; restart and gap detection demonstrated; preliminary latency budgets recorded. | M0. |
| M2 / EPIC-03 | Solana and Base adapters, allowlists, correct pool calculations, bounded route discovery. | Each supported pool type passes golden/differential cases; stale or inconsistent state cannot produce an executable estimate. | M1; chain implementations can run in parallel. |
| M3 / EPIC-04 | Paper portfolios, cost model, delay/inclusion scenarios and replay; minimal execution artifacts/encoders, complete transaction builders/atomic guards, and read-only local-fork/Solana simulation support. No production deployment or signing. | At least one supported atomic route per chain fully simulates under declared virtual funding; capital reservations and costs are correct; quote replay stays CANDIDATE. If unavailable, release only a labeled quote-research preview, not executable-paper comparison. | M1 types and M2 adapters. |
| M4 / EPIC-05 | Operator dashboard, controls, comparisons, exports and observability. | One complete comparison explains coverage and assumptions; worker acknowledgement, pause/drain/restart behavior is demonstrated; initial release has no signer. | M1 API; integrate alongside M2–M3. |
| M5 / EPIC-06 | Observation campaign, economics report and operating runbook. | Agreed coverage achieved; anomalies resolved or explicitly excluded; feasibility decision recorded with costs and uncertainty. | M2–M4. |
| M6 / EPIC-07 | Optional bounded live execution: reviewed deployment of execution artifacts, isolated signer, submission integration and settlement/reconciliation recovery. | Execution-specific tests and independent review completed; ambiguous submission, restart, limit and reconciliation exercises pass. | M5 decision plus live budget and operator authorization. |
| M7 / EPIC-08 | Optional limited live pilot and later venue/chain expansion. | Operator-approved limits; actual outcomes reconciled; each expansion passes adapter and execution gates. | M6; reviewed deployment and funding. |

M0–M4 are the initial build. M3 simulation uses declared virtual funding and requires no funded wallet; the complete transaction plans and guards must exist before executable-paper claims. The M3 simulation gate is a technical capability test using clearly labelled success and rejection fixtures; it does not require a profitable current-market opportunity. Synthetic fixture states never enter market-return reports. M5 runs the experiment needed to assess live suitability. Work on observability, UX and retention begins early; it is not deferred to a final polishing phase.

## 3. Effort and schedule assumptions

A reasonable planning range for M0–M4 is **22–36 engineering person-weeks**, plus **3–5 person-weeks of product/UX/QA/operations support**. With two senior engineers working consistently, supported part-time by product, design and operations, allow approximately **12–20 elapsed weeks** including integration. One engineer would more plausibly require **6–10 months**, with fewer opportunities for independent review.

Assumptions: existing hosted RPC/data services; a small supported pool set; no custom validator infrastructure; a conventional web interface; no enterprise tenancy; and no attempt to make every venue live-compatible immediately. Personnel are experienced in Rust and at least one relevant chain. Adapter complexity and capture quality dominate uncertainty.

Allow a further **4–6 elapsed weeks** for the initial observation campaign and investigation, overlapping noncritical improvements where possible. Insufficient market coverage extends the campaign; calendar time alone does not satisfy its exit gate.

M6–M7 can add approximately **12–24 engineering person-weeks**, plus independently scheduled security review and remediation. Reserve **8–16 or more elapsed weeks** with two engineers after a go decision. Custom execution programs, flash loans, difficult submission infrastructure or external review findings can increase this substantially. A two-chain live launch is not required: advance one reviewed strategy first.

These ranges are original planning judgments, not measured delivery guarantees. Re-estimate after M1 using actual adapter work, latency profiles and failure investigations. Provider subscriptions, hosting, review fees, execution fees and trading capital are separate monetary budgets; a provider spending ceiling must be agreed before purchases.

## 4. Company-style ownership

Roles below describe responsibilities even when several belong to one person. R = responsible for doing the work; A = accountable for acceptance; C = consulted; I = informed. Independent review means a qualified reviewer outside the implementation authorship, not another automated agent.

| Decision / deliverable | R | A | C | I |
|---|---|---|---|---|
| Scope, outcomes and research priorities | Product Owner | Operator | Architect, chain engineers | Designer, operations |
| Architecture and shared interfaces | Architect | Technical Lead | Chain engineers, security reviewer | Product Owner |
| Adapter and arithmetic correctness | Chain engineers | Technical Lead | QA, protocol/security reviewer | Product Owner |
| Paper assumptions and experiment methodology | Engine engineer | Product Owner | Architect, QA, operator | Designer |
| Dashboard and control usability | UI/UX Designer, frontend engineer | Product Owner | Operator, operations | Technical Lead |
| Deployment, recovery and monitoring | Operations engineer | Technical Lead | Security reviewer, chain engineers | Operator |
| Security findings and remediation | Implementers, independent reviewer | Technical Lead | Architect, operator | Product Owner |
| Live arming, funding and risk limits | Operator | Operator | Technical Lead, independent reviewer | Operations |

## 5. Verification and release process

Use reviewable changes with requirement IDs, meaningful tests and representative fixtures. Required test classes include amount/rounding boundaries, protocol comparison, recorded-state replay, stale/gapped data, provider failover, competing inventory reservations, stop during queued work, and recovery after a simulated crash. Live work additionally covers simulation rejection, fee changes, timeouts with unknown submission status, expired transactions, reorganizations and reconciliation discrepancies.

Release evidence includes the supported-venue matrix, known limitations, configuration schema, benchmark environment, test results and recovery runbook. ESTIMATED_EXECUTABLE additionally requires fresh/coherent state, complete costs, reserved principal/native fees, atomic guards/limits and delay/inclusion assumptions. Maintain a decision log when scope or assumptions change. Performance regressions are evaluated against the measured budget and freshness impact, rather than an arbitrary language benchmark.

Automatic promotion from paper to live is forbidden by product design. A live proposal presents a concrete reviewed configuration, limits, signer permissions, execution version and rollback/stop plan to the operator. Manual arming is the final activation step. No planned return or paper win rate overrides a failed correctness, recovery or security gate.

## 6. Risks, scope controls and next decisions

The primary risks are misleading paper inclusion, protocol rounding errors, incomplete data capture, missing venue coverage, and expansion before a valid experiment exists. Control them through explicit evidence labels, conservative scenarios, adapter acceptance tests and narrowly bounded releases.

The first backlog refinement should select initial venues and data providers, agree a monthly operating budget and retention policy, and choose deployment hardware. Add chains only after a new adapter can satisfy the same evidence and control requirements. If M5 shows no credible economic advantage, the valid outcome is a documented research result and a stopped experiment; live execution is optional.


## 7. Complete backlog and delivery tracking

[planning/backlog.json](../planning/backlog.json) is the canonical structured backlog. Every implementation ticket has a stable ID, milestone/epic, scope, exclusions, dependencies, role owner, acceptance criteria and validation evidence. Native GitHub issue numbers and project item IDs are publication metadata. Keep those identities separate so references survive re-import or a project migration.

The eight epics correspond to M0–M7 above. Implemented capture, math, persistence and UI source does not by itself close tickets requiring provider qualification, full protocol coverage, exact deployed-state comparison, rendered UI verification or an integrated recovery exercise. Imported tickets start with honest status and remain open until their own acceptance evidence is linked. Later live and expansion tickets are fully specified for planning, but remain gated by M5/M6 and explicit operator activation.

Work order is dependency-driven: settle essential scope and data access; complete shared types/configuration/control persistence; qualify state ingestion and exact protocol math on both chains; build complete transaction simulation and realistic paper accounting; integrate the approved dashboard; run the comparative research campaign; decide whether any bounded live implementation is justified. Frontend demo work and design review can proceed while adapters are under development.

The issue body is the review contract. A completed pull request links its ticket, relevant PRD IDs, changed behavior, representative fixtures, checks actually run and remaining limitations. A phase is complete only when its exit gate is accepted; completing all code-looking tasks without the research or operational evidence is insufficient.
