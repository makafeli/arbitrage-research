# Common-anchor collection and immutable hypothetical costs

This cohort advances ARB-018 and ARB-025, with integration work for capture, replay, storage, API, dashboard and exports. It follows merged PRs #90 and #91. The accepted preceding main checkpoint is `b65571ecd0e4e2b7f3ab6ed2a6612afd4055323a`: [all four validation jobs](https://github.com/makafeli/arbitrage-research/actions/runs/34725234701) and the [planning import](https://github.com/makafeli/arbitrage-research/actions/runs/34725234706) passed. That runtime had 225 Rust tests including 52 PostgreSQL tests, 36 Chromium scenarios and 25 Node tests; the corrected importer passed 23 regressions. These are baseline counts, not inferred results for this new source.

The accepted runtime checkpoint is `6c8e39628282b95b4531cd2b1006fd4c2455ef64` in [PR #92](https://github.com/makafeli/arbitrage-research/pull/92). [All four validation jobs](https://github.com/makafeli/arbitrage-research/actions/runs/34728060968) passed on that exact source: **261 Rust tests**, including all **57 mandatory real PostgreSQL tests**, with zero failures, ignored or filtered tests; **43 Chromium scenarios**; **30 Node tests**; **23 importer regressions**; **43 negative contract cases across 26 API operations**; and **all three Railway container builds**. Rust formatting, strict Clippy, independent Uniswap oracle reproduction and the actual TypeScript-client/Rust-HTTP/PostgreSQL smoke also passed. The latter verifies cost capability, bounded empty history, an inaccessible source returning 404, six export source counts and repeatable export hashes. Successful assessment creation, idempotent retries, scoped rejection, persistence and export replay run against PostgreSQL in the HTTP/storage integration suites.

The five new PostgreSQL tests are included in the 57, not counted separately. The independent cost fixture is reproduced by both Rust and TypeScript. Local checks covered 204 non-PostgreSQL Rust tests; full PostgreSQL, process and browser claims come from the linked remote execution. Subsequent evidence-only commits retain this runtime checkpoint; their exact-head CI results belong to the PR. No test result or synthetic capture establishes provider qualification, market coverage, profitability, a deployed Railway environment or independent security certification.

## Review evidence

The first CI run on `b356c9db35a18cb12d67484fecab808ea5d93e46` passed all 43 Chromium scenarios, 30 Node tests, specifications and all three container builds. Its Rust job exposed an incorrect new process-test expectation: it asked a worker that had already faulted and exited to acknowledge STOP. The revised test retains the five-second bound and requires durable FAULTED state, a closed local fence, a newer generation, zero outstanding attempts and process exit code 2. A STOP submitted after that exit remains PENDING without an applied timestamp. It still requires zero admitted captures/decisions and exactly the two previous readiness artifacts. Production lifecycle behavior was not weakened to satisfy the test.

The three full-page screenshots below were recovered from that run's web job, with byte counts, sequence, dimensions and SHA-256 verified before visual inspection. The cost form, saved negative result and explicit unknown allocation were reviewed at each width. Wide audit tables scroll horizontally on narrow screens. These synthetic screenshots and automated cases do not complete keyboard, assistive-technology, zoom or contrast acceptance.

| Viewport | Retained screenshot | Pixels | SHA-256 |
|---|---|---|---|
| 320 | [Cost assessment](review/cost-assessment-320.png) | 320 × 10182 | `499fcd02a556ca0e55bb56b313dbe6b6e65c5d10fff5771885e79a646a8e46a8` |
| 390 | [Cost assessment](review/cost-assessment-390.png) | 390 × 9462 | `f6b85ac756def4a298bc45ba498779bada5248f72cfe273d40eeacccf20fe401` |
| 1440 | [Cost assessment](review/cost-assessment-1440.png) | 1440 × 6337 | `f2b2ce732159487f368dc71f663a64171954a52735c0a2302abfac8ad283405a` |

## Shared acquisition and original evidence

Base pool-set collection chooses one finalized block and pins every code/state/tick read to its hash with `requireCanonical: true`. A final canonical check covers the complete batch. A provider advertising a later finalized tip during pool reads cannot change the selected anchor; canonical invalidation rejects the batch. There is no fallback to unpinned state. The semantics are defined by [EIP-1898](https://eips.ethereum.org/EIPS/eip-1898).

Solana pool-set collection deduplicates the required accounts for 1–8 registries and obtains them in one finalized `getMultipleAccounts` response. It rejects unions over the official 100-address bound instead of combining responses into an asserted common state. Each pool still validates its own program, program-data hash, token/mint/vault identities and tick arrays. Request order and response positions are preserved. A shared RPC context does not establish independent bank/write coherence; existing qualification flags remain conservative. See the [official Solana method contract](https://solana.com/docs/rpc/http/getmultipleaccounts).

The worker obtains and validates the complete batch before writing any artifact. An acquisition failure writes no partial batch. If a later disk or quota failure follows an earlier successful file write, that file remains unadmitted raw evidence; the batch cannot admit decisions. Existing generation, worker-epoch, deadline and STOP fences remain authoritative.

New pool-set bundles use adapter version `arb_evm-pool-set-v2` or `arb_solana-pool-set-v2`, while the registry schema remains version 1. Each selected pool bundle contains the complete original batch RPC transcript. Reusing its encoded bytes avoids inventing per-pool RPC responses. This duplicates retained evidence across pool bundles and consumes capture quota accordingly. Offline replay validates every pool and consumes every original request/response before selecting the requested snapshot. Single-pool and pool-set-v1 replay paths remain supported.

Tests exercise moving finalized tips, final canonical failure, an invalid second pool, absent hash-pin support, duplicate registries, shared Solana account deduplication, wrong or missing account identity, excessive account unions, unselected-pool transcript corruption, trailing records and legacy compatibility. Worker/process tests check the chain of actual fixture HTTP responses, original artifact bytes, stored admissions/decisions and authenticated API results. The process test `canonical_batch_failure_admits_no_partial_captures_or_decisions` retains successful readiness artifacts and rejects subsequent invalidated research batches.

## Decision-bound manual costs

The API accepts a stored observation identity and a versioned manual scenario. It obtains the sealed source decision itself; callers cannot replace its network, asset, quote, experiment or configuration. Only QUOTED decisions can receive assessments. The source quote remains unchanged, including its unavailable external-cost fields.

A scenario includes explicit provenance identifiers, a maximum historical valuation age, chain-specific fee decomposition, expenses, funding assumptions and overhead. Applicable omitted categories become unknown. Known zero is an explicit input. Execution/L1/priority/relay costs retain native currency; conversions use exact ratios and round costs upward. Costs already denominated in the starting asset require SAME_ASSET valuation. They cannot be reduced by declaring an arbitrary ratio for identical currency. The review found and corrected that identity issue in both the general reducer and scenario boundary, with model and HTTP rejection coverage.

Valuations cannot occur after the source decision and must fall within the declared 1–86,400,000 ms historical age policy. That policy is an operator assumption, not measured oracle quality. Identifier restrictions reject URLs, whitespace and malformed digest references; they do not certify that arbitrary user-entered text contains no sensitive information. ACCOUNT_SETUP is a declared expense, without a recoverable-rent, refund-timing or capital-lock model.

The complete sealed decision, scenario and assessment have independent canonical SHA-256 identities. Reads recompute every binding and result. The [independent Python fixture](../specs/cost-assessment.example.json) is checked by Rust: a native amount converted with upward rounding yields 10001 starting-asset units, transaction-net -2001 and fully allocated net -2501. These synthetic amounts describe arithmetic only. Complete assumed costs retain CANDIDATE evidence and do not establish full transaction simulation, executable funding or actual inclusion.

## Persistence, API and dashboard

Migration 0004 adds append-only assessments with operator/session/source-decision foreign keys. An idempotency key serializes identical requests and rejects changed payloads; no worker lifecycle row is locked for assessment serialization. Assessment writes append their own record and audit event, without changing the source decision or paper ledger. Restart reads revalidate the original stored source and the complete report, including hashes.

The authenticated API provides create, list and detail routes. POST returns 201 for a new result or identical retry. Invalid scenarios and non-QUOTED sources are rejected; inaccessible sources remain unavailable. Read pagination is bounded to 100 rows and is distinct from a frozen export. The existing 16 KiB request and 15-second handler limits apply; cost writes use an eight-second SQL statement timeout.

The dashboard selects a QUOTED observation, collects explicitly manual fee/valuation/funding/overhead inputs and shows immutable history. Missing inputs remain distinct from zero; negative results remain visible. An uncertain write retains its original request/key and scope for retry. A complete browser reload loses this transient state; the operator must inspect authoritative history before issuing equivalent new work.

Export schema 1.1.0 adds cost assessments as the sixth counted dataset under the same REPEATABLE READ snapshot, 10,000-row and 8 MiB limits. Assessment source links and recomputed reports are checked against decisions from that snapshot. JSON and CSV preserve exact amounts; CSV formula guards and payload_json remain in use. API and frontend must be upgraded together. Raw capture availability/expiry, asset decimals and full configuration bodies remain outside the complete database snapshot claim.

## Remaining acceptance

- ARB-018 still requires qualified providers, chain-lag limits, rolling state reconstruction and durable invalidation of prior work after reorgs, gaps or rollbacks. Final acquisition canonical checking does not provide those features.
- ARB-025 still requires qualified recorded fee/valuation/funding inputs, broader conservation review and its original dependencies. Manual scenario arithmetic does not establish observed transaction costs.
- ARB-027 still requires complete portfolio/outcome and inclusion-scenario replay with genuine recorded-input qualification.
- ARB-041 still requires broader comparison summaries, qualified decimals and independently verified raw dependency availability/expiry.
- Complete Base/Solana atomic transaction plans and simulation, delay/inclusion scenarios and automatic paper settlement remain subsequent work. No research process signs or broadcasts.

Railway remains the selected host. The migration and three container definitions are reviewable; a connected target environment, provider configuration and deployed backup/restore evidence remain required. Native owner Project and Wiki publication remain separate from repository sources and native issue updates.
