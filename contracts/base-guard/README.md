# base-guard

Research-only Base guard artifact and its offline test harness (ARB-028, issue #42).

## What is here

- `src/ArbGuard.sol` — the atomic execution guard: validates a cyclic
  two-or-more-leg route, pulls principal from the plan's spending account via
  an existing allowance, swaps through each allowlisted pool, and reverts the
  entire transaction if any route, callback-authorization or final-balance
  check fails. Also declares the shared `Plan`/`Leg` structs and the minimal
  `IERC20`/`IUniswapV3Pool`/`IUniswapV3SwapCallback` interfaces. Its
  `uniswapV3SwapCallback` is one-shot per leg: the pool identity and the
  amount it must pay are captured in storage before the pool is called, and
  cleared before that amount is paid out, so a second callback in the same
  swap is rejected as `UnauthorizedCallback` rather than paid twice. Its
  `RouteExecuted.digest` is a guard-local execution digest for observability
  only (zero fee tiers, no allowances, principal = the first leg's input) —
  it is **not** the off-chain `BasePlan` digest and the two are never
  compared; see `_planDigest`'s doc comment.
- `src/PlanEncoding.sol` — a pure library that encodes a `Plan` (plus the
  principal, per-leg fee tier, allowances and callback pools the on-chain
  `Plan` does not itself carry) into the same canonical bytes as
  `crates/arb-evm/src/plan.rs::BasePlan::canonical_bytes`, and hashes them
  with `sha256` into a digest.
- `test/mocks/MockERC20.sol`, `test/mocks/MockV3Pool.sol`,
  `test/mocks/GreedyV3Pool.sol` — minimal ERC-20 and Uniswap V3 pool test
  doubles; never a deployed token or pool. `GreedyV3Pool` is a malicious pool
  double used only to prove the guard's callback defenses: it can invoke the
  swap callback twice, or report a different input amount than it actually
  received, so the harness can assert the guard rejects both.
- `test/ArbGuard.t.sol` — the offline execution harness: proves a valid route
  executes atomically and returns profit, and that each required guard
  (unauthorized callback including a double callback and a lying pool,
  unsupported pool, non-cyclic/discontinuous/too-short route, an
  exact-input mismatch between legs, insufficient output/final
  balance/principal, a failed residual sweep, past deadline, non-owner
  caller) reverts the whole transaction.
- `test/PlanEncoding.t.sol` — reads `test/fixtures/plan-digest.json` and
  `test/fixtures/plan-digest-irregular.json`, and checks `PlanEncoding.digest`
  against each fixture's committed `expected_digest`.
- `test/fixtures/plan-digest.json`, `test/fixtures/plan-digest-irregular.json`
  — two literal plans with deterministic fake addresses, read by both these
  Solidity tests and `crates/arb-evm/tests/plan_parity.rs`, proving the Rust
  and Solidity canonical encodings agree byte-for-byte on the same input. The
  "irregular" fixture deliberately breaks coincidences the first fixture
  has (principal equal to the first leg's input, one allowance per leg, a
  callback pool set equal to the leg pools in leg order), so a field-order or
  field-substitution bug in either encoder cannot hide behind them.
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
