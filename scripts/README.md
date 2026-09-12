# Repository automation

The repository source of truth is `planning/backlog.json`. `github_bootstrap.py` creates or enriches GitHub planning from those specifications using the operator's existing GitHub CLI authentication. It uses Python's standard library, `gh`, and (for Wiki only) Git. It does not create or rename the code repository, change its visibility, assign people, close work, purchase services, deploy a trading system or handle wallet keys.

## What runs on GitHub

The `Set up repository planning` workflow runs on a main-branch change to the backlog, importer or workflow, and is also manually dispatchable. It creates labels, M0–M7 milestones, all epic/task issues, native epic sub-issue links and native blocked-by relationships. Its repository token has `contents:read`, `issues:write` and `actions:read`; the last permission restores prior importer progress before a rerun. No owner Project token is requested by this workflow.

Only an actual completed workflow run establishes that this setup is present. See its run summary and `setup-progress` artifact for phase results and issue URLs. A failing phase stops subsequent phases and remains recorded as failed; an issue created before that failure is still a real issue and is discovered on the next run. The workflow never commits its generated state or automatically closes an imported ticket.

Issue `status:*` labels are initialized only when a ticket is created. Reruns preserve the current workflow labels, including an intentional absence of status labels, so the original planned backlog cannot move active or accepted work back to planned. Non-status specification labels and linked dependencies are still enriched; implementation evidence outside the specification's managed block remains intact.

The `Validate project` workflow checks the Rust workspace, frontend, browser interactions and source specifications. It retains generated lockfiles and browser evidence as artifacts. Both dependency lockfiles are committed. If a future checkout lacks Cargo.lock, the initial scaffold fallback generates it for review, then compilation and tests use `--locked`. Maintainers must review and commit such a generated lockfile; the workflow does not publish source changes itself. If source formatting fails, the Rust job still runs compile/test checks, exports `rust-format.patch`, and ends failed so the formatting gap cannot be mistaken for a pass.

## Offline review

Run from the repository root:

```sh
python scripts/github_bootstrap.py --repo makafeli/arbitrage-research --dry-run
python scripts/test_github_bootstrap.py
```

The dry run validates the backlog and prints intended component counts. It does not authenticate, call GitHub, compare remote state or claim that anything is already configured. Every mutating invocation needs `--apply`; `--repo OWNER/REPOSITORY` is always explicit.

## Apply from an authenticated workstation

Install GitHub CLI and authenticate with `gh auth login`. The account must be able to manage issues in the selected repository. Review the offline result, then run:

```sh
python scripts/github_bootstrap.py --repo makafeli/arbitrage-research --apply --components labels,milestones,issues,relationships
```

This is the same repository phase as Actions. Run only one importer against a repository at a time. Actions uses a concurrency group to serialize its own runs; it cannot lock a separate workstation importer.

To finish the owner-level Project setup, grant the local GitHub CLI the `project` scope (or use an appropriately authorized GitHub App), and run the Project phase:

```sh
gh auth refresh -s project
python scripts/github_bootstrap.py --repo makafeli/arbitrage-research --apply --components project,views
```

The script creates or finds one uniquely named private Project, links it to the repository, creates `Delivery status`, `Priority`, `Stage`, `Role`, `Release gate` and `Dependency IDs` fields, adds all imported issues and initializes absent values. Existing workflow values are preserved. It retains GitHub's built-in `Status` field; `Delivery status` is the project's intended workflow field. It does not automatically close issues, alter repository visibility, or reset assigned values during reruns. M6–M7 items start `Deferred`.

The separate `views` phase creates the missing named board/table views from [github-project.json](../planning/github-project.json), including filters, grouping and priority sorting: Delivery board, Research release, Optional live, and Blocked work. Existing view customizations are preserved. Current user-owned REST Project field/view endpoints require supported OAuth/classic authentication; GitHub App tokens and fine-grained PATs are not supported for those endpoints. If the account uses an unsupported token type, the view phase stops with the API error. The same view specification can be applied through GitHub's UI as a fallback. A repository's `has_projects` setting does not prove that this owner Project or its views exist.

## Publish native Wiki pages

The `wiki/` directory remains browsable in the code repository. To publish it as a native GitHub Wiki, enable Wiki in repository settings and create its first Home page in the GitHub UI if the separate Wiki repository has not yet been initialized. Then run:

```sh
python scripts/github_bootstrap.py --repo makafeli/arbitrage-research --apply --components wiki
```

The script clones `OWNER/REPOSITORY.wiki.git` using the existing `gh` credential helper, copies Markdown pages, rewrites source links to canonical repository/native Wiki URLs, commits changed pages with the authenticated account's GitHub no-reply identity, and performs an ordinary push. It never deletes unrelated pages or force pushes. A concurrent Wiki edit that prevents a fast-forward causes a failure; rerun against the new head to review/apply the source again. Credentials are not written into Git URLs or printed. GitHub Free supports public Wikis; private repository Wikis require an eligible paid plan. This repository's visibility is never changed to obtain Wiki access.

## Idempotency and recovery

