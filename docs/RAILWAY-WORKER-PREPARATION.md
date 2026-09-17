# Private Base worker deployment preparation

Existing task: ARB-044 / #58. This record is not acceptance of the full deployment,
provider qualification, automatic paper settlement or live execution.

## Connected infrastructure checkpoint: 17 September 2026

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

## Repository and image changes

The full `.railway/railway.ts` graph now includes the prepared worker and dedicated
volume. It deliberately does not attach a source or add an initialization/start
command. Review the complete target plan before any later `config apply`; the
existing web/API/database resources must remain. This change has not executed an
authenticated CLI plan and makes no zero-drift claim. Source omission is not a
runtime lock: do not attach one out of band before reviewing configuration.

The existing worker image packages both `research-worker` and `base-ingest` from
the same source, plus only the disabled research template. Owner activation data,
RPC values and enabled production registries are not copied. The default command
remains `research-worker`; adding a CLI never invokes its migrations, initialization
or collection actions automatically. The image build runs only `base-ingest --check`,
which reports NOT_STARTED, zero provider requests and no execution authorization.

The entrypoint rejects `/data` indirection and a capture path that is a symlink or
regular file before attempting directory ownership changes. It prepares only
`/data/captures` with mode 0700 and UID/GID 10001, then drops privileges. A previously
prepared unprivileged invocation still works. These checks do not constitute a
lock against a hostile concurrent volume writer; keep the existing single-owner
volume/process rule.

## Actual container verification

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

Both successful check invocations must return the exact inert CLI result. The
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
- Attach the reviewed GitHub source with Wait for CI, verify mounted-file
  permissions and seed/source binding, then deploy and inspect the actual worker.
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
