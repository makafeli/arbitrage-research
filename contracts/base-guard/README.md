# base-guard

Research-only Base guard artifact and its offline test harness (ARB-028, issue #42).

- `src/` — the guard contract and the mock Uniswap V3 pool/token used by the harness.
- `test/` — forge tests; every test runs offline (no RPC, no fork URL, no deploy script).
- `test/fixtures/` — committed state fixtures for real-pool replays, if any.
- `lib/forge-std` — pinned submodule (v1.16.2).

Commands (foundry 1.8.3, pinned in CI):

```sh
forge fmt --check
forge build --sizes
forge test -vv
```

Nothing here is deployed. No private key, RPC endpoint or broadcast configuration belongs in this directory.
