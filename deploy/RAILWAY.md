# Railway deployment runbook

Railway is the selected host, confirmed by the user on 12 September 2026. The initial `postgres` + private `control-api` + public `web` foundation was applied to the dedicated `arbitrage-research` production environment on 13 September 2026; see the [deployment verification](../docs/21-RAILWAY-DEPLOYMENT-VERIFICATION.md). The checked-in definition remains the desired-state source for later reviewed changes.

## Services and isolation

| Service | Public access | Persistent state | Startup behavior |
|---|---|---|---|
| `web` | One Railway HTTPS domain or selected custom domain | None | Caddy serves the built dashboard and proxies `/v1/*` to the API |
| `control-api` | Private networking only | PostgreSQL | Validates configuration, applies migrations, then serves authenticated requests |
| `postgres` | Private connection string | Railway database storage | Stores immutable configurations, sessions, commands and audit data |
| Base research worker | None; add after qualification | Dedicated capture volume and PostgreSQL | Claims an existing OBSERVE or PAPER session with the same frozen mode/configuration, recovers fenced, waits for explicit START |
| Solana research worker | None; add after qualification | A different capture volume and PostgreSQL | Independent OBSERVE or PAPER session and worker epoch; same configuration/recovery rule |

Use one API replica because browser authentication is stored in that process. Use one owner for each research session. API restart revokes browser login cookies, while durable research state remains. Workers must not share writable capture volumes or be scaled horizontally as a latency shortcut.

The API binds to `::` inside its container only after an explicit HTTPS public origin is configured. Local development continues to default to loopback. Railway's private network connects services within the same project environment; the public browser speaks only to the web origin. [Railway private networking](https://docs.railway.com/networking/private-networking)

## Prepare and review the environment

1. Merge the tested implementation into the repository branch selected by the deployment definition.
2. Link the dedicated Railway project and `production` environment for `arbitrage-research`. Do not link this definition to an unrelated existing project: a full desired-state plan can affect resources that are absent from its resource list.
3. Configure shared variables `ARB_OPERATOR_SECRET` and `ARB_PUBLIC_ORIGIN` in Railway. Generate the operator secret with a password manager or a cryptographically secure generator; it must contain at least 32 bytes as text. The origin is the exact HTTPS dashboard origin, with no path or trailing slash.
4. Authenticate the Railway CLI and link the intended project/environment. Keep secret values in Railway; the definition uses references, and the frontend receives neither database credentials nor RPC secrets.
5. Evaluate the plan and inspect resource additions, deletions, volumes, service count, region and projected resource settings before applying it.

```sh
railway login
railway link
railway config plan
```

