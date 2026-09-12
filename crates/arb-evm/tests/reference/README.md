# Independent Uniswap reference oracle

This directory supplies synthetic arithmetic fixtures produced by the published
MIT-licensed `@uniswap/v3-sdk@3.11.0`. The corresponding inspected source commit
is [4e16fe8e56c8c26541545f138c89133794c7ce72](https://github.com/Uniswap/v3-sdk/tree/4e16fe8e56c8c26541545f138c89133794c7ce72).
These inputs are manually constructed test scenarios. They are not captured
market data, venue qualification evidence, transaction simulations or profit.

The runner executes the package's actual compiled `Pool.swap`,
`TickListDataProvider`, `TickMath`, `SqrtPriceMath` and `SwapMath`. It does
not implement another version of the swap algorithm. A wrapper observes original
swap-step results to total input, output and fees, returning all values unchanged.
The provider wrapper records traversed words and crossed initialized ticks and
rejects a request outside the fixture's finite capture window. Every full-swap
fixture must consume all of its input and complete within 4,096 steps.

The published private `Pool.swap` method is invoked through its prototype with
explicit arithmetic state. Currency/address convenience APIs are not involved.
The runner uses actual `jsbi@3.2.5`; native BigInt is used only for fixture
construction, bitmap encoding, range validation and observing decimal results.
Pinned `@uniswap/sdk-core@4.2.0` prevents a transitive upgrade of that dependency.
The committed npm lockfile fixes all remaining versions and integrity hashes.

From the repository root, with Node 24:

```sh
npm ci --ignore-scripts --no-audit --no-fund --prefix crates/arb-evm/tests/reference
node crates/arb-evm/tests/reference/generate.cjs --check
```

For an intentional fixture update, run the generator with `--write`, inspect
the resulting `golden.json` diff, and rerun the Rust differential tests. The
reference comparison must not overwrite expected results during the final CI
validation pass. Rust tests read the committed JSON directly, so ordinary Rust
test execution does not require Node or npm network access.

Cases cover both directions, small input and fee rounding, multiple initialized
ticks, empty bitmap words, negative word floor division, zero-liquidity gaps and
large exact integers. Separate primitive vectors cover the token0 uint256 product
overflow fallback, zero input and token1 rounding. The extreme primitive is a
math boundary check and does not claim that its liquidity could sit at one
initialized tick in a realizable pool.

Initial recovery CI uses the preparation workflow to create `package-lock.json`
and `golden.json` before compiling their Rust consumers. It emits both files as
checksummed review artifacts. Those generated files must be committed before the
final `npm ci`, reference `--check`, Rust `--locked`, and container checks
are treated as release evidence.

## Provenance and licenses

- Uniswap v3 SDK 3.11.0: MIT, [source license](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/LICENSE).
- Uniswap SDK Core 4.2.0: MIT, as declared by its pinned npm package.
- JSBI 3.2.5: Apache-2.0, as declared by its pinned npm package.
- Other test-only transitive packages retain their own packaged license notices.
  No upstream source is copied into the application runtime by this runner.

The inspected source SHA-256 values for the arithmetic path are recorded in
`source-provenance.json`. npm package integrity is independently recorded in
the generated, reviewed lockfile. A source commit reference and a package
integrity hash serve different purposes; neither is a market-data provenance
claim.
