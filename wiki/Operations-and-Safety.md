# Operations and safety

Current source includes authenticated research controls, durable PostgreSQL history, bounded read-only adapters, virtual accounting and a connected dashboard. Railway deployment is prepared, but deployed service/volume checks, provider qualification and remaining acceptance are not established by source alone. Read [implementation status](https://github.com/makafeli/arbitrage-research/blob/main/docs/12-IMPLEMENTATION-STATUS.md) and the [security/operations requirements](https://github.com/makafeli/arbitrage-research/blob/main/docs/06-SECURITY-OPERATIONS-AND-TESTING.md).

## Research boundary

A research deployment has no production trading key, signer socket or transaction-broadcast API. Current RPC access is restricted to a closed read-only method list with bounded request, response and time budgets. Complete transaction simulation is a later capability and must use explicitly declared virtual funding where needed. A paper flag alone is not an adequate capability boundary.

Providers, market state, token metadata and account/program responses are untrusted inputs. Allowlist exact identities and supported behavior; reject incomplete, contradictory or upgraded state until qualified. Never copy a ticker, fixture identifier or unverified contract address into an enabled production registry.

## Start, pause and stop

| State | Operator meaning |
|---|---|
| RECOVERING | Restore durable state and reconcile outstanding work; no new trades |
| STOPPED | No new submissions and no unresolved dispatched attempts |
| RUNNING | Eligible work proceeds within this immutable mode |
| PAUSING | Closing admission/submission and establishing the local fence |
| PAUSED | No new evaluations/submissions; feeds and reconciliation continue |
| DRAINING | Fence applied; previously dispatched outcomes still need resolution |
| FAULTED | Blocking failure requires remediation; read-only reconciliation continues where possible |

Boot enters RECOVERING and reaches STOPPED before explicit start. A command accepted by the API is PENDING. APPLIED confirms the worker's effective local fence, with revision and outstanding count. Stop remains DRAINING while outcomes are unresolved. A browser timeout or force-killed process cannot prove cancellation.

The current OBSERVE/PAPER worker can keep collecting raw state while PAUSED or STOPPED. A completed call from an old generation can remain an unadmitted capture, but it cannot become a newly admitted decision after the effective fence. Stop the process separately when no more provider reads should occur. Standalone capture CLIs are not controlled by dashboard session commands.

For future live execution, already admitted network calls may still execute. An in-flight signer request may finish, but returned bytes must be quarantined after the local fence. Later DISARM also requires durable signer-epoch revocation acknowledgement. An unreachable signer leaves that acknowledgement pending; previously signed bytes are not cryptographically invalidated.

## Failure and recovery

Current research admission requires a valid PostgreSQL worker lease, matching configuration and generation, and a durably recorded capture. Storage, stale ownership or capture failures cannot be replaced with an in-memory success acknowledgement. A restarted worker recovers fenced, reaches STOPPED and needs a fresh START. See the [controlled worker runbook](https://github.com/makafeli/arbitrage-research/blob/main/apps/research-worker/README.md) for exact budgets and recovery behavior.

Railway uses a public web origin and private API/PostgreSQL, one API replica and a dedicated volume for each worker. API restart invalidates its process-local browser login; durable sessions remain in PostgreSQL. Verify volume permissions/fsync, backups and restore against the deployed commit before accepting operational readiness. The [Railway runbook](https://github.com/makafeli/arbitrage-research/blob/main/deploy/RAILWAY.md) records the prepared layout without claiming deployment.

The following dispatch rules are future live requirements. Missing durable storage blocks new live dispatch. Before every send, commit an UNKNOWN dispatch-start record tied to the exact transaction identity. Timeouts remain unknown until chain evidence resolves them. Never blindly refresh a nonce/blockhash or create a fresh trade to make an ambiguous outcome disappear.

A database lease is cooperative fencing, not revocation of signed bytes. Initial live deployment has no automatic takeover before the former process is fenced or terminated and outstanding attempts are reconciled. Restored backups begin in RECOVERING and check current chain state before allowing any start.

Provider gaps invalidate affected snapshots. Reorganizations invalidate dependent state and provisional accounting. Storage pressure sheds replaceable telemetry first, never durable intents. Unexplained reconciliation differences block further activity for the affected account.

## Targets and retention

The initial proposed operational targets remain **unmeasured**: healthy-host stop-fence acknowledgement within two seconds; critical local faults visible within 30 seconds; a 24-hour bounded-memory soak with no lost durable intents; backup recovery point within 15 minutes; isolated restore within 60 minutes. They are acceptance targets, not achieved guarantees. Freshness limits remain chain/strategy-specific and require calibration.

Proposed retention is seven days of detailed selected-pool capture, 30 days of reproducibility bundles and 12 months of compact trade/configuration/audit records. Capacity measurements and explicit policy determine actual values. A missing raw capture must be visible in historical replay claims. Current capture quota includes retained old and incomplete bundles; declared expiry/retention metadata does not automatically delete files. Capacity and cleanup/export operations require an explicit policy. Page exports retain selected received data; session snapshot exports freeze defined database records and catalog references. Neither proves complete raw history remains available. Collection starts and terminal diagnostics are durable operational evidence; they are not disposable metrics and must not be silently pruned.

## Future live gate

Live execution requires reviewed exact transactions, bounded authority and capital, separate signer custody, tested atomic guards, durable journal/reconciliation, independent review and explicit operator activation. On-chain surplus guards cannot guarantee business profit after all external fees. Repository setup and passing paper tests do not authorize funding or arm a live session.
