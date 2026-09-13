# Fair comparison cohorts

This document implements a bounded analysis primitive for [ARB-034 / #48](https://github.com/makafeli/arbitrage-research/issues/48). It is not a campaign result, provider qualification, simulation result, or claim that one chain is globally better.

`scripts/comparison_cohorts.py` consumes a pre-registered protocol and observation rows. It only compares `recorded-live` rows that match the same starting asset, capital, configured route sizes, scenario, time window and usable bucket. Synthetic and manually constructed fixtures are counted as excluded provenance and never enter market totals.

The protocol fixes an exact half-open time window `[start,end)`, a bucket width, minimum number of overlapping usable buckets, starting asset, capital, route sizes, scenario ID and `EXPLORATORY` or `HOLDOUT` phase. The canonical protocol digest changes when any of those assumptions changes. Changing assumptions therefore creates a new comparison identity rather than rewriting the old result.

A bucket is usable for a chain when at least one matching recorded-live observation in that bucket is explicitly usable. If observations exist but are unusable, the bucket remains a gap. If no observation exists, it is also a gap. Neither state becomes a zero-opportunity observation. A valid `NO_ROUTE` observation, by contrast, remains part of the usable denominator.

Ranking is permitted only when both Base and Solana have at least the protocol's minimum number of overlapping usable buckets. If overlap is insufficient, `ranking` is `null` with `INSUFFICIENT_OVERLAPPING_USABLE_COVERAGE`. The tool still returns chain-native descriptive views, but those are explicitly never ranked because their coverage may differ.

Quoted `net_minor` values are exact signed decimal strings. Negative values remain in totals. The tool does not infer USD parity, confidence intervals, fill probability, transaction execution, realized profit, provider completeness, or a global best/busiest chain. Those require the later scenario, simulation, campaign and review gates.

## Input shape

```json
{
  "schema_version": 1,
  "protocol": {
    "protocol_id": "holdout-2026-01",
    "version": "1",
    "window_start_ms": 1780000000000,
    "window_end_ms": 1780086400000,
    "bucket_ms": 3600000,
    "minimum_overlap_buckets": 20,
    "starting_asset": "USDC",
    "capital_minor": "1000000000",
    "route_sizes_minor": ["100000000"],
    "scenario_id": "delay-scenario-v1",
    "phase": "HOLDOUT"
  },
  "observations": []
}
```

Observation rows carry an ID, network, observation time, origin, outcome, usability/exclusion reason and the same comparison assumptions. `net_minor` is allowed only for `QUOTED` rows and remains hypothetical research economics. Up to 200,000 rows and 64 MiB input are accepted by the CLI.

Run:

```sh
python3 scripts/comparison_cohorts.py /private/comparison-input.json > /private/comparison-report.json
python3 scripts/test_comparison_cohorts.py -v
```

The report contains deterministic input/protocol SHA-256 digests, matched/native views, gap counts, exclusions and the explicit ranking gate. Treat both input and report as research evidence and retain them with the registered methodology. The CLI does not fetch data, call RPC, mutate PostgreSQL, start workers or validate that the caller supplied a complete campaign dataset.

## Remaining ARB-034 acceptance

This implementation establishes deterministic cohort matching and suppression behavior. Original #48 remains dependent on ARB-023/025/026/032/033 and still needs real campaign/export integration plus Product Owner methodology review. A green fixture suite cannot establish adequate real coverage, fair capital assumptions, holdout integrity or market profitability.
