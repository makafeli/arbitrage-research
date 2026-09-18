# base-guard

Research-only Base guard artifact and its offline test harness (ARB-028, issue #42).

## What is here

- `src/ArbGuard.sol` — the atomic execution guard: validates a cyclic
  two-or-more-leg route, pulls principal from the plan's spending account via
  an existing allowance, swaps through each allowlisted pool, and reverts the
  entire transaction if any route, callback-authorization or final-balance
  check fails. Also declares the shared `Plan`/`Leg` structs and the minimal
  `IERC20`/`IUniswapV3Pool`/`IUniswapV3SwapCallback` interfaces.
- `src/PlanEncoding.sol` — a pure library that encodes a `Plan` (plus the
  principal, per-leg fee tier, allowances and callback pools the on-chain
  `Plan` does not itself carry) into the same canonical bytes as
  `crates/arb-evm/src/plan.rs::BasePlan::canonical_bytes`, and hashes them
  with `sha256` into a digest.
- `test/mocks/MockERC20.sol`, `test/mocks/MockV3Pool.sol` — minimal ERC-20 and
  Uniswap V3 pool test doubles; never a deployed token or pool.
- `test/ArbGuard.t.sol` — the offline execution harness: proves a valid route
  executes atomically and returns profit, and that each required guard
  (unauthorized callback, unsupported pool, non-cyclic/discontinuous route,
  insufficient output/final balance/principal, past deadline, non-owner
  caller) reverts the whole transaction.
- `test/PlanEncoding.t.sol` — reads `test/fixtures/plan-digest.json` and
  checks `PlanEncoding.digest` against its committed `expected_digest`.
- `test/fixtures/plan-digest.json` — one literal plan with deterministic fake
  addresses, read by both this Solidity test and
  `crates/arb-evm/tests/plan_parity.rs`, proving the Rust and Solidity
  canonical encodings agree byte-for-byte on the same input.
- `lib/forge-std` — pinned submodule (v1.16.2).

Every test runs offline (no RPC, no fork URL, no deploy script, no signing
key). Nothing here is deployed to a public network.

Commands (foundry 1.8.3, pinned in CI):

```sh
forge fmt --check
forge build --sizes
forge test -vv
```

Nothing here is deployed. No private key, RPC endpoint or broadcast configuration belongs in this directory.
