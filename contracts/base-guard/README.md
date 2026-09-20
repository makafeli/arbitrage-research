# base-guard

Research-only Base guard artifact and its offline test harness (ARB-028, issue #42).

## What is here

- `src/ArbGuard.sol` — the atomic execution guard: validates a cyclic
  two-or-more-leg route, pulls `legs[0].exactIn` from the plan's
  `spendingAccount` via a single guard-level allowance (`principal` is a
  separately declared, guard-checked balance — see below, not the amount
  pulled), swaps through each allowlisted pool, and reverts the entire
  transaction if any route,
  fee-tier, allowance, callback-authorization or final-balance check fails.
  Also declares the shared `Plan`/`Leg`/`Allowance` structs and the minimal
  `IERC20`/`IUniswapV3Pool`/`IUniswapV3SwapCallback` interfaces. On-chain
  `Plan` is now a full field-for-field mirror of Rust's `BasePlan`
  (`chainId`, `executor`, `spendingAccount`, `principal`, `startingAsset`,
  `legs` — each carrying its own `feeTier` — `allowances`, `deadline`,
  `minFinalBalance`, `callbackPools`), so nothing the guard checks is
  guard-local or a stand-in for the off-chain plan. Its
  `uniswapV3SwapCallback` is one-shot per leg: the pool identity and the
  amount it must pay are captured in storage before the pool is called, and
  cleared before that amount is paid out, so a second callback in the same
  swap is rejected as `UnauthorizedCallback` rather than paid twice. Its
  `RouteExecuted.digest` is the real `BasePlan` digest —
  `sha256(BasePlan::canonical_bytes())` for the same plan — proven equal by
  `test_fixturePlanExecutesWithBasePlanDigest`, which executes the committed
  fixture plan through mock pools and checks the emitted digest against the
  fixture's own `expected_digest`.
- **Allowance model**: exactly one allowance entry is expected, from
  `spendingAccount` to `executor`, covering at least `legs[0].exactIn`. The
  guard pulls `legs[0].exactIn` once via `transferFrom` and pays every pool
  from its own balance through the swap callback; pools never pull from the
  spending account directly. A missing or insufficient allowance for that
  pair is
  `MissingAllowance`; any other entry (wrong token, wrong spender, or a
  second entry even if itself valid) is `UnexpectedAllowance`. This replaces
  an earlier "one allowance per leg pool" model, which did not match how the
  guard actually moves funds.
- **Principal pre-check**: `principal` is a declared, guard-checked value,
  not merely `legs[0].exactIn`. `FeeAccountOnly` rejects `principal == 0` (a
  plan that pays no route, only observability), and `InsufficientPrincipal`
  rejects a declared `principal` the spending account cannot actually back —
  checked before anything is pulled — as well as a `legs[0].exactIn` that
  exceeds the declared `principal`.
- `src/PlanEncoding.sol` — a pure library that encodes a `Plan` into the
  same canonical bytes as
  `crates/arb-evm/src/plan.rs::BasePlan::canonical_bytes`, and hashes them
  with `sha256` into a digest. Because `Plan` now mirrors `BasePlan` field
  for field, `canonicalBytes`/`digest` take only the plan itself — no more
  guard-supplied substitutes for principal, fee tiers, allowances or
  callback pools.
- `test/mocks/MockERC20.sol`, `test/mocks/MockV3Pool.sol`,
  `test/mocks/GreedyV3Pool.sol` — minimal ERC-20 and Uniswap V3 pool test
  doubles; never a deployed token or pool. `GreedyV3Pool` is a malicious pool
  double used only to prove the guard's callback defenses: it can invoke the
  swap callback twice, or report a different input amount than it actually
  received, so the harness can assert the guard rejects both.
- `test/ArbGuard.t.sol` — the offline execution harness: proves a valid route
  executes atomically and returns profit, that the emitted `RouteExecuted`
  digest for the committed fixture plan equals the fixture's own Rust-derived
  `expected_digest` (`test_fixturePlanExecutesWithBasePlanDigest`), and that
  each required guard (unauthorized callback including a double callback and
  a lying pool, unsupported pool, wrong chain/executor, a fee-tier mismatch,
  non-cyclic/discontinuous/too-short route, an exact-input mismatch between
  legs, a missing or unexpected allowance, insufficient output/final
  balance/principal, a failed residual sweep, past deadline, non-owner
  caller) reverts the whole transaction.
- `test/PlanEncoding.t.sol` — reads `test/fixtures/plan-digest.json` and
  `test/fixtures/plan-digest-irregular.json`, and checks `PlanEncoding.digest`
  against each fixture's committed `expected_digest`.
- `test/fixtures/plan-digest.json`, `test/fixtures/plan-digest-irregular.json`
  — two literal plans with deterministic fake addresses, read by
  `crates/arb-evm/tests/plan_parity.rs`, `test/PlanEncoding.t.sol` and (the
  "regular" fixture only) `test/ArbGuard.t.sol`'s execution proof, proving
  the Rust and Solidity canonical encodings agree byte-for-byte on the same
  input and that the digest they agree on is also what a real
  `ArbGuard.execute` call emits. The "regular" fixture's `allowances` is the
  single guard-allowance entry `execute` requires, so it also passes
  `BasePlan::validate()` and can be executed on-chain; the "irregular"
  fixture deliberately breaks coincidences the first one has (principal
  equal to the first leg's input, a callback pool set equal to the leg pools
  in leg order) and stays encoding-only, so a field-order or
  field-substitution bug in either encoder cannot hide behind them.
- `lib/forge-std` — pinned submodule (v1.16.2).

Every test runs offline (no RPC, no fork URL, no deploy script, no signing
key). Nothing here is deployed to a public network, and nothing in this
directory performs or authorizes a live trade.

Commands (foundry 1.8.3, pinned in CI):

```sh
forge fmt --check
forge build --sizes
forge test -vv
```

Nothing here is deployed. No private key, RPC endpoint or broadcast configuration belongs in this directory.
