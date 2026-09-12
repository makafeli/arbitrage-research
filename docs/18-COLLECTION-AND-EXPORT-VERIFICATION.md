# Collection telemetry and frozen research exports

This record covers the implementation cohort after PR #89: durable collection attempts, frozen research export and the connected diagnostics/export UI. It advances ARB-013, ARB-023, ARB-041 and ARB-042. The original acceptance criteria and dependency graph remain authoritative; partial implementation does not close these tickets.

## Evidence checkpoint

Implementation is on `feat/research-telemetry-exports`. Local Rust 1.91.1 all-target compilation, formatting and Clippy with warnings denied pass. The selected changed-component runtime cohorts pass: 22 API, four storage and four worker tests (30 total). These deliberately exclude PostgreSQL tests. Frontend type checking and production build pass, together with 25 Node tests. The importer passes 21 regressions; the specification validator resolves 23 API operations and rejects 31 invalid contract cases. No full CI result for this cohort is claimed here yet. The established baseline is merged PR #89 at `bf3a186a26993252060712e4bac66abdef545452`: 210 Rust tests, including 41 real PostgreSQL tests, 30 Chromium scenarios and 20 Node tests. Its [main validation](https://github.com/makafeli/arbitrage-research/actions/runs/34718464924) is historical baseline evidence, not verification of subsequent edits.

## Research denominators

