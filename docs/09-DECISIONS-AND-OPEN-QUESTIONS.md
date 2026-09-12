# Architecture decisions and unresolved choices

Status: v0.2 baseline for implementation, 12 September 2026. User-confirmed preferences include Railway for deployment (confirmed 12 September 2026), Rust for performance, multiple tokens/chains, paper trading, operator start/stop and use of the existing dashboard design. The user authorized complete GitHub project setup and supplied `https://github.com/makafeli/arbitrage-research` as the destination. Other technical choices below are recommendations, not claims of explicit approval or completed implementation.

## Decision record

| ID | Decision | Reason and consequence | Revisit trigger |
| --- | --- | --- | --- |
| ADR-01 | Rust for server, strategy engine, adapters and paper/replay | Matches the user's preference and shares domain code across modes; requires more integration work where vendor SDKs are JavaScript-focused. | A required venue lacks a maintainable Rust/native interface; isolate an adapter and measure any bridge. |
| ADR-02 | Solana and Base first | Two architectures with substantial observed market activity provide a useful research comparison. Neither is declared the current global arbitrage winner. | Captured data shows poor coverage, unsuitable routes or costs beyond budget. |
| ADR-03 | Private single-operator product on Railway initially | Matches the personal research workflow and bounds access/custody complexity. | Confirmed commercial/multi-user use case; redesign authorization, isolation, quotas and custody before expansion. |
| ADR-04 | One modular Cargo workspace; independent chain workers | Shared financial types and tooling, chain-specific state/recovery, limited process/network overhead. | Measured resource or deployment boundaries justify a service split. |
| ADR-05 | Same-chain cyclic DEX arbitrage only initially | A route begins and ends in one asset; independent chains can be compared without bridge settlement risk. | Dedicated CEX inventory or cross-chain product PRD is commissioned. |
| ADR-06 | Own-capital paper model first; flash loans later | Borrowing availability, fees, callbacks and repayment differ by protocol. Paper research can start before any lender integration. | Verified lender capability and complete transaction tests justify adding a funding adapter. |
| ADR-07 | Bounded initial venues and verified token allowlists | Correct concentrated-liquidity mathematics and account state matter more than listing many incomplete integrations. | Each additional adapter passes the capability gates and fixtures. |
| ADR-08 | Shared deterministic evaluator with separate execution implementations | Paper results remain comparable to live decisions; avoids maintaining two financial engines. | Chain-native execution constraints require explicit model extensions. |
| ADR-09 | Separate signer and durable execution journal for live | Limits signing authority and makes unknown outcomes recoverable; adds a local communication/durability cost that must be measured. | A reviewed alternative preserves both policy enforcement and recovery guarantees. |
| ADR-10 | TypeScript/React for the dashboard; Rust/Axum API | The UI is outside the trading decision/submission path. It displays typed data and commands. | A specific requirement for an all-Rust UI outweighs component and accessibility tooling costs. |
| ADR-11 | PostgreSQL for durable state, bounded local capture for replay | A familiar transactional store handles configuration, audit and intent uniqueness without introducing a distributed queue by default. | Measured capture volume warrants a separate analytical store or object storage. |
| ADR-12 | No automatic paper-to-live promotion | A simulated outcome and a successful transaction are different evidence. Live capabilities, limits and arming are separate gates. | No automatic trigger; any future change requires an explicit new product/security decision. |
| ADR-13 | Implement the existing dashboard design | User selected the current dashboard reference; React keeps its six views, visual tokens and explicit synthetic-data boundary. | A reviewed usability finding or an explicit design change. |
| ADR-15 | Railway is the deployment platform | User already uses Railway; prepare a private API/database and independent workers with volumes. No authenticated deployment has occurred. | Provider RTT, capacity or recovery measurements justify another host. |
| ADR-14 | GitHub is the project delivery source | User selected `makafeli/arbitrage-research`; repository docs, Wiki source, stable planning IDs and issue metadata stay versioned together. | Repository ownership or publishing workflow changes. |

## Performance position

Rust makes native, memory-efficient implementation possible; it does not specify a universal fastest implementation or a profitable strategy. Performance work will measure feed freshness, decode, snapshot consistency, route evaluation, simulation, journal commit, signing, submission and eventual inclusion separately. Report hardware, network location, pool universe, load and p50/p95/p99. The time from provider observation to our receipt is also part of the opportunity budget.

