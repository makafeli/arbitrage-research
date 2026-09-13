# arb-storage

PostgreSQL session and command journal for OBSERVE/PAPER/REPLAY. `Store` applies embedded migrations and persists operator-scoped immutable configurations, idempotent session creation, bounded session queries, revisioned control acceptance, command receipts and append-only audit events. SQL row locks serialize concurrent commands; STOP supersedes older PENDING intent without inventing an APPLIED acknowledgement.

Wire records match the v1 OpenAPI field shapes. Revisions use decimal strings and PostgreSQL's signed 64-bit nonnegative range, returning an explicit error at exhaustion. Missing configuration digests cannot create sessions; callers register validated redacted server configuration first. PostgreSQL errors are deliberately redacted in Display output. DTO validation does not authorize providers or strategies; the API validates those against its server configuration.

Worker-only methods support epoch/lease ownership, explicit recovery, local-fence acknowledgement, durable research-attempt identities with idempotent resolution and durable state. The API must not invoke worker acknowledgement. `arb-control` supplies the cooperating exclusive local gate contract. These are research controls, without signing, transaction transmission, automatic live failover or financial accounting.

Run `TEST_DATABASE_URL=postgres://... cargo test -p arb-storage`. Database integration tests intentionally fail if PostgreSQL is not configured; they never silently skip. See [migration recovery instructions](../../migrations/README.md).

An active worker lease currently reports `DEGRADED`, since a control heartbeat alone does not establish protocol/data readiness. Missing heartbeat reports UNKNOWN; an expired lease with prior heartbeat reports UNREACHABLE. Chain/data health aggregation is separate future integration work.

OBSERVE capture admissions link a resolved research attempt to a committed immutable bundle path/digest/generation. Raw capture admission is not a trade, paper fill, or REALIZED result. The artifact must be committed before database admission, and orphaned raw artifacts remain unadmitted after fencing or a crash.

Collection telemetry records a bounded batch identity before capture I/O, including READINESS batches that run with admission closed. `begin_collection_attempt` commits this identity before the worker starts its task. `finish_collection_attempt` records typed terminal evidence without storing transport messages or credentials; missing terminal evidence stays IN_PROGRESS, including after a crash or replacement. Successful decision batches use `append_collection_decision_traces`, which atomically commits the normal fenced decisions and the terminal receipt. Exact observation IDs link each decision to at most one collection batch. This telemetry never changes execution/drain counters or opens the local gate.

Collection coverage counts registered batches only. Readiness, research, provider/acquisition errors, evaluation errors, deadlines and deliberate suppression remain distinguishable. The complete observation schedule and the failed input's market/synthetic origin are unknown; these counts cannot establish market coverage or profitability. Listings are bounded live pages, and counts are separate from candidate, fill or transaction counts. See [research accounting](RESEARCH-ACCOUNTING.md) for receipt and recovery rules.


Migration `0004_cost_assessments.sql` adds append-only cost assessment records and a
composite source-decision foreign key. Creation derives a typed `arb-paper` assessment
from the stored QUOTED trace and manual scenario, serializes concurrent idempotent retries
with a scoped advisory lock, and commits only the sidecar plus its audit receipt. It never
updates the decision, session lifecycle, paper journal or balances. Previous migrations
remain unchanged. The source trace, request, payload and deterministic assessment are
cross-checked on read and export; a matching payload hash alone cannot authenticate a
changed report. The original scenario and report remain reproducible after process restart.

Frozen export 1.1.0 includes six datasets and six source counts. Cost assessment rows and
payload bytes count toward the existing all-or-error 10,000-row/8 MiB bounds. Rows are
validated against source decisions from the same read-only REPEATABLE READ snapshot;
concurrent new decisions/assessments cannot change counts or reappear midway through an
export. Native fees require explicit valuations; negative nets remain exact signed strings
and missing costs remain null. This adds manual research assumptions, not cost estimates
from a provider, transaction simulation, execution eligibility or automatic paper fills.

Decision schema 1.1.0 retains optional chain-time observations and recomputes their
arithmetic, statuses, ordered capture bindings and content-addressed identity when reading.
The exact `ChainFreshnessPolicy` is also compared with the same operator/configuration's
immutable snapshot on append, retry, decision/opportunity reads, cost-source reads and
frozen export. Policy removal or replacement cannot be legitimized by resealing a trace.
Malformed present policy values fail closed. Existing absent-policy schema 1.0.0 records
retain their serialization and hashes. Reads project the small policy value and immutable
session network in the existing SQL query rather than fetching a full configuration per
record. This verifies policy binding; full configuration validation remains the registration
caller's responsibility. No schema migration is required.
