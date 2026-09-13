#!/usr/bin/env python3
"""Build deterministic research comparison cohorts without inventing market evidence."""
from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
import hashlib
import json
from pathlib import Path
import re
from typing import Any

NETWORKS = ("base-mainnet", "solana-mainnet")
ORIGINS = {"recorded-live", "synthetic", "manually-constructed"}
OUTCOMES = {"QUOTED", "REJECTED", "DATA_UNAVAILABLE", "NO_ROUTE"}
PHASES = {"EXPLORATORY", "HOLDOUT"}
MAX_OBSERVATIONS = 200_000
MAX_INPUT_BYTES = 64 * 1024 * 1024


class ComparisonError(ValueError):
    pass


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ComparisonError(reason)


def label(value: Any, limit: int = 128) -> bool:
    return isinstance(value, str) and 1 <= len(value) <= limit and re.fullmatch(r"[A-Za-z0-9_.:-]+", value) is not None


def u64(value: Any) -> bool:
    return type(value) is int and 0 <= value <= 2**64 - 1


def amount(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]*", value) is not None


def canonical_digest(value: Any) -> str:
    raw = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def parse_decimal(value: Any, field: str) -> Decimal:
    require(isinstance(value, str), f"INVALID_{field}")
    try:
        result = Decimal(value)
    except InvalidOperation:
        raise ComparisonError(f"INVALID_{field}") from None
    require(result.is_finite(), f"INVALID_{field}")
    return result


def validate(document: Any) -> dict:
    require(isinstance(document, dict), "INVALID_DOCUMENT")
    require(set(document) == {"schema_version", "protocol", "observations"}, "INVALID_DOCUMENT_FIELDS")
    require(document["schema_version"] == 1 and type(document["schema_version"]) is int, "UNSUPPORTED_SCHEMA")
    protocol = document["protocol"]
    required = {"protocol_id", "version", "window_start_ms", "window_end_ms", "bucket_ms", "minimum_overlap_buckets", "starting_asset", "capital_minor", "route_sizes_minor", "scenario_id", "phase"}
    require(isinstance(protocol, dict) and set(protocol) == required, "INVALID_PROTOCOL")
    require(label(protocol["protocol_id"]) and label(protocol["version"]) and label(protocol["starting_asset"]) and label(protocol["scenario_id"]), "INVALID_PROTOCOL_IDENTITY")
    require(protocol["phase"] in PHASES, "INVALID_PHASE")
    start, end, bucket = protocol["window_start_ms"], protocol["window_end_ms"], protocol["bucket_ms"]
    require(u64(start) and u64(end) and u64(bucket) and start < end and bucket > 0 and (end - start) % bucket == 0, "INVALID_WINDOW")
    total_buckets = (end - start) // bucket
    require(type(protocol["minimum_overlap_buckets"]) is int and 1 <= protocol["minimum_overlap_buckets"] <= total_buckets, "INVALID_OVERLAP_THRESHOLD")
    require(amount(protocol["capital_minor"]), "INVALID_CAPITAL")
    sizes = protocol["route_sizes_minor"]
    require(isinstance(sizes, list) and 1 <= len(sizes) <= 64 and all(amount(v) for v in sizes) and len(set(sizes)) == len(sizes), "INVALID_ROUTE_SIZES")
    observations = document["observations"]
    require(isinstance(observations, list) and len(observations) <= MAX_OBSERVATIONS, "OBSERVATION_LIMIT")
    seen = set()
    fields = {"observation_id", "network", "observed_at_ms", "origin", "outcome", "usable", "exclusion_reason", "starting_asset", "capital_minor", "route_size_minor", "scenario_id", "net_minor"}
    for row in observations:
        require(isinstance(row, dict) and set(row) == fields, "INVALID_OBSERVATION")
        require(label(row["observation_id"]) and row["observation_id"] not in seen, "INVALID_OBSERVATION_ID")
        seen.add(row["observation_id"])
        require(row["network"] in NETWORKS and u64(row["observed_at_ms"]), "INVALID_OBSERVATION_CONTEXT")
        require(row["origin"] in ORIGINS and row["outcome"] in OUTCOMES and type(row["usable"]) is bool, "INVALID_OBSERVATION_STATE")
        require(row["exclusion_reason"] is None or label(row["exclusion_reason"]), "INVALID_EXCLUSION_REASON")
        require(label(row["starting_asset"]) and amount(row["capital_minor"]) and amount(row["route_size_minor"]) and label(row["scenario_id"]), "INVALID_OBSERVATION_ASSUMPTIONS")
        require(row["net_minor"] is None or re.fullmatch(r"-?(0|[1-9][0-9]*)", row["net_minor"]) is not None, "INVALID_NET_AMOUNT")
        require(row["outcome"] == "QUOTED" or row["net_minor"] is None, "INVALID_NET_OUTCOME")
        require(row["usable"] or row["exclusion_reason"] is not None, "MISSING_EXCLUSION_REASON")
    return document


@dataclass(frozen=True)
class BucketState:
    usable: bool
    reason: str | None


