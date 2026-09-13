#!/usr/bin/env python3
"""Build bounded descriptive cohorts; v1 inputs cannot establish economic rankings."""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import re
import stat
from typing import Any

NETWORKS = ("base-mainnet", "solana-mainnet")
ORIGINS = {"recorded-live", "synthetic", "manually-constructed"}
OUTCOMES = {"QUOTED", "REJECTED", "DATA_UNAVAILABLE", "NO_ROUTE"}
PHASES = {"EXPLORATORY", "HOLDOUT"}
MAX_OBSERVATIONS = 200_000
MAX_INPUT_BYTES = 64 * 1024 * 1024
MAX_BUCKETS = 100_000
MAX_CELLS = 200_000  # Per network: bucket count times configured route sizes.
MAX_AMOUNT = 2**256 - 1
PROTOCOL_FIELDS = {
    "protocol_id", "version", "window_start_ms", "window_end_ms", "bucket_ms",
    "minimum_overlap_buckets", "starting_asset", "capital_minor",
    "route_sizes_minor", "scenario_id", "phase",
}
OBSERVATION_FIELDS = {
    "observation_id", "network", "observed_at_ms", "origin", "outcome", "usable",
    "exclusion_reason", "starting_asset", "capital_minor", "route_size_minor",
    "scenario_id", "net_minor",
}


class ComparisonError(ValueError):
    """A fixed, non-sensitive validation reason."""


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ComparisonError(reason)


def label(value: Any, limit: int = 128) -> bool:
    return (isinstance(value, str) and 1 <= len(value) <= limit
            and re.fullmatch(r"[A-Za-z0-9_.:-]+", value) is not None)


def enum(value: Any, choices: Any) -> bool:
    return isinstance(value, str) and value in choices


def u64(value: Any) -> bool:
    return type(value) is int and 0 <= value <= 2**64 - 1


def amount(value: Any, signed: bool = False) -> bool:
    if not isinstance(value, str) or len(value) > 79:
        return False
    pattern = r"0|-?[1-9][0-9]*" if signed else r"0|[1-9][0-9]*"
    return re.fullmatch(pattern, value) is not None and abs(int(value)) <= MAX_AMOUNT


def canonical_digest(value: Any) -> str:
    raw = json.dumps(value, sort_keys=True, separators=(",", ":"),
                     ensure_ascii=True, allow_nan=False).encode()
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def validate(document: Any) -> dict:
    require(isinstance(document, dict), "INVALID_DOCUMENT")
    require(set(document) == {"schema_version", "protocol", "observations"}, "INVALID_DOCUMENT_FIELDS")
    require(type(document["schema_version"]) is int and document["schema_version"] == 1,
            "UNSUPPORTED_SCHEMA")
    p = document["protocol"]
    require(isinstance(p, dict) and set(p) == PROTOCOL_FIELDS, "INVALID_PROTOCOL")
    require(all(label(p[k]) for k in ("protocol_id", "version", "starting_asset", "scenario_id")),
            "INVALID_PROTOCOL_IDENTITY")
    require(enum(p["phase"], PHASES), "INVALID_PHASE")
    start, end, width = p["window_start_ms"], p["window_end_ms"], p["bucket_ms"]
    require(u64(start) and u64(end) and u64(width) and start < end and width > 0
            and (end - start) % width == 0, "INVALID_WINDOW")
    buckets = (end - start) // width
    require(buckets <= MAX_BUCKETS, "BUCKET_LIMIT")
    require(type(p["minimum_overlap_buckets"]) is int
            and 1 <= p["minimum_overlap_buckets"] <= buckets, "INVALID_OVERLAP_THRESHOLD")
    require(amount(p["capital_minor"]) and int(p["capital_minor"]) > 0, "INVALID_CAPITAL")
    sizes = p["route_sizes_minor"]
    require(isinstance(sizes, list) and 1 <= len(sizes) <= 64
            and all(amount(v) and 0 < int(v) <= int(p["capital_minor"]) for v in sizes)
            and len(set(sizes)) == len(sizes), "INVALID_ROUTE_SIZES")
    require(buckets * len(sizes) <= MAX_CELLS, "CELL_LIMIT")
    rows = document["observations"]
    require(isinstance(rows, list) and len(rows) <= MAX_OBSERVATIONS, "OBSERVATION_LIMIT")
    seen = set()
    for row in rows:
        require(isinstance(row, dict) and set(row) == OBSERVATION_FIELDS, "INVALID_OBSERVATION")
        require(label(row["observation_id"]) and row["observation_id"] not in seen,
                "INVALID_OBSERVATION_ID")
        seen.add(row["observation_id"])
        require(enum(row["network"], NETWORKS) and u64(row["observed_at_ms"]),
                "INVALID_OBSERVATION_CONTEXT")
        require(enum(row["origin"], ORIGINS) and enum(row["outcome"], OUTCOMES)
                and type(row["usable"]) is bool, "INVALID_OBSERVATION_STATE")
        require(row["exclusion_reason"] is None or label(row["exclusion_reason"]),
                "INVALID_EXCLUSION_REASON")
        require((row["usable"] and row["exclusion_reason"] is None)
                or (not row["usable"] and row["exclusion_reason"] is not None),
                "INCONSISTENT_USABILITY")
        require(not (row["usable"] and row["outcome"] == "DATA_UNAVAILABLE"),
                "UNAVAILABLE_CANNOT_BE_USABLE")
        require(label(row["starting_asset"]) and amount(row["capital_minor"])
                and int(row["capital_minor"]) > 0 and amount(row["route_size_minor"])
                and int(row["route_size_minor"]) > 0 and label(row["scenario_id"]),
                "INVALID_OBSERVATION_ASSUMPTIONS")
        require(row["net_minor"] is None or amount(row["net_minor"], signed=True), "INVALID_NET_AMOUNT")
        require(row["outcome"] == "QUOTED" or row["net_minor"] is None, "INVALID_NET_OUTCOME")
    return document


