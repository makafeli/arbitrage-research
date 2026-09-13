# Descriptive comparison cohorts and holdout partitions

Bounded research tooling for [ARB-034 / #48](https://github.com/makafeli/arbitrage-research/issues/48). These commands do not establish a campaign result, qualified provider, complete transaction simulation, opportunity profitability or a globally better chain.

## Resume checkpoint: 13 September 2026

Main was verified at `94d045cebcd0c4087302c2faefdf13913300c698`. PR #106 previously ended at `f3676f46620ce7ac80a3d84c58ceb4cd614656cb` with nine passing comparison tests and green CI. Source review and newly failing regressions nevertheless exposed missing-net, route-size matching, duplicate-sampling and malformed-type defects. The continuation keeps the existing issue/branch rather than creating a duplicate delivery ticket.

The other open PRs at inspection were #105 (capture dependency audit), #86 (base64) and #87 (sha2). #105 had passing CI but an unfulfilled external-review gate; its CodeRabbit status represented a rate-limit notice, not approval. Its GOAL.md correction already covers the provisioned Railway foundation and recovery checkpoint, so this slice does not overwrite that claimed path. No deployment was changed or freshly verified.

## Cohort behavior

`scripts/comparison_cohorts.py` consumes input schema **1** and emits report schema **2**. This is an intentional correction to the unmerged prototype: do not interpret its former schema-1 ranking as accepted research evidence.

The protocol binds a half-open time window `[start,end)`, bucket width, minimum overlapping bucket count, starting-asset label, capital, route sizes, scenario and EXPLORATORY/HOLDOUT phase. Every protocol change produces a different canonical SHA-256 digest.

Only matching caller-declared `recorded-live` rows enter descriptive cohorts. Synthetic and manually constructed rows remain counted exclusions. The origin label and timestamps are declarations; the CLI does not authenticate a collector or verify actual mainnet observations.

A bucket qualifies only when **every configured route size** has matching observations for that network and none of those observations is unusable. Missing quoted net amounts make their cell unusable, even beside a valid quote. An explicit provider gap also taints its cell; later good sampling cannot erase retained missing evidence. A matching, usable NO_ROUTE remains a valid no-opportunity observation. Absent observations remain gaps, never zero-result evidence.

Covered bucket span is reported in exact millisecond strings. It is a sampling-based coverage measure, **not measured continuous uptime or usable market hours**. Coarse buckets may exclude substantial spans after one unusable row; choose and retain the bucket methodology before evaluation.

Matched views include only jointly qualifying buckets. Chain-native views describe their own matching submitted observations and are never ranked. Reports retain origin, assumption, window and unusability exclusions, missing route-size cells and missing-net counts. Negative amounts are retained exactly. Unknown quote totals are null, with a separately labelled known-value partial sum; a completely absent cohort is not a zero-profit observation.

### Why schema 1 cannot produce an economic winner

Its `starting_asset` is only a label, not a chain-qualified asset identity with verified decimal units or a valuation mapping. Its `observation_id` identifies a measurement, not a persistent opportunity episode. Repeated sampling can therefore inflate a raw quote sum. Neither symbol equality nor a larger number of observations establishes comparative profitability.

Report schema 2 always sets `ranking` to null and `ranking_allowed` to false. It separately reports `coverage_threshold_met`. Below the threshold the reason is `INSUFFICIENT_OVERLAPPING_USABLE_COVERAGE`; otherwise it is `UNVERIFIED_ASSET_UNITS_AND_OPPORTUNITY_DEDUPLICATION`. Raw observation counts/sums are not unique opportunity counts or P&L. `unique_opportunities` remains null. No asset conversion, deduplication or zero-cost assumption is invented.

## Input and limits

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

Rows require `observation_id`, `network`, `observed_at_ms`, `origin`, `outcome`, `usable`, `exclusion_reason`, `starting_asset`, `capital_minor`, `route_size_minor`, `scenario_id` and `net_minor`. Only QUOTED may carry a signed net amount; null remains unknown. DATA_UNAVAILABLE cannot be usable. A usable row has no exclusion reason; an unusable row must supply one.

Limits are 200,000 observations, 64 MiB actual input bytes, 100,000 buckets, 64 configured route sizes and 200,000 bucket-size cells per network. Amount magnitudes are bounded to 256-bit unsigned integers, represented as canonical decimal strings; capital and route sizes must be positive. Protocol sizes cannot exceed capital. Integer arithmetic retains exact sums beyond an individual amount's range.

The reader bounds both file metadata and the actual read. It rejects duplicate JSON keys, non-finite JSON constants and nonregular files; on POSIX it uses no-follow/nonblocking opens. Errors return fixed redacted JSON with exit code 2, without source paths or tracebacks. File reads are not atomic snapshots or hard filesystem deadlines. Indexing is bounded by observations plus bucket-size cells rather than rescanning all rows for every bucket.

```sh
python3 scripts/comparison_cohorts.py /private/comparison-input.json > /private/comparison-report.json
python3 scripts/test_comparison_cohorts.py -v
```

## Structural exploratory/holdout validation

`scripts/comparison_holdout.py` checks two complete submitted input documents against separately caller-retained protocol digests. It requires the correct phases, identical comparison-methodology fields, exploration ending no later than holdout starts, all supplied rows inside their own half-open window, disjoint observation IDs, and no more than 200,000 combined observations. Invalid excluded/synthetic rows do not bypass the partition checks.

The result retains two separate cohort reports, their input/protocol digests, the inter-partition gap and both coverage gates. Empty or under-covered holdout data remain explicitly insufficient. Neither report is pooled into tuning results or ranked.

```sh
python3 scripts/comparison_holdout.py /private/exploratory.json /private/holdout.json \
  --exploratory-protocol-sha256 "${EXPLORATORY_PROTOCOL_DIGEST:?Use the retained sha256 digest}" \
  --holdout-protocol-sha256 "${HOLDOUT_PROTOCOL_DIGEST:?Use the retained sha256 digest}" \
  > /private/partition-check.json
python3 scripts/test_comparison_holdout.py -v
```

A supplied digest verifies structural consistency, **not when registration happened**. The checker cannot detect renamed/re-timestamped/omitted observations, hidden exploratory use of holdout data, or an incomplete campaign. All such attestation, completeness, profitability and execution claims remain false. A `STRUCTURALLY_SEPARATED` result is not a certified clean holdout.

## Verification and remaining acceptance

Local continuation evidence: 28 cohort tests and 15 partition tests pass, including real CLI success/failure, negative/large integer sums, incomplete costs, cross-size coverage, repeated sampling, invalid shapes, bounded grids/reads, duplicate keys, deep JSON, FIFO/symlink refusal, protocol binding and partition contamination. The new regression selection failed against the exact original cohort blob `666b363d00b750a5a1c8465812d834f058ecda63` before the correction. Both suites run independently in the Comparison review CI matrix. A workflow definition is not evidence of a successful remote run; attach the exact tested source and actual CI results to #106/#48.

Implementation and source self-review are by one integrating assistant. Concurrent test subprocesses/CI jobs are not AI subagents or independent methodology reviewers.

#48 remains open pending original ARB-023/025/026/032/033 acceptance, verified canonical asset/valuation mappings, persistent-opportunity deduplication, authenticated export/application integration, real campaign coverage, pre-registration evidence and Product Owner methodology review. No RPC, database mutation, worker start, signing, broadcasting, wallet funding or live activation is performed by these tools.
