# Native Codex: five workers and one orchestrator

Tracking issue: #107. This setup extends the planner from #96; it is not a
second planner and does not create an unattended service.

## Actual status

At setup on 13 September 2026, the assistant environment had no Codex binary,
native agent dispatcher or connected runner action. **Zero AI workers were
started.** The local source tree matched main's merge tree from #106. A TOML
parse, test double or passing CI job cannot prove actual multi-agent startup.
The real six-thread acceptance criterion remains open in #107.

## Start in the local project

Use an existing trusted checkout in a current Codex app/CLI with authorized
ChatGPT sign-in. Python 3.11+ is required by the launcher and existing delivery
tools. Do not paste credentials into chat, source files, tickets or commands.

```sh
# From a checkout containing these files. This does not start or contact Codex.
python3 scripts/start_codex_team.py

# Explicit interactive start. Requires codex on PATH and existing ChatGPT login.
python3 scripts/start_codex_team.py --start
```

The explicit start opens **one foreground primary Codex session** with the
orchestrator instructions. That primary must actually spawn the five children
using native tools, confirm separate worktrees and read their runtime states.
The launcher never says that opening the parent means all workers are running.
It refuses a missing executable, noninteractive terminal, wrong repository,
dirty worktree, failed auth check or non-ChatGPT authentication. It never
installs software, buys credits, switches to API-key billing or edits user
credentials/configuration. It selects the built-in OpenAI provider and ChatGPT
authentication for this invocation. Your existing account quota and usage
policy still apply; remaining quota/extra billing cannot be checked here.
Do not approve extra spending. Authentication status output is not logged.
The launcher accepts only the complete positive `Logged in using ChatGPT`
status line and a zero exit code. Negative, ambiguous, duplicate or additional
status output is refused without echoing it. A future CLI output format may
require an explicit parser update; the launcher never guesses from the word
ChatGPT. This checks a local authentication-mode acknowledgement, not live
credential validity, remaining quota or the availability of agent tools.

In the app, open a **new local project task** after retrieving these files,
trust the checkout, and submit: `Read .codex/ORCHESTRATOR.md and start the 5+1 team.`
An existing chat may not acquire new tools just because a file was added. App
launch does not run the Python authentication preflight; check the selected
account, quota and permission mode before submitting the task.

## Git environment isolation

Preflight subprocesses and the final foreground process replacement receive a
copy of the caller environment with Git checkout-local overrides removed.
This includes repository, working-tree, common-directory, object-store and
index variables, plus injected Git configuration parameters that could spoof
the intended remote. The caller's environment is not modified. Unrelated
PATH, home, proxy, authentication-socket and Codex settings are retained.

Regression tests use real disposable repositories and five linked worktrees:
a foreign repository/index cannot redirect a check, a dirty worktree is still
refused, and clean siblings remain valid. A separate subprocess test verifies
environment inheritance; the final execve boundary is checked with a test
double, not a real Codex invocation. These checks do not validate malicious
machine-local Git configuration, native agent permissions or running agents.
See the Git guidance on checkout-local environment variables:
https://git-scm.com/docs/githooks#_description

## Team

| Native role | Focus | Existing manager lane |
|---|---|---|
| Primary session, not an extra child | Current-state reconciliation, assignment, integration, GitHub and acceptance | Orchestrator |
| platform | Storage/control/recovery correctness | foundation-operations |
| base | Base plans and simulation | base |
| solana | Solana plans and simulation | solana |
| financial | Costs, valuation, virtual accounting and opportunities | platform-engine |
| dashboard | UI, authenticated reports/exports and browser checks | dashboard |

The project sets `agents.max_concurrent_threads_per_session = 5`, which the
current documentation defines as children excluding the primary. There are
five standalone custom-agent TOML files and no sixth orchestrator child.
No model name is pinned: retain a model already available to your account.
Each child is instructed not to delegate, mutate GitHub or write shared paths.
These instructions do not replace runtime sandbox permissions or file locks.

The root uses the existing `delivery.py` planner and full-wave assignment
validator with `--capacity 5`, not the generic default of 25. Manager labels
are routing labels, not five more running managers. Tasks and paths are chosen
from fresh open issues at runtime; configured roles are not issue claims.
If fewer than five compatible tasks/runtime slots are available, report the
actual restriction rather than hiding it behind five role names.

## Startup and completion evidence

Require five distinct native child IDs, their open/active states, overlapping
execution, and each child's acknowledged branch/base/worktree. Review the
agent panel or `/agent` in the CLI. The root must report CONFIGURED, STARTING,
RUNNING, BLOCKED and COMPLETED distinctly, and re-check before replacing a
worker. On a partial spawn failure, reconcile the real children and claims.

Permission/worktree isolation failures must not lead to a shared index or
unsafe sandbox bypass. Root alone merges after exact-head CI and required
reviews. Original ticket completion remains separate from merged slices.
No production operations, paid providers or trading are introduced.

## Verification

`python3 scripts/test_codex_team.py -v` tests configuration shape, capacity,
role definitions, preflight refusal and typed launcher arguments. Tests mock
external processes; they do not spend tokens, open an agent runtime or count
as independent reviews. The path-scoped Codex team review runs these checks
in CI. Actual role discovery, effective runtime capacity, authentication flow,
six live threads and worktree isolation still require a real Codex session.

## Continuation verification: 13 September 2026

Source review found that the original substring-based authentication check
accepted some negative or unrecognized status text on exit code zero. Four new
regression tests reproduce ten failed assertions against the original source;
the corrected launcher requires exactly one complete positive status line.
All 24 startup-contract tests pass after the correction. They include refusal
before `execv`, redacted timeout/failure handling and stdout/stderr separation.
These tests do not start agents or independently validate any account.

The Codex connector previously replied on PR #108 that this repository needs
a cloud environment. Its reply links to the account's environment settings.
No startup or automatic retry is implied by merging this setup. Issue #107
retains the latest runtime blocker and any subsequent verified startup result.

## Official references checked on 13 September 2026

- Subagent configuration and custom standalone roles:
  https://learn.chatgpt.com/docs/agent-configuration/subagents
- Project trust and configuration precedence:
  https://learn.chatgpt.com/docs/config-file/config-reference
- CLI flags and login status:
  https://learn.chatgpt.com/docs/developer-commands?surface=cli
- Isolated worktrees:
  https://learn.chatgpt.com/docs/environments/git-worktrees

User/profile/legacy runtime settings can override expected capacity. Verify
native capability and status rather than assuming a parsed setting is active.
