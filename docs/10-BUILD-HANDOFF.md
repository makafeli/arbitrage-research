# Implementation and team handoff

Version: 0.3, 12 September 2026. This handoff covers the approved dashboard implementation, integrated research/virtual-account source, delivery requirements and GitHub project material. Product, architecture, engineering, operations and UX work has been carried out in parallel. It does not imply a staffed company, an independent audit, deployed services or an operating live trader.

## 1. Start here

The supplied destination is [makafeli/arbitrage-research](https://github.com/makafeli/arbitrage-research). The user selected the existing [dashboard reference](../design/dashboard-wireframe.html) for implementation. The repository contains a React/TypeScript Demo/Connected dashboard, Rust capture/math/decision and virtual-account services, versioned requirements, a structured delivery backlog and Wiki source. Railway is the selected host, with a [prepared private-service runbook](../deploy/RAILWAY.md).

Read [implementation status](12-IMPLEMENTATION-STATUS.md) and [integration verification](17-RESEARCH-INTEGRATION-VERIFICATION.md) before interpreting the current source. The latter owns pending CI and the exact validated checkpoint; earlier main-branch test counts do not prove this integration. Read-only capture and CANDIDATE calculations are distinct from a qualified live observation campaign. Durable virtual accounts are distinct from automatic paper fills. Complete atomic simulation, current deployed-provider/protocol qualification, signing and submission remain unavailable. The original uploaded script is not imported as trusted production code.

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

The source now connects retained captures, bounded exact math, durable decision admission and authenticated UI inspection, with a separate virtual-account workflow. Its fixtures and implementation reviews do not establish qualified market capture or the complete release gate. Pool acquisition is still independent; differing contexts are rejected, and finalized-state age is distinct from receipt age. Both chains need equivalent provider/state/simulation evidence before comparative executable-paper results are published.

The remaining acceptance sequence is:

1. Settle data access, provider cost ceilings, host assumptions and a verified token/pool universe.
2. Finish the current locked build, PostgreSQL, reference-oracle, browser and container checks; link the exact CI commit.
3. Accept implemented durable commands, decision/virtual-account idempotency, worker fencing and restart behavior against their integration evidence.
4. Qualify coherent multi-pool state on each chain and retain representative actual-capture fixtures.
5. Establish current deployed-protocol equivalence for the implemented exact math and configured routes.
6. Accept the connected CANDIDATE/coverage/account UI against browser and API-contract evidence.
7. Add complete transaction simulation, cost/inclusion scenarios and automatic paper execution without weakening the existing virtual ledger or replay boundaries.

Keep the first sprint narrow. The full backlog specifies the complete route to later releases so dependencies and omitted capabilities remain visible; it does not imply the entire product fits one sprint.

## 4. Ticket and pull-request contract

Stable ticket IDs in the structured backlog replace the initial v0.1 `BUILD-01`–`BUILD-12` starter table. Avoid creating an additional competing set of those legacy issues. Milestones are M0–M7 and epics EPIC-01–EPIC-08. Native GitHub issue numbers are recorded after publication and are not predictable from stable IDs.

Before starting a ticket, verify that its dependency evidence exists, its scope is still valid and an owner is available. If a capability spike fails, report the gap and update dependent scope; do not mark the product behavior complete. A source file, SDK call, mock result or positive quote is not a substitute for the ticket's acceptance evidence.

A pull request names the problem, resulting behavior, stable ticket and PRD IDs, affected capability boundary, representative fixtures, checks run and remaining limitations. Reviewers should be able to understand the final change without reading conversation history. Attach actual command output or CI links; distinguish a check not run from a check that failed.

Definition of done is ticket-specific. Build the relevant target, run meaningful checks, document operational behavior, update schemas/UI contracts where changed, and link acceptance evidence. Do not close a durable-control ticket based only on an in-memory state machine, or a UI accessibility ticket based only on source-level assertions.

## 5. Release boundaries

The first deployment against mainnet data remains a research deployment with no production trading key, signer or broadcast path. Complete simulation may use a local fork or simulation RPC with declared virtual funding. It must preserve exact plan/state/cost/guard evidence.

If either chain lacks a complete intended atomic route simulation, release only a clearly labeled quote-research preview. Do not present that as a completed executable-paper comparison. M3 requires at least one fully simulatable supported route per chain; synthetic success fixtures prove mechanics and never enter market-return reports.

M5 produces a comparable observation study and an explicit feasibility decision, including negative results and insufficient evidence. An initial suggested 30-calendar-day research window is a planning assumption; adequate coverage and controlled methodology are still required. Scope and thresholds remain those in the PRD and operations requirements; source-level validation does not satisfy them.

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

See [the decision register](09-DECISIONS-AND-OPEN-QUESTIONS.md) for remaining budgets, hosting parameters, integrations and future live limits, and [the delivery plan](07-DELIVERY-PLAN.md) for effort assumptions and phase gates.
