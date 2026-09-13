# Captured chain time and explicit adapter support

This cohort advances ARB-018, ARB-021 and ARB-042 with historical chain-time assessment, immutable policy binding and a bounded operator support catalog. It follows merged PR #92. The preceding main checkpoint `5a340e55cdfc6ce726fd80869f45850e9f4c2a3c` passed [all four CI jobs](https://github.com/makafeli/arbitrage-research/actions/runs/34728563243): 261 Rust tests including 57 PostgreSQL tests, 43 Chromium scenarios, 30 Node tests, actual HTTP-client/API/database smoke and three Railway image builds. Those counts describe the preceding accepted source.

The accepted runtime checkpoint is `1dd5ffbd75caf69e8de691d2e1a87a2027df2339` in [PR #93](https://github.com/makafeli/arbitrage-research/pull/93). [All four CI jobs](https://github.com/makafeli/arbitrage-research/actions/runs/34751924393) passed: **294 Rust tests**, including **all 61 real PostgreSQL tests**, with zero failures, ignored or filtered tests; **52 Chromium scenarios**; **39 Node tests**; **23 importer regressions**; **59 negative contract cases across 27 API operations**; and **all three Railway container builds**. Formatting, strict Clippy, independent Uniswap oracle reproduction and actual TypeScript-client/Rust-HTTP/PostgreSQL interoperability passed. The service smoke includes authenticated adapter support, scoped sessions, collection outcomes, costs and consistent six-dataset exports.

The Rust count was independently checked against 51 complete harness summaries and 294 individual successful test lines. All 61 required database test names were matched in the actual PostgreSQL run. They include durable stale Base state through the worker and authenticated API, immutable-policy preservation/downgrade/tamper rejection and repeatable-read export isolation. Subsequent screenshot or documentation commits retain this runtime checkpoint and must pass their own complete CI before merge. This evidence does not establish provider/pool qualification, deployed recovery or market profitability.

## What the time report means

The previous engine bound time spent acquiring and processing inputs. A provider could repeatedly return an old finalized block while those elapsed-time checks passed. The new opt-in policy compares captured chain time with the original observation UTC time plus monotonic processing elapsed time. It records both quantities rather than calling recent acquisition proof of recent chain state.

Each network can declare `chain_freshness = { version = "finalized-chain-time-v1", max_chain_age_ms = ... }` in its immutable configuration. Limits range from 1 through 86,400,000 ms. The [opt-in example](../config/chain-freshness.example.toml) keeps both networks disabled and declares example bounds of 1,800,000 ms for Base and 60,000 ms for Solana. These are explicit research assumptions, not calibrated provider-health, latency or inclusion guarantees. The original example remains unchanged, so loading old configurations does not silently enable a new policy or alter its digest.

Base already captures the chosen finalized block's timestamp, number, hash and parent hash. The new assessment uses those retained values. Solana uses the new `capture_pools_with_chain_time` path: after one complete shared-account capture it requests `getBlockTime` for exactly the returned context slot. The official API describes an [estimated production time](https://solana.com/docs/rpc/http/getblocktime), not independent evidence of bank coherence. The original time response is retained with the original account transcript. Null remains unknown; RPC/transport errors remain acquisition failures. Invalid types, negative values and times beyond the supported exact UTC range are rejected.

Reports distinguish `WITHIN_POLICY`, `STALE`, `FUTURE` and `UNKNOWN`. Future timestamps have no fabricated zero age. The aggregate precedence is FUTURE, UNKNOWN, STALE, then WITHIN_POLICY; an empty evidence set is UNKNOWN. Every source retains its capture identity and Base or Solana context. All report arithmetic and classifications are reproducible with integers.

Missing, future or over-age time prevents an initial QUOTED result. Expiry during a route produces a rejection. A final check also refreshes all earlier quoted records before returning a completed batch, so later routes cannot leave an expired early quote behind. The report describes that historical evaluation; subsequent persistence delay or dashboard refresh does not turn it into current health. Both schemas still produce CANDIDATE evidence, and opportunity projection keeps `state_fresh_and_coherent` false because complete state and protocol qualification remain unmet.

## Compatibility and replay

Absent policy, the historical configuration serialization, calculation path and trace schema 1.0.0 remain unchanged. New report-bearing decisions use schema 1.1.0 and calculation version `capture-pair-research-v2;bounds8x63;group1000;finalized-chain-time-v1`. Version 1.1 requires the report; version 1.0 cannot carry it. Optional absent fields are omitted, preserving earlier content hashes and cost bindings.

Policy-enabled Solana captures use `arb_solana-pool-set-v3`, including a legacy one-pool registry treated as a batch of one. Old policy-free v1/v2 paths keep their exact call sequence. Replay verifies the frozen policy, original selected slot, full raw transcript, optional timestamp and every selected pool. Removing the time lookup and snapshot value while relabeling the manifest v2 cannot downgrade a policy-enabled input. Base retains its previous capture format because its original context already contained chain time.

Replay supplies explicit historical observation and processing times. Current wall time is used only for raw-retention checks. Known versus null Solana time, altered slot/time/type, missing or extra RPC calls, incompatible formats and policy downgrade have dedicated regressions. The [independent Python decision fixtures](../specs/chain-freshness.example.json) cover all four statuses and canonical observation/grouping identities. Domain and TypeScript checks reproduce their exact arithmetic and hashes. Fixed prior configuration and Solana snapshot bytes, and the prior cost/export fixtures, check that adding fields does not rewrite historical evidence.

## Immutable storage binding

Append, idempotent retry, decision/opportunity reads, cost-source reads and frozen exports compare a report's policy with the policy extracted from the operator/session's immutable configuration snapshot. The stored network is checked against the immutable session network. A caller cannot remove the report, widen its policy and reseal the observation to bypass a configured rule.

The existing queries project only the relevant frozen policy and network. They do not transfer a complete configuration body for every result or add a separate query per export row. This check validates the extracted policy and binding; it does not re-hash and revalidate every field of the complete configuration body. No database migration is required. Tests cover preserved reports, downgrade, malformed policy and self-consistent tampering with newly computed payload hashes.

The worker/process regression follows recently acquired old Base state through real fixture HTTP, capture admission, PostgreSQL and the authenticated decision API. The expected result is a durable `DATA_UNAVAILABLE` with `CHAIN_TIME_STALE`, two captured references and no fabricated route or opportunity. The collection outcome is `DECISIONS_RECORDED`: successful acquisition of stale state is distinct from transport failure and from a valid no-route result.

## Operator support catalog

Authenticated `GET /v1/adapter-support` serves an immutable startup catalog with schema 1.0.0 and catalog version `immutable-scope-v1`. Code declarations come directly from `arb_engine::capabilities`. Decode and bounded research math are implemented; qualified quote, transaction build, full simulation and submit remain false.

Optional `ARB_SUPPORT_REGISTRY_FILES` accepts up to 16 `network=path` entries, for example `base-mainnet=/app/registries/base.json,solana-mainnet=/app/registries/solana.json`. Each file is limited to 1 MiB and eight pools. This is a local diagnostic input; it performs no provider requests or activation. No configured paths, provider secret references, raw RPC data or credentials are returned. Files are read at startup only; the catalog does not monitor later file edits, provider health or worker heartbeat.

The catalog distinguishes configured allowlists from a loaded registry that matches the frozen digest and authorized identities. `NOT_LOADED`, `LOADED_BLOCKED` and `LOADED_AUTHORIZED` have explicit public reasons. Only the last state exposes concrete pool/token/program relationships. A matching content digest demonstrates structural agreement, not deployed token/program correctness or economic qualification. No eligible route or coverage count is invented from two independent allowlists.

There are at most 16 configuration reports, each with both supported networks. Up to eight pool identities and sixteen asset identities are expanded per network. Larger existing allowlists remain valid: exact full counts, empty projected arrays, `identities_expanded=false` and `DECLARED_SCOPE_NOT_EXPANDED` explain the bounded view. Authorization still checks the full underlying allowlists. Equivalent duplicate configurations are coalesced. Loaded registry pools stay bounded to eight. This avoids changing control-service startup merely to accommodate a diagnostic table.

The dashboard shows the catalog in System and captured-time reports in the decision inspector. Unknown legacy evidence, absent API support, missing registry, transport failure and historical stale/future data are distinct states. Failed refresh retains the previous catalog visibly; navigation cancels pending reads. JSON/CSV preserves the new nested decision report without changing the six-dataset export format. API and frontend should be deployed together; old strict readers can reject new decision versions.

## Review corrections and remaining acceptance

Independent source review found and corrected final-batch freshness expiry, contradictory Solana times for one context, unintended startup rejection of existing large scopes, and frontend acceptance of contexts that Rust rejected. The storage policy check was extended to every shared decision-record path, including cost/export reads. These are engineering reviews, not an independent security audit.

- ARB-018 still needs rolling state reconstruction, durable invalidation of previously retained dependent work after reorgs/rollbacks/gaps, genuine recorded cases and calibrated provider/chain limits. Captured timestamp assessment does not establish those properties.
- ARB-021 still needs qualified activation per actual pool/token/program and its original dependency acceptance. Local registry authorization is not provider or quote qualification.
- ARB-042 still needs heartbeat, queue/drop/storage telemetry, measured operational thresholds, alerts and delivery-failure handling. The catalog and historical reports do not claim current system health.
- ARB-041 retains raw artifact availability/expiry, qualified decimals, complete replay packages and comparison-summary gates. Keeping a nested report in an export does not qualify all dependencies.
- Full Base/Solana atomic transaction guards and simulation, inclusion scenarios and automatic paper settlement remain subsequent work.

Railway remains the selected host. Container and service-backed CI evidence does not establish a deployment. The target environment, credentials, qualified pools and deployed backup/restore proof remain external prerequisites. Native owner Project and Wiki publication remain distinct from repository wiki sources and native issue updates.

## Local verification before publication

All 233 distinct non-PostgreSQL Rust tests passed locally, with zero unresolved failures. The compiled workspace inventory contains 294 tests, including 61 tests requiring actual PostgreSQL execution in CI. Workspace formatting and strict all-target Clippy passed. All 39 Node tests, TypeScript checking and the production frontend build passed; 52 Chromium scenarios were discovered for CI. Contract validation resolved 27 operations and rejected 59 negative examples. These local results do not count the pending PostgreSQL, Chromium or container executions as passing.

Three support catalog tests initially omitted the positive trade size required by an enabled paper configuration. The test fixture was corrected and all four catalog tests then passed. A zero-byte generated test executable and invalid cached Rust metadata were repaired by rebuilding affected local artifacts; they did not require runtime source changes. Local Rust is 1.91.1; CI retains the locked 1.90.0 toolchain.

## First remote run and fixture correction

The initial [PR #93 CI run](https://github.com/makafeli/arbitrage-research/actions/runs/34751690343) exposed a browser-fixture routing error. The client correctly percent-encodes observation IDs, but the new mock detail handler compared the encoded path against an unencoded `sha256:` ID. It returned a fixture 404, leaving the expected inspector content absent. Forty-eight browser scenarios passed and four chain-time scenarios failed; screenshot emission consequently reported missing chain-time images.

The fixture now decodes the path parameter, matching actual API routing. Each inspector helper also checks HTTP 200 and the exact returned observation identity. Production readers and canonical fixture hashes are unchanged. The corrected run passed all four jobs at the accepted runtime checkpoint above.

## Retained visual review

The focused screenshot and evidence commit `78beecb71e3123bf5e23c5734ada118f30649691` also passed [all four CI jobs](https://github.com/makafeli/arbitrage-research/actions/runs/34752205913), again with 294 Rust tests, 61 PostgreSQL tests, 52 Chromium scenarios, 39 Node tests, actual client/API/database smoke, specifications and three containers. It changes screenshot positioning and records the accepted runtime; production behavior remains that of the runtime checkpoint above.

All six retained PNGs were recovered from the successful CI logs with complete chunk order, byte length, PNG dimensions and SHA-256 verified before visual inspection. Adapter images come from runtime run 34751924393; focused chain-age images come from run 34752205913. Both sets use synthetic browser responses and are not live-provider evidence.

The first chain-age images only revealed the heading near the bottom of the modal. The revised tests scroll each source heading into the viewport, then align the historical report at the top and assert that the heading, declared policy and first capture heading are in view. The retained 1100-pixel-high images show the policy, independent processing time and complete first source at all three widths; the desktop image shows both sources. On narrow screens, the second source requires scrolling and is exercised by the test. The adapter cards retain readable labels and wrapped identities, separating local structural authorization from unqualified quote and unavailable execution capabilities.

| View | Width | Retained original | Pixels | SHA-256 |
|---|---:|---|---|---|
| Adapter support | 320 | [PNG](review/adapter-support-320.png) | 320 × 4170 | `3893838c2fece46fd5046d3e07f16d28bef2e09733ea00aca26644ed2fd5f0ce` |
| Adapter support | 390 | [PNG](review/adapter-support-390.png) | 390 × 3969 | `534b756c2251ee13f1cd3ef4cbbfeb826600962a7027ddc0c37d35e025c0ebe4` |
| Adapter support | 1440 | [PNG](review/adapter-support-1440.png) | 1440 × 2117 | `95476c5f0a689ae7bba86055ee6e573cc981aa65fd81ac8ced3e3aa6c92c6376` |
| Captured chain age | 320 | [PNG](review/chain-freshness-320.png) | 320 × 1100 | `23f645e1909d9e6c003bedf6f4db7bc1f45d883535a2fdcc6a64b3a52735a44c` |
| Captured chain age | 390 | [PNG](review/chain-freshness-390.png) | 390 × 1100 | `4f3d109d7ca572591236161f6086c8df7d467621df67eebdb4739b80de79e0fe` |
| Captured chain age | 1440 | [PNG](review/chain-freshness-1440.png) | 1440 × 1100 | `06978138ab43eb270e7caf96ffdac0bb00a10135042c992c3d44f3583783b0bb` |

These visual checks establish readable presentation for the selected fixtures and viewport widths. They do not complete all keyboard, screen-reader, zoom, contrast or operational acceptance. The full original ticket gates remain authoritative.
