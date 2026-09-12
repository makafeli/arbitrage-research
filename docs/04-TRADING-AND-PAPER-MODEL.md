# Trading and paper model

Status: v0.3 TARGET economics and paper-release specification, 12 September 2026. Source now implements bounded Base/Solana acquisition, exact candidate math, same-chain route decisions, retained-input replay and a durable virtual ledger. These do not establish a market-return study, current deployed-program equivalence, full transaction simulation or automatic paper execution. The [integration verification record](17-RESEARCH-INTEGRATION-VERIFICATION.md) tracks pending CI; this document does not certify a provider, deployed address or profitable strategy.

## Current research boundary

Current quotes include protocol pool fees and price impact. External execution, inclusion and operating costs are unavailable; opportunity net amounts therefore remain `null`, even when gross output exceeds input. Decision history keeps raw observations and rejection reasons separately from grouped candidates. Collection completeness remains `UNKNOWN`, while eligible attempts and reconciled transactions remain unavailable rather than invented zero totals.

A stopped PAPER session can receive an immutable virtual account with token principal and native fee inventory. The ledger supports checked reservations and journaled internal transitions, but the HTTP surface exposes account creation and inspection, not arbitrary reservation/settlement or balance reset. Running a PAPER worker captures and evaluates candidates; it never automatically posts a quote as a fill. Replay evaluates retained inputs under explicitly modeled historical timing. These implemented foundations do not satisfy the full-simulation and inclusion gates below.

## 1. Product question and bounded scope

The research engine should answer: which eligible chain, route, starting asset and trade size produces repeatable opportunities under explicit execution assumptions and measured costs? It must also be able to conclude that none is attractive. Chain-wide volume and other searchers' historical profits are background information, not evidence of this engine's attainable return.

The first strategy is an exact-input, same-chain cycle `A → B → A`, using two distinct pools. The initial asset universe is USDC/WETH on Base and USDC/wrapped SOL on Solana. The default starting asset is USDC on both chains, with separate virtual native ETH/SOL fee reserves. WETH-start and wrapped-SOL-start experiments are optional new configurations/sessions. Asset identity is the chain and contract/mint address; ticker symbols are display metadata. Only explicitly reviewed token addresses enter the universe.

Start with one venue adapter per chain, enabling cycles across two distinct verified pools on that same venue when available, for example different supported fee tiers. Two legs through the same pool are excluded. Cross-venue cycles enter phase two after the second adapter passes verification. If selected pools cannot form a cycle, show “no eligible routes” as a healthy result; the engine must not silently widen its universe. Three-hop cycles are a later capability, with separate performance and execution acceptance. Cross-chain bridging, centralized exchanges, derivatives, token launches and automatic token discovery are outside this baseline.

Baseline capital is configurable virtual inventory. Flash loans are excluded until a specific lender's deployment, supported assets, liquidity, fees, callback rules and atomic repayment have been verified and tested. A “flash-loan supported” SDK flag is insufficient.

## 2. Venue capability plan

| Chain / adapter | Initial capability | State needed | Execution status and verification ticket |
|---|---|---|---|
| Base / Uniswap V3 | First Base adapter: allowlisted exact-input quotes and two-pool cycles | Pool identity, token metadata, sqrt price, active liquidity, tick bitmap and traversed ticks | `VEN-01`: verify factory/pools, bytecode and fee tiers; prove quote parity; test executor on a pinned fork |
| Base / SushiSwap V2 candidate | Second Base venue, conditional on deployment and useful overlapping liquidity | Verified pair reserves, fee rules and router/pair semantics | `VEN-02`: verify current Base deployment and supported pair behavior before selecting; otherwise record replacement decision |
| Solana / Orca Whirlpools | First Solana adapter: explicitly supported Whirlpool variants | Pool, vaults, mint metadata, tick arrays and applicable fee/oracle accounts | `VEN-03`: pin program/SDK versions; verify account ownership, pool variants, quote parity and composed simulation |
| Solana / Raydium CPMM candidate | Second Solana venue; CLMM and AMM v4 excluded initially | Pool configuration, reserves/vaults, token metadata and all applicable fee fields | `VEN-04`: verify deployment, instruction layout, Rust integration and overlapping pools; reject unsupported extensions |

