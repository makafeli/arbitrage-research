# Product requirements: private arbitrage research platform

Version: 0.2 — implementation handoff, 12 September 2026. Owner: Product Owner. The approved dashboard reference now guides a React/TypeScript scaffold; Rust foundation code and the complete delivery backlog accompany this specification. The requirements below describe the intended product. Live observations, exchange adapters, paper accounting and trading are not implemented by the scaffold. See [implementation status](12-IMPLEMENTATION-STATUS.md) for the boundary and [delivery plan](07-DELIVERY-PLAN.md) for release gates.

## 1. Product outcome

Enable one private operator to investigate where a particular arbitrage implementation has an advantage, compare Solana and Base under documented assumptions, and move selected strategies from observation to realistic paper trading before considering controlled live execution.

The product answers: Which chain, venues, tokens, routes and trade sizes produce repeatable opportunities after modeled costs? What evidence supports each result? How much changes when latency, fees or competition become less favorable? It must make a negative result useful and distinguish missing data from a lack of opportunity.

Performance is a means to obtain fresher data and make decisions sooner. Rust is the default language for server components and shared calculation logic. Profitability is an experimental outcome; the product must not imply that a fast implementation guarantees earnings.

## 2. Users and responsibilities

The primary persona is the **operator/researcher**: a technically capable private user running the platform on infrastructure they control. They select the markets, allocate virtual balances, inspect results, and decide whether to stop or advance an experiment. One authenticated operator is sufficient initially.

A **maintainer** configures providers, upgrades adapters, investigates gaps and restores the service. The operator may also perform this role. A later **reviewer** needs read-only access to recorded evidence and configuration changes; a dedicated multi-user permissions system is deferred until there is an actual second user.

Only the operator can authorize and arm a live strategy. Agents can assist with implementation and review; their approval does not replace a human decision or an independent security review.

## 3. Scope and release boundaries

**Initial release:** Solana and Base; same-chain, pool-to-pool cyclic routes that end in the starting token; allowlisted tokens, pools and venues; observation, live-data paper trading, deterministic replay, comparison and controls. Phase 1 targets USDC-starting two-leg cycles across distinct eligible Uniswap V3 pools on Base (USDC/WETH) and distinct eligible Orca pools on Solana (USDC/wSOL). A WETH- or wSOL-starting experiment requires an explicit new configuration/session. These can be pools on the same venue. Pool identities and suitable liquidity must be verified; zero opportunities is a valid result. Cross-venue coverage, beginning with Sushi V2 and Raydium CPMM subject to verification, and additional tokens/routes are gated Phase 2 expansion. Configuration alone cannot add an unsupported pool model.

Paper portfolios use virtual, prefunded balances. Initial operation requires no funded wallet, signing key or transaction-broadcast capability. M3 includes minimal execution artifacts/encoders, complete transaction builders and atomic guards, with read-only local-fork and Solana simulation support using explicitly declared virtual funding. These artifacts are tested locally without production deployment or signing. Public read-only RPC access and optional paid data-provider credentials remain necessary.

**Later release:** separately enabled live execution using operator-funded, tightly limited inventory and an execution path that enforces required balance and repayment conditions. Flash loans are an optional subsequent funding adapter. They are not required for demonstrating that a strategy works in paper mode.

**Excluded initially:** cross-chain atomicity, bridging, CEX accounts or hedging, sandwiching, liquidation strategies, newly launched token discovery, unrestricted token listings, copy trading, managed customer funds, subscription billing and a public marketplace. No target income, winning-chain promise or automatic promotion to live mode is included.

## 4. Product vocabulary and evidence

Operating modes are **OBSERVE**, **PAPER**, **REPLAY** and, later, **LIVE**. A session has one immutable mode; changing modes starts a new session. Modes describe what a worker is permitted to do. Evidence labels describe what has been established for an individual opportunity:

| Label | Meaning | Required display |
|---|---|---|
| CANDIDATE | A route quote has been evaluated under a captured state, whether its spread is positive or negative. Local mathematical replay remains at this level. | State reference, age, size, calculation method, spread and excluded costs. |
| SIMULATED | The complete intended atomic transaction successfully simulated against identified state with the exact transaction plan recorded. | Plan reference, simulation state, result and limitations; simulation is not proof of inclusion. |
| ESTIMATED_EXECUTABLE | A successfully simulated route passes coherent/fresh-state checks, complete cost accounting, principal/native-fee balance and reservation checks, atomic guards/limits, and a named delay/inclusion scenario. | Estimated outcome range, capital/cost assumptions and explicit inclusion uncertainty. |
| REALIZED | A submitted transaction has reached the configured chain-specific settlement state and its balance changes and costs have been reconciled. | Transaction reference, settlement policy, reconciled outcome and separately allocated overhead. |

