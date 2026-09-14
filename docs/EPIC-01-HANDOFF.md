# EPIC-01: original-scope identity and foundation acceptance

## Current evidence and scope

The missing Base access is resolved by the owner-triggered run
[34892533716](https://github.com/makafeli/arbitrage-research/actions/runs/34892533716)
on 14 September 2026, source `bc9b8777b24b6882bfc948be6e07fa3220642b27`.
All 27 read-only requests succeeded. The prior HTTP403 attempts remain historical
failed observations; they are not a current access blocker or proof of inactivity.
The provider URL and credentials were neither retrieved nor committed.

The [initial identity inventory](registries/initial-identities.json) now combines
this actual Base evidence with retained Solana observations. It is an
**identity-only review catalogue**, not a runtime pool-set configuration. The
existing runtime parser must reject it; no bitmap range or tick-array address is
invented to make an incomplete execution configuration load. Same-venue routes
are permitted in the original research scope, but no experiment is enabled by
this inventory or by acceptance of ARB-003.

The five other original foundation tasks were already accepted: #14, #15, #17,
#18 and #19. Do not rebuild them. Native #16 and #2 state plus the linked PR's
actual merge/review/check evidence determine whether formal closure occurred.
The criterion assessment below covers the whole epic, not another partial
provider-connection feature.

## Reviewed universe

| Network | Venue | Pair | Pool address | Fee millionths | Tick spacing |
|---|---|---|---|---:|---:|
| Base | Uniswap V3 | WETH / USDC | `0xd0b53d9277642d899df5c87a3966a349a798f224` | 500 | 10 |
| Base | Uniswap V3 | WETH / USDC | `0x6c561b446416e1a00e8e93e221854d6ea4171372` | 3000 | 60 |
| Solana | Orca Whirlpool, static fee | wSOL / USDC | `Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE` | 400 | 4 |
| Solana | Orca Whirlpool, static fee | wSOL / USDC | `FpCMFDFGYotvufJ7HrFHsWEiiQCGbkLCtwHiDnh7o28Q` | 200 | 2 |

Each asset/pool/vault identity includes its network and exact address or mint.
Observed decimals are WETH 18, Base USDC 6, wSOL 9 and Solana USDC 6. Base
factory-to-pool lookup and reverse factory/token/fee links were validated by the
existing observer. The Base factory owner and runtime fingerprints are retained.
Solana pool/vault ownership, mint identities, legacy token-program layouts,
program/ProgramData linkage, upgrade authority/slot and config fingerprints were
checked in the final account batch. Mint/freeze authorities are retained, not
assumed absent.

All four pools had positive active liquidity at their own observed context. This
is enough to retain them as identity-reviewed candidates, not proof of liquidity
for a proposed trade size. Historical balances are not a live liquidity feed.
Solana's other 29 discoveries remain classified: ten adaptive-fee exclusions,
seven with zero active liquidity and twelve not selected for the final batch.
The two networks were observed at different times and are never presented as a
matched profitability experiment.

## Primary-source cross-check, 14 September 2026

- [Uniswap Base deployment records](https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments)
  identify chain 8453, factory `0x33128a8fc17869897dce68ed026d694621f6fdfd` and
  WETH `0x4200000000000000000000000000000000000006`, matching the observed values.
  The page documents factory `getPool` discovery; pool addresses are supported by
  the actual lookup, not invented from ticker names.
- [Circle's canonical USDC list](https://developers.circle.com/stablecoins/usdc-contract-addresses)
  matches Base `0x833589fcd6edb6e08f4c7c32d4f71b54bda02913` and Solana
  `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`. Bridged lookalikes are not added.
- [Orca's official repository](https://github.com/orca-so/whirlpools) identifies
  program `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`. Its verifiable-build
  documentation is not a claim that we reproduced that deployed build here.
- [Solana's native-mint documentation](https://solana.com/docs/tokens/basics/sync-native)
  identifies legacy wSOL `So11111111111111111111111111111111111111112`. Wrapped
  balances require token-program semantics, not treating native SOL as an ERC20.
- [Anza's mainnet reference](https://docs.anza.xyz/clusters/available) identifies
  the genesis `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`, matching the report.
- [Circle's stablecoin source](https://github.com/circlefin/stablecoin-evm)
  documents issuer controls and upgradeable architecture. USDC mint/freeze/
  blacklist/upgrade authority and unknown future transfer restrictions remain
  operational risks. No no-fee/rebasing/extension assumption is silently granted.

## Evidence provenance and independent checks

**Base:** artifact 10368035396, ZIP SHA-256
`ae0a20af0cd6dadead1277c817814c75cc9cc9c667f2b85e9403edbdc32a3528`;
original redacted report SHA-256
`29a70af5262cfc204f1e89c6110f859ea46afc2888f8a1421d0aef5ac5c2d0f9`.
Observed 20:22:53-20:23:07 UTC. The 24 code/state reads share finalized block
51,312,756, hash
`0x122aa3fb2222e4e392a20e3e663afc2bbc5dd4cf65c061d6bacfeb8d185d58d3`;
the complete final header recheck passed. The wrapper intentionally retained
hashes and validated projections, not raw provider answers. We can verify the
GitHub artifact and internal consistency, not independently decode unavailable
Base response bytes or attest the provider's identity.

**Solana:** run 34839690433, source
`5370a07a23959958966668acc31da2df7b037f75`, artifact 10345572213, ZIP SHA-256
`09c8b955f569a11c3ddd0322cc1a6959f9d9aef28957179271a76e91732c8238`.
Observed 11:44:10-11:44:16 UTC, discovery slot 446968176 and final account slot
446968178. All retained raw response hashes were recomputed. The existing
observer's offline replay reproduces the selected pool identities and authorities.
ProgramData hash `sha256:b5ee20ce8a99d4f111e408b4a6e610accb9637089a568db116de9e4658048fa0`
fingerprints the observed bytes, including metadata; it is not a source-build
attestation. These older observations are not silently relabelled as current.

The committed Base and Solana projections retain source/run/artifact IDs,
original report hashes and exact derived fields. The generator pins their own
file hashes and regenerates the catalogue byte-for-byte. Artifact expiry in
October does not erase this reviewed projection but does end raw reconstruction
when raw bytes are no longer retained. A hash is not a substitute for those bytes.

## Original ARB-003 criteria and result

| Original requirement | Evidence and bounded result |
|---|---|
| Network plus address/mint keys, exact pool asset identities | Four assets and four pools in the deterministic inventory; cross-network, wrong-pair, duplicate and decimal mutation tests. |
| No fixture identifier passes the production registry validator | Rust tests exercise the actual RegistryDocument parser with fixture pool/asset/genesis/config/program/vault/tick-array values on both chains. The inventory itself is also refused as a runtime registry. |
| Seek two eligible pools per chain, record absence and block rather than fabricate | Two actual identity candidates per chain with positive observed liquidity and exact fees. The generator's missing-second-pool test preserves absence and every experiment authorization remains false. Size-specific eligibility is a later gate. |
| Same-venue permitted; Sushi/Raydium and extra behavior unqualified | Both pairs are intentionally same-venue. Explicit unsupported expansion, fee, token-extension and transfer-behavior notes; no additional chain or venue enabled. |

The implementing assistant acts as the user-authorized integrating Technical Lead
under CONTRIBUTING.md's solo-maintainer research convention. This is a named
source/identity review, not a second independent chain engineer, independent
security audit or consensus verification. The final exact-source PR review and
actual CI results are required before native issue closure.

## Whole-EPIC-01 acceptance assessment

1. **Representative data, scope, costs and owners:** actual identity/state samples
   now exist for both original chains. The universe above is deliberately small.
   #14 fixes the research goal; #15 establishes provider limits and a zero-additional-
   purchase boundary; #17 documents host/retention assumptions and accountable roles.
   The owner supplied their existing Base endpoint. No new plan, credit purchase
   or paid entitlement is inferred; production limits and costs require later review.
2. **All required children:** #14/#15/#17/#18/#19 have prior native acceptance.
   #16 is closed only after this final review, successful CI, merge and closeout.
   No child is excluded or converted to not-planned to lower the count.
3. **Claims match evidence:** catalogue identity review is separated from actual
   raw replay, source equivalence, current quote eligibility and deployed operation.
   GOAL.md and capability status are updated to remove the obsolete Base-access
   blocker without changing downstream gates.
4. **Blocking findings:** all scoped review findings must be resolved before
   closure. The independent live/security gates from #18 remain outside this
   foundation release; no research-to-live path is introduced.

## Verify and resume

```sh
python3 scripts/build_identity_inventory.py --check
python3 scripts/test_identity_inventory.py -v
cargo test --locked -p arb-registry --test identity_boundaries
```

These checks are offline. The pinned full Rust/PostgreSQL, HTTP, browser and
container CI still governs integration. Do not report Rust as locally executed
when it was run only in CI. After #16 closure, synchronize its register state,
verify all six children and the epic checklist, then close #2 as completed.

The next epic is the existing platform/control chain (#23, #24, #25 and related
original tasks). Identity acceptance supplies its foundation prerequisite; it
neither accepts those tasks nor deploys workers. Complete ticks, protocol math,
transaction construction/simulation, economic costs, campaign measurement,
production-provider quotas and independent live review remain at their existing
milestone gates. No new tickets are needed for this closeout.
