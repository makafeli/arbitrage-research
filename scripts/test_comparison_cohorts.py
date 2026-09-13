#!/usr/bin/env python3
"""Synthetic fixtures exercise contracts; recorded-live is only a test label here."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import comparison_cohorts as c


def document():
    p = {
        "protocol_id": "comparison-v1", "version": "1", "window_start_ms": 1000,
        "window_end_ms": 5000, "bucket_ms": 1000, "minimum_overlap_buckets": 2,
        "starting_asset": "USDC", "capital_minor": "1000000",
        "route_sizes_minor": ["100000"], "scenario_id": "scenario-a", "phase": "HOLDOUT",
    }
    rows = []
    for i, (network, t, outcome, usable, origin, net, reason) in enumerate([
        ("base-mainnet", 1100, "QUOTED", True, "recorded-live", "10", None),
        ("solana-mainnet", 1200, "QUOTED", True, "recorded-live", "5", None),
        ("base-mainnet", 2100, "NO_ROUTE", True, "recorded-live", None, None),
        ("solana-mainnet", 2200, "QUOTED", True, "recorded-live", "7", None),
        ("base-mainnet", 3100, "QUOTED", False, "recorded-live", "1", "PROVIDER_GAP"),
        ("solana-mainnet", 3200, "QUOTED", True, "recorded-live", "99", None),
        ("base-mainnet", 4100, "QUOTED", True, "synthetic", "1000", None),
        ("solana-mainnet", 4200, "QUOTED", True, "synthetic", "1000", None),
    ]):
        rows.append({
            "observation_id": f"o{i+1}", "network": network, "observed_at_ms": t,
            "origin": origin, "outcome": outcome, "usable": usable,
            "exclusion_reason": reason, "starting_asset": "USDC",
            "capital_minor": "1000000", "route_size_minor": "100000",
            "scenario_id": "scenario-a", "net_minor": net,
        })
    return {"schema_version": 1, "protocol": p, "observations": rows}


class ComparisonTests(unittest.TestCase):
    def test_matched_overlap_and_native_views_are_separate(self):
        report = c.build_report(document())
        self.assertTrue(report["overlap"]["coverage_threshold_met"])
        self.assertEqual(report["overlap"]["usable_buckets"], 2)
        self.assertEqual(report["matched_view"]["base-mainnet"]["quoted_net_minor_sum"], "10")
        self.assertEqual(report["matched_view"]["solana-mainnet"]["quoted_net_minor_sum"], "12")
        self.assertEqual(report["excluded"]["by_origin"], {"synthetic": 2})
        self.assertFalse(report["network_native_views"]["base-mainnet"]["native_view_ranked"])

    def test_unqualified_v1_never_ranks(self):
        report = c.build_report(document())
        self.assertFalse(report["overlap"]["ranking_allowed"])
        self.assertIsNone(report["ranking"])
        self.assertEqual(report["overlap"]["ranking_suppressed_reason"],
                         "UNVERIFIED_ASSET_UNITS_AND_OPPORTUNITY_DEDUPLICATION")

    def test_insufficient_overlap_suppresses_winner(self):
        value = document()
        value["protocol"]["minimum_overlap_buckets"] = 3
        report = c.build_report(value)
        self.assertFalse(report["overlap"]["ranking_allowed"])
        self.assertIsNone(report["ranking"])
        self.assertEqual(report["overlap"]["ranking_suppressed_reason"],
                         "INSUFFICIENT_OVERLAPPING_USABLE_COVERAGE")

    def test_unusable_and_missing_buckets_are_not_zero_opportunities(self):
        report = c.build_report(document())
        self.assertEqual(report["network_native_views"]["base-mainnet"]["gap_buckets"], 2)
        self.assertEqual(report["network_native_views"]["solana-mainnet"]["gap_buckets"], 1)
        self.assertEqual(report["matched_view"]["base-mainnet"]["outcome_counts"]["NO_ROUTE"], 1)

    def test_assumption_mismatch_is_excluded_not_compared(self):
        value = document()
        value["observations"][0]["capital_minor"] = "999"
        report = c.build_report(value)
        self.assertEqual(report["excluded"]["assumption_mismatch"], {"base-mainnet": 1})
        self.assertFalse(report["overlap"]["coverage_threshold_met"])

    def test_exploratory_and_holdout_have_distinct_protocol_hashes(self):
        first = document()
        second = copy.deepcopy(first)
        second["protocol"]["phase"] = "EXPLORATORY"
        self.assertNotEqual(c.build_report(first)["protocol_digest"],
                            c.build_report(second)["protocol_digest"])

    def test_synthetic_never_enters_market_totals_even_if_profitable(self):
        report = c.build_report(document())
        self.assertEqual(report["network_native_views"]["base-mainnet"]["native_quoted_net_minor_sum"], "10")
        self.assertFalse(report["claims"]["synthetic_in_market_results"])
        self.assertFalse(report["claims"]["execution_or_realized_profit"])

    def test_negative_results_are_retained(self):
        value = document()
        value["observations"][0]["net_minor"] = "-25"
        self.assertEqual(c.build_report(value)["matched_view"]["base-mainnet"]["quoted_net_minor_sum"], "-25")

    def test_missing_net_does_not_establish_usable_overlap(self):
        value = document()
        value["observations"][0]["net_minor"] = None
        report = c.build_report(value)
        self.assertEqual(report["overlap"]["usable_buckets"], 1)
        self.assertIsNone(report["network_native_views"]["base-mainnet"]["native_quoted_net_minor_sum"])
        self.assertEqual(report["excluded"]["unusable_reasons"]["base-mainnet"]["MISSING_NET_AMOUNT"], 1)

    def test_missing_net_taints_a_cell_with_a_known_quote(self):
        value = document()
        duplicate = dict(value["observations"][0], observation_id="unknown", net_minor=None)
        value["observations"].append(duplicate)
        report = c.build_report(value)
        self.assertEqual(report["overlap"]["usable_buckets"], 1)
        self.assertIsNone(report["network_native_views"]["base-mainnet"]["native_quoted_net_minor_sum"])

    def test_provider_gap_taints_a_cell_with_a_valid_quote(self):
        value = document()
        value["observations"].append(dict(value["observations"][0], observation_id="gap",
                                          usable=False, exclusion_reason="PROVIDER_GAP"))
        self.assertEqual(c.build_report(value)["overlap"]["usable_buckets"], 1)

    def test_route_sizes_must_match_in_every_bucket(self):
        value = document()
        value["protocol"]["route_sizes_minor"] = ["100000", "200000"]
        for row in value["observations"]:
            if row["network"] == "solana-mainnet":
                row["route_size_minor"] = "200000"
        self.assertEqual(c.build_report(value)["overlap"]["usable_buckets"], 0)

    def test_complete_multi_size_bucket_counts_once(self):
        value = document()
        value["protocol"]["route_sizes_minor"].append("200000")
        value["observations"] += [dict(r, observation_id=r["observation_id"] + "b", route_size_minor="200000")
                                   for r in value["observations"][:]]
        self.assertEqual(c.build_report(value)["overlap"]["usable_buckets"], 2)

    def test_repeated_samples_do_not_create_a_winner_or_unique_count(self):
        value = document()
        value["observations"] += [dict(value["observations"][0], observation_id=f"repeat{i}") for i in range(50)]
        report = c.build_report(value)
        self.assertIsNone(report["ranking"])
        self.assertIsNone(report["matched_view"]["base-mainnet"]["unique_opportunities"])
        self.assertEqual(report["overlap"]["usable_buckets"], 2)

    def test_coverage_span_is_not_measured_uptime(self):
        report = c.build_report(document())
        self.assertEqual(report["overlap"]["covered_bucket_span_ms"], "2000")
        self.assertFalse(report["claims"]["continuous_uptime_verified"])

    def test_exact_large_signed_amounts(self):
        value = document()
        value["observations"][0]["net_minor"] = str(-(2**256 - 1))
        self.assertEqual(c.build_report(value)["matched_view"]["base-mainnet"]["quoted_net_minor_sum"],
                         str(-(2**256 - 1)))

    def test_empty_dataset_has_gaps_not_a_zero_profit_winner(self):
        value = document()
        value["observations"] = []
        report = c.build_report(value)
        self.assertIsNone(report["ranking"])
        self.assertIsNone(report["matched_view"]["base-mainnet"]["quoted_net_minor_sum"])
        self.assertEqual(report["overlap"]["usable_buckets"], 0)

    def test_valid_no_route_is_a_zero_opportunity_observation(self):
        value = document()
        for row in value["observations"]:
            row.update(outcome="NO_ROUTE", net_minor=None)
        report = c.build_report(value)
        self.assertEqual(report["overlap"]["usable_buckets"], 2)
        self.assertEqual(report["matched_view"]["base-mainnet"]["quoted_net_minor_sum"], "0")

    def test_observation_order_does_not_change_metrics(self):
        first = document()
        second = copy.deepcopy(first)
        second["observations"].reverse()
        a, b = c.build_report(first), c.build_report(second)
        self.assertNotEqual(a.pop("input_digest"), b.pop("input_digest"))
        self.assertEqual(a, b)

    def test_half_open_window_boundary(self):
        value = document()
        value["observations"][0]["observed_at_ms"] = 5000
        report = c.build_report(value)
        self.assertEqual(report["excluded"]["outside_window"], {"base-mainnet": 1})
        self.assertEqual(report["overlap"]["usable_buckets"], 1)

    def test_invalid_inputs_fail_closed(self):
        mutations = [
            lambda v: v.update(schema_version=True),
            lambda v: v["protocol"].update(window_end_ms=4500),
            lambda v: v["protocol"].update(minimum_overlap_buckets=99),
            lambda v: v["protocol"].update(capital_minor=1),
            lambda v: v["protocol"].update(capital_minor="0"),
            lambda v: v["protocol"].update(route_sizes_minor=["0"]),
            lambda v: v["protocol"].update(route_sizes_minor=["2000000"]),
            lambda v: v["protocol"].update(capital_minor="9" * 79),
            lambda v: v["observations"][0].update(network="ethereum"),
            lambda v: v["observations"][0].update(usable=False, exclusion_reason=None),
            lambda v: v["observations"][0].update(usable=True, exclusion_reason="GAP"),
            lambda v: v["observations"][0].update(outcome="REJECTED", net_minor="1"),
            lambda v: v["observations"][0].update(outcome="DATA_UNAVAILABLE", net_minor=None),
            lambda v: v["observations"][1].update(observation_id="o1"),
            lambda v: v["observations"][0].update(net_minor="-0"),
        ]
        for mutate in mutations:
            value = document()
            mutate(value)
            with self.subTest(value=value), self.assertRaises(c.ComparisonError):
                c.build_report(value)

    def test_wrong_enum_and_net_types_raise_domain_error(self):
        for field in ("origin", "outcome", "network", "net_minor"):
            for invalid in ([], {}, True, 42):
                value = document()
                value["observations"][0][field] = invalid
                with self.subTest(field=field, invalid=invalid), self.assertRaises(c.ComparisonError):
                    c.build_report(value)
        value = document()
        value["protocol"]["phase"] = []
        with self.assertRaises(c.ComparisonError):
            c.build_report(value)

    def test_bucket_and_cell_budgets_before_allocation(self):
        value = document()
        value["protocol"].update(window_start_ms=0, window_end_ms=2**64 - 1, bucket_ms=1)
        with self.assertRaisesRegex(c.ComparisonError, "BUCKET_LIMIT"):
            c.build_report(value)
        value = document()
        with mock.patch.object(c, "MAX_CELLS", 3), self.assertRaisesRegex(c.ComparisonError, "CELL_LIMIT"):
            c.build_report(value)

    def test_observation_limit(self):
        with mock.patch.object(c, "MAX_OBSERVATIONS", 1), self.assertRaises(c.ComparisonError):
            c.build_report(document())

    def run_cli(self, path):
        return subprocess.run([sys.executable, c.__file__, str(path)],
                              capture_output=True, text=True, check=False, timeout=10)

    def test_deterministic_output_and_cli(self):
        value = document()
        self.assertEqual(c.build_report(value), c.build_report(copy.deepcopy(value)))
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "input.json"
            path.write_text(json.dumps(value))
            result = self.run_cli(path)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), c.build_report(value))
            path.write_text("not json")
            result = self.run_cli(path)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(json.loads(result.stdout)["status"], "COMPARISON_FAILED")

    def test_cli_rejects_duplicate_keys_and_nonfinite_json(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "input.json"
            for payload in ('{"schema_version":1,"schema_version":1}', 'NaN', 'Infinity', '\ufeff{}'):
                path.write_text(payload)
                result = self.run_cli(path)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stderr, "")

    def test_cli_rejects_deep_json_without_traceback_or_path(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "private-name.json"
            path.write_text("[" * 2000 + "]" * 2000)
            result = self.run_cli(path)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stderr, "")
            self.assertNotIn(td, result.stdout)

    def test_bounded_read_and_nonregular_input(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "input.json"
            path.write_bytes(b" " * 33)
            with mock.patch.object(c, "MAX_INPUT_BYTES", 32), self.assertRaises(c.ComparisonError):
                c.load_document(path)
            self.assertEqual(self.run_cli(Path(td)).returncode, 2)
            link = Path(td) / "symlink"
            link.symlink_to(path)
            self.assertEqual(self.run_cli(link).returncode, 2)
            fifo = Path(td) / "fifo"
            os.mkfifo(fifo)
            self.assertEqual(self.run_cli(fifo).returncode, 2)


if __name__ == "__main__":
    unittest.main()
