# Public provider and registry qualification checkpoint

**Status: ARB-002 and ARB-003 remain open.** Documentation and bounded probes are complete for this checkpoint. Neither chain's provider, pool registry or current deployment has been qualified for production observation, quoting or execution. The disabled production configuration is unchanged.

## What was actually attempted

At **2026-09-12 18:39:08 UTC**, the probe sent one public read-only request to each documented mainnet endpoint. Each had a 10-second request timeout, a 2 MiB response limit, no retries, no credentials and no signing or transaction methods. Both calls ended in a transport `URLError` after roughly 10 seconds. No HTTP response, chain ID, genesis hash or chain state was obtained. The available error record does not distinguish a provider failure from this environment's network restrictions; the elapsed times are **not provider latency measurements**.

| Chain | Endpoint from official documentation | Request actually sent | Result | Follow-up requests |
|---|---|---|---|---|
| Base | `https://mainnet.base.org` | `eth_chainId`, ID 0 | Transport error after 10,034 ms | None |
| Solana | `https://api.mainnet.solana.com` | `getGenesisHash`, ID 0 | Transport error after 10,035 ms | None |

The current Solana reference publishes `api.mainnet.solana.com`. This checkpoint did not substitute older aliases or another provider after the failure. The [Base connection reference](https://docs.base.org/get-started/connect-to-base) supplies the Base endpoint and chain ID 8453. The [Solana cluster reference](https://solana.com/docs/references/clusters) supplies the Solana endpoint and explains that its public service is rate-limited and unsuitable as an assumed production service.

The [machine-readable evidence](qualification-fixtures/public-rpc-probe-2026-09-12.json) records UTC timestamps, exact request bodies, outcome classes, bounded settings and the stop reason. The [probe source](qualification-fixtures/probe_public_rpc.py) is reproducible and contains only public endpoint/contract identities. It has no secret lookup, wallet query, funded account, signing key or broadcast capability. Do not classify this evidence as a market capture or a performance experiment.

## Primary-source identities established from documentation

These are documented protocol/token identities. Their presence in this table is **not an independently qualified runtime-code, owner, token-behavior or pool-state record**.

| Network | Identity | Primary source |
|---|---|---|
| Base | Chain ID `8453` | [Base network reference](https://docs.base.org/get-started/connect-to-base) |
| Base | Uniswap V3 factory `0x33128a8fC17869897dcE68Ed026d694621f6FDfD` | [Uniswap Base deployments](https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments) |
| Base | WETH `0x4200000000000000000000000000000000000006` | [Uniswap wrapped-token deployment table](https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments) |
| Base | Circle-native USDC `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | [Circle mainnet USDC registry](https://developers.circle.com/stablecoins/usdc-contract-addresses) |
| Solana | Whirlpool program `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc` | [Official Orca repository](https://github.com/orca-so/whirlpools/tree/408c945fef4c49ab70def4303377cfaf8f0f3c99) |
| Solana | Legacy wrapped SOL mint `So11111111111111111111111111111111111111112` | [Official SPL Token native-mint definition](https://github.com/solana-program/token/blob/main/interface/src/native_mint.rs) |
| Solana | Circle-native USDC mint `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | [Circle mainnet USDC registry](https://developers.circle.com/stablecoins/usdc-contract-addresses) |

The primary SPL native-mint file checked at this checkpoint had Git blob SHA `376520229c4446a51c0442b7b8383fd548683db1`. The decoded legacy token program is `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`; its exact owner/layout behavior still needs runtime capture evidence for every selected mint/vault. The Solana mainnet genesis value remains unverified by this probe and must not be invented from a truncated CAIP reference.

## Pool discovery prepared; zero eligible pools established

The objective is at least **two distinct qualified pools per selected pair** so that a two-leg cycle can be evaluated. This checkpoint established **zero qualified pool addresses and zero eligible pools**. Identity probes failed before the dependent discovery requests were sent.

For Base, the prepared discovery targets are two separate factory queries for the documented WETH/USDC pair at fee values **500 and 3000**. Those are search inputs, not assertions that both pools exist or are suitable. The [official Uniswap deployment reference](https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments) describes discovery through `getPool`. A later successful probe should select a finalized block and pin all factory/pool queries using [EIP-1898 `blockHash` and `requireCanonical`](https://eips.ethereum.org/EIPS/eip-1898). It must reject a zero address, verify factory membership and code hashes, confirm the exact token order/fee/tick spacing and retrieve the full quote-relevant tick state before an amount-specific eligibility decision.

For Solana, the prepared discovery request is a bounded `getProgramAccounts` query against Whirlpool, with a 653-byte pool-layout filter, the wrapped-SOL mint at offset 101, Circle USDC at offset 181, a 245-byte data slice and a returned context. These offsets come from the [pinned official Whirlpool layout](https://github.com/orca-so/whirlpools/blob/408c945fef4c49ab70def4303377cfaf8f0f3c99/programs/whirlpool/src/state/whirlpool.rs). [Solana's RPC reference](https://solana.com/docs/rpc/http/getprogramaccounts) documents owner queries, data filters/slices and context. The result still needs discriminator/owner checks, verified pool/program-data identities, full account reads and a supported static-fee tier. A sliced discovery result is incomplete quote state. At least two distinct returned pools must pass all requirements; seeing two entries alone is insufficient.

The probe script never substitutes the manually constructed Rust test registries for real discovery. No pool address from a blog, search snippet, historic example or unverified aggregator was entered in an enabled registry.

## Capability and evidence matrix

| Required capability | Base checkpoint | Solana checkpoint | Evidence required before acceptance |
|---|---|---|---|
| Correct mainnet identity | Documented ID; RPC response unavailable | Genesis response unavailable | Record chain/genesis response and compare with the independently reviewed network identity |
| Canonical block/account context | Not probed after connection failure | Not probed after connection failure | Base hash-pinned reads and reorg rejection; documented Solana bank/context semantics and account-set coherence |
| Correct deployed factory/program | Documented address only | Documented address only | Runtime-code or program-data hash, owner and deployment/build qualification |
| At least two eligible pools | None verified | None verified | Distinct approved pools, supported assets, complete amount-specific tick coverage and exact math |
| Reconnect/backfill/gap handling | Unmeasured | Unmeasured | Bounded reconnect/backfill test with injected gaps and immutable invalidation reasons |
| Historical/replay availability | Unmeasured | Unmeasured | Capture retention and historical state limits exercised at known block/slot references |
| Full-transaction simulation | Not attempted | Not attempted | A later exact-plan simulation qualification; read-only pings cannot satisfy it |
| Sustained throughput/latency | No measurement | No measurement | Representative multi-day capture workload with loss/gap/queue/resource metrics |
| RPC service budget | Dedicated service and quota unknown | Dedicated service and quota unknown | Operator-selected service, quotas, terms, region, retention and spend ceiling |

## Budget and next acceptance work

This checkpoint incurred **zero paid RPC spend**: it used two public, credential-free requests and purchased no plan or infrastructure. It does not establish the monthly cost of a useful observation campaign. Provider plans, data volume, archive access, Solana account-stream access, deployment region and retention remain unknown. Railway deployment cost is tracked separately from RPC/data-service cost; a small application host is not evidence of an adequately provisioned RPC service.

To advance ARB-002/003, run the bounded identity/discovery checks from the eventual authorized worker environment with its selected RPC service. Record provider aliases, exact configuration/registry digests and the chosen source of mainnet identities. Then capture and independently review factory/program hashes, token behavior, full pool/account state, at least two distinct pools, completeness, reconnect behavior and measured workload limits. Only that reviewed artifact can populate `registry_qualification_digest` and enable a network. Until those results exist, these tickets and all production qualification flags remain open/false.
