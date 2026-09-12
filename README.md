# Arbitrage Research

A Rust-first research platform for comparing same-chain arbitrage opportunities on Base and Solana, with a React and TypeScript dashboard. The first release targets observation, reproducible replay and paper experiments. Live execution is a separately gated future milestone.

**Current release: v0.2 foundation.** The dashboard uses clearly labelled synthetic data and local controls. The Rust workspace is an inert development foundation. Chain ingestion, real quoting, transaction simulation, durable paper accounting and live execution are still implementation work. No profitability or security-audit claim is made.

## Start here

- [Product requirements](docs/01-PRD.md) and [architecture](docs/02-ARCHITECTURE.md)
- [Implementation status](docs/12-IMPLEMENTATION-STATUS.md): what exists and what remains planned
- [Dashboard application](apps/web/README.md) and [approved design reference](design/dashboard-wireframe.html)
- [Complete delivery backlog](planning/BACKLOG.md) and [machine-readable ticket specifications](planning/backlog.json)
- [Project wiki source](wiki/Home.md) and [GitHub issues](https://github.com/makafeli/arbitrage-research/issues)
- [GitHub setup instructions](scripts/README.md) and [verification record](docs/11-PACKAGE-VALIDATION.md)

## Research scope

Initial experiments compare USDC-start, two-leg cycles through distinct pools on each chain: USDC → WETH → USDC on Base and USDC → wSOL → USDC on Solana. Uniswap V3 and Orca Whirlpools are the initial adapter qualification targets. Enabled pools and assets require verified identities and supported behavior; example configuration starts with both networks disabled.

Paper outcomes remain hypothetical. CANDIDATE, SIMULATED, ESTIMATED_EXECUTABLE and REALIZED have different evidence requirements. Paper sessions never report REALIZED returns. Missing costs, stale state and incomplete simulation block stronger claims.

## Controls and safety boundary

Session modes are immutable. Pause and stop require a worker acknowledgement; API acceptance alone does not establish STOPPED. A stopped admission gate can coexist with unresolved previously dispatched attempts, shown as DRAINING. Already emitted transactions cannot be recalled by a dashboard button. Recovery returns to STOPPED and never automatically enables live execution.

The checked-in dashboard demonstrates these distinctions using local synthetic fixtures. It does not operate a worker or wallet. The intended research deployment has no signing or broadcast capability. Funding, signing, executor deployment and any live pilot require the later documented review and activation gates.

## Delivery workflow

The backlog contains product, architecture, chain integration, simulation, UX, security, operations and optional live milestones from M0 through M7. Scaffold files are starting points, not evidence that a ticket meets its acceptance criteria. Select an issue, implement the behavior, attach reproducible verification and review it against the linked requirements before closing it.

Repository documentation is available immediately. Native GitHub Projects and Wiki setup have separate capability and authentication requirements described in the setup guide; their existence must be verified before being reported as complete.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development and review expectations. A software distribution license has not yet been selected; public visibility alone does not grant an open-source license.
