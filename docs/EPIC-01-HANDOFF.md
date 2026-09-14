# EPIC-01: completion boundary and operator evidence

## Actual status

Five original foundation tasks have been accepted: ARB-001/#14, ARB-002/#15,
ARB-004/#17, ARB-005/#18 and ARB-006/#19. ARB-003/#16 remains open for Base
identity evidence. The epic itself is not complete. Its source definitions,
acceptance criteria and dependencies are unchanged; already completed tasks must
not be rebuilt to create activity.

Two Solana pool/account identity observations are retained in
[the registry verification record](27-INITIAL-REGISTRY-VERIFICATION.md).
Public Base RPC attempts failed at HTTP403 before state acquisition. A bounded
GET of the documented Blockscout contract-record API also returned HTTP403 on
14 September 2026 at 17:26:03 UTC, run34874706244, artifact10359933155. The artifact
ZIP SHA-256 is `fe4411ec2fdedd96de9a3bfb0ea5d649e200657e9e431f0a4c024b7b25833e2d`.
There was one request and no fallback. This is an access limitation from the
measurement environment, not evidence that Base or its pools are inactive.

## One missing external input

The repository owner must provide an existing, permitted Base read-only HTTPS
endpoint through the repository secret **ARB_BASE_RPC_URL**. No key or account
has been created, purchased or assumed. A secret that points at a still-refusing
service does not solve access. The observer needs `eth_chainId`,
`eth_getBlockByNumber`, `eth_getCode` and `eth_call`, including the existing
finalized hash-pinned reads. It will not silently fall back to `latest` or another
provider when a method fails. Do not paste an endpoint containing credentials
into a ticket, chat, command-line argument or source file.

After the secret is configured, manually run **Operator Base identity evidence**
on `main`. Its collection job is restricted to the repository owner and a manual
main-branch run. Pull requests execute offline tests only and receive no provider
secret. The workflow checks out the selected commit, reuses the reviewed observer,
records the source SHA and uploads only a redacted summary plus checksums.
The report remains blocked on any access/identity failure. No automatic retry,
issue closure, registry activation, deployment or trade is performed.

A local operator can use the same collector after setting ARB_BASE_RPC_URL through
their existing secret mechanism:

```sh
# No provider access and no credential read.
python3 scripts/collect_operator_base_evidence.py

# Explicit read-only collection. The output file must not already exist.
python3 scripts/collect_operator_base_evidence.py --collect --output base-identity.json
```

The file is reserved with mode0600 before provider access. Missing/malformed
credentials, existing output files and unsupported methods fail without a request.
Only HTTPS port443 is accepted; local literals, userinfo and fragments are refused.
This is trusted operator configuration, not a public URL-fetch service. DNS and
network egress authorization remain the operator/environment's responsibility.
It is not a complete defense against DNS rebinding or every network proxy policy.

## Evidence and credential boundaries

The endpoint is read only from the named environment variable. The summary names
the secret reference, not its URL. Raw provider payloads are removed from the
transcript in memory before export; they may contain echoed credentials. Report
fields and request records are allowlisted. The artifact contains validated
identity/state projections and original response hashes, **not raw responses**.
Consequently it cannot independently reproduce byte decoding or attest which
provider supplied the data. A hash alone cannot reconstruct unavailable bytes.

A successful collection says that the existing observer accepted those responses;
it is not consensus proof, source-build equivalence, complete tick coverage,
profitable arbitrage or production qualification. The final identity evidence still
needs source/criterion review against official deployment records. The previous
Solana evidence and all failed attempts keep their original source and dates.

## Finish the epic, not merely the collector

1. Obtain and review the actual Base report with canonical contract/token identities
   and both sought fee-tier pools; retain any absent or ineligible pools explicitly.
2. Finalize the small evidence-backed identity registry and verify that fixture
   identities cannot enter production registries. No executable experiment is
   enabled by this task's acceptance.
3. Complete ARB-003's original checklist and dependency-aware closeout with actual
   evidence. Do not close it on a passing mock or an unconfigured workflow.
4. Reconcile all six child states and the epic's own outcome/release criteria,
   close EPIC-01 only when they are met, then continue the existing #23/#24 chain.

Internal code progress and external access are separate. No new task issue is
needed for these steps. Until that external input works, the accurate state is
**EPIC-01 blocked, five of six original tasks accepted**, not completed.
