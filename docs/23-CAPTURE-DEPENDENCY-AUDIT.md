# Local capture dependency audit

Existing task: [ARB-014 / #28](https://github.com/makafeli/arbitrage-research/issues/28).
Downstream consumers are [exports / #55](https://github.com/makafeli/arbitrage-research/issues/55)
and [recovery / #58](https://github.com/makafeli/arbitrage-research/issues/58).
This is a bounded local storage inspection, not closure of those tickets.

## What it does

`scripts/capture_audit.py` checks every explicitly requested capture against a retained
manifest digest, capture identity, network and immutable configuration digest. It
reads the current `arb-capture` v1 manifest and hashes the declared objects. It does
not connect to RPC, PostgreSQL, the control API or Railway; execute archive contents;
start workers; delete files; or rewrite historical decisions/retention dates.

An available file is not necessarily unexpired, a complete quote input, replayable,
or authentic market data. These properties stay separate in each report row:

| Field | Meaning |
|---|---|
| `raw_artifact_status` | `AVAILABLE`, `MISSING`, `INCOMPLETE`, `CORRUPT`, `UNSUPPORTED`, `UNREADABLE`, or `LIMIT_EXCEEDED`. |
| `expiration_status` | `EXPIRED` at `now >= raw_expires_at_ms`; otherwise `NOT_EXPIRED`, `NOT_DECLARED` or `UNKNOWN`. |
| `capture_time_status` | A verified creation time is `AT_OR_BEFORE_AUDIT` or `AFTER_AUDIT`; otherwise `UNKNOWN`. |
| `origin` | Only the verified manifest's `synthetic`, `manually-constructed` or `recorded-live` declaration; otherwise `UNKNOWN`. |
| `quote_inputs_declared_complete` | The structurally validated declaration, not independent verification of protocol state. |
| `replay_status` | Always `NOT_ASSESSED`; no adapter/replay engine runs here. |
| `market_performance_eligible` | Always `false`; this tool never qualifies observations or computes performance. |

For example, a digest-valid manifest can identify an **expired and missing** raw
object. The row retains both facts. A file can also be **available but expired**;
it is not deleted and the historical expiry is not extended. Unknown evidence is
not inferred from directory names or a database catalog entry.

## Input and invocation

Use Python 3.11+ on POSIX. Put the request **outside** the capture root, and keep both
in a private operator-controlled directory. The request is an explicit retained
reference list, not a free-form file path or a frozen export accepted blindly.

```json
{
  "schema_version": 1,
  "network": "base-mainnet",
  "config_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "captures": [
    {
      "capture_id": "capture-123",
      "manifest_digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    }
  ]
}
```

Replace all example identities/digests with the trusted retained values. For a
verified frozen session export, these correspond to `data.session.network_id`,
`data.session.configuration_digest`, and the `capture_id`/`manifest_digest` pairs
in `data.capture_dependencies`. This CLI **does not validate or import the whole
export** and does not automatically discover every decision in a database. A caller
must supply the complete reference set and bind it to its trusted session export.
The audit verifies all supplied pairs; it cannot discover an omitted dependency.

Each capture resides at `<root>/<capture_id>/manifest.json` with flat object files
alongside it. For separate worker volumes, audit each volume's own reference set;
an unavailable or wrong root is an error, not proof that all captures are missing.
Do not create an empty replacement root to hide a failed volume mount.

```sh
python3 scripts/capture_audit.py \
  --root /protected/captures \
  --request /protected/audit-request.json \
  --now-ms 1789322400000 \
  --max-bytes 268435456
```

`--now-ms` is an explicit unsigned epoch-millisecond audit time. Retain that value
with the report; a later audit can legitimately show a different expiry status.
All reported times and byte counts use decimal strings, including values above
JavaScript's exact integer range. The example time is illustrative, not an instruction
to backdate a production audit. Files created after the supplied time produce a gap.

The report is printed to stdout and can be retained in a new private file through
the operator's usual secure logging mechanism. It contains validated capture IDs,
digests, fixed reason codes and bounded counts, not root paths, provider aliases,
raw RPC responses or source exception details. It is still sensitive operational
metadata: keep it private and do not publish production reports or source captures.

Exit `0` means all requested references passed storage checks, are not expired or
future-dated at the supplied time, and declare complete quote inputs. It is **not**
a replay/simulation/readiness certificate. `NOT_DECLARED` remains explicit when the
manifest has no expiry. Exit `1` means a complete report with dependency gaps
(including an empty reference list); exit `2` means invalid input/root, unstable
root, unreadable source or exhausted audit budget. A per-row I/O/budget refusal
still leaves every requested reference represented in the report.

## Integrity and operating boundaries

Capture and object names must be flat bounded ASCII labels matching the Rust
storage contract. Duplicate reference pairs are rejected before capture reads;
the same capture ID with different expected digests remains two distinct rows.
Duplicate JSON keys, unknown v1 fields, wrong types, inconsistent identity/network/
configuration, invalid ordering/context/completeness and unsupported versions fail.
The audit never trusts a digest recomputed from an untrusted replacement as proof
of origin: independently retain the expected manifest digests. `request_sha256` is
a deterministic digest of this small request, not a signature or native export hash.

An `INCOMPLETE` marker, even a dangling link, blocks availability. Missing manifests
remain incomplete. Extra bundle entries are rejected rather than traversed or
ignored. This strict inventory is intentionally stronger than the current Rust
loader's declared-file loop; it does not silently redefine the Rust loader.
Symlinked roots/parents are refused during initial resolution; directory/file reads
use retained no-follow descriptors and check identity/metadata afterward. Objects
must be regular single-link files. FIFOs, devices and links are not followed.

Keep roots and their ancestors trusted and quiescent for the entire operation.
This is a **non-atomic filesystem audit**, not a lock, backup or hostile multi-user
filesystem defense. Concurrent writes/replacements detected during reads fail;
same-owner changes outside a read window or to ancestor paths are not fully
prevented. Read access can update filesystem access times; source content is not
rewritten. The result describes the inspected files, not their future availability.

Fixed bounds are 1 MiB per request/manifest, 1,000 references, 4,096 declared objects
per bundle, 64 MiB declared object bytes per bundle, 4,098 directory entries and
64 JSON nesting levels. Default aggregate file-read budget is 256 MiB, configurable
up to 1 TiB; each opened file's initial size is charged before reading. Repeated
references consume the budget again. Objects are hashed in 64 KiB chunks rather
than retained in memory. OS reads on a stalled/network filesystem are **not** given
a hard wall-clock deadline; use a reviewed local mounted filesystem and supervisor
limits when needed. Exceeding a budget never truncates the requested reference list.

## Verification and remaining work

```sh
python3 scripts/test_capture_audit.py -v
cargo test --locked -p arb-capture --test capture_audit
```

The Python tests use explicitly synthetic manifests and exercise both networks,
identity/configuration binding, exact times, expiry, missing/corrupt/incomplete/
unsupported input, path and link refusal, byte limits, complete reporting,
non-disclosure, source-content preservation and real CLI exit codes.

The Rust tests call **the actual `arb_capture::write_bundle`** for Base and Solana
and run the Python CLI on those files, including expired/missing object cases.
Python 3.11+ is consequently required for this workspace interoperability test;
missing Python is a failure, never a silently skipped pass. These fixtures prove
storage-format interoperability, not qualified mainnet acquisition or protocol replay.
The Recovery review workflow runs the Python audit tests and lint alongside the
existing backup/recovery checks; normal workspace CI runs the Rust tests.

Before closing original #28: accept its ARB-008/009/010 prerequisites, verify a real
provider-backed redacted capture and complete application/report integration.
Before closing #55: integrate per-volume availability evidence with authenticated
exports, actual summaries and qualified asset metadata. Before closing #58:
application-aware replay/ledger recovery, worker restart/degraded-provider tests,
scheduled retention and protected offsite backups remain. No signing/broadcasting,
funding, provider purchase or research-worker activation is part of this change.