Orca publishes Rust client and math/quoting libraries; use them as pinned integration candidates and retain independent fixtures. Raydium's CPMM repository is distinct from its other AMM families: support for one is not support for all. [Orca source and SDKs](https://github.com/orca-so/whirlpools), [Raydium CPMM source](https://github.com/raydium-io/raydium-cp-swap)

Each adapter declares discovery, snapshot, quote, instruction construction, transaction simulation and live execution as separate capabilities. Initially, no adapter advertises live execution. Token-2022 extensions, transfer taxes, rebasing, transfer hooks, dynamic fees and administrative freeze behavior require explicit classification; unsupported mechanisms fail closed. A familiar ticker does not override those checks.

## 3. Data ingestion and consistent state

Rust workers receive subscriptions, recover gaps through bounded RPC reads, and maintain immutable snapshots in memory. Every event records chain, provider, receive timestamp, provider timestamp when available, block/slot identity, commitment/finality and sequence information. Raw input and decoding version accompany replay records. Monotonic clocks measure local latency; UTC timestamps support correlation. Track clock error rather than treating provider timestamps as perfect clocks.

Base snapshots pin all required reads to the same canonical block hash. Track parent hashes, removed logs and restart checkpoints. Streaming preconfirmations are a separate, explicitly provisional data tier, reconciled against subsequent blocks. A fresh pool combined with a stale tick bitmap is invalid. Base documents several distinct finality stages; preconfirmation must not appear as final settlement in research or the ledger. [Base finality documentation](https://docs.base.org/specifications/transactions/transaction-finality)

Solana routes require a coherent account set. Record context slots and commitment, preserve update ordering, and obtain the full route account set in one RPC response where possible. `getMultipleAccounts` exposes context and a maximum account count; `minContextSlot` is a lower bound, not a request for an exact historical slot. Independent subscriptions carrying the same slot number are not sufficient proof of a complete transaction-consistent account snapshot. [Solana account RPC](https://solana.com/docs/rpc/http/getmultipleaccounts)

If one coherent read is impossible, use a verified reconstruction mechanism or classify the candidate as inconsistent. Do not blend independently fetched account batches into a “confirmed” quote. Initial publication uses confirmed data; lower-latency processed data is a separately labelled experiment requiring fork reconciliation. Missing updates, failed gap recovery, provider disagreement or unknown program changes make affected routes ineligible until resynchronization.

## 4. Route search, arithmetic and amounts

Maintain a graph of verified assets and pools, updating only routes affected by changed state. Candidate generation may use approximate arithmetic for ranking, but every accepted quote uses checked integer math with protocol-specific widths, fixed-point formats and rounding. No floating-point number decides profitability or minimum output. Uniswap V3's swap math distinguishes input, output and fee rounding and advances through liquidity ranges; multiplying spot prices is not a substitute. [Uniswap V3 swap math](https://github.com/Uniswap/v3-core/blob/main/contracts/libraries/SwapMath.sol)

Evaluate a bounded size grid against available virtual inventory and configured notional limits, then refine promising intervals. Concentrated liquidity can make the objective discontinuous around tick boundaries; do not assume a smooth function or guaranteed global optimum. Cap work per update and expose candidates skipped by the search budget.

Apply each leg to a private mutable copy of route state, passing the actual output into the next leg. Initially prohibit repeated pools in one cycle. Account for price impact, rounding, liquidity exhaustion, token account constraints and transaction size/compute limits. A route must end in precisely the starting asset, with explicit residual-token accounting.

The later live executor must bind the allowed pools/programs, input cap, route, deadline/expiry, minimum output and final balance increase. A successful route simulation does not establish this protection: test the actual transaction and its final invariant. Solana may require a custom executor/guard; ordinary client-side checks alone cannot enforce a final on-chain profit condition.

## 5. Four evidence levels

Session modes are immutable `OBSERVE`, `PAPER`, `REPLAY` or `LIVE`. Changing mode requires a new session, preserving the source session's audit history. The evidence levels below are independent of session mode: a live worker still produces many candidates that never become trades.

UI/API labels are `CANDIDATE`, `SIMULATED`, `ESTIMATED_EXECUTABLE` and `REALIZED`. A local mathematical quote or arithmetic replay remains `CANDIDATE`. `SIMULATED` requires successful complete atomic-transaction simulation tied to the exact plan and identified state; a failed simulation is a rejection with its evidence retained. Only reconciled actual execution at the configured settlement stage qualifies as `REALIZED`.

`ESTIMATED_EXECUTABLE` additionally requires coherent fresh state, current exact-plan simulation, supported atomic execution and final balance protection, valid principal and native-fee reservations, complete cost estimates, applicable limits and a named inclusion/delay scenario. Paper reservations are virtual and remain visibly hypothetical. Unknown or unsupported checks cannot pass by default; a positive result or earlier successful simulation does not establish current eligibility.

| Level | What is executed | What the result means |
|---|---|---|
| Quote | Local adapter math against identified state | Mathematical route outcome at that state |
| Transaction simulation | Complete unsigned/dummy-funded transaction in a controlled environment or supported RPC simulation | Instructions execute under the recorded simulation conditions |
| Shadow paper | Live observations plus virtual inventory and explicit execution scenarios; no broadcast | Counterfactual estimate with known limitations |
| Replay | Recorded observations processed through the same engine with a virtual clock | Reproducible historical experiment at the recorded information resolution |

Solana's simulation RPC permits simulation without signature verification and reports its context. Replacing a blockhash or overriding accounts changes the experiment and must be recorded. Simulation success does not prove inclusion or that the required real balances exist. [Solana transaction simulation](https://solana.com/docs/rpc/http/simulatetransaction)

On Base, local fork tests can fund test accounts and instantiate the proposed executor without production deployment. Record the fork hash, overrides and code version. Solana local program tests similarly record imported accounts/program binaries. Synthetic funding, modified blockhashes and incomplete fixtures remain declared assumptions; they cannot establish live eligibility. Incomplete fixtures cannot establish complete atomic-transaction simulation. Such fixtures test mechanics; they are not live trading results. Paper infrastructure receives no production private keys and cannot broadcast transactions.

## 6. Profit and cost accounting

Let `q0` be the starting amount in asset A, `q1` the final amount after all swaps, `F_A` any financing fee in A, and `C_k` an external cost paid in asset k. Let `V_t(x, asset → R)` convert an amount to reporting currency R using a recorded contemporaneous valuation policy.

`route_surplus_A = q1 − q0 − F_A`

`trade_contribution_R = V_t(route_surplus_A, A → R) − Σ V_t(C_k, k → R)`

`period_result_R = Σ settled_or_scenario_contributions_R − fixed_operating_costs_R`

Adapter outputs already include their pool swap fees and price impact; never subtract those fees again. Attach a fee-inclusion manifest to every quote. Financing defaults to zero. Native transaction fees and tips remain explicit external costs unless already included in a measured balance delta; reconciliation must prevent double counting.

Base costs include execution and L1-related data costs under the applicable fee model, plus any separately paid service/builder charges. Estimate against the constructed transaction and reconcile actual receipt fields later. Solana costs include applicable transaction and priority fees, configured tips, and account setup costs; refundable rent is tracked as committed capital rather than automatically treated as a permanent loss. [Base network fees](https://docs.base.org/specifications/transactions/network-fees), [Solana fees](https://solana.com/docs/core/fees)

A failed atomic trade normally has no swap surplus but can incur transaction fees. An unlanded transaction may incur different costs from an included revert; provider subscription costs continue in either case. Model each outcome explicitly. Fee caps are affordability limits, not statements that a transaction will land. Show contribution before infrastructure and period results after infrastructure separately, with the allocation policy visible.

Maintain start-asset balances and native-fee reserves separately. Value inventory using a documented external/reference source with timestamp and staleness limit; do not use the detected mispriced pool as the sole reporting oracle. Stablecoins are not unconditionally valued at one dollar. Show inventory price movements separately from arbitrage contribution and compare against holding the same starting inventory.

## 7. Inclusion scenarios and honest paper results

Paper mode offers named assumptions rather than invented fill probabilities:

- **Immediate state:** execute against the observed snapshot. This is an optimistic diagnostic bound, not the default attainable result.
- **Measured delay:** advance to the first complete recorded state after observed processing and configurable submission delay, then re-quote and enforce the same limits.
- **Competitive displacement:** remove an opportunity after a competing state-changing trade, or apply a documented additional delay; count the candidate as missed or failed according to the scenario.
- **Fee stress:** re-evaluate execution costs against explicit elevated fee/tip inputs and configured caps.

These are sensitivity cases, not statistical confidence intervals. Do not assign a “70% chance” without a validated model and appropriate observations. End-of-block data cannot reconstruct unavailable intra-block opportunities. Label that resolution limit prominently and omit conclusions it cannot support.

Virtual trades reserve capital and fee budgets, incur their simulated price impact and cannot reuse the same funds simultaneously. Deduplicate by chain, route/pool sequence, starting asset and overlapping opportunity interval; repeated observations update one episode. Alternatives competing for the same state or inventory are mutually exclusive. Reconcile simulated state when real observations arrive and record the counterfactual approximation rather than repeatedly harvesting an unchanged price discrepancy.

Partition data chronologically into development, calibration and untouched evaluation windows. Choose thresholds on development/calibration data; freeze the strategy before evaluating the holdout. Report selection rules, excluded hours and changes of universe. Do not optimize on the holdout or count replayed versions of the same interval as independent evidence.

## 8. Comparison and evidence gates

Compare chains over matching observation windows with equivalent capital budgets, start-asset valuation, route classes and data quality. Report eligible pool-hours and missing-data time beside outcomes. A chain with broader adapter coverage has a sampling advantage; it has not necessarily become more profitable.

Required metrics are distinct opportunity episodes per eligible hour; quote-to-simulation agreement; delay survival; missed/rejected/failed outcomes; scenario contribution distributions; total costs; capital utilization; inventory drawdown; concentration by pool and day; and p50/p95/p99 processing latency. Keep quote candidates, simulated transactions and actual settled trades in separate totals. Include “insufficient evidence” and “negative after costs” as ordinary conclusions.

Provisional gates for expanding scope are reproducible quote parity, coherent replay, functioning cost caps, no unexplained balance reconciliation differences, and holdout reports covering both busy and quiet periods. These are engineering/research policies, not proof of future profitability. Live readiness additionally needs executor verification, operational recovery tests and an explicit live configuration under the product's authorization workflow.

## 9. Acceptance fixtures

Fixtures must capture inputs, expected integer outputs, code version and provenance. Include:

1. A V3 tick crossing, fee rounding boundary, exhausted range and exact minimum-output boundary; compare against pinned protocol execution.
2. Solana missing tick arrays, incompatible mint extensions, changed program ownership and mismatched account contexts; reject without a usable quote.
3. A cycle positive before network fees and negative after them; reject while preserving the calculation breakdown.
4. A USDC cycle with ETH/SOL fees and a changing reference price; reconcile currencies without fee duplication.
5. Duplicate events, stream disconnection, reorg/fork rollback and restart from a checkpoint; replay yields identical canonical results.
6. Two simultaneous candidates using the same inventory, and a repeatedly observed discrepancy; only one compatible allocation receives simulated capital.
7. An included revert versus expiry without inclusion, with distinct fee outcomes; retain unresolved status until evidence resolves it.
8. A profitable quote whose composed transaction exceeds compute/gas/account limits; classify as non-executable, with no simulated profit credited.

Completion means the system can reproduce and explain these outcomes. It does not mean a profitable trading strategy has been discovered.