These are evidence levels, not guaranteed successive states. A CANDIDATE can expire or fail. Failed full-transaction simulation produces a rejection with its simulation evidence; it does not earn SIMULATED status. A paper opportunity can be ESTIMATED_EXECUTABLE without submission. Pending transactions are not REALIZED; REALIZED may include a loss. Operational labels additionally include rejected, expired, submission-unknown, pending, failed and reconciled. A modeled quote result is always labeled as such and never presented as successful full-transaction simulation.

## 5. Requirements and acceptance criteria

**PRD-F01 — Experiment configuration.** As an operator, I can select chain, venues, tokens, starting asset, sizes, cost assumptions and limits. Acceptance: each run receives an immutable configuration version and mode; invalid addresses, units or unsupported combinations fail validation before starting; defaults are visible and editable before a new run.

**PRD-F02 — Market allowlist.** As an operator, I can restrict the markets the system evaluates. Acceptance: identities use chain plus address/mint, not ticker alone; decimals and supported token behavior are checked; adding an unknown venue never silently falls back to another pricing model.

**PRD-F03 — Data quality.** As an operator, I can tell whether results are trustworthy. Acceptance: each chain exposes freshness, provider health, missing updates, reorganization/rollback status and usable coverage; inconsistent or stale state is excluded from executable estimates with a recorded reason. Cross-chain timestamps support comparison without pretending both chains share one atomic state.

**PRD-F04 — Route discovery.** As a researcher, I can identify allowlisted cyclic opportunities. Acceptance: Phase 1 supports bounded two-swap routes through distinct supported pools; three-swap and cross-venue routes require later adapter gates; routes identify every pool and token; duplicate observations of the same persisting opportunity can be grouped without discarding raw evidence; unsupported route types are explicit.

**PRD-F05 — Economic evaluation.** As a researcher, I can inspect the complete economics. Acceptance: record gross output, DEX fees, price impact, execution fees, priority fees/tips where applicable, funding costs, modeled failure costs and allocated operating expenses. Fields identify whether measured, estimated, already included or unavailable, preventing double counting. The start-asset result and reference-currency conversion are both reproducible.

**PRD-F06 — Evidence labels.** As an operator, I can understand what a number proves. Acceptance: every result has an evidence label and method version; quotes and local mathematical replay remain CANDIDATE, including positive modeled results; only a successful simulation of the complete intended atomic transaction earns SIMULATED. Failed simulation remains a rejection with evidence. Unknown inputs remain visibly unknown and prevent ESTIMATED_EXECUTABLE status; quoted profit never appears as REALIZED profit.

**PRD-F07 — Paper execution.** As a researcher, I can test a strategy against a virtual portfolio. Acceptance: configurable delay, fee and inclusion scenarios affect modeled fills without promoting evidence labels; inventory and native network-fee balances constrain activity; overlapping uses of the same virtual capital cannot all fill; rejected, expired and failed scenarios remain in the denominator. At least one supported atomic route per chain must be fully simulatable under declared virtual funding; this requires complete builders/guards and read-only local-fork or Solana simulation support, not real funding. If either chain lacks this capability, the deliverable is a quote-research preview and cannot present an executable-paper comparison. The process cannot sign or broadcast transactions.

**PRD-F08 — Replay.** As a maintainer, I can reproduce a decision. Acceptance: a replay pins data, adapter/calculation versions and configuration; the same captured inputs yield the same calculations; incomplete captures are flagged and cannot claim full reproducibility. Replay does not silently fetch today's state to fill yesterday's gaps.

**PRD-F09 — Chain comparison.** As an operator, I can compare Solana and Base fairly. Acceptance: a comparison states the overlapping observation window, coverage, starting capital, sizes, quote/reference assets and execution assumptions. Display both matched experiments and each chain's native opportunity set. Downtime is reported separately from zero opportunities. No aggregate ranking is produced when comparable data is insufficient.

**PRD-F10 — Portfolio accounting.** As an operator, I can see why the balance changed. Acceptance: inventory, fees, transfers and trading P&L are separate; token valuation changes are separated from arbitrage results; virtual portfolios reset only through an explicit new run; metrics distinguish gross, transaction-net and fully allocated results.

**PRD-F11 — Start, pause and stop.** As an operator, I can control each worker or all workers. Acceptance: Pause gates new evaluation and submission while feeds and reconciliation continue, then reaches PAUSED. Stop closes the submission gate, cancels unsent work and enters DRAINING until unresolved submissions settle, then STOPPED. API acceptance/PENDING is distinct from APPLIED after durable worker acknowledgement. Acknowledgement cannot recall broadcast transactions or in-flight network calls; ambiguous submissions remain tracked. State vocabulary is RECOVERING, STOPPED, RUNNING, PAUSING, PAUSED, DRAINING and FAULTED. Boot follows RECOVERING to STOPPED and never automatically rearms live execution. Commands have durable IDs and outcomes.

