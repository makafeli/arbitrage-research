"""Policy tests and actual local-Git lease checks; never contact GitHub."""
import copy
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import cleanup_merged_branches as cleanup

HEAD = "a" * 40
MERGE = "b" * 40
BRANCH = "feat/ARB-016-bounded-reconnect"


def pull():
    return {"number": 136, "state": "closed", "merged": True, "merged_at": "2026-09-16",
            "merge_commit_sha": MERGE,
            "head": {"ref": BRANCH, "sha": HEAD, "repo": {"full_name": cleanup.REPOSITORY}},
            "base": {"ref": "main", "repo": {"full_name": cleanup.REPOSITORY}}}


def branch():
    return {"name": BRANCH, "protected": False, "commit": {"sha": HEAD}}


class FakeApi:
    token = "SYNTHETIC_TEST_TOKEN"

    def __init__(self):
        self.pr = pull()
        self.branch = branch()
        self.open = []
        self.comparison = {"status": "ahead"}
        self.branches = [self.branch]
        self.calls = []

    def get(self, path, optional=False):
        self.calls.append(path)
        if path == "/branches?per_page=100":
            return self.branches
        if path.startswith("/branches/"):
            return self.branch
        if path.startswith("/pulls?state=open"):
            return self.open
        if path.startswith("/pulls?state=closed"):
            return [{"number": 136}]
        if path == "/pulls/136":
            return self.pr
        if path == "/git/ref/heads/main":
            return {"object": {"sha": "c" * 40}}
        if path.startswith("/compare/"):
            return self.comparison
        raise AssertionError(path)


class PolicyTests(unittest.TestCase):
    def test_only_canonical_arb_work_branches(self):
        for value in (BRANCH, "fix/ARB-044-merged-branch-cleanup", "docs/ARB-016-readme"):
            self.assertTrue(cleanup.safe_name(value))
        for value in ("main", "archive/old", "refs/tags/v1", "integrator/source", None,
                      "feat/ARB-016-../main", "fix/ARB-044-x.lock", "fix/ARB-044-x.",
                      "fix/ARB-044-x\nmain", "fix/ARB-044-$(id)", "fix/ARB-044-x/y"):
            self.assertFalse(cleanup.safe_name(value))

    def test_merged_same_repository_main_is_required(self):
        self.assertIsNotNone(cleanup.candidate(pull()))
        for key, value in (("merged", False), ("merged", "true"), ("state", "open"),
                           ("merged_at", None), ("number", True), ("merge_commit_sha", "bad")):
            pr = pull()
            pr[key] = value
            self.assertIsNone(cleanup.candidate(pr))
        for side in ("head", "base"):
            pr = pull()
            pr[side]["repo"]["full_name"] = "other/repository"
            self.assertIsNone(cleanup.candidate(pr))
        pr = pull()
        pr["base"]["ref"] = "develop"
        self.assertIsNone(cleanup.candidate(pr))

    def test_changed_and_protected_branches_are_preserved(self):
        item = cleanup.candidate(pull())
        self.assertTrue(cleanup.unchanged(item, branch()))
        for record in (None, {}, {"name": BRANCH, "protected": True, "commit": {"sha": HEAD}},
                       {"name": BRANCH, "protected": False, "commit": {"sha": "d" * 40}}):
            self.assertFalse(cleanup.unchanged(item, record))

    def test_open_head_and_stacked_base_are_preserved(self):
        item = cleanup.candidate(pull())
        for side in ("head", "base"):
            pr = {side: {"ref": BRANCH, "repo": {"full_name": cleanup.REPOSITORY}}}
            self.assertTrue(cleanup.referenced(item, [pr]))
            pr[side]["repo"]["full_name"] = "other/repository"
            self.assertFalse(cleanup.referenced(item, [pr]))

    def test_plan_checks_current_main_history(self):
        api = FakeApi()
        self.assertEqual(cleanup.plan(api), [cleanup.candidate(pull())])
        for status in ("behind", "diverged", None):
            api.comparison = {"status": status}
            self.assertEqual(cleanup.plan(api), [])

    def test_collection_requires_individual_merged_read(self):
        api = FakeApi()
        api.pr.pop("merged")
        self.assertEqual(cleanup.plan(api), [])
        self.assertIn("/pulls/136", api.calls)

    def test_plan_refuses_incomplete_or_oversized_lists(self):
        api = FakeApi()
        api.branches = [branch()] * (cleanup.MAX_BRANCHES + 1)
        with self.assertRaises(cleanup.CleanupError):
            cleanup.plan(api)
        api = FakeApi()
        api.open = [pull()] * 100
        with self.assertRaises(cleanup.CleanupError):
            cleanup.open_pulls(api)
        self.assertEqual(len(api.calls), cleanup.MAX_OPEN_PAGES)

    @patch.object(cleanup, "delete_with_lease")
    def test_dry_run_never_deletes(self, delete):
        api = FakeApi()
        result = cleanup.execute(api, cleanup.plan(api), False)
        self.assertEqual(result[0]["status"], "DRY_RUN")
        delete.assert_not_called()

    @patch.object(cleanup, "delete_with_lease")
    def test_recheck_preserves_new_commits_and_open_references(self, delete):
        for mode in ("head", "reference", "pr"):
            api = FakeApi()
            items = cleanup.plan(api)
            if mode == "head":
                api.branch["commit"]["sha"] = "d" * 40
            elif mode == "reference":
                api.open = [pull()]
            else:
                api.pr["merged"] = False
            result = cleanup.execute(api, items, True)
            self.assertEqual(result[0]["status"], "PRESERVED_CHANGED_OR_REFERENCED")
        delete.assert_not_called()

    @patch.object(cleanup, "delete_with_lease")
    def test_already_removed_is_an_idempotent_noop(self, delete):
        api = FakeApi()
        api.branch = None
        result = cleanup.execute(api, [cleanup.candidate(pull())], True)
        self.assertEqual(result[0]["status"], "ALREADY_ABSENT")
        delete.assert_not_called()

    @patch.object(cleanup, "delete_with_lease")
    def test_apply_passes_only_the_verified_expected_head(self, delete):
        api = FakeApi()
        result = cleanup.execute(api, cleanup.plan(api), True)
        self.assertEqual(result[0]["status"], "DELETED")
        delete.assert_called_once_with(BRANCH, HEAD, token=api.token)


