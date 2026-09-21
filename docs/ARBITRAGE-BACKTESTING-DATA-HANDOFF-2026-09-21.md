# Arbitrage Backtesting and Historical Evidence
## Project implementation handoff

| Item | Value |
| --- | --- |
| Project | `makafeli/arbitrage-research` |
| Prepared | 21 September 2026 |
| Document version | 1.0 |
| Status | Proposed implementation plan, not a completed backtest |
| Primary scope | Base, same-chain atomic arbitrage, research and paper testing only |
| Later scope | Solana after the Base evidence pipeline passes its gates |
| Delivery principle | Extend the existing platform; do not build a parallel backtesting engine |
| Authorization | This document does not authorize purchases, deployment changes, wallet funding, signing, or live transactions |

> **Objective:** Build a reproducible evidence pipeline that first validates our simulator against historical executions, then tests whether our own strategy finds opportunities without hindsight and remains profitable under explicit execution-cost and competition assumptions.
>
> **Critical distinction:** Reproducing someone else's profitable transaction validates parts of our software. It does not establish that our bot could have discovered, won, or executed that transaction.

## 1. Executive decision

Use **Goldphish as a methodological reference**, **Dune as a candidate-discovery source**, and **historical chain records and state as the reconciliation and replay foundation**. Do not treat any of these as a ready-made guarantee of profitable trading.

Deliver two separate datasets:

1. **Execution benchmark:** reconciled historical arbitrages and negative controls, used to test classification, accounting, and replay accuracy.
2. **Strategy evaluation corpus:** a continuous, preselected time window containing the supported market state, including periods with no trades, rejected candidates, failures, and missing-data intervals.

The first answers, "Does our replay reproduce the evidence?" The second answers, "Does our strategy independently find an economically interesting opportunity under the declared assumptions?"

A valid outcome can be positive, negative, or inconclusive. Do not modify the evaluation until it produces a positive result. Findings about historical third-party profits, hypothetical strategy returns, and our own realized returns must never be combined.

## 2. Verified project baseline and integration boundary

The repository README describes a Rust-first Base/Solana research platform with a React/TypeScript dashboard. It identifies capture, durable research records, virtual accounts, and frozen exports, while listing protocol/provider qualification, complete atomic simulation, automatic paper execution, and live execution as unfinished. Its initial examples use USDC-start two-leg routes, with Uniswap V3 and Orca Whirlpools as qualification targets. [R1]

The replay README documents offline capture verification and candidate evaluation, with no network fallback. It explicitly says these capabilities do not simulate complete transactions or settle paper trades. Existing route mathematics includes pool fees and price impact; external costs remain unknown in the opportunity projection. [R2]

The architecture document identifies the shared engine, adapter, persistence, and control boundaries. It distinguishes implemented capture/research behavior from target execution architecture. [R3]

**Inspection limitation:** These are documentation-level observations from the default branch, not an independent audit of the running deployment. No project tests, historical backtests, provider calls, Dune queries, or profitability calculations were executed to prepare this handoff. No repository or Railway changes were made.

Preserve the current evidence boundaries and existing capture provenance. Read the latest implementation status and backlog before assigning work. This handoff does not establish the current status of individual GitHub issues.

## 3. Data-source register

### 3.1 Goldphish and the USENIX study: methodology reference

