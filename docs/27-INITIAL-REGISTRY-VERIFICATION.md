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
`requireCanonical: true`, then rechecks its canonical block hash, number, parent
hash and timestamp. Both headers require full 32-byte hashes and canonical
unsigned 64-bit RPC quantities. A matching hash with contradictory or missing
retained context is rejected; unrelated provider response fields do not change
the comparison. The
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

## Observed result and actual remaining acceptance

On 14 September 2026 the first diagnostic (run 34838825128, head `a83ccfbe`)
made one Base request and four Solana requests. Base returned HTTP 403 before
any state/pool lookup. Solana discovered 31 canonical-pair pools but its final
program-data response exceeded the old 8-MiB wire cap. Neither initial result
is reported as qualified. No Base retry was issued.

The explicitly authorized Solana-only follow-up (run 34839690433, exact head
`5370a07a23959958966668acc31da2df7b037f75`) made four successful requests from
11:44:10 to 11:44:16 UTC. Its ProgramData account was exactly 10,485,760 decoded
bytes; the final JSON response was 13,985,955 bytes. This explains the original
size refusal without weakening the new 16-MiB wire and 10-MiB account bounds.

Discovery slot was 446968176 and the final account-batch slot was 446968178.
Of 31 discovered pools, ten have unsupported adaptive fees and seven had zero
active liquidity. Two of the remaining 14 static-fee candidates were selected
for the final identity/vault/config checks; the other 12 were not fully checked.

| Final-batch candidate | Fee, millionths | Tick spacing | Status |
|---|---:|---:|---|
| `Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE` | 400 | 4 | Identity, active liquidity and vault ownership/state observed |
| `FpCMFDFGYotvufJ7HrFHsWEiiQCGbkLCtwHiDnh7o28Q` | 200 | 2 | Identity, active liquidity and vault ownership/state observed |

Both contain the exact canonical wSOL and USDC mints, use the legacy token
program and have initialized, non-frozen vaults with no delegate or close
authority. They share observed config `2LecshUwdy9xi7meFgHtFJQNSKk4KdTrcpvaB56dP2NQ`.
The observed Whirlpool ProgramData hash is
`sha256:b5ee20ce8a99d4f111e408b4a6e610accb9637089a568db116de9e4658048fa0`.
Its upgrade slot/authority and code fingerprints are retained in the summary.
These fingerprints are not proof of equivalence to a reproduced source build.

The committed `qualification-fixtures/registry-observation-2026-09-14.json`
retains the exact two candidates, point-in-time balances, contexts, source/run
and artifact hashes. Raw transcript bytes remain in the CI artifacts, not in
an enabled registry. Both ZIP hashes, all retained file checksums and every
retained response hash were recomputed locally. An offline replay of the four
actual Solana responses reproduces the complete derived result. Mutated context
and vault-owner checks are rejected. The first ad-hoc negative check selected
an uninspected vault and failed in its harness; correcting that selection to
an actually inspected vault produced the expected owner refusal. No additional
network access was used for these checks.

Twenty committed synthetic regressions cover CLI refusal, exact decoding,
identity/owner errors, contexts, byte budgets and HTTP behavior. The redirect
regression uses the real urllib redirect/error-handler pipeline with a local
stub transport, proving one request and HTTP_REFUSED for a disallowed 302
Location. These tests are not independent mainnet observations.

**#16 remains open:** Base still needs permitted, functioning read-only access
and verified chain/contract/token/pool state. Full amount-specific tick data and
deployed-source equivalence are not established by the Solana identity batch.
No candidate is promoted to production or enabled for an experiment. #23 and
#24 retain their original prerequisite gates; neither closes through this PR.
The pre-existing #15/#22 acceptance remains valid; old issue notes referring
to those as unaccepted are superseded by their actual completed native states.

Source review and fresh exact-head CI must be checked on PR #112 before
integration. A merged evidence collector is not acceptance of these three
original tickets. This record is a concrete partial result with a read-only
access blocker, not an automatic service or a promise of background work.
