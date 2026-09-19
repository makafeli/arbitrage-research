# Historical WETH/USDC gap study (research-only)

Owner decision 2026-09-19 (option 1, issue #189): run a historical gap study
against public Base RPC data in parallel with the live OBSERVE run, so pool
choice and size expectations do not have to wait for live data.

## What it answers

How often, how large and how long price gaps between the Uniswap V3
WETH/USDC pools on Base were over a historical window: gaps per hour, size
distribution in bps, and persistence in blocks.

## What it does not answer

- Whether such a gap would have been **won** — competition, mempool
  inclusion and latency are not modelled at all.
- What **gas or slippage** would have cost.
- Any **profit figure**. The reported "net" gap only subtracts the two
  pools' stated swap fees; it is not a P&L estimate.
- It is **not acceptance evidence** for any ARB ticket, and does not
  qualify a provider, a pool, or a trading strategy.

Every `summary.md` this script produces repeats this caveat.

## How it works

`scripts/history_gap_study.py` is stdlib-only Python, in the same defensive
style as `scripts/inspect_pool_candidates.py`:

1. **Pool discovery** — calls the Uniswap V3 factory's `getPool(WETH, USDC,
   fee)` for fee tiers 100, 500, 3000 and 10000 (ppm). Pools that do not
   exist (the factory returns the zero address) are skipped; the pools that
   were found are recorded in `pools.json`.
2. **Swap collection** — fetches `Swap` events for each discovered pool over
   the requested block range via `eth_getLogs`, with:
   - polite pacing (`--min-interval-ms`, default and floor 250ms between
     requests),
   - bounded retries with backoff on transport failures,
   - adaptive block-range halving when the provider rejects a request for
     being too large (range or result-count limits),
   - a resumable per-pool checkpoint file under `--out`, so an interrupted
     run can continue without refetching already-collected ranges.
3. **Price math** — token0 is WETH, token1 is USDC (lower address first, per
   the factory's own convention). USDC per WETH is computed as
   `(sqrtPriceX96 / 2^96)^2 * 10^(18-6)` using `decimal.Decimal` throughout;
   no floating point appears anywhere in the price path.
4. **Gap analysis** — for every block where at least one pool swapped, each
   pool's latest known price is carried forward. For every pair of pools:
   - gross gap (bps) = `|p_a - p_b| / min(p_a, p_b) * 10000`,
   - net gap (bps) = gross gap minus both pools' fee tiers (converted from
     parts-per-million to bps),
   - the study reports, per pair: the average number of blocks per hour
     with a net gap greater than 0, 10 and 25 bps; the p50/p90/max size of
     the positive net gaps (block-weighted); and the persistence (in
     blocks) of each contiguous episode where the net gap stayed positive.

## Running it

```sh
# Dry run (default): prints {"status": "NOT_RUN", ...} and touches no network.
python scripts/history_gap_study.py

# Actually collect ~1 hour of blocks (~1800 blocks at 2s/block) against the
# public RPC:
python scripts/history_gap_study.py --collect --days 1 \
  --from-block <head-1800> --to-block <head> --out history-study/proof-run

# Full 14-day study (long-running; the orchestrator runs this separately):
python scripts/history_gap_study.py --collect --days 14
```

The RPC endpoint comes from the `ARB_STUDY_RPC_URL` environment variable
(default `https://mainnet.base.org`) — **never** `ARB_BASE_RPC_URL` or any
other Railway variable. Output goes under `--out` (default
`history-study/<from>-<to>/`): `pools.json`, `gaps.csv` and `summary.md`.
`history-study/` is gitignored; nothing under it is ever committed.

## Tests

```sh
python scripts/test_history_gap_study.py -v
```

The test suite is fully offline: it exercises the price math against known
`sqrtPriceX96` values, the gap/net-of-fees math, carry-forward across pool
pairs, adaptive range halving and checkpoint resume against fake RPC
objects. It makes no network calls.
