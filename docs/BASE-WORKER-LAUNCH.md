# Launch the registered Base observation worker

Existing #58 / ARB-044, using the source and publication boundaries in #30/#32.
The operator account and existing Base RPC setup are complete. Do not repeat them.

## Existing state and first launch

The anchored profile is `/data/runtime/base-v1`. `worker-session --status` reads
its original registration and validates the exact configuration and registry.
The launcher uses that real session ID, not an invented ID or the most recent
session in an unscoped list. It never creates or resets a session or issues START.

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

The launcher's private file lock is held across exec of the existing research
worker. It prevents two launchers sharing this volume from starting together;
it is not a distributed lock against another service or a hostile volume writer.
The original database lease and exact source binding remain authoritative.

Only selected settings reach child processes. The database connection requires
TLS; transport encryption does not claim authenticated server identity.
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

One configured source and session remain the scope. The existing 16-block recovery
limit, shared RPC time/byte/request budgets, capture quota and invalidation rules
are unchanged. A gap beyond those bounds still fails rather than skipping history.
A running process is not proof of qualified data, complete simulation, automatic
virtual settlement or executable profit. Paper/Real navigation remains unchanged;
this worker has no signing or broadcast capability.

## Verification

`test_worker_launch_base.py` checks offline boundaries, including no implicit
initialization, anchored session scope, differing registry files, source refusal,
private environment/TLS, bounded subprocess output and inert invocation. Test
doubles are not real provider evidence. The existing mandatory image/session drill
also runs the shipped wrapper against its TLS-enabled disposable PostgreSQL and
verifies missing-source refusal without new sessions, streams or commands.

Actual CI, deployed revision, provider observations, source and session state,
and remaining START/STOP/restart tests are recorded on #58 and the implementing PR.
No whole original task or epic is closed by a launcher or successful deployment.
