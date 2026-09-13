# Dependency-aware delivery coordination

Tracking issue: [#96](https://github.com/makafeli/arbitrage-research/issues/96). This is delivery tooling, not implementation acceptance for any of the original 68 tickets. The original register and acceptance bodies are not rewritten by this tool.

## What is executable

`scripts/delivery.py` uses Python 3.11+ and the standard library. It produces a deterministic work report, checks a complete wave's proposed ownership, audits the original tickets against a supplied GitHub snapshot and checks closeout evidence packets. It does not invoke a language model, create workers, merge PRs, edit issue bodies, change labels or close tickets. A configured capacity of 25 or 50 is not an active agent fleet. Actual parallel coding requires a separately available agent runtime and isolated worktrees; no such runner or paid API usage is enabled by this change.

The default is one orchestrator, five manager lanes, five slots per lane. At 50 capacity each lane receives ten slots. Unused lanes remain idle instead of inventing tasks or bypassing dependencies. Round-robin proposals are not a claim or a lock. Managers separate foundation/operations, platform/engine, Base, Solana and dashboard work. A single orchestrator owns shared build files, schemas, planning, workflow changes and all remote mutations. The [root agent instructions](../AGENTS.md) define the full claim → implement → test → review → integrate → accept → close → read-back/cleanup protocol.

## Run the planner and audit

```sh
python scripts/test_delivery.py -v
python scripts/delivery.py plan --capacity 25 > /tmp/arb-work-plan.json
python scripts/delivery.py snapshot > /tmp/arb-github-issues.json
python scripts/delivery.py audit --input /tmp/arb-github-issues.json
```

Only `snapshot` accesses the network. It invokes paginated `gh api --method GET` using the operator's existing authentication, never a token entered into a ticket or chat. A failed page produces no complete snapshot; no failed read is treated as an empty repository. It follows collection pages (not issue search's result limit), removes PR rows, rejects duplicate issue IDs and records start/end timestamps. This is **not an atomic GitHub snapshot**: repeat the read before consequential changes. The audit requires all mapped original issues, exact markers and completed-versus-not-planned semantics, and reports duplicate markers and local/remote acceptance drift. Additional delivery and bug issues remain explicitly outside the original 68-ticket acceptance denominator.

Exit codes: `0` means a report/preflight completed without reported drift; `1` means audit findings; `2` means malformed inputs, missing prerequisites or failed collection. None of these exit codes changes GitHub. A successful file parse is not permission to close work.

The work report separates accepted tasks, acceptance reviews, implementation proposals, dependency blockers, explicit/external blockers and later gates. Implemented dependency code can support an isolated implementation proposal while the same dependency still blocks final acceptance. The original register currently includes substantive external qualification and release gates. More slots cannot supply that evidence.

## One ownership manifest per actual work wave

The example below illustrates the shape, not an active assignment. Use a real reviewed source SHA; select an eligible ticket from the current work report. All paths are literal repository-relative file or directory prefixes, not glob patterns. Parent/child overlap, duplicate workers/branches, unrecognized managers, stale source SHAs, excess capacity and shared integration paths are rejected.

```json
{
  "schema_version": 1,
  "base_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "assignments": [
    {
      "ticket_id": "ARB-008",
      "manager": "platform-engine",
      "worker": "worker-domain-1",
      "branch": "fix/ARB-008-scoped-regression",
      "write_paths": ["crates/arb-domain"]
    }
  ]
}
```

```sh
python scripts/delivery.py assignments --input /tmp/arb-wave.json \
  --head-sha ACTUAL_40_CHARACTER_SOURCE_SHA --capacity 25
```

Validation compares the explicit expected SHA; the caller must verify it against current GitHub. Two orchestrators must not concurrently validate separate files and assume they own the same paths. There is no cross-host lock. A central, serialized claim record and one publishing orchestrator are required. Do not put actual runtime state or credentials in the committed example.

## Closeout evidence packet

`closeout` checks an **offline, caller-supplied** evidence packet. It validates internal consistency, not the authenticity or freshness of GitHub responses. The orchestrator must obtain and inspect actual GitHub PR metadata, latest exact-head job results, reviews and native issue/dependency bodies before filling the packet, and re-read before any mutation. Editing JSON can fabricate a consistent packet; therefore this command intentionally exposes no `--apply` or automatic closure.

The packet includes `schema_version: 1`, `repository`, `ticket_id`, and:

- `pull_request`: canonical `number`/`url`, `base_ref: main`, `merged: true`, exact `head_sha` and `merge_commit_sha`.
- `checks`: uniquely named `specifications`, `rust`, `web` and `containers` records, each with exact `head_sha`, `status: completed`, `conclusion: success` and canonical `run_url`. Missing, skipped or wrong-head checks fail.
- `review`: exact `head_sha`, `status: completed`, `kind: solo-maintainer` or `independent`, actual `reviewer`, `evidence_url`, and explicit empty `unresolved_findings` and `unresolved_threads`. Never invent independent approval.
- `criteria`: every canonical acceptance sentence in original order, each with `text`, `result: passed` and nonempty `evidence`. The native issue checklist must match and be checked.
- `github_snapshot`: complete issue collection from the read-only snapshot command.

The register must already retain evidence and acceptance review and no remaining acceptance/external prerequisite. All original dependencies must be accepted. Keep the target `implemented_pending_acceptance` while its remote issue is still open; after explicit closure, update it to `completed` and audit the read-back state. An interrupted two-system update is reported as drift, not silently “fixed” in either direction. M6/M7 operator/independent gates cannot be bypassed by this preflight.

```sh
python scripts/delivery.py closeout --ticket ARB-008 \
  --input /tmp/arb-closeout.json --head-sha ACTUAL_40_CHARACTER_SOURCE_SHA
```

A consistent packet returns `EVIDENCE_PACKET_CONSISTENT`, **not** `DONE` or `AUTHORIZED_TO_CLOSE`. This separation prevents a local JSON assertion from becoming remote administrative authority.

## CI and resumable evidence

The Delivery review workflow runs the coordination tests, original project validator, read-only GitHub issue audit and work planner. Existing Rust/PostgreSQL, frontend/browser, contract and container jobs remain separate and mandatory for their own acceptance claims. The new workflow is not branch protection: repository settings must require it separately if enforcement against bypass is desired.

Its seven-day artifact contains the work report, issue snapshot, audit report and a checksummed source archive created with `git archive HEAD`. It contains only committed tracked files, not `.git` or untracked environment data. On PR events `HEAD` is normally GitHub's synthetic merge commit: `SOURCE_COMMIT` deliberately records that exact archived commit, which is different from the PR source head. Match the workflow's PR head and base metadata before reusing its test evidence. No archive claims to include deployment secrets, market captures outside the repository or an active worker process.

To resume: verify the artifact hash and archived commit, read the current repository and issue states, rerun the planner/audit, then dispatch only actually available workers with compatible ownership. Disconnected or failed tasks retain explicit blockers and do not produce fictional completion counts.
