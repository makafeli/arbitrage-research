# Uniswap V3 exact-input math provenance

The Rust implementation in `crates/arb-evm/src/math.rs` adapts the integer
algorithms in the official MIT-licensed Uniswap `v3-sdk` 3.11.0. The selected
source is commit `4e16fe8e56c8c26541545f138c89133794c7ce72`, not a moving branch.

| Upstream file | Git blob SHA-1 |
| --- | --- |
| src/utils/tickMath.ts | 37910bcf15c2fb2f72e260423ff2602bfce0b7b3 |
| src/utils/sqrtPriceMath.ts | 2c9af3755bd410ef9f36bf975fc3a0ab715ca18f |
| src/utils/swapMath.ts | 8189ee1715bacad806645e808619b609ace9a76c |
| LICENSE | b2517ee60b692a6235f500fa8a6651885a803dee |
| package.json | f1ea981e6a59b9b275db9c3764f89e003b1df902 |

Source links: [TickMath](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/src/utils/tickMath.ts),
[SqrtPriceMath](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/src/utils/sqrtPriceMath.ts),
[SwapMath](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/src/utils/swapMath.ts),
[Pool.swap](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/src/entities/pool.ts),
[pinned package manifest](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/package.json),
[pinned license](https://github.com/Uniswap/v3-sdk/blob/4e16fe8e56c8c26541545f138c89133794c7ce72/LICENSE).

Only the MIT SDK math is adapted into this Rust component. No GPL/BUSL Solidity
implementation is copied into the Rust sources. The dependency choice and notices
record provenance; they do not constitute legal review.

## Adaptations and limits

The Rust runtime uses the already pinned `primitive-types = 0.14.0` integers.
There is no JavaScript runtime, new protocol package, signing or broadcast path
in this quote module. U512 intermediates preserve exact multiplication/division;
every narrowing and liquidity change is checked. Tick-to-price retains the
official multiplier constants and upward Q128.128-to-Q64.96 rounding. Price-to-tick
uses bounded integer binary search instead of the SDK logarithm approximation.
The token0 overflow branch deliberately retains the protocol's rounded fallback
formula rather than substituting an apparently equivalent wider fraction.

Tick traversal preserves uninitialized 256-bit word boundaries because each
swap step rounds amounts and fees. The input contains a maximum of 16 contiguous
words and the exact initialized tick states selected by every set bit. The
directional window edge and protocol price limit bound the quote, with a hard
step budget and a zero-progress guard. Partial consumption returns an error;
a successful quote consumes the whole requested amount. Empty initial liquidity,
invalid tick/price relationships, missing/extra ticks, gross liquidity above the
protocol per-tick cap, unknown registry schema, invalid fee/spacing, numeric
overflow and zero rounded output fail closed. An intermediate zero-liquidity
gap can be crossed if the complete required tick data is captured.

The public API takes `&PoolSnapshot` and `&PoolRegistry` separately. Existing
capture snapshot v1 serialization is unchanged. Mathematical completeness is
direction and amount dependent; this does not promote acquisition
`complete_for_quote` or `quote_implementation_qualified` flags.

The exact quote applies the V3 static swap fee to ordinary token0/token1 amounts.
Fee-on-transfer, rebasing and other nonstandard token behavior requires separate
qualification. Output is always `CANDIDATE`, not transaction simulation,
executable liquidity or profit. RPC qualification and block coherence checks
remain external gates. The gross fee returned is already included in the quoted
output and must not be charged twice; protocol fee distribution does not alter
the user's total swap fee.

## Reproducible differential validation

`crates/arb-evm/tests/reference/` contains the independent official SDK oracle
runner, pinned dependency metadata and generated `golden.json`. Its README
records source/artifact integrity and exact regeneration commands. The fixture
inputs are synthetic mathematical states, not observed market opportunities,
RPC evidence or approved production registry entries. The Rust test compares
full input, output, gross pool fee, ending square-root price and initialized
tick crossings against the oracle; primitive fixtures exercise overflow
fallback rounding independently.

Local unit tests additionally cover finite-window exhaustion in both directions,
exact boundary state, signed negative-tick compression, empty words, invalid
liquidity transitions and strict numeric/state validation. Passing these tests
is an implementation check, not an external security audit.

## Upstream MIT license

```text
MIT License

Copyright (c) 2021 Uniswap Labs

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
