# Go/no-go note: the Base 2-pool USDC/WETH cyclic strategy

Date: 2026-09-21. Author: Claude (orchestrator session), at the owner's request. Status: **evidence note, decision pending with the owner.** Nothing in this note changes scope, tickets, deployments or the OBSERVE run.

## 1. The question

The only strategy implemented and run so far is `cyclic-exact-in-2leg-v1`: quote USDC → WETH → USDC through two Uniswap V3 pools on Base (`0xd0b53d9277642d899df5c87a3966a349a798f224`, `0x6c561b446416e1a00e8e93e221854d6ea4171372`) with `amount_in` 1 USDC, once per collection (≈ every 50 s), in OBSERVE mode. Should the project keep building towards PAPER and LIVE on this strategy?

## 2. Evidence

### 2.1 Our own run (production database, read-only)

- 21.5 h of OBSERVE on 2026-09-20 (session `16f9c9f6`, deployment `0f7181a0`, commit `86cd5e1`): 470 admitted batches, **968 decision traces, all `QUOTED`, 0 positive**.
- `gross_delta_minor` per 1 USDC probe: min −6185, max −806, mean **−3498** (≈ −0.35 %). The two pool fees add up to 0.35 %, so on almost every look the two pools were priced level and the quote lost exactly the fees.
- `paper_runs` 0, `cost_assessments` 0. Diagnostics on every trace: `EXTERNAL_COSTS_UNAVAILABLE`, `FULL_TRANSACTION_SIMULATION_NOT_RUN`, `SNAPSHOT_NOT_ATOMIC`.

### 2.2 What other searchers did on the same pools (Dune, 28 days, 2026-08-24 … 2026-09-20)

Source and method: `dex.trades` + `base.transactions` on Dune, ad-hoc `dune query run-sql` (30.2 of 2,500 credits in total, including the §2.4 check, no saved queries, no purchases). A "cycle" is a transaction whose first sold token equals its last bought token. Net = gross delta of the swap legs − (`gas_used × gas_price + l1_fee`) at the day's median USDC/WETH price from pool `0xd0b5…f224`. Full query text and per-day/per-route results are on issue #189 (comments of 2026-09-21); the appendix repeats the tables that matter.

| 28 days on Base | value |
|---|---|
| transactions with at least one swap | 39.4 M |
| closed cycles | 2,616,005 |
| closed cycles touching at least one of our two pools | 58,409 |
| transactions on **exactly our route** (2 legs, our two pools) | **131** (≈ 4.7 / day, 0 on 8 days, 61 on 2026-09-15) |
| of those, clean 2-leg cycles (first/last amounts within 5 %) | 61 (70 are branching flows / flash-loan shaped, unverified) |
| clean net-positive | **46** (≈ 1.6 / day, 0 on 18 of 28 days) |
| total net of all 46 winners | **USD 1,231** (≈ USD 44 / day for every bot combined) |
| median / max net per winner | USD 2.22 / USD 198 |
| median trade size of winners | **USD 9,426**; no winner below USD 1,000 |
| median return per winner | 0.023 % |
| clean net-negative | 15, median −USD 0.03 |

On 2026-09-20 alone (our OBSERVE day): 1,387 cycles touched our pools, 689 were net-positive, **0 used our exact route**. Winners were 3-leg and longer routes across Uniswap v3/v4, Aerodrome Slipstream and PancakeSwap v3.

### 2.3 The wider Base cycle market

Top 40 routes ranked by net-positive transactions: 35 have a **median net of USD 0.001–0.02 per transaction** at median sizes of USD 4–450, shared by 25–137 bots each (dust-level backrunning). 5 routes (`metric` / `tessera_v` pairs) show medians of USD 180–3,906 per trade; §2.4 checks them and finds no profit. Neither of our pools appears in any top-40 route.

### 2.4 Outlier check: the "rich" routes are settlement flows, not arbitrage

Method (Dune, ≈ 5 credits): took the 25 most extreme `metric`/`tessera_v`/`pancakeswap → tessera_v` transactions of 2026-09-14 … 09-20 (query D, per-leg amounts), then for 8 of them summed every ERC-20 transfer (`erc20_base.evt_Transfer`, query E) and every native ETH value transfer (`base.traces`, query F) per address, and for 3 of them listed every address with a non-zero balance change (query G).