class GitLeaseTests(unittest.TestCase):
    def test_actual_remote_lease_deletes_only_an_unchanged_branch(self):
        with tempfile.TemporaryDirectory() as root:
            remote = str(Path(root) / "remote.git")
            local = str(Path(root) / "local")

            def git(*args, cwd=None):
                return subprocess.check_output(["git", *args], cwd=cwd, stderr=subprocess.DEVNULL,
                                               text=True, timeout=15).strip()

            git("init", "--bare", "--quiet", remote)
            git("init", "--quiet", local)
            git("config", "user.name", "Synthetic Test", cwd=local)
            git("config", "user.email", "test@example.invalid", cwd=local)
            git("commit", "--allow-empty", "--quiet", "-m", "first", cwd=local)
            original = git("rev-parse", "HEAD", cwd=local)
            git("push", remote, f"HEAD:refs/heads/{BRANCH}", cwd=local)
            git("commit", "--allow-empty", "--quiet", "-m", "new work", cwd=local)
            newer = git("rev-parse", "HEAD", cwd=local)
            git("push", remote, f"HEAD:refs/heads/{BRANCH}", cwd=local)
            with self.assertRaises(cleanup.CleanupError):
                cleanup.delete_with_lease(BRANCH, original, remote)
            self.assertEqual(git("--git-dir", remote, "rev-parse", f"refs/heads/{BRANCH}"), newer)
            cleanup.delete_with_lease(BRANCH, newer, remote)
            self.assertEqual(git("ls-remote", "--heads", remote, BRANCH), "")


if __name__ == "__main__":
    unittest.main()
