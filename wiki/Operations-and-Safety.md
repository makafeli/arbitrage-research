# Operations and safety

The current scaffold is for local development and synthetic demonstrations. Production authentication, durable storage, chain adapters and the complete operational controls remain delivery work. Do not treat an example service as the completed private deployment. Read [implementation status](https://github.com/makafeli/arbitrage-research/blob/main/docs/12-IMPLEMENTATION-STATUS.md) and the [security/operations requirements](https://github.com/makafeli/arbitrage-research/blob/main/docs/06-SECURITY-OPERATIONS-AND-TESTING.md).

## Research boundary

A research deployment has no production trading key, signer socket or transaction-broadcast API. Selected RPC access is limited to required reads and complete transaction simulation, with explicit virtual funding where needed. A paper flag alone is not an adequate capability boundary.

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

Already admitted network calls may still execute. An in-flight signer request may finish, but returned bytes are quarantined after the local fence. Later DISARM also requires durable signer-epoch revocation acknowledgement. An unreachable signer leaves that acknowledgement pending; previously signed bytes are not cryptographically invalidated.

## Failure and recovery

Missing durable storage blocks new live dispatch. Before every send, commit an UNKNOWN dispatch-start record tied to the exact transaction identity. Timeouts remain unknown until chain evidence resolves them. Never blindly refresh a nonce/blockhash or create a fresh trade to make an ambiguous outcome disappear.

A database lease is cooperative fencing, not revocation of signed bytes. Initial live deployment has no automatic takeover before the former process is fenced or terminated and outstanding attempts are reconciled. Restored backups begin in RECOVERING and check current chain state before allowing any start.

Provider gaps invalidate affected snapshots. Reorganizations invalidate dependent state and provisional accounting. Storage pressure sheds replaceable telemetry first, never durable intents. Unexplained reconciliation differences block further activity for the affected account.

## Targets and retention

The initial proposed operational targets remain **unmeasured**: healthy-host stop-fence acknowledgement within two seconds; critical local faults visible within 30 seconds; a 24-hour bounded-memory soak with no lost durable intents; backup recovery point within 15 minutes; isolated restore within 60 minutes. They are acceptance targets, not achieved guarantees. Freshness limits remain chain/strategy-specific and require calibration.

Proposed retention is seven days of detailed selected-pool capture, 30 days of reproducibility bundles and 12 months of compact trade/configuration/audit records. Capacity measurements and explicit policy determine actual values. A missing raw capture must be visible in historical replay claims.

## Future live gate

Live execution requires reviewed exact transactions, bounded authority and capital, separate signer custody, tested atomic guards, durable journal/reconciliation, independent review and explicit operator activation. On-chain surplus guards cannot guarantee business profit after all external fees. Repository setup and passing paper tests do not authorize funding or arm a live session.
