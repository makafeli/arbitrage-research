# Implementation status and capability boundary

Version: 0.2, 12 September 2026. This file records the initial GitHub handoff. Detailed requirements elsewhere describe the target product; this page describes the source scaffold. The [validation record](11-PACKAGE-VALIDATION.md) owns actual check results, and publication metadata records which GitHub resources were created.

## 1. What this handoff contains

| Area | Source present | Current boundary |
|---|---|---|
| Product and architecture | PRD, architecture, financial/evidence model, UX, security/operations, API/data contracts, decisions and delivery plan | Specifications for future implementation; requirements are not acceptance results |
| Approved dashboard | Original self-contained HTML reference and React/TypeScript application in `apps/web` | Synthetic fixtures, six screens, chain filtering, theme, opportunity inspection and local demo controls; no backend/market connection |
| Rust workspace | `arb-domain`, `control-api`, `evm-worker`, `solana-worker`, `replay` | A deliberately small foundation; most target crates/services remain future work |
| Domain | Checked base-unit amount type, mode/evidence restriction and in-memory lifecycle model | No complete route model, protocol math, durable state, fees, ledger or transaction capability |
| Control API | Axum loopback development liveness endpoint `GET /healthz` | No `/v1` sessions/commands, authentication, PostgreSQL or durable acknowledgements |
| Chain workers | Explicit unavailable entry points for Base/EVM and Solana | Exit without making an RPC connection; no adapters, ingestion, signing or broadcast |
| Replay entry point | Offline `--lifecycle-demo` | Synthetic control-state illustration only; no market capture replay or P&L |
| Planning | Complete structured epic/ticket backlog and GitHub setup definitions | Prepared/published work items remain open until ticket acceptance evidence exists |
| Wiki | Nine source pages with navigation and canonical links | Native GitHub Wiki publication is a separate action, recorded in setup results |

The scaffold does not establish that a real arbitrage bot works. It establishes a reviewable code organization, design reference, selected invariant models and the delivery contract for building one.

## 2. Local entry points

Use the root README for current prerequisites, lockfile handling and exact frontend commands. The Rust toolchain and dependencies are declared in the workspace. These commands describe intended use in a development environment with the prerequisites installed; validation results must be checked separately.

```sh
cargo run -p control-api
cargo run -p replay -- --lifecycle-demo
```

The development API binds to `127.0.0.1:8080` and provides `/healthz`. It explicitly reports unavailable persistence and trading. Stop that local process with Ctrl-C. Stopping a development process is not the future product's durable worker-stop protocol.

`evm-worker` and `solana-worker` are unavailable stubs, not idle trading workers. They exit with a diagnostic and nonzero code. The replay executable requires `--lifecycle-demo` for its synthetic demonstration; the ordinary replay workflow is not implemented.

The frontend development server uses loopback binding and a synthetic data provider. Its Start, Pause, Resume and Stop interactions change local demonstration state. Reloading the page resets the demo; no durable command or chain state is implied. The chain filter only changes visible records.

## 3. What is explicitly not implemented

- Verified production token/pool registries, provider configuration and supported live state ingestion.
- Complete coherent snapshots, reorganization/rollback handling or replay capture retention.
- Protocol-exact Uniswap V3/Orca quote mathematics, route sizing or bounded evaluation workers.
- Complete intended atomic transaction builders, guards or successful full-transaction simulations.
- Virtual trading inventory, fee reservations, counterfactual execution, paper accounting or comparable market-return reports.
- The proposed authenticated `/v1` API, CSRF/session protections, command idempotency storage or durable worker acknowledgements.
- PostgreSQL migrations, critical intent journal, checkpoint/restore procedures or operational monitoring.
- A signer, production key custody, live arming, contract/program deployment, network dispatch or transaction reconciliation.
- An independent security review, measured trading latency, operational service targets or any profitability result.

The OpenAPI file remains a target contract. A working `/healthz` harness does not establish conformance with its seven planned operations. The JSON schema demonstrates structural evidence constraints but cannot establish actual chain state, full simulation or real economic eligibility.

## 4. What the foundation tests can establish

The in-memory domain model can check exact base-unit parsing/overflow boundaries, immutable research-mode restrictions, pending versus applied transitions, stale revisions, stop/drain behavior and the prohibition on non-live REALIZED evidence. It cannot establish database durability, OS process fencing, actual transport cancellation, signer revocation or chain settlement.

Frontend unit/model checks can establish fixture filtering and local control transitions. Type/build checks establish a compilation/bundling result for the tested versions. Rendered browser review is separately required for layout, keyboard behavior, dialog focus, zoom, light/dark contrast and narrow screens.

Do not mark a future integration ticket complete merely because a similarly named foundation test passes. A test result must match the acceptance claim and identify the version and environment actually checked.

## 5. Status of the full backlog

The structured [backlog](../planning/backlog.json) is the canonical complete implementation plan. It includes qualification, shared foundations, both chain adapters, complete simulation, paper/replay, the integrated dashboard, operations, the research campaign and optional reviewed live/expansion work. Initial source preparation contributes to that backlog, but does not close its broader acceptance gates.

Use stable IDs in code reviews and published issues. Native issue/project IDs belong in publication metadata. No future live task is activated merely by being described or imported into GitHub.

## 6. First milestone that changes this status

The next substantive capability is one reproducible observation slice: verify a small pool universe, capture coherent complete state, evaluate an exact route in Rust, persist a CANDIDATE decision and show it in the dashboard, then demonstrate a durable stop acknowledgement and replay from the capture. Until that evidence exists, the product remains a scaffold with synthetic examples.

After both chain adapters pass their fixtures, M3 must demonstrate one complete intended atomic route simulation per chain under declared virtual funding plus correct paper accounting and delayed scenarios. Without that gate, the appropriate release label is quote-research preview. Neither label authorizes live execution.
