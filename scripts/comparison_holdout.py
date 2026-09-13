#!/usr/bin/env python3
"""Check submitted exploratory/holdout partitions against caller-retained protocols."""
from __future__ import annotations

import argparse
import json
import re

import comparison_cohorts as cohorts

METHODOLOGY_FIELDS = (
    "version", "bucket_ms", "minimum_overlap_buckets", "starting_asset",
    "capital_minor", "route_sizes_minor", "scenario_id",
)


def check_partitions(exploratory: dict, holdout: dict,
                     expected_exploratory_digest: str, expected_holdout_digest: str) -> dict:
    """Structural checks only; digests supplied now cannot attest earlier registration."""
    for document, expected, phase in (
        (exploratory, expected_exploratory_digest, "EXPLORATORY"),
        (holdout, expected_holdout_digest, "HOLDOUT"),
    ):
        cohorts.validate(document)
        cohorts.require(isinstance(expected, str)
                        and re.fullmatch(r"sha256:[0-9a-f]{64}", expected) is not None,
                        "INVALID_RETAINED_PROTOCOL_DIGEST")
        p = document["protocol"]
        cohorts.require(p["phase"] == phase, "INCORRECT_PARTITION_PHASE")
        cohorts.require(cohorts.canonical_digest(p) == expected, "RETAINED_PROTOCOL_MISMATCH")
        # Unlike the descriptive cohort view, a partition checker must not
        # silently discard contaminated rows outside the declared window.
        cohorts.require(all(p["window_start_ms"] <= r["observed_at_ms"] < p["window_end_ms"]
                            for r in document["observations"]), "OBSERVATION_OUTSIDE_PARTITION")
    ep, hp = exploratory["protocol"], holdout["protocol"]
    cohorts.require(ep["window_end_ms"] <= hp["window_start_ms"], "OVERLAPPING_OR_REVERSED_PARTITIONS")
    cohorts.require(all(ep[k] == hp[k] for k in METHODOLOGY_FIELDS), "PARTITION_METHODOLOGY_MISMATCH")
    cohorts.require(len(exploratory["observations"]) + len(holdout["observations"])
                    <= cohorts.MAX_OBSERVATIONS, "COMBINED_OBSERVATION_LIMIT")
    seen = {r["observation_id"] for r in exploratory["observations"]}
    cohorts.require(not any(r["observation_id"] in seen for r in holdout["observations"]),
                    "REUSED_OBSERVATION_ID")
    reports = {"exploratory": cohorts.build_report(exploratory), "holdout": cohorts.build_report(holdout)}
    return {
        "schema_version": 1,
        "kind": "HOLDOUT_PARTITION_CHECK",
        "status": "STRUCTURALLY_SEPARATED",
        "methodology_digest": cohorts.canonical_digest({k: ep[k] for k in METHODOLOGY_FIELDS}),
        "retained_protocol_digests": {
            "exploratory": expected_exploratory_digest, "holdout": expected_holdout_digest,
        },
        "inter_partition_gap_ms": str(hp["window_start_ms"] - ep["window_end_ms"]),
        "both_coverage_thresholds_met": all(r["overlap"]["coverage_threshold_met"] for r in reports.values()),
        "partitions": reports,
        "ranking": None,
        "claims": {
            "submitted_windows_disjoint_and_ordered": True,
            "submitted_observation_ids_disjoint": True,
            "preregistration_time_attested": False,
            "timestamps_independently_verified": False,
            "renamed_or_omitted_observations_detectable": False,
            "holdout_integrity_verified": False,
            "campaign_completeness_verified": False,
            "market_profitability_established": False,
            "execution_authorized": False,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exploratory")
    parser.add_argument("holdout")
    parser.add_argument("--exploratory-protocol-sha256", required=True)
    parser.add_argument("--holdout-protocol-sha256", required=True)
    args = parser.parse_args()
    try:
        report = check_partitions(
            cohorts.load_document(args.exploratory), cohorts.load_document(args.holdout),
            args.exploratory_protocol_sha256, args.holdout_protocol_sha256,
        )
        print(json.dumps(report, sort_keys=True, ensure_ascii=True))
        return 0
    except (OSError, ValueError, TypeError, KeyError, RecursionError):
        print(json.dumps({"status": "HOLDOUT_CHECK_FAILED", "reason": "INVALID_OR_UNREADABLE_PARTITIONS"}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