def build_report(document: dict) -> dict:
    validate(document)
    p = document["protocol"]
    start, end, step = p["window_start_ms"], p["window_end_ms"], p["bucket_ms"]
    bucket_starts = list(range(start, end, step))
    by_network = {network: [] for network in NETWORKS}
    excluded_origin = Counter()
    excluded_assumption = Counter()
    outside_window = Counter()
    for row in document["observations"]:
        if row["origin"] != "recorded-live":
            excluded_origin[row["origin"]] += 1
            continue
        if not (start <= row["observed_at_ms"] < end):
            outside_window[row["network"]] += 1
            continue
        if (row["starting_asset"] != p["starting_asset"] or row["capital_minor"] != p["capital_minor"] or row["route_size_minor"] not in p["route_sizes_minor"] or row["scenario_id"] != p["scenario_id"]):
            excluded_assumption[row["network"]] += 1
            continue
        by_network[row["network"]].append(row)

    network_reports = {}
    states = {}
    for network in NETWORKS:
        rows = by_network[network]
        buckets = {}
        for bucket_start in bucket_starts:
            selected = [r for r in rows if bucket_start <= r["observed_at_ms"] < bucket_start + step]
            usable = [r for r in selected if r["usable"]]
            if usable:
                buckets[bucket_start] = BucketState(True, None)
            elif selected:
                reasons = sorted({r["exclusion_reason"] or "UNUSABLE" for r in selected})
                buckets[bucket_start] = BucketState(False, "+".join(reasons))
            else:
                buckets[bucket_start] = BucketState(False, "NO_OBSERVATION")
        states[network] = buckets
        usable_rows = [r for r in rows if r["usable"]]
        native_quotes = [r for r in usable_rows if r["outcome"] == "QUOTED"]
        network_reports[network] = {
            "usable_buckets": sum(s.usable for s in buckets.values()),
            "gap_buckets": sum(not s.usable for s in buckets.values()),
            "usable_observations": len(usable_rows),
            "native_outcome_counts": dict(sorted(Counter(r["outcome"] for r in usable_rows).items())),
            "native_quoted_net_minor_sum": str(sum(int(r["net_minor"]) for r in native_quotes if r["net_minor"] is not None)),
            "native_view_ranked": False,
        }

    overlap = [b for b in bucket_starts if all(states[n][b].usable for n in NETWORKS)]
    matched_rows = {n: [r for r in by_network[n] if r["usable"] and (r["observed_at_ms"] - start) // step * step + start in overlap] for n in NETWORKS}
    matched = {}
    for network in NETWORKS:
        rows = matched_rows[network]
        quotes = [r for r in rows if r["outcome"] == "QUOTED"]
        matched[network] = {
            "observations": len(rows),
            "outcome_counts": dict(sorted(Counter(r["outcome"] for r in rows).items())),
            "quoted_net_minor_sum": str(sum(int(r["net_minor"]) for r in quotes if r["net_minor"] is not None)),
        }
    rank_allowed = len(overlap) >= p["minimum_overlap_buckets"]
    ranking = None
    reason = None
    if rank_allowed:
        sums = {n: int(matched[n]["quoted_net_minor_sum"]) for n in NETWORKS}
        ranking = sorted(NETWORKS, key=lambda n: (-sums[n], n))
    else:
        reason = "INSUFFICIENT_OVERLAPPING_USABLE_COVERAGE"

    return {
        "schema_version": 1,
        "kind": "FAIR_COMPARISON_COHORT",
        "protocol_digest": canonical_digest(p),
        "input_digest": canonical_digest(document),
        "phase": p["phase"],
        "window": {"start_ms": str(start), "end_ms": str(end), "bucket_ms": str(step), "total_buckets": len(bucket_starts)},
        "matched_assumptions": {"starting_asset": p["starting_asset"], "capital_minor": p["capital_minor"], "route_sizes_minor": p["route_sizes_minor"], "scenario_id": p["scenario_id"]},
        "network_native_views": network_reports,
        "overlap": {"usable_buckets": len(overlap), "minimum_required": p["minimum_overlap_buckets"], "ranking_allowed": rank_allowed, "ranking_suppressed_reason": reason},
        "matched_view": matched,
        "ranking": ranking,
        "excluded": {"by_origin": dict(sorted(excluded_origin.items())), "assumption_mismatch": dict(sorted(excluded_assumption.items())), "outside_window": dict(sorted(outside_window.items()))},
        "claims": {"synthetic_in_market_results": False, "global_best_chain": False, "execution_or_realized_profit": False},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input")
    args = parser.parse_args()
    try:
        path = Path(args.input)
        require(path.stat().st_size <= MAX_INPUT_BYTES, "INPUT_SIZE_LIMIT")
        document = json.loads(path.read_text(encoding="utf-8"), parse_constant=lambda _: (_ for _ in ()).throw(ComparisonError("INVALID_JSON")))
        print(json.dumps(build_report(document), sort_keys=True, ensure_ascii=True))
        return 0
    except (OSError, ValueError, TypeError, KeyError, ComparisonError):
        print(json.dumps({"status": "COMPARISON_FAILED", "reason": "INVALID_OR_UNREADABLE_INPUT"}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
