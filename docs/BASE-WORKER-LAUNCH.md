# Launch the registered Base observation worker

Existing #58 / ARB-044, using the source and publication boundaries in #30/#32.
The operator account and existing Base RPC setup are complete. Do not repeat them.

## Existing state and first launch

The anchored profile is `/data/runtime/base-v1`. `worker-session --status` reads
its original registration and validates the exact configuration and registry.
The launcher uses that real session ID, not an invented ID or the most recent
session in an unscoped list. It never resets a session or issues START; it
registers a session only for generation ≥ 2 on `--initialize-and-start` (see
Generation).

The first explicitly authorized deployment command is:

```text
worker-entrypoint worker-launch-base --initialize-and-start
```

The existing initializer checks source absence before any provider request,
verifies finalized Base identity and atomically creates its seed. Existing ACTIVE
or HALTED sources are refused rather than reset. Initialization and exec run in
one container so a separate image build cannot age the new checkpoint before
its first collection. No automatic retries or alternate provider are introduced.
An interrupted initializer has an uncertain outcome: inspect the stored source
and use normal startup if it exists. Never reset/reseed to make a retry pass.

After confirmed initialization, retain this command for subsequent deployments:

```text
worker-entrypoint worker-launch-base --start
```

This is the command declared in Railway IaC. It cannot create a missing source.
The initializer flag is an explicit, first-use operation, not a permanent restart
policy. Do not apply the entire IaC graph without checking the target plan.
Preserve the separately managed RPC secret, other services and the existing volume.

## Runtime and control boundaries

The CI gate (`worker_ci_gate.py`) runs twice per Railway deployment: as the
pre-deploy command and again as the first step of `worker-launch-base` and
`worker-readiness-check` inside the main container (#58: Railway started the
container after the pre-deploy gate exited non-zero). The in-container gate runs
whenever `RAILWAY_ENVIRONMENT_ID` is set; it is skipped only off Railway, such
as the offline container tests. A blocked gate exits 2 before the launcher or
the inspection runs, so with restart policy NEVER the deployment ends CRASHED
and writes nothing; redeploying the same commit is a safe retry. A launch log
without `CI_GATE_PASSED` from the main container is not an authorized launch.

The launcher's private file lock is held across exec of the existing research
worker. It prevents two launchers sharing this volume from starting together;
it is not a distributed lock against another service or a hostile volume writer.
The original database lease and exact source binding remain authoritative.

Only selected settings reach child processes. The database connection requires
authenticated TLS (`verify-full`), including hostname matching. The public
`ARB_DATABASE_CA_PEM` is supplied through authenticated deployment configuration,
not obtained from an unverified database connection. It is parsed and bounded
before database access, then passed as `PGSSLROOTCERT` to the existing SQLx clients.
The session checker independently forces `VerifyFull`; no helper may downgrade it.
Missing/invalid trust, a different CA, an incorrect hostname or a malformed server
certificate blocks launch before source initialization. No fallback to `require`.
Proxy, account-secret, signer and arbitrary inherited settings are not forwarded.
Profile JSON is read through a no-follow, regular-file descriptor with a hard
1-MiB read limit, including when a file grows after its metadata check. Parent
directories remain trusted; this is not isolation from a hostile volume owner.
The ingestion registry must equal the two-pool document validated by Rust.
Stored source state must be ACTIVE and recorded; the worker independently checks
its complete typed binding before claiming the session and before collection.

No START is issued. The original worker claims the session, recovers to STOPPED,
collects bounded readiness input and awaits commands through the existing API.
The launcher replaces itself with that worker, preserving PID1 signal handling.
SIGTERM during preparation cancels the child and never proceeds to exec.