Finding: the two "legs" never chain. Example `0xcd8721f2…` (2026-09-17): leg 1 sells 0.0019 WETH for 4.56 USDC on a PancakeSwap pool; leg 2 sells 2.863 WETH for 7,003.63 USDC on the `tessera_v` contract `0x5555…9e3e`. The closed-cycle heuristic reads that as "0.0019 WETH in, 2.861 WETH out" and reports thousands of dollars of profit. The real balance changes: counterparty `0x3dbe…` +7,003.63 USDC / −2.863 WETH, counterparty `0x69a9…` −6,999.78 USDC / +2.861 WETH, fee receiver `0x7c97…` +0.70 USDC (0.01 % of the order), **the searcher contract `0xa654…` and its EOA: 0 in every token, 0 native ETH**. The other checked transactions look the same (searcher net 0 to 0.0002 WETH ≈ USD 0–0.50). `tessera_v` is an order-settlement venue; the "bots" are fillers matching large orders with a tiny public-pool hedge, earning nothing on-chain beyond a sub-dollar fee. **Conclusion: the outliers are artifacts of the first/last-amount heuristic; there is no hidden large-margin route in the top 40.**

## 3. What the evidence means

1. **The route, not the timing, is the limit.** The gap on our pair opens a few times a day on volatile days, lasts about one block (2 s) and is taken by searchers trading USD 1k–85k. A 50 s sampler with a 1 USDC probe cannot observe it; more OBSERVE hours will not change the result.
2. **Even a perfect executor on this pair competes for ≈ USD 44 / day in total**, needs ≈ USD 10k of working capital per trade, block-level monitoring and same-block execution, and shares the pie with at least four established bots. Upper bound for a new entrant: a few hundred USD per month before infrastructure cost.
3. **The rest of Base is not better for a newcomer.** Realistic routes pay cents; the routes that looked rich are settlement flows with zero searcher profit (§2.4).
4. **Flash loans do not change this.** A flash loan is borrowed money for one block: it removes the need to hold USD 10k, it does not make the gap larger. The 46 winners already sized their trades to the pool depth (USD 1k–85k), so the pie stays ≈ USD 44 / day. A lender fee (Aave v3: 0.05 %) is larger than the median winner's return (0.023 %) and would turn most winners into losers; a zero-fee lender only keeps the same cents. The repo excludes flash loans until a lender is verified (#81, M7).

## 4. Options

**Option A — No-go for the 2-pool strategy as a production target (recommended).**
Keep the platform (worker, ingestion, dashboard, evidence chain) as research infrastructure. Stop M5/M6 build-out for this strategy. Record the decision on #189 and the M4/M5 epics. Next research step, if any: BT-04 reconciliation of the 70 unbalanced exact-route transactions (the outlier routes are already settled, §2.4) before choosing a new direction.

**Option B — Go, but as a different product.**
Change the strategy to what the winners do: multi-DEX 3-leg routes, block-level (2 s) state tracking, same-block execution, USD 10k+ capital. That is M3–M6 work over months, competing for cents against bots that already own the fast lane. Only defensible if the owner accepts that ceiling in writing first.

Recommendation: **Option A.** The measured pie is too small and too contested to justify PAPER/LIVE on this route.

## 5. Boundaries and limits of this note

- No purchases, deployment changes, wallet funding, signing or live transactions were made or are authorized by this note.
- Dune numbers are candidate-discovery evidence (handoff §3.2), not a net-profit ledger: flash-loan fees, token transfer fees, bribes and external transfers are not reconciled. The clean-cycle filter (first/last amounts within 5 %) removes the obvious artifacts but is a heuristic.
- The ETH price used for gas and WETH-denominated deltas is a daily median, not per block.
- The OBSERVE evidence covers one day; the Dune window covers 28 days. Both point the same way.
- The scope question (Base-only vs Base+Solana) is separate and remains with the owner.

## Appendix

### A. Base per day (Dune query A)

