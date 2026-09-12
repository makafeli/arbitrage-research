#!/usr/bin/env python3
"""Offline regression tests for duplicate avoidance and preservation of operator edits."""
from copy import deepcopy
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import github_bootstrap as setup


class FakeBootstrap(setup.Bootstrap):
    def __init__(self, root, items):
        args = SimpleNamespace(repo="test-owner/test-repo", state=root / "state.json",
                               acknowledge_missing_create=[])
        super().__init__(args, {"epics": [], "tickets": items}, items)
        self.branch = "main"
        self.remote = []
        self.created = 0
        self.fail_create = None
        self.stale_listing = False
        self.fail_issue_read = False
        self.individual_reads = 0
        self.remote_milestones = [{"title": key, "number": i + 1} for i, key in enumerate(setup.MILESTONES)]

    def api(self, endpoint, method="GET", payload=None, paginate=False):
        if "/milestones?" in endpoint:
            return deepcopy(self.remote_milestones)
        if method == "GET" and "/issues?" in endpoint:
            return [] if self.stale_listing else deepcopy(self.remote)
        if method == "GET" and "/issues/" in endpoint:
            self.individual_reads += 1
            if self.fail_issue_read:
                raise setup.SetupError("simulated individual read failure")
            number = int(endpoint.rsplit("/", 1)[1])
            return deepcopy(next(x for x in self.remote if x["number"] == number))
        if method == "POST" and endpoint.endswith("/issues"):
            self.created += 1
            value = dict(payload, id=self.created, node_id=f"I_{self.created}", number=self.created,
                         html_url=f"https://github.com/test-owner/test-repo/issues/{self.created}", state="open")
            value["labels"] = [{"name": x} for x in payload["labels"]]
            value["milestone"] = {"number": payload["milestone"]}
            if self.fail_create != "not-applied":
                self.remote.append(value)
            if self.fail_create:
                raise setup.SetupError("simulated unknown response")
            return deepcopy(value)
        if method == "PATCH" and "/issues/" in endpoint:
            number = int(endpoint.rsplit("/", 1)[1])
            old = next(x for x in self.remote if x["number"] == number)
            old.update(payload)
            old["labels"] = [{"name": x} for x in payload["labels"]]
            old["milestone"] = {"number": payload["milestone"]}
            return deepcopy(old)
        raise AssertionError((endpoint, method))