The first integrated run on `c02e6cabc26f607343ffcc286d38fbc6d3f3cff1` ([34723515064](https://github.com/makafeli/arbitrage-research/actions/runs/34723515064)) passed specifications, all 36 browsers, 25 Node tests and the three container builds. Rust reported 224 passed and one failed, with zero ignored/filtered tests; the separate real-client smoke did not run after that failure. The new different-context process test incorrectly expected a whole-batch evaluation error. Existing engine behavior correctly preserves two `REJECTED` observations with `CAPTURE_CONTEXT_MISMATCH`. The corrected test retains its 20-second deadline and verifies those exact rejections, zero quoted opportunities and no misclassification as provider or fatal evaluation failure. Production behavior was not changed. A successful complete follow-up run is still required.

Collection attempts and decision observations measure different things. An attempt begins durably before acquisition; it can produce several decisions, fail before a decision exists, or remain unresolved after a crash. Readiness collection while stopped is distinct from evaluation in a running session. Intentional suppression by STOP or a newer generation must not be presented as provider downtime.

A successful collection outcome is not a profitable opportunity, executable route or virtual fill. Quoted routes remain CANDIDATE; incomplete external costs retain null net results. A completed attempt count describes registered attempts only. It cannot prove that a configured observation schedule was followed or that no work was missed while the process or database was unavailable. `collection_completeness` therefore remains `UNKNOWN`.

These operational attempt records have no asserted market-input origin; failure counts must not be silently treated as recorded-market observations. Raw failures use finite reason codes. Provider URLs, authentication headers and raw exception text do not belong in public telemetry. Generation and worker-epoch fences remain authoritative for decision admission. Recording a terminal diagnostic after STOP must never admit a late decision.

## Export semantics

A reproducible database export needs all its rows and counts from the same snapshot. PostgreSQL REPEATABLE READ keeps successive reads on the snapshot established by the first non-transaction-control statement; plain read-only queries do not require row locks. See the [primary PostgreSQL isolation documentation](https://www.postgresql.org/docs/current/transaction-iso.html#XACT-REPEATABLE-READ).

Database snapshot completeness and replay-input availability are separate claims. An admitted capture reference establishes a recorded association; it does not prove that a raw bundle on a worker volume still exists or passes its retention and integrity checks. Exports must preserve references and explicitly disclose unavailable or unverified dependencies. Provider configuration, local artifact paths and authentication material are excluded or redacted.

JSON retains exact integer amounts as strings. CSV serialization needs both structural quoting and protection against spreadsheet formula interpretation, including prefixes hidden behind whitespace. CSV scalar identity columns may carry a protective apostrophe. Each payload_json cell contains the lossless JSON record, so exact values and authoritative identities should be recovered from that JSON rather than by guessing which scalar prefixes were added. Generic spreadsheet automatic number conversion is not a precision guarantee.

## Review and validation

The required review covers immutable attempt identity, terminal retry conflicts, old-worker and generation suppression, failure-versus-no-route counts, consistent export counts under concurrent writes, finite resource limits, operator isolation, credential redaction, exact large integers and spreadsheet injection. Real PostgreSQL integration and Chromium results must be attached to the reviewed commit before claiming they passed.

The importer also gains regression coverage for preserving current workflow labels. Original `status:*` defaults apply only to newly created issues; reruns cannot silently restore planned status after triage. This does not change issue state or acceptance.

## Engineering review corrections

- Unique collection/decision associations prevent a second batch from counting an existing observation again. Duplicate observation IDs within a batch are rejected; exact retries preserve the terminal result.
- Ordinary immutable-scope/admission conflicts are distinct from a STOP/generation fence. Persisting the wrong diagnosis would obscure an implementation or input error.
- Public digest fields consistently use the declared `sha256:` prefix. A Rust regression verifies the independent Python-generated export fixture's canonical hash.
- Invalid collection cursors fail at the HTTP boundary. Collection UUIDv7 IDs support ordinary forward pagination; the live page still does not claim a frozen history.
- Export admission is capped independently from the ordinary request budget. Source counts and predecode byte limits refuse excess work without partial output.
- Paper command/attempt pseudonyms and removed freeform reasons retain exact journal replay; original source payload hashes distinguish the accounting projection from original evidence.
- The isolated TypeScript-client smoke uses a clearly synthetic OBSERVE configuration with its provider environment reference deliberately unset. It creates a RECOVERING session without starting a worker and checks telemetry/export interoperability. This fixture does not qualify a pool or issue market requests.

These are agent-assisted engineering reviews, not an independent security audit.

## Visual verification

The web job for `c02e6cabc26f607343ffcc286d38fbc6d3f3cff1` in [run 34723515064](https://github.com/makafeli/arbitrage-research/actions/runs/34723515064) passed all 36 Chromium scenarios and 25 Node tests. Root integration inspected the six new synthetic Connected-mode screenshots below. Export controls wrap within the page at 320/390 pixels; large tables retain their own horizontal scroll area. Desktop navigation, navy/lime styling, source counts and the distinct failure/suppression/unresolved outcomes remain legible. Browser checks establish no whole-page horizontal overflow and minimum export-button target sizes. Manual screenshot inspection supplements those checks; it is not full keyboard/screen-reader certification.

The retained images were reconstructed from the CI artifact stream with verified chunk order, byte length and SHA-256. They use synthetic API responses, including deliberately missing capture dependencies and unresolved attempts. They establish rendering and interactions, not actual provider performance or market activity.

| Image | Dimensions | SHA-256 |
|---|---|---|
| [frozen-export-320.png](review/frozen-export-320.png) | 320 × 6084 | `b37704c90bba88da34f82aaa67b5793d871bf08ecd3d69b7f6f487dc0c8f4014` |
| [frozen-export-390.png](review/frozen-export-390.png) | 390 × 5511 | `395fbd7dfeb50f987636ead0178548a781727d362a15bf8435a4799551da386f` |
| [frozen-export-1440.png](review/frozen-export-1440.png) | 1440 × 3386 | `f2820380b09978091f835b07d1688415699b18d41de4da0a5b5a3920ef0398af` |
| [collection-health-320.png](review/collection-health-320.png) | 320 × 4051 | `807f5c9a015a6ca83cff3dd66760896c0355b87f9310e1ab27e21410447fdcf6` |
| [collection-health-390.png](review/collection-health-390.png) | 390 × 3860 | `18647fccb210f67d5cb844cec83f4a954f585f9e160ec932da253c234ce2ba89` |
| [collection-health-1440.png](review/collection-health-1440.png) | 1440 × 2354 | `afa5dd1dda949f041e6a9626ea819a951732848fd1a40bdb6a8a40e70fdd2cd9` |

## Remaining acceptance

- ARB-013: measured stage percentiles on a named host, bounded overload and slow-analytics isolation evidence, and the remaining prerequisite gates.
- ARB-023: coherent recorded market state and a complete observation schedule remain unqualified. Persisted attempt outcomes improve the denominator without establishing those claims.
- ARB-041: wider comparison/cost summaries and complete independently verified replay-input availability remain separate from a frozen database bundle.
- ARB-042: measured alert thresholds, alert delivery and failure handling, complete queue/storage/chain-freshness diagnostics and their original dependencies remain open.
- Market-data qualification, full atomic transaction simulation and scenario-driven paper settlement are required before research can support an economic comparison. This cohort does not establish profitability.

Railway remains the selected deployment target. Container builds and deployment source are reviewable; an authenticated target environment, provider configuration and actual backup/restore and deployment evidence are still required.
