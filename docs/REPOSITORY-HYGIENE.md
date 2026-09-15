# Repository reconciliation and safe branch cleanup

Snapshot: 15 September 2026. Starting main:
`ff61b29e75fe4f368eab521a1c9dfdd8c2d90cb8` (merged PR #126).
This is repository maintenance, not a feature release or a completed EPIC-02.

## Verified result

| Item | Before | After the cleanup transaction |
|---|---:|---:|
| Repository branches, excluding the temporary maintenance branch | 40 | 2 |
| Open task/epic issues, excluding pull requests | 62 | 62 |
| New task issues | 0 | 0 |
| Issues closed by this cleanup | 0 | 0 |
| Original task records with native-state disagreement | 0 | 0 |

Thirty-three branches were verified as ancestors of main or exact unchanged heads
of merged PRs whose merge commits are ancestors of main. Five other branches were
retired preparation/diagnostic work or the superseded PR #101. Their entire commit
history was preserved before deleting those refs. The one-shot maintenance branch
was also removed, so the execution log contains 39 removals: 38 original branches
plus its own temporary branch.

The remaining branches are `main` and `feat/ARB-013-dual-worker-isolation`, which
belongs to the open PR #127. No unfinished feature branch was discarded or folded
into main merely to make the repository appear clean. A subsequent maintenance PR
can briefly add its own branch; remove that branch only after a verified merge.

Twenty-two task headers and three epic rollups were reconciled with current native
states and the existing progress register. Older planned labels were removed only
where actual implementation is present. All 25 issue bodies preserve their
original acceptance sections, issue open/closed state, dependencies and release
requirements. Dated old implementation notes remain as history; new checkpoints
state the actual dependency status explicitly. The 68 original progress records
are unchanged because their completed states already matched GitHub.

## Recovery and exact evidence

- Read-only audit: run **34944130239**, artifact **10385714277**,
  `repository-audit-before`. All branches, all paginated issues/PRs, main source,
  unintegrated diffs and a portable Git bundle are retained. ZIP SHA-256:
  `dafccc04088dccabba5ae5b635248e4db578ceaa0778342ff2f7cdc786ae322a`.
- Applied cleanup: run **34945257799**, artifact **10387420751**,
  `repository-cleanup-result`. Every deletion/update and resulting native snapshot
  is recorded. ZIP SHA-256:
  `1e02b9b1c7f1c8d7e58539e4a513ffeb6904ec09923b10d9f8574486b75e18d2`.
- Persistent archival ref: `archive/retired-branches-2026-09-15`, pointing to
  `9c263c02f74bf87ef4e6a080eae34c331bc307f6`. Its `branches.json` maps the five
  retired names to exact commits; each is an explicit parent and remains reachable.
  This is an archive-only commit, not a release or an integration into main.

Both downloaded ZIP digests and their contained file checksums were recomputed.
The artifacts have 90-day retention, through 14 December 2026; the archival Git
ref separately preserves the retired commit history and must not be deleted as
an ordinary obsolete development branch. A full pre-cleanup bundle also exists in
the conversation export. Do not execute retired helper workflows as current code.

Deletion used one atomic Git ref transaction with an explicit expected-head lease
for every target. Current branch SHA, protection, merged-PR proof and absence of
an open PR were checked before applying. A locally exercised stale-head case
refused the whole deletion set rather than deleting another unchanged branch.
Native issue edits were separately re-read before each mutation; the entire issue
update sequence is not claimed to be an atomic transaction. The result confirms
25 updates and no skipped concurrent change. Main and active PR #127 were unchanged.

## Current continuation point

All 68 original tasks divide into 14 completed, 14 implemented pending acceptance,
eight in progress and 32 planned. The 62 open issues are 54 original tasks, seven
epics and the separate native Codex-startup task. The last seven green checks for
PR #126 belong to head `66a1ce15b9bb2cec0c543c7bb3fd626a61502057`; its verified
merge is the starting main above. A newer PR cannot inherit those checks.

PR #127 head `4e63775a80d8d99d674468fed484cc36b41ca66d` has failing project and
pipeline checks. Its retained `isolation.log` from run **34943035804**, artifact
**10386291835**, shows both `network_isolation` tests rejecting an enabled Base
configuration before the intended two-process experiment:
`enabled research network requires at least two distinct qualified pools and assets`.
This is a test-input validation failure, not measured evidence that a blocked
provider stalls the other chain. Fix the bounded synthetic inputs while retaining
the production validator and original process/control assertions, then rerun and
review the complete combined head before merging or accepting #27.

The configured Base connection already works; do not ask to recreate its secret.
#29 still needs the original real recorded observation/control/replay trace and
accepted ingestion/snapshot/route prerequisites. Current identity-only evidence
cannot substitute for that raw quote capture. Do not rebuild the seven accepted
EPIC-02 platform tasks or close its remaining gates as part of tidying branches.

## Keep later work tidy

The existing AGENTS.md merge/acceptance protocol remains authoritative. After each
verified merge, remove only its unchanged integrated branch if no open PR or
unmerged follow-up uses it. Preserve exact-head evidence and source histories;
archive exceptional retired work rather than manufacturing a merge. Keep current
status short and dated, and identify old checkpoints as historical. Never use
branch counts, passing test counts or removed labels as product-completion claims.

The pipeline workflow now also has five small offline regression checks in the
independent main CI. They guard its job-level contexts, shared evidence directory,
required test commands and no-secret/read-only boundaries. These are targeted
repository checks, not a replacement for the full GitHub workflow schema or
actual Rust/PostgreSQL process execution. No dependency package, provider request,
secret value, deployment, signing or broadcasting is introduced by maintenance.
