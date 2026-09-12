# research-worker

An OBSERVE-only worker process with PostgreSQL session controls and real read-only Base/Uniswap V3 or Solana/Orca capture. It has no quoting, opportunity classification, paper fills, transaction simulation, signing, broadcasting or live capability.

## Prerequisites and startup

Create an OBSERVE session through the control API using a validated, enabled configuration and the exact configuration digest. The API and worker must use the same configuration. The supplied disabled example deliberately cannot start market capture: qualify the actual pool/program/code identities and complete its allowlists first. Do not substitute fixture qualification data for a real deployment review.

Set references to existing files and the session:

```sh
export ARB_WORKER_CONFIG=/app/config/research.observe.toml
export ARB_POOL_REGISTRY=/app/config/qualified-pool.json
export ARB_OPERATOR_ID=your-operator-id
export ARB_SESSION_ID=the-api-created-session-id
cargo run --locked -p research-worker
```

Resolve the configuration's `storage.database_secret_reference` and network `rpc_secret_reference` in the process environment, for example `env:DATABASE_URL` and `env:BASE_RPC_URL`. Values stay outside the repository. Configured capture directory must be on persistent storage. Run one worker process per capture volume; use separate directories/volumes for chain workers. The DB session lease rejects a second active owner of that session. There is no automatic live failover or auto-start.

For Railway, run the binary as a separate worker service with one replica, a persistent volume at the configured capture path, and PostgreSQL's private `DATABASE_URL`. No public worker port is needed. Do not configure an HTTP health check for this worker binary. Avoid overlapping deployments: the replacement rejects an active lease and starts only after its prior owner releases/expires. A replacement starts RECOVERING then STOPPED and rejects any pending command inherited from the previous owner. Issue a fresh START only after the new worker is STOPPED.

## Control and capture behavior

1. Startup validates mode, enabled network, matching immutable configuration digest, registry digest, pool/assets and expected genesis/code/program identity through the adapter. It claims a 15-second research lease and completes recovery only with no unresolved research attempts. It remains STOPPED.
2. A raw capture qualifies read-only input acquisition. START remains PENDING until that acquisition has succeeded recently. This readiness is only for raw capture; quote completeness remains false. Loss of readiness while RUNNING faults the worker and requires explicit recovery.
3. PostgreSQL controls poll every 250 ms independently of RPC capture. At most one blocking capture runs, with a 5-second per-request timeout and the transport's 60-second total deadline. A fresh capture begins five seconds after its predecessor completes. This interval targets research collection; latency-sensitive route search is separate work.
4. Capture inputs are written to an immutable bundle and fsynced before admission. Only results tagged while RUNNING and still carrying the current generation can be admitted. The admission stores a unique research-attempt ID linked to the manifest digest, path and generation in one database transaction. That attempt is resolved atomically as read-only evidence; no financial outcome or outstanding transaction is invented.
5. PAUSE and STOP fence admission and invalidate older results. Feed capture may continue while paused/stopped. An already running request cannot be recalled. Its completed bundle is retained as `UNADMITTED_RAW_CAPTURE`; it is never an opportunity or paper fill. This distinction is visible in structured logs and the durable admission table.
6. SIGINT/SIGTERM closes local admission immediately, then waits for the bounded read to finish. PostgreSQL retains its last observed session state until lease expiry/recovery; shutdown logging never fabricates an APPLIED operator command.

The capture quota includes all retained files under the dedicated configured root, including old process captures and incomplete bundles. The bounded scan rejects symlinks, excessive nesting and over 100,000 entries. A quota/validation/transport failure closes a running pipeline. Retention metadata does not automatically delete files: deletion/retention operations remain a separate ticket. Keep one process per volume because this initial quota check is not a cross-process storage reservation service.

## Recovery evidence

A completed bundle without a matching `capture_admissions` row is unadmitted raw evidence. A row contains `session_id`, `capture_id`, `attempt_id`, `manifest_digest`, `artifact_path`, `generation` and admission time. Inspect the bundle against that exact digest using the chain worker's `inspect` command. Do not promote orphaned files automatically after a restart or use them as financial results.

All malformed endpoint errors are redacted. Registry/config files and capture snapshots do not contain resolved endpoint secrets. The control/adapter/capture integration tests and transcript fixtures cover deterministic behavior; a successful real market capture still requires qualified configuration, provider access and an observation window on the intended deployment.

The process integration test uses manually constructed fixtures served over loopback HTTP; those bundles are explicitly labelled `ManuallyConstructed`. It blocks an RPC request, applies STOP through PostgreSQL while that request is still blocked, verifies the late bundle has no admission, then requires a new START before admitting a fresh capture. The test does not establish provider reliability or market profitability.
