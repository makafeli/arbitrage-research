# Frozen export to local capture audit

Existing scope: ARB-014 / #28, with the bounded export consumer relevant to
ARB-041 / #55. This does not accept the original tickets or their dependencies.

## Operator workflow

1. In Connected mode, select a session and prepare its frozen export. The existing
   authenticated API client checks the complete stored snapshot, source counts,
   decision references, retained cost assessments and canonical content digest.
2. Choose **Download capture audit request**. The request includes every capture
   dependency in that exact export, including catalog-missing entries and the
   same capture ID with different retained manifest hashes. It contains no local
   root, provider credentials, raw responses or arbitrary command text.
3. Retain the request outside the capture volume and run the checked-in auditor
   beside the trusted, quiesced volume. Supply the actual audit time as Unix
   milliseconds. This example uses paths selected by the operator, not the file.

```sh
python3 scripts/export_capture_audit.py \
  --root /private/captures \
  --request /private/request.json \
  --now-ms "${AUDIT_TIME_MS:?Supply the explicit Unix millisecond audit time}" \
  > /private/report.json
```

4. Choose **Import local audit JSON** in the same frozen export. A matching report
   shows all raw-status counts and the first 20 detailed rows. Download the bound
   audit report to retain every row. Replacing the export or signing out clears
   the imported result. Hidden/inactive controls cannot act.

Exit 0 means the local auditor reported COMPLETE; exit 1 means a complete list
with gaps (including no references); exit 2 means incomplete/refused auditing or
invalid input. A structured INCOMPLETE result can be imported when its complete
reference list survives a byte-budget or file-read refusal. EXPORT_AUDIT_FAILED
is not an attachable report. A missing root fails globally rather than claiming
that every capture is absent. Root/path failures do not expose local paths.

## Binding and compatibility

The new request envelope is schema 1 / FROZEN_EXPORT_CAPTURE_AUDIT_REQUEST. It
contains a source-export schema/version, export ID, session ID and content hash,
plus the unchanged schema-1 capture request used by `capture_audit.py`. The source
identity fields are bounded ASCII labels; configuration and manifest identities
must be canonical lowercase SHA-256 strings. Old synthetic examples with opaque
non-hash placeholders remain valid examples of other UI functions but cannot be
used as real audit requests.

The Python wrapper validates the entire envelope before capture-root access and
then calls the existing auditor. It returns FROZEN_EXPORT_CAPTURE_AUDIT with the
source binding, a canonical request hash and the original capture audit. The
browser recomputes both envelope and inner request hashes and checks exact
export/session/network/configuration, reference identities/order/counts, byte
budget, status counts, expiration/time consistency and research-only claims.
Unknown report fields and missing, substituted or duplicate rows are rejected.

This is a separate report. Original frozen JSON/CSV, source hashes, catalog
statuses and historical unknown retention disclosures are never changed. A
later filesystem audit is not retroactively part of a database MVCC snapshot.
No server endpoint, database migration or schema-1.1.0 export change is needed.
The API never opens a user-supplied storage path. Imported files stay in browser
memory and are not uploaded to the service or persisted in browser storage.

The browser integration deliberately reuses `parseFrozenExport` and
`verifyFrozenExport`, not a second permissive parser for the complete financial
export. The Python wrapper cannot independently reconstruct or authenticate the
source export from the request alone.

## Limits and trust

Requests have at most 1,000 references and 1 MiB. Larger valid exports are refused
as audit requests, not truncated or silently split into batches with reset
budgets. Imported reports have a 2 MiB bound checked before and after reading.
The existing auditor retains its manifest/object/entry/depth limits and one
aggregate byte budget, default 256 MiB. See the [capture-audit runbook](23-CAPTURE-DEPENDENCY-AUDIT.md).

An imported report is **operator-supplied, not independently authenticated**.
Checking hashes and structure cannot prove who ran the audit, where it ran or
whether a supplied time/status is true. A malicious operator can fabricate a
structurally valid report. A hash is not a signature. This UI therefore does not
mark the result as independently verified or certify production storage.

Availability, expiration, creation time, declared quote completeness and origin
remain distinct. AVAILABLE does not mean current, complete, genuine market data
or replayable protocol inputs. NOT_DECLARED retention remains explicit. An empty
export cannot establish complete coverage. A per-volume audit cannot prove the
health/completeness of every worker volume, unrecorded observations, a whole
campaign or continuous retention. Snapshot IDs/protocol semantics are not
independently replayed by this storage check. All replay statuses remain
NOT_ASSESSED; market eligibility and execution authorization remain false.

## Verification

- Python envelope tests check exact binding, invalid/extra fields, rejected root
  access, preserved files, complete missing/expired/budget lists, u64 timestamps
  and actual CLI exit codes/redaction.
- Node tests run the actual TypeScript request builder, actual Python process on
  synthetic Base/Solana files, then the TypeScript report validator. Missing raw
  files, expiry, limits, tampering, complete reference matching and separate
  source/report identity are tested without RPC or model processes.
- Browser tests cover request download, local import, mismatched and incomplete
  reports, replacement exports, logout during a pending file read, inactive
  controls and 320/390/1440px layouts. These use synthetic server/report fixtures,
  not actual production storage. Three capture-audit PNGs are retained for review.

Use current exact-source CI and its actual results for acceptance. Defining tests
or a workflow is not evidence of a successful run. No AI agents, production
rollout, signing, funding or market qualification is implied by these tests.