**Research:** [A Large Scale Study of the Ethereum Arbitrage Ecosystem](https://www.usenix.org/conference/usenixsecurity23/presentation/mclaughlin)

**Repository:** [ucsb-seclab/goldphish](https://github.com/ucsb-seclab/goldphish)

The 2023 paper's abstract reports 3.8 million identified arbitrages and $321 million in aggregate profit over a 28-month observation period. Separately, it reports a hypothetical opportunity-detection result of 395 ETH per week. These are author-reported historical results under the paper's definitions, not our independently reproduced results or current expected returns. [S1]

The README separates performed-arbitrage scraping from historical opportunity seeking. It documents PostgreSQL and a geth archive node as dependencies. [S2]

**Project use:** study identification, sizing, fee accounting, and separation of observed executions from hypothetical opportunities. Review the full paper's execution and cost assumptions before reproducing its headline results.

**Do not assume:** a turnkey downloadable profit ledger, compatibility with current Base pools, current infrastructure requirements matching old estimates, or permission to copy code without checking its license. Do not deploy its example infrastructure configuration unchanged.

### 3.2 Dune: candidate discovery, not net-profit ground truth

**Base/EVM:** [dex.trades documentation](https://docs.dune.com/data-catalog/curated/dex-trades/evm/dex-trades)

**Solana:** [dex_solana.trades documentation](https://docs.dune.com/data-catalog/curated/dex-trades/solana/solana-dex-trades)

The EVM table exposes individual swap legs, transaction and event identifiers, contract addresses, and raw token amounts. The Solana table exposes corresponding trades with transaction, slot, and instruction-order fields. [S3][S4]

**Project use:** extract candidate routes, transaction IDs, and pool identities for further inspection. Preserve the original export, query text, parameters, execution ID when available, and extraction time.

**Limitations:** these trade tables are not complete transaction-state snapshots or all-in profit ledgers. Do not assume exhaustive protocol coverage or use them as the sole source of reverted attempts. Reconcile selected records against chain evidence. A documented table does not prove our account has adequate query/export access.

### 3.3 Optimistic-MEV: supporting classification material

**Repository:** [kingbaldwin-iv/Optimistic-MEV](https://github.com/kingbaldwin-iv/Optimistic-MEV)

The repository listing includes `base_period1.parquet`, `base_period2.parquet`, and classification SQL. The inspected `transaction_classification.sql` groups transaction counts and gas usage by date, success, and activity categories. It does not produce a transaction-level net-profit ledger. [S5]

**Project use:** review classification ideas and possible negative-control sources. The Parquet files were not downloaded or inspected for this handoff. Do not assert their row-level semantics, date coverage, completeness, or profitability fields until their schemas and contents are checked.

### 3.4 Tardis: deferred centralized-exchange research

**Documentation:** [Downloadable historical datasets](https://docs.tardis.dev/downloadable-csv-files/overview)

Tardis documents historical order-book updates, snapshots, trades, and related market data. It states that historical files for the first day of each month are available without an API key. Access outside that allowance must be checked before use. [S6]

**Project use:** a possible later input for exchange-to-exchange research. It is not a profitable-strategy dataset and is not a dependency of the Base milestone. Do not purchase access under this handoff.

### 3.5 Reference excluded pending verification

The previous discussion cited `https://arxiv.org/abs/2606.00720` and a large Base arbitrage dataset. The abstract and HTML endpoints could not be retrieved in this verification pass. This does not establish that the paper is invalid, but its identity, counts, methodology, and public dataset availability remain **unverified for this handoff**.

Do not use its headline numbers in project requirements, benchmarks, or presentations as verified evidence. Reintroduce it only after checking the primary paper and the actual accessible artifact.

## 4. First milestone: a bounded Base experiment

**Proposed scope, not a claim of existing qualification:**

| Dimension | Initial boundary |
| --- | --- |
| Network | Base only |
| Route | Configured USDC -> WETH -> USDC, through two distinct supported pools |
| Protocol | The project's Uniswap V3 qualification target; exact deployments must be verified |
| Identity | Chain plus contract address, never ticker alone |
| Capital | Explicit virtual principal and separate virtual fee reserve |
| Funding model | Own-capital hypothetical execution first; flash loans are a separately modeled extension |
| Data access | Existing authorized access or bounded public exports only |
| Runtime | Existing Rust research/replay architecture; no signer or public-chain broadcaster |
| First output | Reconciled benchmark plus a deterministic replay report |

Do not add unsupported pools merely to obtain more positive examples. Do not expand to Solana, cross-chain routes, centralized exchanges, or multi-transaction inventory strategies before the Base data and accounting gates pass.

**Suggested dataset targets:** begin with 20 verified in-scope executions and at least 10 negative, incomplete, or failure controls. These are engineering targets, not promises that the source contains that mix. Record shortfalls honestly. Synthetic failure fixtures may supplement testing but cannot replace missing historical evidence or inflate real-data counts.

Begin with a one-day extraction to qualify the pipeline. Expand to a fixed seven-day engineering pilot only after access, coverage, and resource usage are understood. These small windows test the pipeline, not long-term profitability.

## 5. Pipeline and evidence boundaries

```text
Historical swap export + chain records
                    |
                    v
       Source manifest and coverage checks
                    |
                    v
        Candidate route classification
                    |
                    v
   Receipts, transfers, code, state, and fees
                    |
                    v
      Reconciled external-execution dataset
                    |
          +---------+----------+
          |                    |
          v                    v
   Golden replay tests   Unfiltered time-window data
          |                    |
          v                    v
  Simulator validation   Blind strategy discovery
                               |
                               v
                 Counterfactual execution scenarios
                               |
                               v
                   Separate hypothetical P&L ledger
                               |
                               v
                    Evidence report and decision
```

Acquisition may use approved read-only network access. After a dataset is frozen, replay must use retained inputs only. Missing historical evidence is an explicit failure or unknown state, never permission to query today's pool state.

Use a separate provenance dimension for **external historical evidence**, **synthetic fixtures**, **model replay**, and **forward paper observations**. An imported successful transaction must not become one of our own `REALIZED` trades.

| Evidence statement | Required support |
| --- | --- |
| Swap candidate | Source record and identifiers; no profitability claim |
| Historical cyclic execution | Reconciled route and transaction outcome |
| Historical net on-chain profit | Defined accounting boundary, complete relevant flows and costs, documented valuation |
| Route-math replay passes | Correct inputs and exact supported protocol arithmetic |
| Complete transaction replay passes | Full plan executed against the correct historical environment with reconciled effects |
| Hypothetical strategy profit | Independent decisions, declared scenario, complete modeled costs and denominators |
| Our realized profit | Our actual settled transactions and reconciled accounting; outside this milestone |

Missing costs must remain `null` or explicitly unbounded. Never convert unknown to zero to permit a positive label.

## 6. Dataset design

### 6.1 Keep the benchmark separate from the evaluation corpus

**Benchmark corpus:** curated examples for correctness. It may deliberately contain known profitable executions, ordinary multi-hop swaps that are not arbitrages, completed but unprofitable cycles, reverted attempts with evidence supporting their classification, and incomplete records. Its selection bias is intentional and must be disclosed.

**Evaluation corpus:** all necessary historical inputs within a fixed window and supported universe, not only transactions classified as arbitrage. Include quiet intervals, unsupported-state exclusions, collection failures, and data gaps. Do not discard periods because their results are negative.

Freeze the universe and period before inspecting performance. For historical market-selection tests, use information available at the decision time. A universe selected from today's surviving pools is a restricted retrospective benchmark, not an unbiased reconstruction of a deployable historical selection strategy.

### 6.2 Proposed logical records

Map these records into the existing model after reviewing its contracts. Names below are proposals, not existing tables or API fields.

| Record | Required content |
| --- | --- |
| `dataset_manifest` | Schema version, source references, extraction/query metadata, UTC interval, covered blocks, content hashes, license/access notes, exclusions, completeness status |
| `historical_execution` | Chain, transaction ID, block number/hash, transaction index, canonicality, outcome, route classification and confidence |
| `swap_leg` | Ordered leg identity, verified protocol/pool, asset addresses, exact raw input/output, source-row reference |
| `balance_effect` | Account, asset, exact signed delta, transfer/state evidence, economic-owner attribution status |
| `cost_component` | Asset, exact amount, observed/modeled/unknown status, accounting inclusion flag, evidence reference |
| `state_checkpoint` | Exact prestate anchor, code/storage objects, replay method, chain upgrade/environment version, content hashes |
| `decision_record` | Observation availability, strategy/configuration versions, sizes considered, admission/rejection reason, input digests |
| `scenario_attempt` | Candidate ID, scenario ID, inclusion position/assumption, outcome, cost effects, unavailable evidence |
| `run_report` | Dataset/config/code digests, denominators, metrics, limitations, outcome, reproducible command and artifacts |

Store raw amounts as checked integers or decimal integer strings. For interchange, support the full required range rather than silently narrowing into floating point or a smaller decimal type. Persist decimals and valuation provenance separately.

Maintain an explicit cost-inclusion flag such as `already_in_balance_delta`, `already_in_route_output`, or `additional_cost`. Enforce that a cost affects P&L once.

The manifest must preserve raw-source checksums and derivation versions. Redact credentials and provider secrets. Historical public addresses and transaction IDs may be retained as evidence, subject to source terms; private account metadata must not leak into exported reports.

## 7. Starter Base extraction query

**Status: illustrative Dune SQL, not executed or validated against an authenticated account.** It screens transactions with multiple swap rows. It does not identify proven arbitrage, compute profits, or guarantee complete coverage.

The column names below follow the documented EVM trade schema. `project_contract_address` may identify an emitting contract rather than the final verified pool identity. [S3]

```sql
-- One-day discovery export. Review query cost before execution.
-- UTC bounds are an example; freeze the chosen interval in the manifest.
WITH scoped AS (
    SELECT
        blockchain,
        block_time,
        block_number,
        tx_hash,
        evt_index,
        project,
        version,
        project_contract_address,
        token_sold_address,
        token_bought_address,
        token_sold_amount_raw,
        token_bought_amount_raw,
        tx_from,
        tx_to
    FROM dex.trades
    WHERE blockchain = 'base'
      AND block_date >= DATE '2026-09-14'
      AND block_date < DATE '2026-09-15'
      AND block_time >= TIMESTAMP '2026-09-14 00:00:00'
      AND block_time < TIMESTAMP '2026-09-15 00:00:00'
), candidate_transactions AS (
    SELECT tx_hash
    FROM scoped
    GROUP BY tx_hash
    HAVING COUNT(*) >= 2
)
SELECT s.*
FROM scoped AS s
JOIN candidate_transactions AS c ON s.tx_hash = c.tx_hash
ORDER BY s.block_number, s.tx_hash, s.evt_index;
```

The transaction-hash sort above is only stable export ordering. Fetch the true transaction index from chain records before reconstructing execution order.

Preserve all legs of each exported transaction. Avoid a row limit that silently cuts a transaction in half. For larger intervals, partition by bounded time/block ranges and record truncation or export limits.

Next steps are to verify identities, group and order legs, detect candidate closed asset flows, reconcile external transfers and loans, and retrieve receipts and relevant historical state. A closed first/last asset pattern alone is insufficient when transactions contain branching flows or multiple owners.

This query is for the benchmark-discovery lane. It is **not** the data source for a blind strategy's complete opportunity denominator.

## 8. Historical replay: required correctness

### 8.1 Reconstruct the actual pre-transaction state

For an execution at block `B`, transaction position `i`, use the state immediately before that transaction. Do not substitute the end of block `B`, and do not assume the end of `B - 1` already includes earlier transactions in `B`.

Acceptable implementation approaches are a qualified chain-aware replay from the parent state through the preceding transactions and applicable system transitions, or a sufficiently complete historical witness/checkpoint that reproduces the target execution and environment. Retain the block header context, code versions, balances, nonces, storage, and fee rules required by the selected implementation.

Geth's prestate tracer can expose accounts and storage touched by a transaction, and its diff mode exposes changed pre/post state. The documentation explicitly notes that this is not a cryptographic proof. Provider support and applicability to the chosen Base history must be qualified independently. [S7]

A witness for one historical winning transaction may be insufficient for a different route or trade size. Missing storage must fail closed rather than defaulting to zero.

### 8.2 Separate quote replay from transaction replay

First validate exact pool mathematics, rounding, tick transitions, and supported fee behavior. Then validate the complete atomic transaction, including callbacks, balances, permissions, transfers, and applicable execution costs.

Passing independent swap calculations is not a substitute for executing the complete transaction. Preserve this distinction in test names, APIs, and dashboard labels.

Use a Base-compatible execution environment pinned to the historical upgrade rules. Do not assume an ordinary Ethereum fork configuration reproduces all Base execution and fee behavior.

### 8.3 Keep exact reproduction and hypothetical execution separate

**Exact reproduction** replays the observed historical transaction for validation.

**Hypothetical execution** inserts our plan at a declared eligible point with declared funding and timing assumptions. It cannot silently borrow the historical winner's account permissions, balances, or priority.

If our simulated trade changes pool state, later outcomes cannot simply reuse the untouched historical outputs. Either maintain a consistent counterfactual branch, or use independent opportunity probes and explicitly prohibit summing them into a portfolio-return claim.

Never delete the historical winner and assign its profit to our bot without labeling this as an optimistic counterfactual. Backrun-dependent opportunities require an explicit explanation of when our bot could observe the triggering state and submit under its actual feed model.

### 8.4 Validate the verifier

Include corrupted captures, missing legs, wrong block hashes, incorrect transaction ordering, altered decimals, unsupported token behavior, unavailable state, and deliberately optimistic fee inputs. These must fail or remain inconclusive, not become profitable examples.

## 9. Accounting specification

Define the economic boundary before calculating profit: executor, initiating account, identified fee payer, loan liabilities, and attributable payouts. Do not equate one wallet's token increase with the entire operation's profit.

Prefer a complete attributed cash-flow/balance ledger with external capital flows removed. Route output minus input is an intermediate diagnostic, not a substitute for that ledger.

```text
Historical net on-chain P&L
  = value of attributable net asset changes
  - external capital contributions
  + external capital withdrawals
  - attributable costs not already included in those changes

Hypothetical operating P&L over a run
  = sum of simulated settled net trading outcomes
  - modeled failed-attempt costs not already charged
  - allocated infrastructure and data costs
```

Use one declared reporting currency and time-consistent valuations. Preserve native-unit results. Stablecoin units must not be silently treated as guaranteed USD, and capital transfers must not be treated as trading profit.

Base documentation describes L2 execution and L1 publication/security fee components. Account for the fee rules applicable to each historical block and transaction, using observed receipt effects where available. Do not apply today's settings to an older interval. [S8]

Required treatment:

- Pool fees and price impact already reflected in modeled execution amounts are not deducted again. Slippage is modeled through state/timing and execution bounds, not a second arbitrary deduction on an already adjusted fill.
- Loan principal is a liability, not revenue. Loan fees and repayment must reconcile. Unsupported funding models remain excluded.
- Priority fees, explicit payments, and other attributable costs must be separated sufficiently to prevent double counting.
- Included reverted attempts can incur fees. Rejected or unsubmitted attempts must not be assigned a paid chain fee unless the scenario actually includes one; provider/compute costs are separate.
- Unknown ownership, incomplete fee attribution, or missing valuation produces an incomplete result. Historical on-chain profit does not establish the third-party operator's net business profit after private infrastructure costs or off-chain payments.

Report aggregate run costs independently of per-trade allocations so allocation choices cannot hide an overall loss.

## 10. Blind evaluation and scenario design

### 10.1 Freeze the research plan

Before inspecting performance, register the supported universe, UTC interval, strategy revision, trade-size grid, virtual capital, fee reserve, information-availability policy, inclusion model, and exclusion rules.

For a larger evaluation, a proposed starting split is 14 chronological development days, seven validation days, and seven untouched test days. This is an engineering default, not a statistically sufficient sample by itself. Change it before results are inspected when coverage or experiment design requires a different interval.

Tune only on development/validation data. A change after test-set inspection creates a new experiment and requires fresh holdout data. Record all tested configurations and negative results. Do not report only the best combination from a large search.

### 10.2 Respect the actual information boundary

The replay clock must distinguish chain event time, when information was available to this strategy, computation completion, hypothetical submission, and hypothetical inclusion.

Do not grant finalized-state observation the capabilities of an earlier feed. If the corpus lacks historical arrival times or intermediate-state information, report timing as modeled and restrict conclusions accordingly. Millisecond-level claims require matching evidence resolution.

A winning historical transaction or its future profitability label cannot be an input to the strategy being evaluated. It may be used afterward to inspect detector behavior within the benchmark's limited coverage.

### 10.3 Required scenarios

| Scenario | Purpose | Claim boundary |
| --- | --- | --- |
| Exact historical reproduction | Validate state, execution, and accounting | Not a strategy return |
| Zero-latency optimistic probe | Estimate an explicitly optimistic opportunity ceiling | Not deployable profitability |
| Configured-feed baseline | Model our feed, decision path, funding, and eligible inclusion point | Hypothetical, with evidence gaps disclosed |
| Delayed/adverse case | Stress later eligible inclusion and less favorable state | Recompute the trade; do not just reduce a chart |
| Higher-cost case | Stress fee/payment and operating-cost assumptions | Derive inputs from the selected period or identify assumptions |
| Data-loss case | Verify behavior under missing, delayed, or inconsistent inputs | Unknown coverage cannot be counted as successful operation |

Use measured timing and fee distributions where available. Otherwise publish the assumptions and sensitivity range, not a fitted probability presented as fact. Do not assign an arbitrary capture probability and call the resulting expected value validated.

### 10.4 Avoid duplicate or impossible earnings

Model virtual inventory reservations, outstanding exposure, pool-state changes, opportunity expiry, and competition between overlapping plans. The same price discrepancy cannot be captured repeatedly against the same unchanged state.

Do not use capital simultaneously in multiple mutually exclusive attempts. A rejected attempt releases reservations only under the declared lifecycle. A chain outcome that is unknown is not automatically a loss, success, or free retry.

Where counterfactual future behavior is not modeled, report isolated opportunity feasibility, not compounded P&L or portfolio returns.

## 11. Metrics and report contract

Every run must expose both evidence quality and economics.

| Category | Required output |
| --- | --- |
| Coverage | Intended and covered interval, block/pool coverage, gaps, unknown state, excluded protocols |
| Source quality | Raw/normalized counts, duplicate handling, missing receipts/traces, truncation, provenance failures |
| Replay validation | Eligible fixtures, exact matches, explicit mismatches, unsupported cases, state/amount/gas discrepancies |
| Strategy activity | Decision opportunities, admitted/rejected candidates, attempt outcomes, rejection reasons |
| Economics | Route gain, each cost category, net trading P&L, failed-attempt spend, run-level operating P&L |
| Capital | Starting capital, available/reserved capital, peak exposure, drawdown, unresolved inventory |
| Robustness | Results by day, pool, size, and scenario; concentration in a few outcomes; sensitivity to outliers |
| Reproduction | Dataset digest, code revision, configuration digest, scenario seed where applicable, exact command |

Define every denominator. "Success rate" must state whether it means profitable candidates, successful simulations, included executions, or positive net outcomes. Detection recall measured against the curated benchmark is not recall across the entire market.

For sparse or dependent observations, disclose that uncertainty estimates may be unstable. Do not treat thousands of near-identical opportunities from one episode as thousands of independent observations. Report daily variability and clustered outcomes alongside totals.

Every report must finish with one of: **SUPPORTED UNDER STATED MODEL**, **NOT SUPPORTED UNDER STATED MODEL**, or **INCONCLUSIVE**. None means a live-profit guarantee.

## 12. Implementation placement

Extend the existing replay application and shared calculation components, preserving their current behaviors. Add a separate acquisition/enrichment path rather than giving deterministic offline replay ambient network access.

Proposed new artifact layout, subject to repository review:

```text
research/backtesting/
  README.md
  source-register.md
  experiments/
    base-two-leg-v1.yaml
  queries/
    base-multiswap-candidates.sql
  schemas/
    dataset-manifest.schema.json
    historical-execution.schema.json
    run-report.schema.json
  fixture-manifests/
    base-golden-v1.json
  reports/
    <run-id>/
      report.md
      metrics.json
      coverage.json
      provenance.json
      decisions.csv
      outcomes.csv
```

Do not add generated bulk state to Git. Retain content-addressed raw objects in an approved storage location with tested retention and reproducible retrieval. Put small redistributable fixtures or their manifests in the repository as permitted by source terms.

Map durable records into existing persistence and export boundaries where appropriate. Avoid a second virtual-account ledger with different rounding, cost treatment, or lifecycle rules. New provenance or schema fields require explicit versioning and backward-compatibility tests.

Use bounded acquisition and replay jobs. Before expanding a window, measure requests, bytes, storage, runtime, and memory. Do not launch a full-chain archive rebuild or an unbounded Railway worker as the default implementation.

## 13. Work packages and dependencies

These are handoff identifiers, **not existing GitHub issue numbers**. Map them to the current backlog before opening or closing tickets. Attach work to existing epics where it overlaps.

### BT-01: Freeze scope and qualify source access

**Owner:** orchestrator / technical lead. **Depends on:** current repository and backlog review.

Confirm supported deployments, proposed universe, available export/RPC access, resource limits, and source licensing. Probe a bounded historical sample for receipts, traces, historical code/storage, fee fields, and exact-state reconstruction capabilities.

**Acceptance:** committed experiment draft, source capability matrix, access/coverage limitations, and explicit blockers. No new paid service, no secret exposure, and no assumption that historical storage exists because current RPC reads work.

### BT-02: Add manifests and candidate import

**Owner:** data engineering. **Depends on:** BT-01.

Implement the bounded export/import path, raw-object hashing, exact amounts, identities, deduplication, query provenance, and completeness checks. Keep imported external evidence distinguishable from platform-origin captures.

**Acceptance:** deterministic re-import; identical normalized output for identical input; corrupted and truncated inputs rejected or quarantined; source/export limits visible. Candidate classification is not labeled profitable.

### BT-03: Reconcile executions and create the benchmark

**Owner:** chain integration. **Depends on:** BT-02.

Enrich candidate transactions with canonical identifiers, transaction order, receipts, logs, transfers, attributable accounts, fee evidence, and classification confidence. Include negative controls.

**Acceptance:** each verified fixture has source-linked route and balance reconciliation; fees are accounted for once; missing attribution remains unknown. Publish the achieved sample counts and exclusions, even when below target.

### BT-04: Implement exact historical replay

**Owner:** replay / protocol engineering. **Depends on:** BT-01 and BT-03.

Qualify historical prestate reconstruction and the chain-aware runtime. Reuse supported pool mathematics. Separate math-level verification from complete transaction reproduction.

**Acceptance:** each eligible fixture reproduces exact raw financial effects; unexplained differences fail. Gas/state differences are explained, not hidden behind permissive tolerances. Offline replay makes no provider calls. Witness completeness and historical upgrade compatibility are demonstrated.

### BT-05: Add cost and capital accounting

**Owner:** quantitative/accounting engineering. **Depends on:** BT-03; integrate against BT-04.

Implement distinct observed and modeled cost components, capital-flow adjustments, virtual reservations, fee reserves, valuation provenance, and immutable scenario results.

**Acceptance:** unit tests catch omitted L1 costs, double-counted pool fees, loan principal treated as profit, capital deposits treated as profit, and reused inventory. Unknown inputs block an all-in positive-profit label.

### BT-06: Run blind discovery and execution scenarios

**Owner:** strategy / research engineering. **Depends on:** BT-04 and BT-05.

Build the unfiltered supported-universe corpus and advance the strategy through it with only eligible information. Execute the frozen baseline and sensitivity scenarios. Model competing and overlapping opportunities consistently.

**Acceptance:** no future labels or states consumed by decisions; holdout isolation; all configurations and outcomes retained; impossible double fills prevented. Independent probes are not aggregated as portfolio profit without a coherent scenario ledger.

### BT-07: Report, dashboard, and exports

**Owner:** application engineering. **Depends on:** stable contracts from BT-02 and BT-05; results from BT-06.

Add a research view showing datasets, replay validation, experiments, coverage, cost breakdown, and scenario comparisons. Reuse the existing dashboard design and exports.

**Acceptance:** every displayed result links to its source/manifest and scenario. External historical profits never appear in our account's realized-return totals. Missing data is visible. CSV/JSON and dashboard totals reconcile.

### BT-08: Independent verification and research verdict

**Owner:** reviewer / QA, independent of parameter selection. **Depends on:** BT-04 through BT-07.

Reproduce the frozen result from a clean environment, exercise negative cases, review source terms and storage retention, and challenge the best-looking scenario's assumptions.

**Acceptance:** reproducible report with hashes, tests, limitations, and a supported/unsupported/inconclusive verdict. No live activation is part of completion. Negative research findings are accepted deliverables.

**Parallelization:** source/schema work and protocol/runtime investigation can proceed together after scope review. Accounting and dashboard contracts can proceed after the record model stabilizes. Blind evaluation cannot bypass state or accounting gates. The orchestrator owns integration and prevents conflicting schema changes.

## 14. Completion gates

### Gate A: Data is usable

- [ ] Sources, access, and redistribution terms are recorded.
- [ ] Exact assets, protocols, blocks, ordering, and coverage are verified.
- [ ] Raw data, query parameters, and derivation versions are retained and hashed.
- [ ] Missing state, missing fees, and incomplete attribution remain explicit.

### Gate B: Replay is trustworthy within its declared scope

- [ ] Correct pre-transaction state and historical chain environment are demonstrated.
- [ ] Financial effects reconcile exactly for every fixture claimed as verified.
- [ ] Supported quote replay and full transaction replay are separately labeled.
- [ ] Deliberately corrupt or incomplete fixtures do not pass as economic evidence.
- [ ] A frozen run reproduces without network fallback.

### Gate C: Strategy evidence is interpretable

- [ ] Evaluation uses a preselected continuous interval, not just historical winners.
- [ ] Information availability, capital, costs, and inclusion assumptions are explicit.
- [ ] Holdout results and all tested configurations are retained.
- [ ] Overlapping opportunities and counterfactual state changes are handled consistently.
- [ ] Net results, failed attempts, missing coverage, and operating costs are reported.

### Gate D: Research verdict is honest

- [ ] Result is positive, negative, or inconclusive under a named model.
- [ ] Benchmark reproduction is not described as money earned by our bot.
- [ ] Source limitations and sensitivity to favorable assumptions are visible.
- [ ] No source-code change or passing test is treated as proof of live profitability.

A completed implementation can pass its engineering gates while the tested strategy fails its economic hypothesis. Keep software completion and research outcome as separate status fields.

## 15. Blockers and fallback behavior

| Blocker | Required response |
| --- | --- |
| No usable Dune export access | Record the blocker; use an existing authorized export or propose a bounded direct-chain extraction. Do not fabricate data or buy access. |
| Historical traces/state unavailable | Continue source classification where possible, but mark exact transaction replay blocked. Quote approximations cannot replace that gate. |
| Few in-scope executions | Publish the shortfall. Extend the predeclared collection interval or qualify another protocol through review, not silent scope expansion. |
| Unknown fees or owner attribution | Retain partial evidence; do not label all-in net profit verified. |
| Unsupported token or pool behavior | Exclude with reason and count it in coverage; do not approximate silently. |
| Missing counterfactual sequence model | Report independent feasibility probes, not additive portfolio returns. |
| Negative or unstable returns | Preserve the result and stop promotion. A new hypothesis receives a new registered experiment and holdout. |
| Resource limits exceeded | Checkpoint and stop bounded work; publish measured requirements before scaling. |

## 16. Ready-to-use project instruction

> Implement the historical-evidence and backtesting work described in this handoff within `makafeli/arbitrage-research`.
>
> First review the current repository, implementation status, and existing issues. Map BT-01 through BT-08 into the existing backlog without duplicating work or claiming unverified completion. Preserve the Rust-first shared-engine architecture, deterministic offline replay, exact amounts, existing provenance boundaries, and paper-only safety controls.
>
> Start with a bounded Base source-access qualification and candidate dataset. Reconcile real historical executions before using them as benchmark truth. Keep that curated correctness benchmark separate from the continuous, preselected corpus used to evaluate strategy performance. Add exact historical replay, complete cost accounting, and explicit inclusion scenarios before making stronger economic claims.
>
> Preserve all negative, unknown, rejected, and missing-data outcomes. Do not optimize for a positive answer, import third-party profits into our realized ledger, replace missing history with current state, buy services, change deployments, fund wallets, sign, or broadcast transactions. Produce reproducible artifacts and an evidence-based verdict. A negative or inconclusive strategy result is a valid completion of the research task.

## 17. Source and verification ledger

Links were inspected on **21 September 2026** unless explicitly marked otherwise. A link being readable establishes access to that page or file, not independent validation of every claim or downloadable dataset it describes.

### Project references

- **[R1]** [Project README](https://github.com/makafeli/arbitrage-research/blob/main/README.md). Read through the connected GitHub tool. File blob SHA: `16aae990435d038185e6a9be35650a66de812242`.
- **[R2]** [Offline replay README](https://github.com/makafeli/arbitrage-research/blob/main/apps/replay/README.md). Read through the connected GitHub tool. File blob SHA: `7bdbaf0f8dd7e6aac59d7ce8ec50b00cde7772c7`.
- **[R3]** [Architecture](https://github.com/makafeli/arbitrage-research/blob/main/docs/02-ARCHITECTURE.md). Relevant architecture sections read through the connected GitHub tool; the response was truncated. No whole-file or running-system audit is claimed.

### External primary sources

- **[S1]** [USENIX Security 2023: A Large Scale Study of the Ethereum Arbitrage Ecosystem](https://www.usenix.org/conference/usenixsecurity23/presentation/mclaughlin). Conference abstract and publication information verified. Full paper assumptions and results not independently reproduced for this handoff.
- **[S2]** [Goldphish README](https://github.com/ucsb-seclab/goldphish/blob/main/README.md). Opening overview/setup sections inspected through GitHub. File blob SHA: `3ff4f0f69c259d929182ee9c7f33cf2a6c7f9792`. Code not executed.
- **[S3]** [Dune EVM DEX trade schema](https://docs.dune.com/data-catalog/curated/dex-trades/evm/dex-trades). Documentation verified; example query not run.
- **[S4]** [Dune Solana DEX trade schema](https://docs.dune.com/data-catalog/curated/dex-trades/solana/solana-dex-trades). Documentation verified; no Solana dataset extracted.
- **[S5]** [Optimistic-MEV repository](https://github.com/kingbaldwin-iv/Optimistic-MEV) and [transaction classification SQL](https://github.com/kingbaldwin-iv/Optimistic-MEV/blob/main/transaction_classification.sql). Directory listing and SQL inspected through GitHub. SQL blob SHA: `6980e63bc0b5bc5ad896ef22c41336a48331ba5d`. Parquet content not inspected.
- **[S6]** [Tardis downloadable datasets documentation](https://docs.tardis.dev/downloadable-csv-files/overview). Documentation verified; no purchase or data download performed.
- **[S7]** [Geth built-in tracers](https://geth.ethereum.org/docs/developers/evm-tracing/built-in-tracers). Prestate/diff tracer behavior documented. Our Base provider's support is not established by this source.
- **[S8]** [Base network-fee documentation](https://docs.base.org/specifications/transactions/network-fees). Fee components verified at the documentation level; historical per-transaction fees still require extraction and reconciliation.

**Verification status of this deliverable:** source review and implementation planning completed. Data collection, importer implementation, fixture reconciliation, replay execution, performance testing, and profitability verification remain future project work.