def unusable_reason(row: dict) -> str | None:
    if not row["usable"]:
        return row["exclusion_reason"]
    if row["outcome"] == "QUOTED" and row["net_minor"] is None:
        return "MISSING_NET_AMOUNT"
    return None


def summarize(rows: list[dict]) -> dict:
    """Amounts describe raw observations, never unique opportunities or profit."""
    quotes = [r for r in rows if r["outcome"] == "QUOTED"]
    missing = sum(r["net_minor"] is None for r in quotes)
    known_sum = str(sum(int(r["net_minor"]) for r in quotes if r["net_minor"] is not None))
    return {
        "observations": len(rows),
        "outcome_counts": dict(sorted(Counter(r["outcome"] for r in rows).items())),
        "quoted_net_minor_sum": None if missing or not rows else known_sum,
        "known_quoted_net_minor_sum": known_sum if rows else None,
        "missing_net_observations": missing,
        "unique_opportunities": None,
        "aggregation": "RAW_OBSERVATIONS_NOT_UNIQUE_OPPORTUNITIES_OR_PNL",
    }


def build_report(document: dict) -> dict:
    validate(document)
    p = document["protocol"]
    start, end, width = p["window_start_ms"], p["window_end_ms"], p["bucket_ms"]
    count = (end - start) // width
    sizes = set(p["route_sizes_minor"])
    rows_by_network = {n: [] for n in NETWORKS}
    cells = {n: defaultdict(list) for n in NETWORKS}
    excluded_origin, excluded_assumption, outside = Counter(), Counter(), Counter()
    unusable = {n: Counter() for n in NETWORKS}
    for row in document["observations"]:
        n = row["network"]
        if row["origin"] != "recorded-live":
            excluded_origin[row["origin"]] += 1
            continue
        if not start <= row["observed_at_ms"] < end:
            outside[n] += 1
            continue
        if (any(row[k] != p[k] for k in ("starting_asset", "capital_minor", "scenario_id"))
                or row["route_size_minor"] not in sizes):
            excluded_assumption[n] += 1
            continue
        rows_by_network[n].append(row)
        index = (row["observed_at_ms"] - start) // width
        cells[n][(index, row["route_size_minor"])].append(row)
        reason = unusable_reason(row)
        if reason is not None:
            unusable[n][reason] += 1

    usable_buckets = {n: set() for n in NETWORKS}
    native = {}
    for n in NETWORKS:
        missing_cells = 0
        # Every size is required. Any unusable observation taints its cell;
        # a later good sample must not erase a retained gap or unknown cost.
        for index in range(count):
            complete = True
            for size in sizes:
                cell = cells[n].get((index, size), ())
                if not cell:
                    missing_cells += 1
                if not cell or any(unusable_reason(r) is not None for r in cell):
                    complete = False
            if complete:
                usable_buckets[n].add(index)
        declared_usable = [r for r in rows_by_network[n] if r["usable"]]
        stats = summarize(declared_usable)
        native[n] = {
            "usable_buckets": len(usable_buckets[n]),
            "gap_buckets": count - len(usable_buckets[n]),
            "covered_bucket_span_ms": str(len(usable_buckets[n]) * width),
            "missing_route_size_cells": missing_cells,
            "usable_observations": sum(unusable_reason(r) is None for r in rows_by_network[n]),
            "native_outcome_counts": stats["outcome_counts"],
            "native_quoted_net_minor_sum": stats["quoted_net_minor_sum"],
            "native_known_quoted_net_minor_sum": stats["known_quoted_net_minor_sum"],
            "missing_net_observations": stats["missing_net_observations"],
            "unique_opportunities": None,
            "native_view_ranked": False,
            "aggregation": stats["aggregation"],
        }
    overlap = usable_buckets[NETWORKS[0]] & usable_buckets[NETWORKS[1]]
    matched = {n: summarize([r for r in rows_by_network[n]
                            if (r["observed_at_ms"] - start) // width in overlap]) for n in NETWORKS}
    enough = len(overlap) >= p["minimum_overlap_buckets"]
    # A shared label does not establish canonical units, and observation_id is
    # not an opportunity-episode identity. Never manufacture a winner from v1.
    reason = ("UNVERIFIED_ASSET_UNITS_AND_OPPORTUNITY_DEDUPLICATION" if enough
              else "INSUFFICIENT_OVERLAPPING_USABLE_COVERAGE")
    return {
        "schema_version": 2,
        "input_schema_version": 1,
        "kind": "FAIR_COMPARISON_COHORT",
        "protocol_digest": canonical_digest(p),
        "input_digest": canonical_digest(document),
        "phase": p["phase"],
        "window": {"start_ms": str(start), "end_ms": str(end), "bucket_ms": str(width),
                   "total_buckets": count},
        "matched_assumptions": {k: p[k] for k in ("starting_asset", "capital_minor",
                                                   "route_sizes_minor", "scenario_id")},
        "coverage_measure": "ALL_ROUTE_SIZES_PER_BUCKET_NOT_CONTINUOUS_UPTIME",
        "network_native_views": native,
        "overlap": {"usable_buckets": len(overlap), "minimum_required": p["minimum_overlap_buckets"],
                    "covered_bucket_span_ms": str(len(overlap) * width),
                    "coverage_threshold_met": enough, "ranking_allowed": False,
                    "ranking_suppressed_reason": reason},
        "matched_view": matched,
        "ranking": None,
        "excluded": {"by_origin": dict(sorted(excluded_origin.items())),
                     "assumption_mismatch": dict(sorted(excluded_assumption.items())),
                     "outside_window": dict(sorted(outside.items())),
                     "unusable_reasons": {n: dict(sorted(unusable[n].items())) for n in NETWORKS}},
        "claims": {"synthetic_in_market_results": False, "global_best_chain": False,
                   "execution_or_realized_profit": False, "continuous_uptime_verified": False,
                   "canonical_asset_units_verified": False, "opportunity_deduplication_verified": False,
                   "holdout_integrity_verified": False, "campaign_completeness_verified": False},
    }


def _unique_keys(pairs: list[tuple[str, Any]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, "DUPLICATE_JSON_KEY")
        result[key] = value
    return result


def _reject_constant(_: str) -> None:
    raise ComparisonError("INVALID_JSON")


def load_document(path: str | Path) -> Any:
    # Nonblocking/no-follow prevents FIFO hangs and symlink surprises on POSIX.
    # Bound the actual read as well as fstat: a file can grow between the two.
    flags = os.O_RDONLY | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags)
    with os.fdopen(fd, "rb") as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode), "INPUT_NOT_REGULAR_FILE")
        require(info.st_size <= MAX_INPUT_BYTES, "INPUT_SIZE_LIMIT")
        raw = stream.read(MAX_INPUT_BYTES + 1)
    require(len(raw) <= MAX_INPUT_BYTES, "INPUT_SIZE_LIMIT")
    return json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_keys,
                      parse_constant=_reject_constant)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input")
    args = parser.parse_args()
    try:
        print(json.dumps(build_report(load_document(args.input)), sort_keys=True, ensure_ascii=True))
        return 0
    except (OSError, ValueError, TypeError, KeyError, RecursionError):
        print(json.dumps({"status": "COMPARISON_FAILED", "reason": "INVALID_OR_UNREADABLE_INPUT"}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
