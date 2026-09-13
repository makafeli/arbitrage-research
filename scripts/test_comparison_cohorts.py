#!/usr/bin/env python3
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import comparison_cohorts as c


def document():
    protocol = {"protocol_id": "comparison-v1", "version": "1", "window_start_ms": 1000, "window_end_ms": 5000, "bucket_ms": 1000, "minimum_overlap_buckets": 2, "starting_asset": "USDC", "capital_minor": "1000000", "route_sizes_minor": ["100000"], "scenario_id": "scenario-a", "phase": "HOLDOUT"}
    rows = []
    def add(i, network, t, outcome="QUOTED", usable=True, origin="recorded-live", net="1", reason=None, **changes):
        row = {"observation_id": f"o{i}", "network": network, "observed_at_ms": t, "origin": origin, "outcome": outcome, "usable": usable, "exclusion_reason": reason, "starting_asset": "USDC", "capital_minor": "1000000", "route_size_minor": "100000", "scenario_id": "scenario-a", "net_minor": net if outcome == "QUOTED" else None}
        row.update(changes); rows.append(row)
    add(1, "base-mainnet", 1100, net="10"); add(2, "solana-mainnet", 1200, net="5")
    add(3, "base-mainnet", 2100, outcome="NO_ROUTE", net=None); add(4, "solana-mainnet", 2200, net="7")
    add(5, "base-mainnet", 3100, usable=False, reason="PROVIDER_GAP"); add(6, "solana-mainnet", 3200, net="99")
    add(7, "base-mainnet", 4100, net="1000", origin="synthetic"); add(8, "solana-mainnet", 4200, net="1000", origin="synthetic")
    return {"schema_version": 1, "protocol": protocol, "observations": rows}


class ComparisonTests(unittest.TestCase):
    def test_matched_overlap_and_native_views_are_separate(self):
        report = c.build_report(document())
        self.assertTrue(report["overlap"]["ranking_allowed"])
        self.assertEqual(report["overlap"]["usable_buckets"], 2)
        self.assertEqual(report["matched_view"]["base-mainnet"]["quoted_net_minor_sum"], "10")
        self.assertEqual(report["matched_view"]["solana-mainnet"]["quoted_net_minor_sum"], "12")
        self.assertEqual(report["ranking"], ["solana-mainnet", "base-mainnet"])
        self.assertEqual(report["excluded"]["by_origin"], {"synthetic": 2})
        self.assertFalse(report["network_native_views"]["base-mainnet"]["native_view_ranked"])

    def test_insufficient_overlap_suppresses_winner(self):
        value = document(); value["protocol"]["minimum_overlap_buckets"] = 3
        report = c.build_report(value)
        self.assertFalse(report["overlap"]["ranking_allowed"])
        self.assertIsNone(report["ranking"])
        self.assertEqual(report["overlap"]["ranking_suppressed_reason"], "INSUFFICIENT_OVERLAPPING_USABLE_COVERAGE")

    def test_unusable_and_missing_buckets_are_not_zero_opportunities(self):
        report = c.build_report(document())
        self.assertEqual(report["network_native_views"]["base-mainnet"]["gap_buckets"], 2)
        self.assertEqual(report["network_native_views"]["solana-mainnet"]["gap_buckets"], 1)
        self.assertEqual(report["matched_view"]["base-mainnet"]["outcome_counts"]["NO_ROUTE"], 1)

    def test_assumption_mismatch_is_excluded_not_compared(self):
        value = document(); value["observations"][0]["capital_minor"] = "999"
        report = c.build_report(value)
        self.assertEqual(report["excluded"]["assumption_mismatch"], {"base-mainnet": 1})
        self.assertFalse(report["overlap"]["ranking_allowed"])

    def test_exploratory_and_holdout_have_distinct_protocol_hashes(self):
        first = document(); second = copy.deepcopy(first); second["protocol"]["phase"] = "EXPLORATORY"
        self.assertNotEqual(c.build_report(first)["protocol_digest"], c.build_report(second)["protocol_digest"])

    def test_synthetic_never_enters_market_totals_even_if_profitable(self):
        report = c.build_report(document())
        self.assertEqual(report["network_native_views"]["base-mainnet"]["native_quoted_net_minor_sum"], "10")
        self.assertFalse(report["claims"]["synthetic_in_market_results"])
        self.assertFalse(report["claims"]["execution_or_realized_profit"])

    def test_negative_results_are_retained(self):
        value = document(); value["observations"][0]["net_minor"] = "-25"
        report = c.build_report(value)
        self.assertEqual(report["matched_view"]["base-mainnet"]["quoted_net_minor_sum"], "-25")

    def test_invalid_inputs_fail_closed(self):
        mutations = [
            lambda v: v.update(schema_version=True),
            lambda v: v["protocol"].update(window_end_ms=4500),
            lambda v: v["protocol"].update(minimum_overlap_buckets=99),
            lambda v: v["protocol"].update(capital_minor=1),
            lambda v: v["observations"][0].update(network="ethereum"),
            lambda v: v["observations"][0].update(usable=False, exclusion_reason=None),
            lambda v: v["observations"][0].update(outcome="REJECTED", net_minor="1"),
            lambda v: v["observations"][1].update(observation_id="o1"),
        ]
        for mutate in mutations:
            value = document(); mutate(value)
            with self.assertRaises(c.ComparisonError): c.build_report(value)

    def test_deterministic_output_and_cli(self):
        value = document(); self.assertEqual(c.build_report(value), c.build_report(copy.deepcopy(value)))
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "input.json"; path.write_text(json.dumps(value))
            result = subprocess.run([sys.executable, str(Path(c.__file__).resolve()), str(path)], capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["kind"], "FAIR_COMPARISON_COHORT")
            path.write_text("not json")
            result = subprocess.run([sys.executable, str(Path(c.__file__).resolve()), str(path)], capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(json.loads(result.stdout)["status"], "COMPARISON_FAILED")


if __name__ == "__main__": unittest.main()