| day | transactions with swaps | closed cycles | cycles via our pools | exactly our route |
|---|---:|---:|---:|---:|
| 2026-08-24 | 2,219,954 | 175,083 | 2,891 | 0 |
| 2026-08-25 | 1,711,569 | 115,702 | 1,928 | 0 |
| 2026-08-26 | 1,310,275 | 86,831 | 1,504 | 2 |
| 2026-08-27 | 1,371,206 | 113,755 | 2,744 | 2 |
| 2026-08-28 | 1,461,982 | 95,124 | 2,266 | 11 |
| 2026-08-29 | 838,450 | 50,561 | 1,063 | 1 |
| 2026-08-30 | 1,068,483 | 73,141 | 1,705 | 1 |
| 2026-08-31 | 1,254,171 | 79,998 | 1,565 | 1 |
| 2026-09-01 | 1,243,229 | 70,967 | 1,759 | 1 |
| 2026-09-02 | 1,289,990 | 75,598 | 1,872 | 0 |
| 2026-09-03 | 1,497,231 | 95,383 | 2,714 | 1 |
| 2026-09-04 | 1,702,436 | 100,067 | 4,498 | 20 |
| 2026-09-05 | 1,337,944 | 65,471 | 1,645 | 2 |
| 2026-09-06 | 1,234,024 | 85,374 | 2,927 | 0 |
| 2026-09-07 | 1,285,470 | 88,949 | 1,826 | 0 |
| 2026-09-08 | 1,444,080 | 88,797 | 1,540 | 2 |
| 2026-09-09 | 1,638,290 | 119,951 | 3,002 | 0 |
| 2026-09-10 | 1,657,531 | 140,117 | 2,110 | 4 |
| 2026-09-11 | 1,475,108 | 104,073 | 4,342 | 15 |
| 2026-09-12 | 783,544 | 43,148 | 869 | 1 |
| 2026-09-13 | 923,462 | 53,601 | 1,431 | 0 |
| 2026-09-14 | 1,244,092 | 78,756 | 1,598 | 2 |
| 2026-09-15 | 1,554,564 | 132,145 | 2,328 | 61 |
| 2026-09-16 | 1,399,584 | 103,923 | 1,611 | 1 |
| 2026-09-17 | 1,343,091 | 116,253 | 1,767 | 1 |
| 2026-09-18 | 1,436,384 | 107,595 | 2,237 | 1 |
| 2026-09-19 | 1,163,891 | 76,507 | 1,280 | 1 |
| 2026-09-20 | 1,185,814 | 79,135 | 1,387 | 0 |
| **total** | 39,375,266 | 2,616,005 | 58,409 | 131 |

### B. Top 15 routes on Base by net-positive transactions (Dune query B)

| # | route (projects) | legs | wins / tx | median net USD | median size USD | bots | flag |
|---:|---|---:|---:|---:|---:|---:|---|
| 1 | `uniswap-4>aerodrome-slipstream` | 2 | 62,116 / 65,329 | 0.002 | 22 | 67 |  |
| 2 | `metric-0>tessera_v-1` | 2 | 16,871 / 17,061 | 180.183 | 2,437 | 62 | unverified outlier |
| 3 | `uniswap-4>pancakeswap-3` | 2 | 10,647 / 22,426 | -0.002 | 79 | 130 |  |
| 4 | `tessera_v-1>metric-0` | 2 | 7,989 / 8,835 | 0.065 | 2,501 | 92 |  |
| 5 | `uniswap-4>uniswap-3` | 2 | 7,749 / 7,901 | 0.001 | 4 | 41 |  |
| 6 | `uniswap-4>uniswap-3` | 2 | 7,628 / 8,023 | 0.004 | 39 | 34 |  |
| 7 | `uniswap-4>metric-0` | 2 | 6,799 / 10,573 | 0.004 | 179 | 25 |  |
| 8 | `aerodrome-slipstream>uniswap-3` | 2 | 6,638 / 6,859 | 0.004 | 31 | 32 |  |
| 9 | `uniswap-3>aerodrome-slipstream` | 2 | 6,574 / 6,758 | 0.004 | 31 | 30 |  |
| 10 | `aerodrome-slipstream>aerodrome-slipstream` | 2 | 6,255 / 6,497 | 0.001 | 17 | 42 |  |
| 11 | `aerodrome-slipstream>aerodrome-slipstream` | 2 | 6,152 / 6,413 | 0.001 | 16 | 39 |  |
| 12 | `uniswap-4>uniswap-3` | 2 | 5,958 / 6,386 | 0.011 | 17 | 71 |  |
| 13 | `uniswap-4>metric-0` | 2 | 5,946 / 6,296 | 0.007 | 102 | 24 |  |
| 14 | `uniswap-4>pancakeswap-3` | 2 | 5,429 / 7,198 | 0.023 | 234 | 58 |  |
| 15 | `metric-0>tessera_v-1` | 2 | 5,428 / 5,435 | 3,906.271 | 1,775 | 3 | unverified outlier |

