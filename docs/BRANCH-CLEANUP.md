# Verified merged-branch maintenance

The owner requested continued delivery without abandoned work branches or duplicate
task issues. This maintenance does not close issues or change acceptance criteria.
It is independent of the trading runtime, accounts, provider settings and Railway
services. No source-publication helper branch is needed for routine cleanup.

## Automatic scope

After a push to this repository's `main`, the workflow first runs policy tests
and an actual local-Git lease regression. Only its dependent main-push job has
write permission. Pull-request checks cannot run deletion. The cleanup job runs
source already integrated into `main`, never code from a candidate work branch.

A branch is a candidate only when all of the following hold:

- It matches `feat/ARB-NNN-*`, `fix/ARB-NNN-*` or `docs/ARB-NNN-*` and is unprotected.
- A same-repository pull request was actually merged into `main`.
- Its current commit equals the exact head recorded by that merged pull request.
- The recorded merge commit is an ancestor of the inspected current `main`.
- No currently open pull request uses that branch as either head or base.

The API and branch conditions are rechecked before deletion. Git then applies an
explicit `--force-with-lease=refs/heads/NAME:EXPECTED_SHA` to the single deletion:
a commit pushed after inspection causes refusal instead of lost new work.
This is a conditional deletion, not an unrestricted force-push. See the official
[Git push documentation](https://git-scm.com/docs/git-push).

There is no distributed lock across GitHub PR metadata and Git refs. An open PR or
main rewrite concurrent with the final checks can still change metadata after the
snapshot. The exact branch-commit lease protects unmerged *new commits*, not every
possible metadata race. Normal integration should remain serialized.

## Conservative bounds and audit

The scan refuses more than 50 branches, more than 1,000 open PRs, or more than eight
deletions in one plan. All candidate planning completes before any deletion.
A branch with no qualifying recent merged PR is preserved, not guessed from age.
Each candidate is checked again before its individual deletion; a later failure
can leave a partially completed cleanup, but never reports complete success.

`main`, protected branches, tags, archive refs, helper/integrator branches, unrelated
names, unmerged branches and branches with new work remain untouched. Historical
PRs and commits are retained. No issue status, label, credential or production
resource is changed. Logs contain branch/PR/commit identities and fixed diagnostic
codes, not tokens or raw API error bodies. API redirects are refused.

Without `--apply`, the script is read-only. Apply additionally requires the explicit
repository and a `push` to `refs/heads/main` environment. In CI its only credential
is the job-scoped GitHub token. The real-Git regression uses a disposable local
bare repository and no network or credentials.

```sh
python3 -m unittest discover -s scripts -p test_cleanup_merged_branches.py -v
```

Passing policy tests are not evidence that cleanup already happened. Inspect the
actual main workflow and read back the branch list before reporting a clean repo.
