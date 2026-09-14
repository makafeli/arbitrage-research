# Parallel research boundary verification — 14 September 2026

This record covers existing original issues #16, #23, #30, #31, #40 and #51.
It creates no new implementation tasks and does not close those original gates.
The starting main was `b24ed4c4a9d76aadbdef1522f2c6dc1858e2191a`; its complete
project, recovery, delivery and comparison workflows passed before dispatch.

## Actual execution and review

The primary used the available ChatGPT collaboration runtime to start five real
children. Each acknowledged an isolated branch/worktree and private index, and
the runtime reported all five running concurrently. The complete assignment
manifest was validated before serialized native issue claims and dispatch.
Planner capacity ten accommodated two platform-engine assignments; only five
children were started. Capacity is not a worker count. The workspace permits
shared filesystem access, so path ownership is procedural isolation, not an
enforced per-agent security boundary.

The recorded runtime evidence is in [#107](https://github.com/makafeli/arbitrage-research/issues/107).
This proves this session's collaboration, not launch through the repository's
custom Codex roles, a cloud environment or an unattended service. That narrower
startup gate remains open. Workers do not continue after the active session.

| Worker scope | Result | Local verification and peer review |
|---|---|---|
| Configuration, ARB-009 | Bound canonical output after escaping/defaults so an accepted snapshot remains reloadable; preserve the inclusive 1 MiB limit and historical bytes/digests | Two expansion regressions failed before the fix; 12 tests, Clippy and formatting passed. Financial peer reviewed local `aaac3fb605d3b708a8c5ae14d786053bb7058ac7` |
| Base capture, ARB-016 | Reject contradictory/missing final number, parent and timestamp despite a matching hash; reject signed RPC quantities | Three regressions failed before the fix; 31 tests, Clippy and formatting passed. Dashboard peer reviewed local `6279642b7a4e2f0a80e14c7b4a6f6a073a2a69a1` |
| Solana capture, ARB-017 | Reject invalid ProgramData authority tags and deployment slots beyond account context even with a matching registry digest | Two negative tests failed before the fix; 37 tests, Clippy and formatting passed. Platform peer reviewed local `3a6dd5a9af5b2442df5b8ce49cf14f4fa92668f1` against loader semantics |
| Exact ledger, ARB-026 | No demonstrated reducer defect; no code change or invented PR | All 28 tests and formatting passed on the starting main. Criterion review retains atomic reservations, unknown outcomes, idempotency, immutable runs and exact replay; database tests were inspected, not locally rerun |
| Dashboard, ARB-037 | Enforce snapshot coherence, quote-included fees and research execution boundaries; accept valid SIMULATED records without imposing the stricter executable balance guard | Eight new Node tests failed before correction; final 107 Node tests and TypeScript passed. Base peer reviewed local `ed2498a446a70603fa2701217ce285588e24f1d9` and `06a5232dfef778d3f9a5eafb2e4dc798a6c92774` |

Root additionally corrected the same Base-header defect in the registry observer
and required explicit opt-in for public sampling. Twenty synthetic subcases
failed before the header correction; all 23 observer tests passed afterward.
Financial independently reran those tests and reviewed the workflow/doc follow-up.
Root inspected all production diffs and owned remote publication/integration.
These are same-session engineering reviews, not an external audit. CodeRabbit
was rate-limited on #114; a passing bot status is not source approval.

## Published and integrated evidence

Local source trees were compared to the complete GitHub trees before publishing.
After main moved, the next branch incorporated it and repeated its required CI;
merges use an expected head and verify the resulting tree. All five source PRs
are merged, with the following native GitHub readback:

| PR | Tested head | Integrated base | Verified merge | Passing CI |
|---|---|---|---|---|
| [#114](https://github.com/makafeli/arbitrage-research/pull/114) | [017b8ea99c43](https://github.com/makafeli/arbitrage-research/commit/017b8ea99c43642f5f473708de272f987a74cbb1) | `b24ed4c4a9d7` | [f77dda1d1e37](https://github.com/makafeli/arbitrage-research/commit/f77dda1d1e3795fb2746043cbf8a21f4a1008458) | [project](https://github.com/makafeli/arbitrage-research/actions/runs/34850646690), [recovery](https://github.com/makafeli/arbitrage-research/actions/runs/34850646691) |
| [#115](https://github.com/makafeli/arbitrage-research/pull/115) | [e45c8fc03ba0](https://github.com/makafeli/arbitrage-research/commit/e45c8fc03ba09488e0d93a065a548c522f139e29) | `f77dda1d1e37` | [ca2b61595819](https://github.com/makafeli/arbitrage-research/commit/ca2b615958198f095670774934b5a37dc798f9d8) | [project](https://github.com/makafeli/arbitrage-research/actions/runs/34851529926), [recovery](https://github.com/makafeli/arbitrage-research/actions/runs/34851529901) |
| [#116](https://github.com/makafeli/arbitrage-research/pull/116) | [29e1c81a2258](https://github.com/makafeli/arbitrage-research/commit/29e1c81a22588c1330abcd1bc150e6f56a758429) | `ca2b61595819` | [83c3eaa37c67](https://github.com/makafeli/arbitrage-research/commit/83c3eaa37c678b72b0d38c64d2cacde190b60bf3) | [project](https://github.com/makafeli/arbitrage-research/actions/runs/34852254896), [recovery](https://github.com/makafeli/arbitrage-research/actions/runs/34852254979) |
| [#117](https://github.com/makafeli/arbitrage-research/pull/117) | [db2e3d758b32](https://github.com/makafeli/arbitrage-research/commit/db2e3d758b32d0ac1a038927233cee3a6ddd44eb) | `83c3eaa37c67` | [6a6a67e1ce03](https://github.com/makafeli/arbitrage-research/commit/6a6a67e1ce031049cbe47e40b90b13ef5797c80a) | [project](https://github.com/makafeli/arbitrage-research/actions/runs/34852879621), [recovery](https://github.com/makafeli/arbitrage-research/actions/runs/34852879737) |
| [#118](https://github.com/makafeli/arbitrage-research/pull/118) | [ba4be3639e9b](https://github.com/makafeli/arbitrage-research/commit/ba4be3639e9b9fe6fc783bf5aff993a65edb3192) | `6a6a67e1ce03` | [2e2fa131f7b6](https://github.com/makafeli/arbitrage-research/commit/2e2fa131f7b679f71797316dd768e5e4e00ed51a) | [project](https://github.com/makafeli/arbitrage-research/actions/runs/34853523533), [recovery](https://github.com/makafeli/arbitrage-research/actions/runs/34853523429) |

The complete merged source tree after #118 is
`ef40d2c2cacff52a14249662e7b3b10115cb3336`; it equals that final tested head's
tree. Every listed project run passed specifications, full Rust/PostgreSQL/HTTP,
web/browser and all three container builds. Recovery and delivery also passed
on each final source. The dashboard job in run 34852879621 reports 107 Node tests
and 75 actual Chromium scenarios, including both new Connected regressions.

The final registry workflow [34853523477](https://github.com/makafeli/arbitrage-research/actions/runs/34853523477)
passed all 23 offline observer tests and explicitly skipped public collection.
The skipped job is not counted as successful sampling. A complete native issue
snapshot was checked against all 68 progress entries with zero audit findings;
all 20 delivery-coordination tests passed. Original criteria remain unchecked
where acceptance is still incomplete.

## Observation workflow correction

Opening #118 unexpectedly activated the existing PR-open public sampler despite
the correction's intended offline scope. Root inspected run 34851153114, then
downloaded its artifact and recomputed the ZIP and enclosed file hashes. One
Base identity request returned HTTP403; four Solana requests completed. This is
recorded separately in [registry verification](27-INITIAL-REGISTRY-VERIFICATION.md),
with exact source/artifact identifiers. It does not establish qualification.

The workflow now excludes every PR event from public collection. Only explicit
manual dispatch or the existing exact-message push on the original scoped branch
can collect. Subsequent code-review CI reports the collection job skipped while
offline observer tests run. No further collection is needed for this correction.

## Remaining acceptance and concurrent work

Seven original tickets are accepted. The other 61 include implemented source,
partial work and later gates; these counts are not a percentage of usable-product
completion. Existing #16 remains the first dependency bottleneck: Base access,
read-only chain-state verification and reviewed registry entries remain incomplete.
The two observed Solana identities are not an activated registry. Amount-specific
quote coverage and deployed-protocol equivalence remain separate downstream
qualifications. The open identity gate still affects #23/#24 acceptance.

Complete unsigned plans/atomic simulation, scenario-approved settlement, durable
stream/rollback/gap evidence, elapsed campaigns and application-aware operational
recovery remain separate tasks. The ledger review does not close #40 or create an
automatic paper-fill endpoint. Current chain decoders retain their unqualified
capability flags; no evidence label is promoted from synthetic tests.

PR #113 is a separate active dashboard-shell/scheduler implementation. Platform's
read-only diagnosis, posted by root, found four mobile shell failures caused by existing CSS
hiding the PAPER MODE DEMO badge at 320/390px; the diagnosis was posted to its
owner without taking the source claim. Its exact changes do not overlap this
cohort's dashboard files. Its browser/screenshot review and original #50/#27
closeout remain separate. Its claimed shared progress file was not overwritten
by this documentation reconciliation; native issue updates retain this cohort's
new evidence and the existing acceptance-state register remains unchanged.

The Railway web/API/PostgreSQL foundation already exists. The source gate follows
main; no fresh Railway deployment IDs or health checks were obtained for these
merges, so no current deployment result is inferred from GitHub CI. Research
workers, provider credentials, capacity/retention scheduling and full restart/STOP
recovery remain unqualified. No account purchase, funding, signer/broadcast or
live activation was introduced by these corrections.
