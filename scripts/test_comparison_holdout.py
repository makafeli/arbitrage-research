#!/usr/bin/env python3
"""Partition tests use synthetic documents, not actual held-out market data."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import comparison_cohorts as c
import comparison_holdout as h
from test_comparison_cohorts import document


def partitions():
    exploratory, holdout = document(), document()
    exploratory["protocol"]["phase"] = "EXPLORATORY"
    holdout["protocol"].update(window_start_ms=5000, window_end_ms=9000, protocol_id="later-v1")
    for row in holdout["observations"]:
        row["observed_at_ms"] += 4000
        row["observation_id"] += "h"
    return exploratory, holdout


def check(ep, hp):
    return h.check_partitions(ep, hp, c.canonical_digest(ep["protocol"]), c.canonical_digest(hp["protocol"]))


class HoldoutTests(unittest.TestCase):
    def test_ordered_disjoint_partitions_preserve_separate_results(self):
        ep, hp = partitions()
        report = check(ep, hp)
        self.assertEqual(report["status"], "STRUCTURALLY_SEPARATED")
        self.assertTrue(report["both_coverage_thresholds_met"])
        self.assertEqual(report["inter_partition_gap_ms"], "0")
        self.assertEqual(report["partitions"]["holdout"], c.build_report(hp))
        self.assertEqual(report["partitions"]["exploratory"], c.build_report(ep))
        self.assertIsNone(report["ranking"])
        for claim in ("preregistration_time_attested", "holdout_integrity_verified",
                      "market_profitability_established", "execution_authorized"):
            self.assertFalse(report["claims"][claim])

    def test_retained_digest_cannot_be_silently_recomputed(self):
        ep, hp = partitions()
        old_ep, old_hp = c.canonical_digest(ep["protocol"]), c.canonical_digest(hp["protocol"])
        hp["protocol"]["minimum_overlap_buckets"] = 3
        with self.assertRaisesRegex(c.ComparisonError, "RETAINED_PROTOCOL_MISMATCH"):
            h.check_partitions(ep, hp, old_ep, old_hp)

    def test_missing_or_malformed_retained_digest(self):
        ep, hp = partitions()
        for invalid in (None, [], "", "a" * 64, "sha256:" + "G" * 64):
            with self.subTest(invalid=invalid), self.assertRaises(c.ComparisonError):
                h.check_partitions(ep, hp, invalid, c.canonical_digest(hp["protocol"]))

    def test_overlapping_windows_fail_even_with_distinct_observation_ids(self):
        ep, hp = partitions()
        hp["protocol"].update(window_start_ms=4000, window_end_ms=9000)
        with self.assertRaisesRegex(c.ComparisonError, "OVERLAPPING_OR_REVERSED_PARTITIONS"):
            check(ep, hp)

    def test_reversed_time_windows_fail(self):
        ep, hp = partitions()
        ep["protocol"]["phase"], hp["protocol"]["phase"] = "HOLDOUT", "EXPLORATORY"
        with self.assertRaisesRegex(c.ComparisonError, "OVERLAPPING_OR_REVERSED_PARTITIONS"):
            check(hp, ep)

    def test_reused_ids_fail_even_when_timestamps_are_changed(self):
        ep, hp = partitions()
        hp["observations"][0]["observation_id"] = ep["observations"][0]["observation_id"]
        with self.assertRaisesRegex(c.ComparisonError, "REUSED_OBSERVATION_ID"):
            check(ep, hp)

    def test_outside_rows_are_not_silently_discarded(self):
        ep, hp = partitions()
        hp["observations"][0]["observed_at_ms"] = 4999
        with self.assertRaisesRegex(c.ComparisonError, "OBSERVATION_OUTSIDE_PARTITION"):
            check(ep, hp)

    def test_synthetic_rows_do_not_escape_partition_validation(self):
        ep, hp = partitions()
        hp["observations"][-1]["observed_at_ms"] = 9000
        with self.assertRaisesRegex(c.ComparisonError, "OBSERVATION_OUTSIDE_PARTITION"):
            check(ep, hp)

    def test_changed_methodology_fails(self):
        for field, value in (("scenario_id", "changed"), ("capital_minor", "2000000"),
                             ("route_sizes_minor", ["200000"]), ("bucket_ms", 2000),
                             ("minimum_overlap_buckets", 3), ("version", "2"),
                             ("starting_asset", "ETH")):
            ep, hp = partitions()
            hp["protocol"][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(c.ComparisonError, "PARTITION_METHODOLOGY_MISMATCH"):
                check(ep, hp)

    def test_incorrect_phase_fails(self):
        ep, hp = partitions()
        hp["protocol"]["phase"] = "EXPLORATORY"
        with self.assertRaisesRegex(c.ComparisonError, "INCORRECT_PARTITION_PHASE"):
            check(ep, hp)

    def test_insufficient_holdout_coverage_remains_explicit(self):
        ep, hp = partitions()
        hp["observations"] = []
        report = check(ep, hp)
        self.assertFalse(report["both_coverage_thresholds_met"])
        self.assertIsNone(report["ranking"])
        self.assertEqual(report["partitions"]["holdout"]["overlap"]["usable_buckets"], 0)

    def test_combined_observation_limit(self):
        ep, hp = partitions()
        with mock.patch.object(c, "MAX_OBSERVATIONS", 10), self.assertRaisesRegex(c.ComparisonError, "COMBINED_OBSERVATION_LIMIT"):
            check(ep, hp)

    def test_protocol_binding_does_not_claim_immutable_data_registration(self):
        ep, hp = partitions()
        first = check(ep, hp)
        hp["observations"][0]["net_minor"] = "999"
        second = check(ep, hp)
        self.assertEqual(first["retained_protocol_digests"], second["retained_protocol_digests"])
        self.assertNotEqual(first["partitions"]["holdout"]["input_digest"],
                            second["partitions"]["holdout"]["input_digest"])
        self.assertFalse(second["claims"]["holdout_integrity_verified"])

    def test_determinism_and_no_mutation(self):
        ep, hp = partitions()
        before = copy.deepcopy((ep, hp))
        self.assertEqual(check(ep, hp), check(*copy.deepcopy(before)))
        self.assertEqual(before, (ep, hp))

    def test_real_cli_success_and_redacted_failure(self):
        ep, hp = partitions()
        with tempfile.TemporaryDirectory() as td:
            ep_path, hp_path = Path(td) / "private-exploratory.json", Path(td) / "private-holdout.json"
            ep_path.write_text(json.dumps(ep))
            hp_path.write_text(json.dumps(hp))
            command = [sys.executable, h.__file__, str(ep_path), str(hp_path),
                       "--exploratory-protocol-sha256", c.canonical_digest(ep["protocol"]),
                       "--holdout-protocol-sha256", c.canonical_digest(hp["protocol"])]
            result = subprocess.run(command, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), check(ep, hp))
            hp_path.write_text("{not json")
            result = subprocess.run(command, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(json.loads(result.stdout)["status"], "HOLDOUT_CHECK_FAILED")
            self.assertEqual(result.stderr, "")
            self.assertNotIn(td, result.stdout)


if __name__ == "__main__":
    unittest.main()
