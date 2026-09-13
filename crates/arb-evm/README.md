# Base Uniswap V3 acquisition

`capture_pool` reads a configured pool at one finalized Base block, verifies chain ID 8453, pinned factory address, caller-reviewed pool/factory runtime hashes, the pool's factory/token/fee/tick-spacing identities and the factory's `getPool` result. All contract reads use EIP-1898 `blockHash` with `requireCanonical: true`; an unsupported provider fails instead of falling back to `latest`. A final canonical block lookup rejects a block changed during acquisition.

`capture_pools(rpc, registries, observed_at_ms)` captures **1–8 distinct pools** against one shared finalized anchor. It validates every registry and duplicate identity before I/O, checks the chain once, performs each pool's same hash-pinned identity/state reads, and checks canonicality once after the entire batch. Any provider, pool or final canonicality failure rejects the whole batch; successful output retains input order. Moving the finalized tip during collection does not select a new anchor. `capture_pool` retains its legacy RPC sequence so previous single-pool transcripts can still replay. This bounded polling API does not establish continuous chain-lag or reorg-gap tracking.

The reader decodes exact Q64.96 price bytes, signed ticks, active liquidity, a configured bounded bitmap window and every initialized tick within that window. ABI lengths, integer/sign bounds, initialization and liquidity consistency are checked. The operator supplies the actual independently verified pool identities and SHA-256 hashes. No example pool here has been verified on Base. Runtime hashes are SHA-256 over decoded `eth_getCode` bytes, not Ethereum's Keccak code hash.

Acquisition quote completeness and quote implementation qualification remain **false**. The `math` module can now calculate a direction- and amount-specific exact-input estimate against the captured window, using checked integers and the pinned MIT Uniswap SDK reference. It takes the unchanged v1 `PoolSnapshot` plus its explicit `PoolRegistry`. It preserves tick-word boundary rounding, includes the pool fee, checks every liquidity transition, and rejects partial input, missing tick data and exhausted coverage. Successful output is always **CANDIDATE**; the quoted fee is already reflected in `amount_out`.

Streaming logs, reconnect/backfill, continuous rollback invalidation, token behavior qualification, real captured fixture comparison and deployed protocol qualification remain unfinished ARB-016/018/019 work. The math implementation does not promote capture readiness or provide transaction simulation. Provenance, license and limitations are recorded in [the Uniswap SDK notice](../../third-party/UNISWAP-V3-SDK-3.11.0.md). Synthetic independent oracle vectors and regeneration instructions are in `tests/reference`; these are not market captures.

Protocol facts were checked against these primary sources on 2026-09-12:

- [Uniswap Base deployment registry](https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments) lists factory `0x33128a8fC17869897dcE68Ed026d694621f6FDfD` and Base chain ID.
- [Pinned pool state interface](https://github.com/Uniswap/v3-core/blob/d0831dc6b8a318df3872b6d68f6de135c9f3ec29/contracts/interfaces/pool/IUniswapV3PoolState.sol) defines `slot0`, liquidity, tick and bitmap units and types. The implementation uses interface facts and does not vendor protocol source.
- [EIP-1898](https://eips.ethereum.org/EIPS/eip-1898) defines hash-pinned canonical state reads.

`tests/fixtures` contains explicitly **manually constructed**, non-market data. Run the worker's `fixture` command to produce an integrity-checked demonstration bundle.
