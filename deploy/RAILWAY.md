# Railway deployment runbook

Railway is the selected host, confirmed by the user on 12 September 2026. This is a prepared deployment definition. No Railway project, billable resource or deployment has been created by this implementation session; an authenticated Railway connection and target environment are not available here.

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
2. Select or create a dedicated Railway project and environment for `arbitrage-research`. Do not link this definition to an unrelated existing project: a full desired-state plan can affect resources that are absent from its resource list.
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

Select region and capacity after measuring provider RTT and expected capture size; this runbook assigns no invented latency benefit to a region. Keep queues and recording quotas bounded. A capture volume is not a backup. Set and test database backups and capture export/retention before an observation campaign.

Pause or STOP through the authenticated API and verify the per-session worker receipt. PENDING is not APPLIED. Stop the worker process only after the requested fence and relevant reconciliation are known. Railway service shutdown, redeployment or database shutdown is not proof of a durable STOP. After restart, the worker recovers fenced and requires a fresh START; loss of readiness faults the session rather than silently resuming. In OBSERVE and PAPER, PAUSE/STOP fence candidate evaluation/admission while raw acquisition can continue. Stop the process separately to stop all provider reads. Standalone capture CLIs are outside these durable session controls.

## Validation boundary

The source includes container build definitions and CI validation. Actual Railway deployment, region RTT measurements, volume permissions, backup restore and provider behavior must be recorded against the deployed image/commit before ARB-004 or ARB-044 can be accepted. Container base images use named versions; resolve and record immutable image digests and vulnerability review before a production release. Research services expose no signing or transaction broadcasting capability.

## Add qualified research workers

`deploy/Dockerfile.worker` builds the controlled OBSERVE/PAPER research binary. Its entrypoint prepares `/data/captures` on the mounted volume and drops to UID/GID 10001 before reading configuration or starting the worker. Supply `ARB_WORKER_CONFIG`, `ARB_POOL_REGISTRY`, `ARB_OPERATOR_ID=operator`, and the API-created `ARB_SESSION_ID`, plus the environment variables named by the immutable configuration's secret references. No API operator secret is needed in a worker.

The API-created session and worker must use the exact same validated enabled configuration, registry digest and immutable mode. To initialize virtual capital, create a PAPER session, let its worker recover to STOPPED, then create the immutable account through the connected Runs page or paper-run API. Pending commands can block creation. Starting that PAPER worker only captures inputs and stores CANDIDATE decisions: it does not automatically reserve capital or settle quotes into balances. Keep token principal and native fee inventory separate, and retain the original idempotency key if account creation has an uncertain response. See [research and paper accounts](../wiki/Using-Research-and-Paper-Accounts.md).

The default IaC graph intentionally provisions only the deployable API/dashboard foundation. Add each worker and its own volume to that same full graph after recording the actual configuration, session and registry choices. Keep those additions in IaC before applying again, since omitting an existing managed service from a full desired-state graph can propose removal. Use `replicas: 1`, `RAILWAY_DOCKERFILE_PATH: "deploy/Dockerfile.worker"`, `volumeMounts: { "/data": workerVolume }`, and no HTTP healthcheck. Pin a deployment commit and verify the plan before provisioning; never set a worker start command that automatically issues START.