Rows with the same project pair are different pool pairs; full pool addresses are in the #189 export.

### C. Our exact route: the 61 clean transactions (Dune query C)

| day | block | size USD | gross USD | gas USD | net USD | tx |
|---|---:|---:|---:|---:|---:|---|
| 2026-08-27 | 50501342 | 9,458 | 2.40 | 0.003 | 2.40 | `0x322b98c1…` |
| 2026-08-28 | 50572209 | 7,267 | 0.00 | 0.003 | -0.00 | `0xad0fee6f…` |
| 2026-08-28 | 50572482 | 3,725 | 0.36 | 0.126 | 0.23 | `0x3cbdfefd…` |
| 2026-08-28 | 50572482 | 9,393 | 2.28 | 0.035 | 2.25 | `0xc4128c42…` |
| 2026-08-28 | 50572482 | 3,735 | 1.80 | 0.007 | 1.79 | `0xccb75ccd…` |
| 2026-08-28 | 50572483 | 19,935 | 10.27 | 0.317 | 9.95 | `0x6c853e39…` |
| 2026-08-28 | 50572483 | 2,657 | 0.18 | 0.061 | 0.12 | `0xd9f0ff8f…` |
| 2026-08-28 | 50572483 | 6,868 | 0.00 | 1.157 | -1.16 | `0xefd21b1b…` |
| 2026-08-28 | 50572484 | 5,742 | 0.97 | 0.639 | 0.33 | `0x344e9c73…` |
| 2026-08-28 | 50572484 | 343 | 0.00 | 0.010 | -0.01 | `0xea204c54…` |
| 2026-08-31 | 50706270 | 7,072 | 1.08 | 0.014 | 1.06 | `0xf2faea1d…` |
| 2026-09-01 | 50749243 | 67,886 | 136.31 | 0.024 | 136.29 | `0x00705ccd…` |
| 2026-09-04 | 50867834 | 24,606 | 0.00 | 0.119 | -0.12 | `0x461d3e27…` |
| 2026-09-04 | 50867834 | 72,205 | 0.00 | 0.080 | -0.08 | `0x58f63337…` |
| 2026-09-04 | 50867837 | 6,069 | 0.75 | 0.019 | 0.73 | `0xa040debc…` |
| 2026-09-04 | 50867838 | 3,556 | 0.26 | 0.343 | -0.09 | `0x0c094eb9…` |
| 2026-09-04 | 50867838 | 68,663 | 95.50 | 0.491 | 95.01 | `0xc0460c27…` |
| 2026-09-04 | 50867838 | 52,264 | 55.29 | 0.102 | 55.19 | `0x07fbc7cf…` |
| 2026-09-04 | 50867839 | 7,231 | 1.06 | 0.062 | 1.00 | `0xf5196f26…` |
| 2026-09-04 | 50867839 | 80,477 | 131.26 | 0.565 | 130.70 | `0x569747d7…` |
| 2026-09-04 | 50867839 | 40,301 | 32.86 | 0.101 | 32.76 | `0x1266941d…` |
| 2026-09-04 | 50867839 | 40,841 | 33.75 | 0.086 | 33.66 | `0x0f7f59d2…` |
| 2026-09-04 | 50867850 | 3,240 | 0.21 | 0.007 | 0.21 | `0x40bf7243…` |
| 2026-09-04 | 50867850 | 13,429 | 3.65 | 0.426 | 3.22 | `0x2d8dbc24…` |
| 2026-09-04 | 50867882 | 29,037 | 17.05 | 0.457 | 16.60 | `0x4745e6dc…` |
| 2026-09-04 | 50867897 | 84,658 | 145.38 | 0.127 | 145.26 | `0x3135c155…` |
| 2026-09-04 | 50867898 | 77,645 | 122.07 | 0.129 | 121.94 | `0x47ea812c…` |
| 2026-09-04 | 50868100 | 9,351 | 1.93 | 0.090 | 1.84 | `0xc2028718…` |
| 2026-09-04 | 50868100 | 6,492 | 0.93 | 0.096 | 0.84 | `0x3fb3acd6…` |
| 2026-09-04 | 50868100 | 6,688 | 0.99 | 0.093 | 0.90 | `0x08657f54…` |
| 2026-09-04 | 50868100 | 8,191 | 0.00 | 0.068 | -0.07 | `0x9f57e588…` |
| 2026-09-08 | 51051993 | 0 | -0.00 | 0.026 | -0.03 | `0x3ffc9399…` |
| 2026-09-08 | 51056867 | 0 | -0.00 | 0.026 | -0.03 | `0x54fec32f…` |
| 2026-09-10 | 51127541 | 2,509 | 0.14 | 0.178 | -0.04 | `0xb9b885bb…` |
| 2026-09-10 | 51127541 | 4,027 | 0.35 | 0.070 | 0.28 | `0x48b156d7…` |
| 2026-09-10 | 51127541 | 7,181 | 1.13 | 0.121 | 1.01 | `0x740202a9…` |
| 2026-09-10 | 51127544 | 18,766 | 0.00 | 0.003 | -0.00 | `0x071877c6…` |
| 2026-09-11 | 51170439 | 12,443 | 1.65 | 0.521 | 1.13 | `0xd6055114…` |
| 2026-09-11 | 51172623 | 6,222 | 0.98 | 0.023 | 0.96 | `0x59fde19e…` |
| 2026-09-11 | 51172627 | 60,651 | 109.34 | 44.535 | 64.80 | `0x35da58d2…` |
| 2026-09-11 | 51172629 | 14,459 | 6.20 | 0.319 | 5.88 | `0x754c2e19…` |
| 2026-09-11 | 51172631 | 3,407 | 0.34 | 0.074 | 0.27 | `0xc65122b6…` |
| 2026-09-11 | 51172695 | 7,670 | 1.82 | 0.949 | 0.87 | `0x397dd3f5…` |
| 2026-09-11 | 51172882 | 2,912 | 0.43 | 0.310 | 0.12 | `0xab8e9f55…` |
| 2026-09-11 | 51172882 | 970 | 0.00 | 0.020 | -0.02 | `0x23a4bbe8…` |
| 2026-09-11 | 51172906 | 5,370 | 0.97 | 0.135 | 0.84 | `0xa43cb946…` |
| 2026-09-11 | 51172969 | 2,801 | 0.00 | 0.013 | -0.01 | `0xf63152fc…` |
| 2026-09-11 | 51173069 | 3,408 | 0.40 | 0.196 | 0.21 | `0x86549634…` |
| 2026-09-11 | 51173598 | 20,697 | 13.33 | 0.012 | 13.32 | `0x9c35115e…` |
| 2026-09-11 | 51187357 | 11,461 | 0.00 | 0.004 | -0.00 | `0x07d026c9…` |
| 2026-09-14 | 51313701 | 7,724 | 1.17 | 0.072 | 1.10 | `0x73712ca1…` |
| 2026-09-14 | 51315207 | 14,157 | 4.20 | 1.639 | 2.57 | `0xe32286fc…` |
| 2026-09-15 | 51344996 | 32,774 | 14.76 | 0.843 | 13.92 | `0xb80bd4ff…` |
| 2026-09-15 | 51344997 | 41,964 | 24.23 | 1.902 | 22.33 | `0xd8bff362…` |
| 2026-09-15 | 51345046 | 15,457 | 3.46 | 0.005 | 3.46 | `0xa374b6a2…` |
| 2026-09-15 | 51345097 | 40,999 | 29.56 | 0.007 | 29.55 | `0x7d3b7e23…` |
| 2026-09-15 | 51345113 | 9,287 | 2.20 | 0.005 | 2.19 | `0xd0007259…` |
| 2026-09-15 | 51354043 | 85,560 | 201.91 | 4.291 | 197.62 | `0x9d923b25…` |
| 2026-09-15 | 51354644 | 25,163 | 0.00 | 0.038 | -0.04 | `0xc2cddc66…` |
| 2026-09-15 | 51354655 | 53,097 | 73.09 | 0.057 | 73.03 | `0x3c1b6803…` |
| 2026-09-18 | 51475021 | 7,269 | 1.35 | 0.005 | 1.35 | `0x0d0909e7…` |
