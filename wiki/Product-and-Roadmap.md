# Product and roadmap

The product helps one operator determine which supported chain, route and trade size survives realistic costs, latency assumptions and operational constraints. A negative result or insufficient evidence is useful. Speed, third-party trading volume and a positive quote do not establish attainable profit.

The [PRD](https://github.com/makafeli/arbitrage-research/blob/main/docs/01-PRD.md) defines 18 functional requirements and eight nonfunctional requirements. The [delivery plan](https://github.com/makafeli/arbitrage-research/blob/main/docs/07-DELIVERY-PLAN.md) owns effort assumptions, responsibilities and phase exit gates.

## Initial scope

Base and Solana are the first comparison targets. Initial routes start and end in USDC and cross two distinct supported pools: USDC/WETH on Base, USDC/wSOL on Solana. Native ETH/SOL fee reserves are separate. Initial adapters qualify Uniswap V3 on Base and Orca Whirlpools on Solana. Pool identities, liquidity and supported behavior must be verified before enabling them.

Other tokens and venues can be added through explicit adapter/registry qualification. A different starting asset needs a new configured session. A network is not supported just because its ticker appears in a UI filter. Cross-chain bridging, CEX hedging, automatic token discovery, customer custody and unrestricted routing are outside the initial release.

## Milestones

| Phase | Required outcome |
|---|---|
| M0 / EPIC-01 | Scope, data access, verified universe, host and cost assumptions |
| M1 / EPIC-02 | Validated foundation, durable controls and one reproducible observation slice |
| M2 / EPIC-03 | Both adapters, exact pool math and bounded route evaluation |
| M3 / EPIC-04 | Deterministic replay, paper accounting and complete atomic-route simulation on both chains |
| M4 / EPIC-05 | Approved dashboard integrated with real evidence, controls, comparison and exports |
| M5 / EPIC-06 | Comparable observation campaign and documented feasibility decision |
| M6 / EPIC-07 | Optional reviewed bounded live implementation and recovery evidence |
| M7 / EPIC-08 | Optional limited live pilot and qualified expansion |

M0–M4 deliver research functionality. If complete atomic simulation is missing on either chain, the release is a quote-research preview. M5 evaluates whether more work is justified. Live funding and activation remain separate explicit operator decisions after the relevant engineering/review gates.

The original 22–36 engineering person-week estimate for M0–M4 remains a planning judgment. It excludes provider purchases, trading capital and independent live review. The scaffold does not consume that whole estimate or imply that future tickets are already accepted. Re-estimate after the first real adapter and capture work.

## Working with tickets

[planning/backlog.json](https://github.com/makafeli/arbitrage-research/blob/main/planning/backlog.json) defines stable IDs and dependencies. [GitHub issues](https://github.com/makafeli/arbitrage-research/issues) host discussion and acceptance evidence. Epics group related outcomes; they are not substitutes for the detailed child tickets. Milestones close only when their exit gates are accepted.

Do not close an implementation ticket merely because a directory or mock exists. Source, build verification, integrated behavior, measured research and live authorization are different stages. Preserve those distinctions in status updates and release notes.

See [development workflow](./Development-Workflow.md) and [implementation handoff](https://github.com/makafeli/arbitrage-research/blob/main/docs/10-BUILD-HANDOFF.md) for ownership and review expectations.