**PRD-F12 — Explain decisions.** As an operator, I can see why a route was accepted or rejected. Acceptance: a decision trace connects state, calculation, risk checks, simulation and submission/rejection; errors have actionable messages; the dashboard reports its last update and clearly indicates a lost backend connection.

**PRD-F13 — History and evidence retention.** As a maintainer, I can investigate a run after restart. Acceptance: store opportunity summaries, decisions, mode changes and settlement records; configurable retention identifies which raw inputs expire; the interface warns when a historical result lacks replay inputs.

**PRD-F14 — Live arming, later release.** As an operator, I can explicitly enable a reviewed strategy within bounded limits. Acceptance: arming names the chain, signer, route/venue permissions, funding source and configuration version; mode changes cannot occur through a paper run; an unready dependency blocks arming with an explanation; approval is recorded.

**PRD-F15 — Settlement and reconciliation, later release.** As an operator, I can distinguish pending funds from final outcomes. Acceptance: persist submission intent before broadcast; resolve timeouts through chain evidence before retrying; recover pending work after restart; apply chain-specific settlement and rollback rules; unresolved balance discrepancies halt further live submissions for the affected account.

**PRD-F16 — Secrets and access.** As an operator, I can run research without exposing wallet keys. Acceptance: initial services contain no signer; provider credentials are redacted from logs and exports; later signing is separated from the public-facing interface with narrow permitted operations; configuration endpoints require authentication.

**PRD-F17 — Trading limits, later release.** As an operator, I can cap exposure. Acceptance: enforce per-trade size, inventory reservations, fee spend, realized-loss limits, allowed contracts/programs and concurrent pending activity at submission time; limits fail closed if their state is unavailable. A displayed target profit is not treated as a guaranteed account-level profit after external fees.

**PRD-F18 — Export.** As a researcher, I can take results elsewhere. Acceptance: export run metadata, summaries and detailed records in documented CSV/JSON formats; amounts retain raw integer units and decimals; exports include methodology, evidence labels, limitations and timezone.

## 6. Nonfunctional requirements

| ID | Requirement and acceptance basis |
|---|---|
| NFR-01 | Financial correctness: use checked integer/fixed-point arithmetic matching protocol units and rounding; golden cases, boundary cases and protocol differential checks cover supported adapters. |
| NFR-02 | Measured performance: instrument ingestion-to-decision and decision-to-submission distributions, queue age and dropped/stale work. Establish per-chain budgets on named hardware during the first vertical slice; do not invent a universal microsecond promise. |
| NFR-03 | Isolation: a slow dashboard or analytics query cannot block submission work; chain failures remain isolated; backpressure has an explicit discard/resync policy. |
| NFR-04 | Recovery: controlled restart preserves configurations and submitted intents, reconciles pending work and returns live execution to an unarmed state. |
| NFR-05 | Observability: every decision has a correlation ID; health, state age, queue depth, provider errors and costs are inspectable. Alert delivery failures are themselves visible. |
| NFR-06 | Security: least-privilege processes, authenticated controls, dependency review and secret redaction precede remote access. Signing and contract reviews precede funding. |
| NFR-07 | Usability: primary mode, evidence level, health and stop control are visible without navigating away; keyboard use and narrow-screen monitoring are supported. |
| NFR-08 | Reproducibility: version data schemas, adapters, assumptions and evaluation code; incompatible versions cannot be silently combined into one experiment. |

## 7. Success measures and release evidence

Success first means reliable learning: known coverage, reproducible sampled decisions, correct accounting, meaningful uncertainty and controls that behave as documented. Track usable observation hours, sampled calculation agreement, replay consistency, costs per observation period and unresolved reconciliation items. Report opportunity survival across latency scenarios and outcome distributions rather than a single optimistic average.

Paper release requires both chain adapters to pass their agreed validation corpus, at least one fully simulatable supported atomic route per chain under declared virtual funding, successful restart/stop exercises, visible data gaps and no signing or submission capability. Without the full-simulation gate, only a clearly labeled quote-research preview may be released; executable-paper comparison remains unavailable. Live consideration additionally requires the delivery gates in `07-DELIVERY-PLAN.md`; a positive paper balance alone is insufficient. An initial suggested observation window is 30 calendar days across varied conditions, subject to adequate coverage. This is a research planning assumption, not a profitability or safety threshold.

## 8. Open decisions

Nonblocking defaults are private single-user deployment, Rust services, a browser dashboard, Solana and Base, virtual prefunded portfolios and no flash-loan dependency. Before implementing adapters, confirm initial venues, provider/data budget, permitted assets, hardware and retention. Before live design is finalized, confirm available capital, acceptable loss/fee limits, key custody, execution mechanism and review budget. Every unanswered live question leaves that feature unarmed while research can continue.