Each imported issue has a stable marker such as `<!-- arb-ticket:ARB-001 -->` or `<!-- arb-ticket:EPIC-01 -->`. The importer discovers open and closed issues by these markers, rejects duplicate matches, and replaces only the block bounded by `<!-- arb-managed:start -->` and `<!-- arb-managed:end -->`. Add operator notes outside that managed block. Existing comments, issue states, assignees and additional labels are preserved. Legacy marker-only bodies are preserved and receive a managed canonical block; a maintainer can remove duplicated legacy prose after checking for operator notes.

Labels and milestone membership are populated from the current specification. Dependency links are resolved in a second issue pass. Native relationships are added only when absent; relationships or children not in the specification are not removed automatically. Existing parents are never replaced implicitly. Removing a requirement or changing dependency topology therefore requires a reviewed cleanup of obsolete native links.

Before creating an issue, milestone, Project, Project field or saved view, the importer saves a pending record to its state JSON. An uncertain issue response triggers a fresh read instead of a repeated POST. If the issue remains absent, the run stops. Existing Project and field values are inspected on rerun, so an interruption between adding an item and initializing its fields can recover without resetting operator work.

The default local state file is `.github-setup-state.json` (ignored by Git). Actions uses `setup-state.json`, restores the last nonexpired `setup-progress` artifact and saves a fresh artifact even on failure. Keep that state for recovery; artifact retention is 30 days. If an interrupted run lost its artifact or it has expired, inspect GitHub before retrying: no remote API can guarantee exactly-once issue creation after both a lost response and lost local state. The importer does not promise distributed atomicity across GitHub resources.

After verifying a particular unknown issue was not created, explicitly acknowledge it on the workstation:

```sh
python scripts/github_bootstrap.py --repo makafeli/arbitrage-research --apply --components issues --state setup-state.json --acknowledge-missing-create ARB-001
```

Download and use the failed run's `setup-progress` state for this command. Unknown milestone/Project/field/view creates require inspecting the corresponding GitHub resource and retaining a backup before clearing just that object's pending state. Do not blindly delete all progress. Rate-limit, permission and transport errors are surfaced; mutations are spaced by at least 1.1 seconds where dispatched through the importer API, and failed requests are not automatically retried.

## Authentication and capability limits

A repository-scoped `GITHUB_TOKEN` is sufficient for the base issue phase when Actions policy allows its requested permissions. Owner Projects require separate project access; they are not created by the base workflow. Native Wiki publishing requires normal Git push access to the separate initialized Wiki repository. The script neither extracts application tokens nor asks for them in chat, issue bodies or committed files. Set any workstation credentials through GitHub's normal authentication flow.

Repo rulesets, required reviews, private vulnerability reporting and spending ceilings are owner configuration tasks. Their desired review gates are specified in project documentation; the presence of configuration source or a policy document is not evidence that GitHub has enabled a setting. No unverified security setting should be represented as active.

## Verified source references

GitHub API behavior and action references were checked on 2026-09-12. The importer uses the documented REST API version `2026-03-10`. Official references:

- [GitHub CLI API command](https://cli.github.com/manual/gh_api): authenticated REST/GraphQL requests and JSON input.
- [GitHub REST sub-issues](https://docs.github.com/en/rest/issues/sub-issues): parent/child issue relationships.
- [GitHub REST issue dependencies](https://docs.github.com/en/rest/issues/issue-dependencies): native blocked-by relationships.
- [Manage Projects using the API](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects): Project authentication, creation, fields and item updates.
- [Project views REST API](https://docs.github.com/en/rest/projects/views) and [Project fields REST API](https://docs.github.com/en/rest/projects/fields): saved view creation, numeric grouping/sorting field IDs and user-token restrictions.
- [Create Project fields](https://cli.github.com/manual/gh_project_field-create) and [link a Project](https://cli.github.com/manual/gh_project_link): CLI field and repository association commands.
- [About Wikis](https://docs.github.com/en/communities/documenting-your-project-with-wikis/about-wikis) and [edit Wiki pages locally](https://docs.github.com/en/communities/documenting-your-project-with-wikis/adding-or-editing-wiki-pages): plan availability and separate Wiki Git repository.
- [Automatic token authentication](https://docs.github.com/en/actions/security-for-github-actions/security-guides/automatic-token-authentication): repository workflow credentials and permissions.
- Pinned official Actions: [checkout commit](https://github.com/actions/checkout/commit/fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09), [setup-node commit](https://github.com/actions/setup-node/commit/a0853c24544627f65ddf259abe73b1d18a591444), [setup-python commit](https://github.com/actions/setup-python/commit/ece7cb06caefa5fff74198d8649806c4678c61a1), and [upload-artifact commit](https://github.com/actions/upload-artifact/commit/ea165f8d65b6e75b540449e92b4886f43607fa02). Dependabot can propose reviewed updates.

Twenty-one offline regression tests cover repeated import, uncertain create outcomes, duplicate markers, preservation of operator notes/status, link rewriting, dependency cycles, shell-safe argument passing and verified recovery from issue collection omissions. They include preservation of deliberately removed or advanced workflow labels across reruns. These tests do not certify live API permissions or native Project/Wiki publication; only completed remote phases establish those results.
