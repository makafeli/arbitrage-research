# Private Base worker deployment preparation

Existing task: ARB-044 / #58. This record is not acceptance of the full deployment,
provider qualification, automatic paper settlement or live execution.

## Current continuation: enforced pre-deploy CI and read-only readiness

The native worker `checkSuites` switch did not persist through the connected
configuration action. It must not be described as enabled. The worker now has an
independent pre-deploy command:

```text
python3 -I /usr/local/lib/arb/worker_ci_gate.py
```

Railway blocks deployment when a pre-deploy command fails. This command checks
only GitHub's public metadata for the exact Railway-provided main commit and
repository. It requires successful main/push runs for ci.yml, recovery-review.yml
and delivery-review.yml; PR-only, missing, cancelled, neutral or skipped mandatory
runs cannot pass. Latest runs govern retries. Any additional failed main/push
workflow also blocks. Unknown response types, truncated results, redirects,
network errors and exhausted waiting are failures, not permission to deploy.

It makes at most 30 API requests, waits 20 seconds between pending results and
has a 660-second wall limit. It needs no GitHub/Railway token or RPC credential.
An API quota failure can legitimately prevent a deployment; rerun only after
checking the recorded reason. This does not promise that the UI checkbox is true,
that builds themselves wait, or that administrators cannot override deployment
configuration. It enforces runtime admission through the explicit pre-deploy step.

