# arb-storage

PostgreSQL session and command journal for OBSERVE/PAPER/REPLAY. `Store` applies embedded migrations and persists operator-scoped immutable configurations, idempotent session creation, bounded session queries, revisioned control acceptance, command receipts and append-only audit events. SQL row locks serialize concurrent commands; STOP supersedes older PENDING intent without inventing an APPLIED acknowledgement.

Wire records match the v1 OpenAPI field shapes. Revisions use decimal strings and PostgreSQL's signed 64-bit nonnegative range, returning an explicit error at exhaustion. Missing configuration digests cannot create sessions; callers register validated redacted server configuration first. PostgreSQL errors are deliberately redacted in Display output. DTO validation does not authorize providers or strategies; the API validates those against its server configuration.

Worker-only methods support epoch/lease ownership, explicit recovery, local-fence acknowledgement, durable research-attempt identities with idempotent resolution and durable state. The API must not invoke worker acknowledgement. `arb-control` supplies the cooperating exclusive local gate contract. These are research controls, without signing, transaction transmission, automatic live failover or financial accounting.

Run `TEST_DATABASE_URL=postgres://... cargo test -p arb-storage`. Database integration tests intentionally fail if PostgreSQL is not configured; they never silently skip. See [migration recovery instructions](../../migrations/README.md).

An active worker lease currently reports `DEGRADED`, since a control heartbeat alone does not establish protocol/data readiness. Missing heartbeat reports UNKNOWN; an expired lease with prior heartbeat reports UNREACHABLE. Chain/data health aggregation is separate future integration work.

OBSERVE capture admissions link a resolved research attempt to a committed immutable bundle path/digest/generation. Raw capture admission is not a trade, paper fill, or REALIZED result. The artifact must be committed before database admission, and orphaned raw artifacts remain unadmitted after fencing or a crash.
