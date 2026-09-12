# Implementation and team handoff

Version: 0.2, 12 September 2026. This handoff covers the approved dashboard reference, initial source scaffold, full delivery specifications and GitHub setup. Product, architecture, engineering, operations and UX contributions are design work. They do not imply a staffed company, a completed audit or an operating trading system.

## 1. Start here

The supplied destination is [makafeli/arbitrage-research](https://github.com/makafeli/arbitrage-research). The user selected the existing [dashboard reference](../design/dashboard-wireframe.html) for implementation. The repository contains a React/TypeScript dashboard scaffold, Rust foundation code, versioned requirements, a structured delivery backlog and Wiki source.

Read [implementation status](12-IMPLEMENTATION-STATUS.md) before running the code. It distinguishes source present from checks completed and capabilities not yet implemented. The demonstration does not observe a blockchain, discover live arbitrage, simulate a complete route, maintain a paper ledger, sign or submit a transaction. The original uploaded script is not imported as trusted production code.

The detailed requirements in `docs` are canonical. [planning/backlog.json](../planning/backlog.json) carries the complete stable ticket set from project qualification through optional live and expansion phases. GitHub issues mirror those work items. Wiki pages summarize and link the source instead of defining a second set of requirements.

## 2. Team responsibilities

Use a technical lead who owns shared interfaces and review quality; a chain engineer for Solana; an EVM/engine engineer for Base and shared economics; and part-time frontend/UX and operations/QA support. In a two-engineer delivery team, the lead may also be one of the chain engineers. Keep independent live-execution review outside the author of the relevant code.

The product owner accepts usefulness and evidence presentation. The technical lead accepts implementation correctness and recovery behavior. The operator accepts market scope, spending ceilings and any future live activation. No generated acceptance result substitutes for a reproducible test or an independently checked transaction.

| Role | First responsibility | Required review evidence |
|---|---|---|
| Product owner | Confirm experiment questions, market scope, meaningful negative results and milestone acceptance | PRD coverage, comparison assumptions and explained exclusions |
| Architect / technical lead | Preserve domain boundaries, immutable modes, control acknowledgements and durable recovery | Architecture changes, interface compatibility and integrated failure exercises |
| EVM engineer | Qualify Base state, supported Uniswap V3 math and complete route simulation | Verified registry/provenance, golden/differential fixtures and exact-plan simulation |
| Solana engineer | Qualify account consistency, supported Orca math and complete route simulation | Program/account provenance, boundary fixtures and atomic execution constraints |
| Engine engineer | Implement exact route calculations, virtual reservations, costs and deterministic replay | No double counting/spend, deterministic captures and delayed scenarios |
| Frontend / UX | Preserve approved design while integrating real typed evidence and commands | Browser review, keyboard/mobile checks, pending/failed controls and stale-data states |
| Operations / QA | Establish repeatable builds, monitoring, bounded load and restore/recovery drills | Reproducible checks, named environment and fault/restart logs |
| Independent live reviewer, later | Review signer, executor/program, funds flow, submission and recovery | Findings with exact code/version, remediation and bounded scope |

No external person is assigned work merely by these role labels. GitHub assignees should be actual consenting team members when one is selected.

## 3. First integrated vertical slice

The first useful market-data demonstration is: capture a small verified pool universe; record a coherent complete snapshot; evaluate a route in Rust; display its CANDIDATE evidence and costs; stop the session with a visible durable worker acknowledgement; restart into STOPPED; and reproduce the calculation from the capture. This is useful even if every route is rejected or unprofitable.

The source scaffold is the starting point for that work, not evidence that the slice has already been achieved. Base can be the first integrated chain while Solana account ingestion proceeds in parallel. This is a delivery convenience, not a claim that Base is more profitable. Both chains must satisfy equivalent data and evidence requirements before comparative executable-paper results are published.

The early dependency sequence is:

1. Settle data access, provider cost ceilings, host assumptions and a verified token/pool universe.
2. Complete workspace/build validation, exact types, capability declarations and fail-closed configuration.
3. Implement durable commands, revisions, idempotency, worker acknowledgements and recovery.
4. Capture complete, coherent state on each chain and preserve reproducible fixtures.
5. Implement protocol-exact quoting and bounded distinct-pool route evaluation.
6. Connect captured CANDIDATE evidence to the approved dashboard and validate real controls.
7. Add deterministic replay, complete transaction simulation and paper inventory/cost scenarios.

Keep the first sprint narrow. The full backlog specifies the complete route to later releases so dependencies and omitted capabilities remain visible; it does not imply the entire product fits one sprint.

## 4. Ticket and pull-request contract

Stable ticket IDs in the structured backlog replace the initial v0.1 `BUILD-01`–`BUILD-12` starter table. Avoid creating an additional competing set of those legacy issues. Milestones are M0–M7 and epics EPIC-01–EPIC-08. Native GitHub issue numbers are recorded after publication and are not predictable from stable IDs.

Before starting a ticket, verify that its dependency evidence exists, its scope is still valid and an owner is available. If a capability spike fails, report the gap and update dependent scope; do not mark the product behavior complete. A source file, SDK call, mock result or positive quote is not a substitute for the ticket's acceptance evidence.

A pull request names the problem, resulting behavior, stable ticket and PRD IDs, affected capability boundary, representative fixtures, checks run and remaining limitations. Reviewers should be able to understand the final change without reading conversation history. Attach actual command output or CI links; distinguish a check not run from a check that failed.

Definition of done is ticket-specific. Build the relevant target, run meaningful checks, document operational behavior, update schemas/UI contracts where changed, and link acceptance evidence. Do not close a durable-control ticket based only on an in-memory state machine, or a UI accessibility ticket based only on source-level assertions.

## 5. Release boundaries

The first deployment against mainnet data remains a research deployment with no production trading key, signer or broadcast path. Complete simulation may use a local fork or simulation RPC with declared virtual funding. It must preserve exact plan/state/cost/guard evidence.

If either chain lacks a complete intended atomic route simulation, release only a clearly labeled quote-research preview. Do not present that as a completed executable-paper comparison. M3 requires at least one fully simulatable supported route per chain; synthetic success fixtures prove mechanics and never enter market-return reports.

M5 produces a comparable observation study and an explicit feasibility decision, including negative results and insufficient evidence. An initial suggested 30-calendar-day research window is a planning assumption; adequate coverage and controlled methodology are still required. Scope and thresholds remain those in the PRD and operations requirements; scaffold validation does not satisfy them.

M6/M7 live work requires the previous phase decision, bounded limits, independent review, deployment/funding arrangements and explicit operator activation. Tickets can be ready for review while activation remains disabled. Repository creation, public source publication, a passing build or a paper profit cannot arm a strategy.

## 6. Design-review invariants

The implementation must preserve these cross-role resolutions:

1. **Simulation:** SIMULATED means a successful complete intended atomic transaction. Local quote arithmetic and replay remain CANDIDATE; a failed full simulation remains a rejection.
2. **Executable estimates:** Fresh coherent complete state, complete costs, reserved principal/native fees, exact-plan simulation, atomic limits/final guards and a named delay/inclusion scenario are all required. The result remains hypothetical.
3. **Funding:** A fee wallet does not supply trading principal. Live balances and authority belong at the actual spending account or executor, with nonconflicting reservations.
4. **Controls:** PENDING records acceptance; APPLIED records the worker's effective local fence. Stop remains DRAINING while dispatched outcomes are unresolved. Already emitted requests cannot be recalled.
5. **Signing:** In-flight signing responses after a stop are quarantined. DISARM also requires durable signer-epoch revocation acknowledgement; an unreachable signer cannot be reported as fully disarmed.
6. **Recovery:** Journal intent and signed identity, then an UNKNOWN dispatch-start record before every transport call. Timeouts do not permit blind re-signing or a fresh trade.
7. **Leadership:** Lease expiry cannot invalidate signed bytes. There is no automatic live takeover before the previous process is fenced and outstanding attempts reconciled.
8. **Scope:** USDC-start cycles are the default. WETH/wSOL-start experiments need a new configuration/session. Same-chain same-venue distinct-pool research precedes qualified expansion.
9. **Evidence:** Paper never earns REALIZED. Unknown cost inputs are not zero, pool fees/impact already in a quote are not subtracted twice, and USDC is not guaranteed to equal a dollar.
10. **Implementation claims:** Design, scaffold, integration, research and live gates are different stages. Record the stage actually achieved.

See [the decision register](09-DECISIONS-AND-OPEN-QUESTIONS.md) for remaining budgets, hosting, integrations and future live limits, and [the delivery plan](07-DELIVERY-PLAN.md) for effort assumptions and phase gates.
