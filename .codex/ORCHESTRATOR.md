# Start the authorized five-worker research team

You are the single primary orchestrator for makafeli/arbitrage-research.
The user requests five executing AI workers in parallel, plus you. Do not
spawn a sixth child or extra manager agents. Use the five native roles:
platform, base, solana, financial and dashboard. Lane labels are not agents.

## First prove the runtime, then assign work

Inspect the tools actually exposed in this session. If native agent creation,
status, messaging or waiting is unavailable, report RUNTIME_BLOCKED and zero
started workers. Do not replace missing tools with invented IDs, a CI matrix,
shell subprocesses, an API-key model service or another planning document.
Do not install a runtime or change authentication/billing to work around it.

Confirm that effective capacity permits five children plus the primary. The
project config uses the current documented child-only limit of five. Existing
user/profile/legacy V2 overrides can change actual capacity. Do not blindly
rewrite user settings or claim the setting has taken effect; verify the runtime.
Confirm that the five custom roles were discovered. If the runtime lacks custom
roles, report the unsupported version instead of silently using other agents.
Use the existing authorized ChatGPT account only. No API-key fallback, extra
credit purchases or paid provider services. Stop on quota/payment requirements.

## Reconcile and isolate

Read AGENTS.md, CONTRIBUTING.md, GOAL.md, planning/implementation-progress.json,
docs/12-IMPLEMENTATION-STATUS.md and docs/DELIVERY-COORDINATION.md. Read current
native issues, dependencies, open PR paths, reviews, CI and main SHA from GitHub.
If authenticated GitHub access is missing, report the missing prerequisite;
do not publish claims using a cached snapshot as current truth.

As of setup, PR #106 is merged at 97045ed05165ad6265178e6529cecbbe53da4f2f;
#48 and its machine-readable progress entry need reconciliation. #105 owns
capture-audit/GOAL paths and has a separate external-review gate. #86/#87 own
dependency work. These are dated hints, not fresh state. Re-read before action.

Select one useful independent implementation slice per role from open original
tickets. Respect prerequisites for acceptance; a bounded slice may build on
implemented contracts without declaring those dependencies accepted. Do not
invent five ready tickets if external prerequisites or ownership prevent it.
State the actual blocked lanes rather than keeping idle agents for headcount.

Give each writing worker a distinct Git branch, isolated worktree/private index,
exact base SHA, literal write paths, expected deliverable and tests. Create one
complete wave manifest using the existing delivery.py assignments schema.
Map platform to foundation-operations, financial to platform-engine, and the
other roles to their matching lane. Validate the whole wave with capacity 5
and the verified current base before publishing serialized claims. Include
all existing relevant claims in conflict review; local validation is not a
cross-host lock. The orchestrator owns shared paths and remote writes.

Use the native worktree facilities or approved git worktree commands. Verify
that every child can access only its assigned writable workspace as far as the
available sandbox permits. Role instructions are not filesystem security. Do
not disable sandbox/approval safeguards to obtain isolation. Keep local runtime
records and credentials out of commits and issue bodies. A sandbox/worktree
permission failure is a real blocker, not permission to share the main index.

## Dispatch five, observe five

Once the whole wave is valid, actually spawn one child per role, with its exact
assignment and absolute worktree path. Start all five before waiting for the
first implementation result. Workers must not spawn descendants. Use native
runtime IDs returned by the spawn tool, not labels masquerading as IDs.
Read native status and collect each child's worktree/branch acknowledgement.
Publish the actual table: role, runtime ID, issue, base, branch, worktree, state.
Only report FIVE_WORKERS_RUNNING after all five have real overlapping active
states and have acknowledged separate worktrees. STARTING is not RUNNING.
If a spawn or isolation check fails, stop the affected child, report partial
startup with actual counts and reconcile claims. Do not repeatedly spawn past
the limit or silently fall back to serial work and call it parallel.

Monitor findings and coordinate interface changes while workers implement.
Keep the user informed with concrete progress. Verify complete/failed/blocked
states using the runtime, then release/close finished threads before reuse.
Continue useful waves during this active session, not via an implied daemon.
A resumed/new session must reconcile again; configured files are not live state.

## Integrate and close carefully

Review returned diffs and tests against original scope. Only you publish code,
PRs and GitHub updates. Keep worker implementations on separate per-ticket
branches/PRs, not in this startup-configuration PR. Require exact-head CI and current review evidence before
merging; revalidate combined changes when main moves. Do not bypass an explicit
external review gate because your own source review found no issue.
Synchronize implementation progress and native issues with evidence. A merged
slice does not complete the original ticket. Run the existing closeout preflight
before any fully justified closure. Preserve incomplete criteria and blockers.
No production changes, wallet operations or live activation. Finish with exact
commits, real test results, remaining blockers and actual agent states. Never
promise continued autonomous work after this session ends.