One configured source and session remain the scope. The existing 32-block
recovery limit, shared RPC time/byte/request budgets, capture quota and
invalidation rules are unchanged. Recovery fetches each step with ranged
`eth_getLogs` calls (`fromBlock`/`toBlock` over the pool addresses, at most
10 blocks per call since #202) instead of one call per block, and checks
factory/pool runtime code only at the step's
first and last block instead of at every block (Base runs Cancun/EIP-6780, so
identity at both ends implies identity in between; issue #180, 2026-09-19).
`plan.quota_bytes` itself is part of the session's `configuration_digest` and
cannot be raised without a new session. Once `/data/captures` crosses 80% of
that quota the worker prunes the oldest committed raw bundles itself, admitted
ones included, down to 50% before their `raw_expires_at_ms` — pressure relief
so a fixed quota survives a multi-day run, not long-term retention. Pruning
runs before every collection attempt, research or readiness alike, so a STOPPED
or PAUSED session does not protect a bundle from it. The `capture_admissions`
rows (capture id, `manifest_digest`) remain in PostgreSQL after a bundle is
pruned, but `scripts/export_capture_audit.py` only audits presence
(`MISSING`/`COMPLETE_WITH_GAPS`) — it copies nothing. To keep raw evidence, copy
the bundle directories to `/data/archive` (a sibling of `/data/captures`, never
pruned) before pruning reaches them. A finalized step beyond that 32-block
limit is no longer a single failing attempt: it is walked across consecutive
captures, each committing at most 32 blocks with nothing skipped and no limit
raised, until the step is closed. A capture is not admitted for research until
the source has reached its own anchor block. A gap the bounded walk cannot
close — a reorg, a finality
regression or a provider failure — still fails exactly as before, rather than
skipping history.
A running process is not proof of qualified data, complete simulation, automatic
virtual settlement or executable profit. Paper/Real navigation remains unchanged;
this worker has no signing or broadcast capability.

## Existing deployment certificate compatibility

Read-only inspection on 18 September 2026 found the existing PostgreSQL public
root CA and `postgres.railway.internal` SAN. The current server certificate also
has `basicConstraints CA:TRUE`. The application's Rustls verifier rejects a CA
used as an end-entity certificate, even with a trusted issuer and correct SAN.
The container drill reproduces this with ephemeral synthetic keys.

Before hosted activation, the server needs a proper `CA:FALSE`, `serverAuth` leaf
and the worker needs the authenticated public CA input. This is infrastructure
integration, not another account/RPC setup request. Never solve it by disabling
certificate verification or changing the research/session anchors. No database
certificate replacement or restart is performed by this launcher or runbook.
A certificate fix must be reviewed/tested separately, preserve the original CA,
keys and data, and include an actual successful authenticated connection.

## Generation

A halted source (`state=HALTED`, any `halt_reason`) can never re-arm: the database
trigger that protects an ingestion cursor rejects any `UPDATE` or `DELETE` of a
HALTED row, and a session's stream binding is immutable. Resetting `railway-base-profile-v1`
or its session is not an option — see #193. A **generation** is the reviewed way
forward: one number selects both a new stream id and a new session key, so the
worker can start again without weakening any database check and without touching
the halted stream, its `ingestion_invalidations` row, the old session or its
captures. Those all stay in place as evidence.

Bump the generation only after a halt the owner has recorded on #58. Do not bump
it to work around a transient failure, a slow provider or a review finding — those
are addressed on the existing stream. Generation is not a substitute for the
checkpoint-anchored rotation of an ACTIVE stream (#177/#183), which still applies
while the current stream is healthy.

**Owner procedure**, once a halt is recorded on #58:

1. Set the `ARB_BASE_GENERATION` deployment variable to the next number (`2`,
   `3`, ... — ASCII digits, no leading zero, `2`..`99`). Leaving it unset, or
   setting it to `1`, keeps today's single stream, byte-identical.
2. Deploy once with the start command `worker-entrypoint worker-launch-base
   --initialize-and-start`. For generation `N >= 2` this one-time deploy first
   runs `worker-session --register` itself, before anything else: it is
   refused while an older `base-mainnet` session is both in a running or
   transitional observed state (anything but `STOPPED`/`FAULTED`) and has a
   live worker lease — a real worker renews its 15s lease every 250ms, so a
   genuinely running session always fails registration this way. A session
   left behind by a crash (for example between the halt path's separate
   halt/finish/fault commits) has no live lease and never blocks. If refused,
   wait for the lease to expire (up to 15s) or stop that worker, then
   redeploy. The call is idempotent, so a retried deploy reuses the same
   generation-N session rather than erroring. Only after registration
   succeeds does the launcher seed the **new** stream
   (`railway-base-profile-v1.g<N>`) at the finalized tip through the existing
   `create_ingestion`/`STREAM_ALREADY_EXISTS` protection — it is not a
   continuation of the halted checkpoint — and then exec the worker.
   Generation 1 never registers here (its session predates this launcher), and
   `--start` never registers, for any generation.
3. Set the start command back to `worker-entrypoint worker-launch-base --start`
   for subsequent deployments, exactly as with generation 1.
4. Read the **new** session id from `worker-session --status` (its idempotency
   key is `railway-base-profile-v1.g<N>`, not the original one) and issue START
   on that session, not the halted generation's.

Neither step auto-initializes on `--start`, and neither issues START itself. The
worker itself is unchanged: it already receives its stream id and session id from
the launcher and has no generation concept of its own.

## Owner procedure: rotation of an ACTIVE stream

Rotation (#177/#183/#204) continues a still-**ACTIVE** stream into the next
generation before it hits the `MAX_BATCHES`/`MAX_STREAM_BYTES` retention
ceiling — unlike the generation bump above, which only ever follows a
recorded **HALT**. The old stream is never touched: it stays ACTIVE, keeps
its checkpoint and batches, and the new generation is anchored at the same
checkpoint the old one had at rotation time.

1. Record the planned rotation on #58 and send STOP to the currently running
   session.
2. Remove the running deployment in the Railway UI so no container keeps
   committing batches on the old stream. Its worker lease expires within 15s;
   `--rotate` refuses with `ROTATION_BLOCKED_BY_LIVE_LEASE` while any
   `base-mainnet` session still holds a live lease, so a refusal here almost
   always means this step was skipped.
3. Stage the `ARB_BASE_GENERATION` deployment variable at the next number and
   the start command `worker-entrypoint worker-launch-base --rotate-and-start`
   in the same patch, then deploy. This registers the new generation's
   session (as generation `>= 2` already does for `--initialize-and-start`),
   rotates the ingestion stream from generation `N-1` onto generation `N` at
   its current checkpoint, then confirms the new source is `ACTIVE`/
   `RECORDED_LIVE` before starting the worker. `ARB_BASE_GENERATION=1` is
   refused with `ROTATION_REQUIRES_GENERATION` — there is no generation `0`
   to rotate from. A refused or uncertain rotation call is reported as
   `ROTATION_REFUSED_OR_UNCERTAIN` and never starts the worker.
4. Stage the start command back to `worker-entrypoint worker-launch-base
   --start` for subsequent deployments, exactly as with a generation bump.
5. Read the new session id from `worker-session --status` and press START on
   that session, not the previous generation's.

`--rotate` itself never contacts a provider: it only reads
`ARB_INGEST_ROTATE_FROM`/`ARB_INGEST_STREAM_ID`, checks the live-lease and
next-generation conditions above, and performs the storage rotation. A retry
of the same rotation is idempotent and stays `ROTATED`.

## Verification

`test_worker_launch_base.py` checks offline boundaries, including no implicit
initialization, anchored session scope, differing registry files, source refusal,
private environment/TLS, bounded subprocess output and inert invocation, plus the
generation rules above (absent/`1` byte-identical, `2`..`99` selecting
`railway-base-profile-v1.g<N>`, anything else `GENERATION_REJECTED` before any
child process). Test doubles are not real provider evidence. The existing
mandatory image/session drill also runs the shipped wrapper against its
TLS-enabled disposable PostgreSQL and verifies wrong-CA/hostname/CA-leaf and
missing-source refusals without new sessions, streams or commands, and now also
a generation-2 registration after a non-running generation-1 session. The Rust
key-derivation and guard-predicate unit tests live alongside `worker-session.rs`.

Actual CI, deployed revision, provider observations, source and session state,
and remaining START/STOP/restart tests are recorded on #58 and the implementing PR.
No whole original task or epic is closed by a launcher or successful deployment.
