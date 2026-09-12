# Research integration verification

PR: [#89](https://github.com/makafeli/arbitrage-research/pull/89). Status: draft; runtime verification is in progress. This file must be updated with the exact final tested commit before acceptance. The first-round 139-test result belongs to PR #83 and is not a result for this change.

## Source changes

The second round adds bounded same-network pool-set registries, exact Base V3 research math, same-engine captured route evaluation and replay, immutable DecisionTrace persistence, authenticated decision queries and immutable PostgreSQL paper accounts. The connected dashboard gains decision and paper-account views. Negative gross quotes and explicit rejections remain visible; incomplete external costs produce a null net amount.

OBSERVE and PAPER workers recover to STOPPED and share generation fences. PAPER support makes stopped virtual-account creation reachable. Running a PAPER worker collects and evaluates research inputs; it never turns a quote into a settlement.

## Verification environment

The shared local execution environment disconnected during implementation with `409 environment_offline`. Source was reconstructed and published through GitHub's repository APIs. The dedicated preparation workflow has read-only repository permissions and produces dependency locks, independently generated Uniswap reference fixtures and formatted-source snapshots for manual review. It cannot commit or move branch references.

The final validation workflow must reproduce committed reference outputs with `--check`, build Rust 1.90 using `--locked`, run mandatory real PostgreSQL tests, exercise the browser and build all container images. Preparation output alone is not compilation or test evidence.

## Review corrections

Peer review identified and addressed, or queued for the final checked commit:

- Runtime PAPER support also requires storage capture admission to permit PAPER; both layers must agree.
- Unknown external costs must not produce a numeric net result, including a misleading zero.
- Trace provenance must remain consistent with source kind for both schema versions.
- Snapshot time must fit the original batch interval, and displayed timestamps must remain representable in the API's year range.
- Decision capture references must belong to the current admitting worker epoch as well as the session and generation.
- Live pagination must disclose its limitations; monotonically generated trace identifiers improve append ordering.
- Loopback endpoint classification must use the transport's normalized URL policy, including uppercase schemes and HTTPS loopback.
- Independent reference generation must preserve negative tick semantics, including JavaScript's distinct negative zero.
- Paper account retries must retain their original idempotency key while an earlier request has an unresolved outcome, including after a later authentication or rate-limit rejection.
- Virtual journal bounds must reserve capacity for terminal outcomes so reaching a finite run limit cannot strand existing reservations.

These are engineering reviews by the implementation team, not an independent security audit.

## Remaining acceptance

Current deployed-program equivalence, provider qualification, measured observation campaigns, complete atomic transaction simulation, inclusion scenarios, automatic paper execution and comparable market reports remain separate gates. No generated fixture or passing arithmetic test establishes realized arbitrage or profitability. Railway definitions remain prepared; no Railway deployment is claimed.

Each pool is currently acquired independently. The engine rejects differing block/slot contexts; the worker does not guarantee a shared live acquisition context across pools. Elapsed batch time measures acquisition and processing age, not the age of the underlying finalized block or slot. Repeatedly receiving old finalized state cannot establish current-market freshness. A common acquisition anchor, chain-lag policy and provider qualification remain necessary before a current-market claim.