class ImportTests(unittest.TestCase):
    def item(self):
        return {"id": "ARB-001", "title": "A test task", "body": "## Scope\n\nA meaningful task.",
                "labels": ["test"], "milestone": "M1", "deps": []}

    def test_rerun_does_not_duplicate_and_preserves_operator_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = FakeBootstrap(Path(tmp), [self.item()])
            app.issues_phase()
            app.remote[0]["body"] += "\nOperator note: preserve me.\n"
            app.remote[0]["labels"].append({"name": "triaged"})
            app.remote[0]["state"] = "closed"
            app.issues_phase()
            self.assertEqual(app.created, 1)
            self.assertEqual(app.remote[0]["state"], "closed")
            self.assertIn("preserve me", app.remote[0]["body"])
            self.assertIn({"name": "triaged"}, app.remote[0]["labels"])

    def test_stale_collection_after_creation_uses_verified_individual_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = FakeBootstrap(Path(tmp), [self.item()])
            app.issues_phase()
            app.stale_listing = True
            result = app.relationships()
            self.assertEqual(result["relationships_added"], 0)
            self.assertEqual(app.created, 1)
            self.assertEqual(app.individual_reads, 1)
            self.assertIn("ARB-001", app.state["issues"])

    def test_stale_collection_after_restart_does_not_create_duplicate(self):
        with tempfile.TemporaryDirectory() as tmp:
            original = FakeBootstrap(Path(tmp), [self.item()])
            original.issues_phase()
            restarted = FakeBootstrap(Path(tmp), [self.item()])
            restarted.remote = deepcopy(original.remote)
            restarted.stale_listing = True
            restarted.issues_phase()
            self.assertEqual(restarted.created, 0)
            self.assertEqual(restarted.individual_reads, 1)
            self.assertIn("ARB-001", restarted.state["issues"])

    def test_failed_individual_read_retains_saved_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = FakeBootstrap(Path(tmp), [self.item()])
            app.issues_phase()
            before = app.state_path.read_text()
            app.stale_listing = True
            app.fail_issue_read = True
            with self.assertRaises(setup.SetupError):
                app.issues_phase()
            self.assertEqual(app.state_path.read_text(), before)
            self.assertEqual(app.created, 1)
            self.assertIn("ARB-001", app.state["issues"])

    def test_individual_read_with_changed_marker_stops_import(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = FakeBootstrap(Path(tmp), [self.item()])
            app.issues_phase()
            before = app.state_path.read_text()
            app.remote[0]["body"] = "Marker removed by an operator"
            app.stale_listing = True
            with self.assertRaises(setup.SetupError):
                app.issues_phase()
            self.assertEqual(app.state_path.read_text(), before)
            self.assertEqual(app.created, 1)

    def test_uncertain_applied_create_is_discovered(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = FakeBootstrap(Path(tmp), [self.item()])
            app.fail_create = "applied"
            app.issues_phase()
            self.assertEqual(app.created, 1)
            self.assertEqual(app.state["pending"], {})

    def test_uncertain_absent_create_is_not_retried(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = FakeBootstrap(Path(tmp), [self.item()])
            app.fail_create = "not-applied"
            with self.assertRaises(setup.SetupError):
                app.issues_phase()
            app.fail_create = None
            with self.assertRaises(setup.SetupError):
                app.issues_phase()
            self.assertEqual(app.created, 1)
            self.assertIn("issue:ARB-001", app.state["pending"])

    def test_duplicate_remote_markers_abort(self):
        issue = {"body": "<!-- arb-ticket:ARB-001 -->"}
        with self.assertRaises(setup.SetupError):
            setup.issue_index([issue, issue])

    def test_managed_block_updates_only_managed_content(self):
        old = "Intro\n<!-- arb-ticket:ARB-001 -->\n" + setup.START + "\nold\n" + setup.END + "\nOperator note"
        new = "<!-- arb-ticket:ARB-001 -->\n" + setup.START + "\nnew\n" + setup.END
        merged = setup.merge_body(old, new)
        self.assertTrue(merged.startswith("Intro\n"))
        self.assertTrue(merged.endswith("Operator note"))
        self.assertIn("\nnew\n", merged)
        self.assertNotIn("\nold\n", merged)

    def test_wiki_links_keep_native_and_repo_destinations(self):
        content = "[Page](./Runbook.md#stop) [PRD](../docs/01-PRD.md) [Web](https://example.com/a.md)"
        output = setup.render_wiki(content, "owner/repo", "release/test")
        self.assertIn("https://github.com/owner/repo/wiki/Runbook#stop", output)
        self.assertIn("https://github.com/owner/repo/blob/release%2Ftest/docs/01-PRD.md", output)
        self.assertIn("https://example.com/a.md", output)

    def test_invalid_dependency_cycle_rejected(self):
        import json
        first = self.item()
        second = dict(first, id="ARB-002", dependencies=["ARB-001"])
        first["dependencies"] = ["ARB-002"]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "backlog.json"
            path.write_text(json.dumps({"epics": [], "tickets": [first, second]}))
            with self.assertRaises(setup.SetupError):
                setup.load_backlog(path)

    def test_command_arguments_are_not_shell_executed(self):
        with patch.object(setup.subprocess, "run") as execute:
            execute.return_value = SimpleNamespace(returncode=0, stdout="ok", stderr="")
            setup.run(["gh", "api", "repos/test/repo"], data='{"body":"`echo dangerous` $(echo no)"}')
            self.assertFalse(execute.call_args.kwargs.get("shell", False))
            self.assertIn("`echo dangerous`", execute.call_args.kwargs["input"])

if __name__ == "__main__":
    unittest.main()