Use the TypeScript configuration in `.railway/railway.ts`. Railway documents this as the replacement for legacy `railway.json` / `railway.toml`; new services cannot opt into that older mechanism. `railway config apply` performs the reviewed changes. This repository does not auto-apply infrastructure from CI. [Railway IaC workflow](https://docs.railway.com/infrastructure-as-code)

After the web service exists, generate its Railway domain or attach the chosen domain, update the exact shared HTTPS origin and redeploy the API. Configure **no public domain or TCP proxy** for the API/database. Check the web `/healthz`, authenticated API health and session capabilities separately. Caddy preserves the browser's Origin and the API's secure cookie behavior.

Both networks in the shipped configuration are disabled. A healthy deployment can authenticate and report capabilities while correctly rejecting session creation. It is not an active arbitrage service. Register qualified enabled configurations and pool registries before adding research workers; deployed-protocol qualification, complete simulation and automatic paper scenarios remain separate gates.

## Worker volumes, shutdown and recovery

Each worker needs a dedicated persistent volume mounted at `/data`, with its configured capture directory under `/data/captures`. Volumes are available at runtime, not during image build or pre-deploy hooks. Provision ownership for the unprivileged worker process and verify a write/fsync/readback before enabling capture. A Railway volume is attached to one service, so use separate Base and Solana volumes. [Railway volume behavior](https://docs.railway.com/volumes), [IaC volume configuration](https://docs.railway.com/infrastructure-as-code/reference)

Select region and capacity after measuring provider RTT and expected capture size; this runbook assigns no invented latency benefit to a region. Keep queues and recording quotas bounded. A capture volume is not a backup. Once used space crosses 80% of `plan.quota_bytes` the worker itself prunes the oldest committed raw bundles, admitted ones included, down to 50% before their `raw_expires_at_ms`; this keeps a fixed-quota worker running, it is not long-term retention. Pruning runs before every collection attempt, research or readiness alike, so pausing or stopping a session does not protect a bundle from it. The `capture_admissions` rows (capture id, `manifest_digest`) in PostgreSQL survive a pruned bundle, but `scripts/export_capture_audit.py` only audits presence (`MISSING`/`COMPLETE_WITH_GAPS`) — it copies nothing. To keep raw evidence, copy the bundle directories to `/data/archive` (a sibling of `/data/captures`, never pruned) before pruning reaches them, and set and test database backups before an observation campaign.

Pause or STOP through the authenticated API and verify the per-session worker receipt. PENDING is not APPLIED. Stop the worker process only after the requested fence and relevant reconciliation are known. Railway service shutdown, redeployment or database shutdown is not proof of a durable STOP. After restart, the worker recovers fenced and requires a fresh START; loss of readiness faults the session rather than silently resuming. In OBSERVE and PAPER, PAUSE/STOP fence candidate evaluation/admission while raw acquisition can continue. Stop the process separately to stop all provider reads. Standalone capture CLIs are outside these durable session controls.

## Worker deploys: watch paths, start-command changes, stuck deployments

`base-research-worker` builds from GitHub `main`, and every deployment restarts the worker: the running OBSERVE session drops to STOPPED and needs a fresh START (#178). Since 2026-09-19 the service therefore has watch paths (`/apps/research-worker/**`, `/apps/evm-worker/**`, `/crates/**`, `/migrations/**`, `/Cargo.toml`, `/Cargo.lock`, `/rust-toolchain.toml`, `/.dockerignore`, `/deploy/Dockerfile.worker`, `/deploy/worker-*`, `/scripts/worker_*`, `/scripts/recorded_base_slice.py`, `/scripts/collect_operator_base_evidence.py`, `/scripts/inspect_pool_candidates.py`, `/config/research.example.toml`, `/docs/registries/**`, `/specs/*.example.json`). A merge that touches none of them is shown as SKIPPED and leaves the worker alone. Read the service config before relying on the list; a new worker file outside these paths needs a pattern first.

Change the start command (for example the one-time `--initialize-and-start` of a new generation, `docs/BASE-WORKER-LAUNCH.md`) only as a **staged** service change followed by an explicit deploy of the staged changes. An unstaged `startCommand` update applies live and triggers nothing, staging a value equal to the live one is a no-op, and a plain redeploy reuses the previous deployment's already resolved command. Set the command back to `--start` the same way and confirm that the service config shows `--start` with no staged changes before the owner issues START. Never redeploy a deployment whose resolved command was `--initialize-and-start`.

The pre-deploy CI gate (`scripts/worker_ci_gate.py`) uses the unauthenticated GitHub API. A rate-limited or unreachable API blocks the deployment with `CI_GATE_BLOCKED / CI_API_UNAVAILABLE` (#181 adds a token). Such a deployment can stay at BUILDING with `deploymentStopped=true` and holds every later deployment in QUEUED. Before cancelling it, list the QUEUED deployments and their resolved start commands: cancelling the blocker starts the next queued deployment by itself, so cancel any queued `--initialize-and-start` deployment first and release only one whose resolved command is `--start`. Cancel in the dashboard, or with the `deploymentCancel` GraphQL mutation run by the owner in a terminal (the repository hook blocks Railway mutations for agents).

## Validation boundary

The source includes container build definitions and CI validation. Actual Railway deployment, region RTT measurements, volume permissions, backup restore and provider behavior must be recorded against the deployed image/commit before ARB-004 or ARB-044 can be accepted. Container base images use named versions; resolve and record immutable image digests and vulnerability review before a production release. Research services expose no signing or transaction broadcasting capability.

## Add qualified research workers

`deploy/Dockerfile.worker` builds the controlled OBSERVE/PAPER research binary. Its entrypoint prepares `/data/captures` on the mounted volume and drops to UID/GID 10001 before reading configuration or starting the worker. Supply `ARB_WORKER_CONFIG`, `ARB_POOL_REGISTRY`, `ARB_OPERATOR_ID=operator`, and the API-created `ARB_SESSION_ID`, plus the environment variables named by the immutable configuration's secret references. No API operator secret is needed in a worker.

The API-created session and worker must use the exact same validated enabled configuration, registry digest and immutable mode. To initialize virtual capital, create a PAPER session, let its worker recover to STOPPED, then create the immutable account through the connected Runs page or paper-run API. Pending commands can block creation. Starting that PAPER worker only captures inputs and stores CANDIDATE decisions: it does not automatically reserve capital or settle quotes into balances. Keep token principal and native fee inventory separate, and retain the original idempotency key if account creation has an uncertain response. See [research and paper accounts](../wiki/Using-Research-and-Paper-Accounts.md).

The default IaC graph intentionally provisions only the deployable API/dashboard foundation. Add each worker and its own volume to that same full graph after recording the actual configuration, session and registry choices. Keep those additions in IaC before applying again, since omitting an existing managed service from a full desired-state graph can propose removal. Use `replicas: 1`, `RAILWAY_DOCKERFILE_PATH: "deploy/Dockerfile.worker"`, `volumeMounts: { "/data": workerVolume }`, and no HTTP healthcheck. Pin a deployment commit and verify the plan before provisioning; never set a worker start command that automatically issues START.

## Optional chain-time and support diagnostics

`config/chain-freshness.example.toml` provides explicit disabled-network policy examples. The shipped historical default is unchanged; select and mount an immutable reviewed configuration deliberately before starting a policy-enabled worker. Limits are research assumptions until provider baselines are measured. API and frontend must ship together for DecisionTrace1.1. No migration is needed for this change.

The API can additionally read `ARB_SUPPORT_REGISTRY_FILES=base-mainnet=/app/registries/base.json,solana-mainnet=/app/registries/solana.json`. Mount those reviewed files read-only in the API service; the worker retains its own registry configuration. This optional catalog performs no RPC or activation and does not monitor edits after startup. Omit the variable to get explicit NOT_LOADED registry status. Never place credentials in registry files. Container builds do not prove a deployed Railway configuration.
