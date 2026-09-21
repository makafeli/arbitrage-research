# Fixtures for offline real-pool replays land here (ARB-028 harness increment).

`mainnet/pools.json` holds the real runtime code, immutables, and storage of two
live Base Uniswap v3 WETH/USDC pools, pinned at one finalized block and read by
`test/ArbGuardMainnet.t.sol`; its `provenance` block records exactly which block,
chain id, and RPC host it was fetched from. Re-fetch it with
`scripts/fetch_base_pool_fixtures.py` (see `contracts/base-guard/README.md` for
the exact command and for which pinned test literals must be re-derived after a
re-fetch).