The gate also reads an optional `ARB_CI_GATE_GITHUB_TOKEN` environment variable
(#181). Unset, it behaves exactly as above with an unauthenticated request; set,
it sends the value as an `Authorization: Bearer` header to reduce Railway's shared
egress hitting GitHub's per-IP rate limit. The owner creates this token themselves
as a fine-grained personal access token scoped to `makafeli/arbitrage-research`
only, with read-only `Actions` permission and a short expiry, then sets it on the
`base-research-worker` service; the orchestrator does not create tokens or
secrets. A malformed value fails closed with `CI_TOKEN_REJECTED` before any
request is sent, and the token is never logged or included in gate output.

The subsequent finite start command is:

```text
worker-entrypoint worker-readiness-check
```

It reuses the mounted volume check, then runs fixed SELECT queries in a read-only
transaction with statement/lock/connect/process limits. Only scoped counts and
bounded session/source identifiers, states and digests are returned. Account
tables, configuration bodies, provider URLs, passwords and free-text errors are
not read or logged. It does not migrate, create a session, initialize/rearm a source,
start the market worker or contact Base. Up to 20 sessions/streams are returned;
counts identify truncation. Missing settings remain listed by name only. Even a
complete metadata report always states runtime_qualified=false.

Database inspection defaults to `sslmode=require` and refuses `prefer`, `allow`,
`disable` and empty mode values before connecting. This enforces encryption without
claiming authenticated server identity; `require` is not `verify-full`. The container
drill first proves refusal of a reachable plaintext-only server, then enables
throwaway TLS and verifies the same read-only role succeeds without state changes.
Workflow basenames may contain dots; this does not relax mandatory checks or allow
additional failed workflows to pass the gate.

The image includes Python and a PostgreSQL client for these finite operational
commands; the research loop is unchanged. Tests use synthetic metadata and a
separate pinned PostgreSQL container with a SELECT-only role. Original package,
Rust, API and browser checks remain required. Check the current PR/#58 for real
CI, merge and hosted results instead of inferring success from this runbook.

The Base URL remains a required confidential input. It is absent from the last
observed Railway namespace inventory and cannot be invented from a variable name.
The integrator owns the remaining matching runtime configuration, state inspection,
explicit initialization and control/recovery exercise. Do not instruct the owner
to invent session IDs, pool registries or disable tests. Automatic paper settlement
and original release acceptance remain separate, unfinished work.

## Historical infrastructure checkpoint: PR144, 17 September 2026

The authenticated Railway connection confirmed the following production resources:

| Resource | Recorded identity and state |
| --- | --- |
| Project | `c7cea88d-24b8-45dc-9602-7c0c272f1da7`, arbitrage-research |
| Environment | `5ff4044a-b0ac-4b02-b9cd-888cd91a3fee`, production |
| Prepared worker | `6b546a9d-ebd1-4443-9526-fbd61acf96e6`, base-research-worker |
| Dedicated volume | `8e809a27-c9f5-4658-bbea-3f2611d7b11a`, base-research-captures |
| Volume placement | 1024 MB, `/data`, europe-west4-drams3a |
| Worker deployment | None; no repository/image source has been attached |

The prepared service has one configured replica in the same region, no public
worker domain or TCP proxy, and restart policy NEVER. Its database settings refer
to the existing private postgres service. Operator IDs are `operator`; request
pacing is 75 ms. No account credential is needed by a research worker.

The preceding web/API deployments were confirmed on source `9ac1c468`. The
postgres deployment and its 50000-MB volume were left unchanged. Check Railway
again for current deployment state rather than treating this dated record as live
health monitoring. No production database session or stream query was performed;
missing environment variables are not proof that stored sessions do not exist.

## Historical PR144 repository and image changes

The desired worker source is the same GitHub repository on main with Wait for CI.
Its explicit start command is `worker-entrypoint worker-volume-check`, a finite
storage preflight, **not** the research loop. Restart policy NEVER prevents that
completed command being restarted. The dated checkpoint above predates attachment;
read the current issue and Railway deployment for the actual applied state.
The preflight writes, syncs, reads back and deletes only one uniquely named probe
inside the dedicated capture directory. It emits VOLUME_CHECK_PASSED and exits.
It performs no provider/database request, source initialization or session START.


The full `.railway/railway.ts` graph now includes the prepared worker and dedicated
volume. Its declared source/start command are limited to the storage preflight, not
initialization or research activation. Review the complete target plan before any later `config apply`; the
existing web/API/database resources must remain. This change has not executed an
authenticated CLI plan and makes no zero-drift claim. The preflight start command is not a
permanent runtime lock: replacing it requires a separate configuration review.

The existing worker image packages both `research-worker` and `base-ingest` from
the same source, plus only the disabled research template. Owner activation data,
RPC values and enabled production registries are not copied. The image default command
remains `research-worker`, while Railway explicitly overrides it with the one-shot
preflight. Adding a CLI never invokes its migrations, initialization or collection
actions automatically. The image build runs only `base-ingest --check`,
which reports NOT_STARTED, zero provider requests and no execution authorization.

The entrypoint rejects `/data` indirection and a capture path that is a symlink or
regular file before attempting directory ownership changes. It prepares only
`/data/captures` with mode 0700 and UID/GID 10001, then drops privileges. A previously
prepared unprivileged invocation still works. These checks do not constitute a
lock against a hostile concurrent volume writer; keep the existing single-owner
volume/process rule.

## Historical PR144 container verification

The mandatory `containers` job now runs:

```sh
bash scripts/test_worker_container.sh arb-worker:test
```

It uses the locally built image, `--pull=never`, `--network none`, a read-only root
filesystem and one newly created disposable volume. Six scenarios check:

1. Missing `/data` is refused instead of using ephemeral image storage.
2. Root startup drops to UID/GID 10001, enforces capture-directory ownership and
   0700 permissions, and successfully writes, syncs and reads back a temporary file.
3. An already unprivileged startup can reuse the prepared volume.
4. The packaged CLI refuses initialization without required settings.
5. A symlink at `/data/captures` is refused.
6. A regular file at `/data/captures` is refused.

The hosted-preflight command must return the inert CLI result and the exact
VOLUME_CHECK_PASSED record. Direct check also verifies the full inert result. The
script removes only its own disposable volume and temporary output directory.
It passes no database or provider credentials and never starts the research loop.
The existing full CI checks remain mandatory. Consult the PR and #58 for actual
results; writing this document does not establish a passing run. A Docker smoke
test is not a filesystem test inside the hosted Railway volume.

## Remaining activation work

A fresh environment-reference read at this checkpoint found no ARB_BASE_RPC_URL
in either shared or worker scope. The owner must store the existing Base RPC URL
directly in Railway, not in a repository, ticket, output artifact or conversation.
References to that variable must only be installed once it exists. Do not invent
an endpoint or reconfigure an unrelated provider account.

The integrator still owns the following deployment work under #58:

- Prepare and review matching enabled configuration and registry files, using the
  existing approved identities and explicit supported tick bounds. The disabled
  template cannot be used as an enabled market profile. A 1024-MB volume also
  requires an explicitly smaller capture quota; do not use the template's 10-GiB
  quota on this volume.
- Inspect existing stored session/source state before creating anything. Register
  the exact same validated profile with API and worker, select the immutable
  research session, and initialize a new source only when genuinely required.
  Use the existing CLI's explicit actions, never implicit reseeding or HALT reset.
- Verify the finite storage preflight deployment, then inspect mounted-file
  permissions and seed/source binding before replacing the preflight command with
  the actual research worker. Source attachment alone does not start research.
  Pre-deploy hooks have no mounted capture volume; do not use one as a substitute
  for mounted runtime initialization.
- Exercise real-data capture, authenticated START/STOP acknowledgement and restart
  in the hosted environment. Preserve checkpoint/quote fences, bounded request
  budgets and the difference between stopped research and ongoing readiness reads.

The CLI and template packaging do not create a session or an ACTIVE source. A
successful image build does not establish a running market feed. Complete
simulation, virtual settlement, backup/restore and original dependency acceptance
remain required. Paper and Real navigation and the owner's account are unchanged;
Real execution remains unavailable.

References: [managed worker](MANAGED-BASE-WORKER.md),
[deployment runbook](../deploy/RAILWAY.md),
[Railway IaC reference](https://docs.railway.com/infrastructure-as-code/reference),
[Railway volume behavior](https://docs.railway.com/volumes).
