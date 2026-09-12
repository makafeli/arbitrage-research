# Research integration verification

PR: [#89](https://github.com/makafeli/arbitrage-research/pull/89). Status: draft; runtime verification is in progress. This file must be updated with the exact final tested commit before acceptance. The first-round 139-test result belongs to PR #83 and is not a result for this change.

## Source changes

The second round adds bounded same-network pool-set registries, exact Base V3 research math, same-engine captured route evaluation and replay, immutable DecisionTrace persistence, authenticated decision queries and immutable PostgreSQL paper accounts. The connected dashboard gains decision and paper-account views. Negative gross quotes and explicit rejections remain visible; incomplete external costs produce a null net amount.

OBSERVE and PAPER workers recover to STOPPED and share generation fences. PAPER support makes stopped virtual-account creation reachable. Running a PAPER worker collects and evaluates research inputs; it never turns a quote into a settlement.

## Verification environment

The shared local execution environment disconnected during implementation with `409 environment_offline`. Source was reconstructed and published through GitHub's repository APIs. The dedicated preparation workflow has read-only repository permissions and produces dependency locks, independently generated Uniswap reference fixtures and formatted-source snapshots for manual review. It cannot commit or move branch references.

The final validation workflow must reproduce committed reference outputs with `--check`, build Rust 1.90 using `--locked`, run mandatory real PostgreSQL tests, exercise the browser and build all container images. Preparation output alone is not compilation or test evidence.

## Local verification and CI correction history

Local Rust 1.91.1 validation passed formatting, all-target compilation, all-target Clippy with warnings denied, and **167 non-database tests**. This count excludes all 40 PostgreSQL tests; they require the canonical service-backed CI run. The paper cohort includes a 4,998-event replay regression. Its diagnostic duration is not a production performance guarantee. Frontend type checking, production build and 20 Node tests also passed locally.

The canonical workflow uses Rust 1.90, PostgreSQL 17.11, the committed Cargo/npm locks, the pinned official Uniswap SDK oracle, the actual API client smoke test and Chromium. CI must reproduce the committed 14 swap cases and three primitive vectors byte for byte; this is arithmetic reference evidence, not current deployed-program equivalence.

The first integrated CI head `6f933099f696b16773b4de7976e5247e160c0099` ([run 34716682208](https://github.com/makafeli/arbitrage-research/actions/runs/34716682208)) passed specifications, reference reproduction and all three container builds. Its Rust job stopped on one test-only Clippy warning; its browser job passed 21 scenarios and failed nine new scenarios on the select-label issue. PostgreSQL execution and downstream browser flows were not counted as passing. Both causes were corrected in `51b31bc3abb89910846937f1312e0701e6047aa2` ([run 34717005526](https://github.com/makafeli/arbitrage-research/actions/runs/34717005526)). That run passed all 30 browsers, the three containers and 206 Rust tests, including 39 of 40 PostgreSQL tests. The remaining PAPER worker fixture omitted the explicit starting asset required by production validation. Its correction supplies the actual fixture registry token and trade size and adds a non-database regression validating both fixture modes; no production guard or process-control assertion is relaxed. Final results are recorded below when available.

## Review corrections

Peer review identified and corrected:

- Runtime PAPER support also requires storage capture admission to permit PAPER; both layers must agree.
- Unknown external costs must not produce a numeric net result, including a misleading zero.
- Trace provenance must remain consistent with source kind for both schema versions.
- Snapshot time must fit the original batch interval, and displayed timestamps must remain representable in the API's year range.
- Decision capture references must belong to the current admitting worker epoch as well as the session and generation.
- Live pagination must disclose its limitations; monotonically generated trace identifiers improve append ordering.
- Loopback endpoint classification must use the transport's normalized URL policy, including uppercase schemes and HTTPS loopback.
- Independent reference generation must preserve negative tick semantics, including JavaScript's distinct negative zero.
- Paper account retries must retain their original idempotency key while an earlier request has an unresolved outcome, including after a later authentication or rate-limit rejection.
- Virtual journal bounds reserve capacity for terminal outcomes so reaching a finite run limit cannot strand existing reservations.
- Historical paper replay applies into a fresh private reducer, validates every recomputed event/posting and discards the reducer on failure. Public mutation retains atomic clone-and-commit behavior; replay no longer copies a growing journal on every event.
- A real worker-epoch takeover regression preserves UNKNOWN reservations and rejects settlement by the expired claim before permitting idempotent resolution under the new claim.
- Worker startup obtains its immutable experiment ID through an operator/session-scoped storage query without widening the session HTTP DTO.
- New select controls have separate, explicitly associated labels. Strict browser selectors exposed the original labels including their option text; production markup was corrected without weakening the assertions.

These are engineering reviews by the implementation team, not an independent security audit.

## Visual review

Root integration inspected the synthetic Connected-mode paper-account screenshots from the successful web job on `51b31bc`. The navy/lime design, legible controls and mobile wrapping remain intact at [320 pixels](review/research-paper-320.png), [390 pixels](review/research-paper-390.png) and [1440 pixels](review/research-paper-1440.png). Wide ledger tables scroll within their own containers; the page itself does not overflow. The mobile retained-run table is horizontally scrolled to the Inspect button because the test selected that run before capture.

These images use synthetic API fixtures, including deliberately very large integer amounts and UNKNOWN reservations. They demonstrate rendering and interactions, not live market data or a complete accessibility certification. CI verifies whole-page overflow and controls; manual inspection supplements those assertions. The retained source files were reconstructed from the CI artifact stream with matching byte counts, ordered chunks and SHA-256 digests:

| Width | Dimensions | SHA-256 |
|---|---|---|
| 320 | 320 × 5720 | `6e28c6b142af5e7dfb1000668234a82e761d8a6ba019b9bdd98517025eb6a7cf` |
| 390 | 390 × 5454 | `56747c73b56e02b128e8b69b552ab420a07a21947fbdf48e0696a06ae4658400` |
| 1440 | 1440 × 3920 | `64ef72d53694bd5e6062fd27849a9286f6a36ed1d771bc658a31f0884cc53bab` |

## Remaining acceptance

Current deployed-program equivalence, provider qualification, measured observation campaigns, complete atomic transaction simulation, inclusion scenarios, automatic paper execution and comparable market reports remain separate gates. No generated fixture or passing arithmetic test establishes realized arbitrage or profitability. Railway definitions remain prepared; no Railway deployment is claimed.

Each pool is currently acquired independently. The engine rejects differing block/slot contexts; the worker does not guarantee a shared live acquisition context across pools. Elapsed batch time measures acquisition and processing age, not the age of the underlying finalized block or slot. Repeatedly receiving old finalized state cannot establish current-market freshness. A common acquisition anchor, chain-lag policy and provider qualification remain necessary before a current-market claim.