Tokio handles asynchronous network activity; CPU-intensive route work receives its own bounded scheduling budget. Cancellation of an async handle is not assumed to interrupt running CPU work: evaluation tasks check deadlines and session revision cooperatively. Runtime documentation explains the limitations of blocking-task cancellation, which the design must accommodate. [Tokio blocking-task guidance](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)

The initial performance acceptance is instrumentation, bounded resource use and reproducible benchmark runs. Suggested numeric thresholds elsewhere in the package are provisional engineering targets until measured on agreed hardware; none is a measured result or trading latency guarantee. We will optimize actual bottlenecks without weakening reconciliation, exact arithmetic or signing constraints.

## Integration qualification register

| Item | Proposed choice | Verification required before implementation is declared usable |
| --- | --- | --- |
| Base client | Alloy | Pin a tested release; verify RPC/subscription support with selected provider, ABI encoding and chain identity. |
| Solana client | Official Rust SDK component crates | Pin a compatible release set; validate account subscriptions, commitment levels and transaction versions. |
| Base first venue | Uniswap V3 | Verified factory/pools and code, liquidity/tick data, exact math fixtures and simulation route. |
| Solana first venue | Orca Whirlpools | Verified program/pools, tick arrays, supported token accounts/extensions and instruction construction. |
| Base second venue | SushiSwap V2 candidate | Verify the actual Base deployment and eligible liquidity before committing; reject or replace if unsupported. |
| Solana second venue | Raydium CPMM candidate | Verify program and pool types; CPMM support does not imply CLMM support. |
| Initial assets | USDC-start by default; WETH on Base and wSOL on Solana as route assets, with optional separate starting-asset experiments | Official addresses/mints, metadata, behavior and pool liquidity; symbol-only matches are insufficient. |
| Full transaction simulation | EVM fork/RPC; supported Solana simulation/local fixtures | Record state context, provider limitations, fees and constraints; deterministic quote is not full execution. |
| Flash loans | No lender selected | Availability, reserve assets, current fees, transaction constraints, audits/code and repayment tests. |
| Private submission/priority routes | No provider selected | Current API, authentication, region, rate limits, cancellation semantics, tip/fee policy and empirical inclusion. |
| Hosting | Railway; private container services and PostgreSQL | Target project/environment, region, service budget, persistent capture volumes, backups and measured provider RTT. |

Alloy and Solana's official documentation establish that the selected client ecosystems exist; they do not establish that any particular DEX adapter or trading strategy has been implemented. [Alloy](https://alloy.rs/), [Solana Rust SDK](https://solana.com/docs/clients/official/rust)

## Decisions needed before spending or live execution

The design can proceed with placeholders for these choices. They are not requests to pause this handoff.

| Question | Working assumption | Must be settled before |
| --- | --- | --- |
| Private tool or commercial platform? | Private single operator | Multi-user API or custody work |
| Monthly infrastructure/data budget? | Unset; measure capture needs and obtain provider quotes | Buying paid feeds/servers |
| Preferred deployment region? | Railway confirmed; region remains unqualified | Selecting feed/provider deployment regions |
| Initial experimental notional sizes? | Explicit synthetic budgets chosen per experiment, zero real funds | First comparable paper study |
| Maximum actual loss/notional/fee budget? | Zero until a separate live policy exists | Enabling live capability or funding |
| Real wallet custody and signing model? | No production keys in the paper deployment | Live signer implementation |
| Which venues have reproducible usable state? | Qualify Uniswap V3 and Orca first | Claiming a paper experiment is complete |
| What denotes adequate evidence? | Data quality, complete costs, delayed evaluation, holdout and repeatability gates | Comparing candidates or considering live |
| What is the project name and brand? | Neutral working title: Arbitrage Research | Public branding or distribution |

## Research claims carried forward

Previous research motivated Solana/Base as candidates. Published arbitrage studies use different dates, detectors and categories; DEX volume is not arbitrage profit, and reported gross value is not the operator's retained income. The product must collect its own comparable observations. Historical counts are deliberately not embedded as live dashboard metrics or promises.

The uploaded script is not an approved foundation. This handoff specifies a new architecture and does not certify that script, its claimed trading record, any future smart contract, or any future deployment. Agent reviews are design reviews; independent testing and specialist security review remain deliverables before real funds.


## Publication and operating privacy

Private single-operator describes the intended application deployment and access model; it does not assert repository visibility. The supplied GitHub repository was public when inspected during setup. Checked-in examples contain synthetic data and no credentials. The repository setup does not purchase providers, fund wallets, expose an application endpoint, or enable live execution. Actual GitHub feature publication is recorded separately from prepared source in the setup record.
