# Initial market identity verification (ARB-003 / #16)

## Scope and trust

This collects evidence within existing #16, not a new feature ticket.
The observer produces diagnostic records, never enabled runtime registries. A
response from a single public RPC is not a consensus proof, verified source
build, amount-specific executable quote, market census or profitability result.
No accounts, keys, purchases, trades or worker activation are involved.

Original #23 configuration and #24 storage acceptance remain dependent on #16.
A successful build or a completed observation process cannot close those issues.

## Primary references checked on 14 September 2026

- Base Uniswap V3 factory and WETH:
  https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments
- Canonical USDC addresses on Base and Solana:
  https://developers.circle.com/stablecoins/usdc-contract-addresses
- Orca's official Whirlpool deployment and verifiable-build references:
  https://github.com/orca-so/whirlpools
- Solana mainnet genesis:
  https://docs.anza.xyz/clusters/available
- Solana finalized account-batch contract:
  https://solana.com/docs/rpc/http/getmultipleaccounts
- Explicit public Base provider selected before sampling:
  https://base.publicnode.com/

The Base provider is PublicNode's published public endpoint. It is not the
previously refusing official endpoint and no alternate-endpoint retry chain is
implemented. Solana uses its official public mainnet endpoint. Any refusal stops
that network immediately. Public endpoints do not confer an SLA or campaign
capacity entitlement.

## What is verified

Base sampling pins every code/state call to one finalized block hash with
`requireCanonical: true`, then rechecks its canonical block number/hash. The
observer looks up two fee tiers (500 and 3000 millionths), checks the reverse
factory link, exact USDC/WETH ordering, decimals, fee, spacing, unlocked price and
active liquidity, and retains observed contract-code hashes and factory owner.
A missing pool, wrong ABI shape, changed canonical block or provider refusal is
explicit. Hashes fingerprint observed bytes; they do not prove source equivalence.
USDC proxy upgrades, freeze/blacklist controls and actual transfer behavior are
not established by decimals and a proxy-bytecode hash.

Solana sampling validates the mainnet genesis, legacy SPL mint layouts, full
653-byte Whirlpool layouts and the upgradeable program link. It discovers the
bounded canonical wSOL/USDC pair, separates static fees from unsupported adaptive
fees, and selects at most two active static-fee candidates for a finalized batch.
That batch rechecks pool identities, vault mint/owner/delegate/frozen state,
program/ProgramData ownership, ELF marker, observed full program-data fingerprint,
upgrade slot/authority, mint/freeze authorities and config identity. The report
keeps discovery and final batch contexts separate, requires a minimum context
slot on the final read, and rejects a regressing finalized response. Selection by active liquidity
is an inspection budget choice, not economic ranking.

Full amount-specific tick coverage, arbitrary token extensions, adaptive-fee
oracle behavior, independently reproduced deployed builds and transaction
simulation remain unqualified. No missing inputs become zero costs or zero
liquidity. Same-venue two-pool candidates are allowed; Sushi, Raydium and other
assets are outside this scope.

## Limits and reproducibility

`python3 scripts/inspect_pool_candidates.py` performs no network I/O.
`--collect --output NEW_DIRECTORY` explicitly samples both fixed endpoints.
It requires a new output directory and never overwrites evidence. Each network
has at most 48 sequential requests, 16 MiB per response, 10 MiB decoded per account, 24 MiB aggregate input,
a ten-second socket timeout and a 240-second admission deadline. The CI job adds
a ten-minute outer deadline. These are bounds, not hard atomic filesystem or
whole-read deadlines. There are no redirects, credentials, retries, transaction
methods or automatic repeating services. HTTP error bodies are not retained.

Exact response bytes and hashes stay in a temporary CI artifact; the small report
omits raw bytes but includes request provenance, contexts, exact decimal amounts
and statuses. A retained hash cannot reconstruct an expired raw artifact. The
artifact's SOURCE_COMMIT identifies the synthetic PR merge checkout. Full
current-head code CI and actual observation results must be inspected separately.
Synthetic decoder/transport tests are not real provider evidence.

## Explicit bounded Solana follow-up

The initial observation hit the earlier 8-MiB response limit on the final
ProgramData batch. Solana permits a 10-MiB account; its base64 representation
requires a larger wire cap. The observer now allows 16 MiB wire/10 MiB decoded
per account, still within a 24-MiB network budget. Source:
https://solana.com/docs/core/accounts

`--network solana-mainnet` isolates the follow-up and never retries the refused
Base endpoint. Only a user-authored push on this scoped feature branch with the
exact commit message `Collect bounded Solana registry evidence` opts into that
one-shot follow-up. Normal synchronization commits run offline checks only.
This is an explicit CI invocation, not an unattended collector or agent runner.

## Acceptance record

Pending actual current-run observation inspection and source/criterion review.
No original issue closure or candidate activation is claimed by this document.
