# Quiesced backup and isolated recovery

Tracking: existing #103 and [ARB-044 / #58](https://github.com/makafeli/arbitrage-research/issues/58). This operator tool does not accept the broader research release, its prerequisites, production backup scheduling or worker recovery. Use Python 3.11+ on POSIX and matching PostgreSQL client/server majors. CI uses PostgreSQL 17.11 against the real migrations with synthetic storage fixtures; both job and service images are pinned to the same Docker Official Images digest.

## Scope and trust

The CLI creates a PostgreSQL custom-format dump, byte-for-byte copies of one raw capture root, database table/sequence evidence, and `manifest.json` as its final completion marker. Keep the SHA-256 printed at successful backup in a **separate trusted record**, not only beside the bundle. Verification hashes that manifest and every listed file and rejects missing, extra, modified or unsafe files. Checksums detect changes relative to that trusted digest; they do not authenticate an attacker-supplied manifest or make an untrusted database archive safe.

The backup is unredacted and **not encrypted**. Raw captures, database rows, identifiers and accidentally stored credentials can be sensitive. New output directories use mode 0700 and copied files/metadata use 0600. Use a protected encrypted volume, approved encrypted offsite storage and separate access controls. Never commit, attach to GitHub, or publish production bundles, diagnostics or evidence JSON. CI uploads none of them.

PostgreSQL restores can execute code supplied by source database owners. Restore only an operator-trusted, reviewed source in an isolated environment without application services, production credentials or unrestricted outbound network access. `--trusted-source` records that acknowledgement; it is not a code scanner. The `arb_restore_*` target restriction and empty-object check are safeguards, **not proof of environmental isolation**.

## Stop all writers before backup

Stop API processes that create sessions, commands, costs or paper runs. Stop both research workers, standalone capture processes, retention jobs, maintenance writers and anything else touching the database or capture volume. A dashboard **STOP is insufficient**: a stopped/paused worker may still collect readiness captures.

The CLI requires `--quiesced` and compares source table/sequence contents and capture inventory before and after collection. These comparisons are not a distributed lock or proof that no intermediate concurrent write occurred. PostgreSQL supplies its own consistent dump, but filesystem and database boundaries are not one atomic transaction. Maintain quiescence throughout. Concurrent DDL, database users and filesystem writers are outside this operating procedure.

Use the correct libpq context from deployment secrets: `PGHOST`, `PGPORT`, `PGUSER`, and preferably a protected `PGPASSFILE` or `PGSERVICE`. Database arguments are literal names, not connection strings. Never put secrets in CLI arguments. Source PostgreSQL connections are read-only. The tool overrides `PGOPTIONS` for UTC/ISO serialization and bounded statement execution; configure TLS through libpq TLS environment/service settings.

With Railway private networking, use an approved administrative context that can reach the private database and the selected capture volume. No public database proxy is required or created. The existing Railway web/API/PostgreSQL deployment is already established and is not reprovisioned by this tool.

## Backup and offline verification

Resolve symlinked parents first, for example `/private/tmp` rather than the macOS `/tmp` alias. Roots and ancestors must be real directories. Capture names permit letters, digits, `_`, `-`, `.`, and directory separators; symlinks, hardlinks, device files and pipes are rejected. The output must be new and outside the capture root.

```sh
# Supply private connection settings before running these commands.
python3 scripts/recovery.py backup \
  --database railway \
  --captures /protected/research-captures \
  --output /protected/backups/research-20260913 \
  --source-commit ACTUAL_40_CHARACTER_DEPLOYED_COMMIT \
  --diagnostics /protected/diagnostics/backup-20260913 \
  --quiesced

python3 scripts/recovery.py verify \
  --bundle /protected/backups/research-20260913 \
  --manifest-sha256 TRUSTED_64_CHARACTER_MANIFEST_DIGEST
```

Replace the placeholders with the deployed commit and the digest returned by successful backup. Diagnostic and artifact parents must already exist; their final output directories must not exist.

Defaults are 50,000 artifact files, 10 GiB combined bundle content, 300 seconds **per subprocess**, at most 1,024 database relations and 16 MiB per metadata document. Change `--max-files`, `--max-bytes`, and `--timeout` only after capacity review. Metadata limits remain fixed. Evidence generation performs full sorted table scans and needs PostgreSQL sort/temp space. This is an offline maintenance tool, not a latency-safe hot-path operation. The dump size is checked after `pg_dump` completes; the bundle limit is not a disk quota. Large deployments require a reviewed filesystem quota or different backup design.

Failure may leave private partial output without a valid final manifest/digest. Do not use it as a complete backup. No existing directory is overwritten or automatically removed. Inspect partial output and confirm its ownership before cleanup. For power-loss durability retain a validated copy in approved storage; a local write is not a distributed durability guarantee.

## Private failure diagnostics

Every PostgreSQL command, including `psql`, `pg_dump` and `pg_restore`, retains stderr in a new operation-specific directory. Pass `--diagnostics` to choose its location. Otherwise the resolved system temporary directory receives `arb-recovery-backup-<diagnostics_id>` or `arb-recovery-restore-<diagnostics_id>`. Output includes only that opaque ID, never the diagnostic path or contents. A precondition failure before a command may leave no directory.

The directory is 0700. `operation.json`, sequential `command-NNNN.json`, and `command-NNNN.stderr` files are 0600. Metadata records completion, exit code, byte counts and truncation, not command arguments, SQL or connection details. stderr itself can contain sensitive database data: inspect it privately, never paste it into public tickets or logs.

Retention is bounded at **1 MiB per command**, **4 MiB stderr per operation**, and **4,096 command records**. After a byte limit the process pipe is still drained to avoid deadlock; truncation is explicitly recorded. Command deadlines and stdout limits remain enforced. Failed process creation also leaves private completion metadata where possible. No retention deletion is automatic: operators own encrypted storage, access control and cleanup of diagnostics.

The diagnostic directory must be separate from, and neither contain nor be contained by, capture, backup or restore roots. It is never part of the backup manifest or restored capture inventory. Keep these roots private and stable throughout the operation. This is not a multi-user hostile-filesystem locking mechanism.

## Isolated restore

Create a **new empty database from `template0`** in a disposable isolated environment with the same PostgreSQL major. Do not point this command at production. Provision required roles/extensions separately; ownership and ACLs are not reinstated by the drill.

```sh
# Connection settings must now address the isolated restore service.
python3 scripts/recovery.py restore \
  --bundle /protected/backups/research-20260913 \
  --manifest-sha256 TRUSTED_64_CHARACTER_MANIFEST_DIGEST \
  --database arb_restore_drill_20260913 \
  --confirm-target arb_restore_drill_20260913 \
  --captures /protected/restore-drill/captures \
  --diagnostics /protected/diagnostics/restore-20260913 \
  --trusted-source
```

The bundle is verified before database access. The target must match its confirmation, differ from the saved source name, and start with `arb_restore_`. It must have no application relations, functions, types, additional schemas, event triggers or extensions other than plpgsql. The capture destination must be new. The CLI copies capture bytes, reverifies the protected bundle, and runs `pg_restore --single-transaction --exit-on-error --no-owner --no-acl`, **never `--clean` or `--create`**.

Afterward all inventoried table/materialized-view rows, sequence states and capture hashes must match source evidence. The success record is `ISOLATED_RESTORE_VERIFIED`, with `workers_started: 0` and `production_accepted: false`. Historical worker states and pending commands are preserved, not normalized or applied. Application startup/recovery must separately establish safe STOPPED behavior before research starts.

Filesystem copying and the database transaction are not atomic together. A failed/cancelled restore can leave copied captures or a restored database without a success report. Keep it quarantined and never attach workers automatically. Do not retry into a nonempty target; after inspection create a fresh isolated target. No rollback command deletes operator data.

## Retention and remaining acceptance

Raw manifest bytes and expiry values remain unchanged. Byte verification does **not** validate protocol semantics, market provenance, expiry eligibility or completeness of every database capture reference. A raw dependency already missing at backup remains a separate acceptance problem. Restored bytes do not establish that an expired capture remains replayable.

The full dump includes stored configurations, command/audit history, paper journals and capture references, rather than only the six dashboard export datasets. Cluster roles, server configuration, deployment secrets, artifacts outside the chosen root, offsite encryption, retention scheduling/deletion, application-aware replay and production crash/restart drills remain outside this slice. No worker or live-trading service starts. Do not close #58 solely because CI passed.

## Tests and evidence

```sh
python3 scripts/test_recovery.py -v
```

The suite contains the original 24 offline cases plus 10 diagnostic regressions. Database calls in the offline suite are mocked; pipe/deadline/diagnostic cases launch real local test subprocesses. This does not establish database integration. Recovery review separately runs the real project migrations on disposable PostgreSQL 17.11 and verifies synthetic storage rows, sequences, exact 24-digit values, pending commands, capture expiry bytes, audit triggers, uniqueness and actual private diagnostics for failing `psql`, `pg_dump` and `pg_restore`. Missing prerequisites fail instead of skip.

CI also runs Ruff 0.11.13 explicitly with E4/E7/E9/F on the three recovery scripts; it does not depend on an assumed repository Ruff configuration. Docker image digest: `sha256:67f41722b7a8cbdb868a44a4995c846eddfdc2973bccb291ce937dce88ad5675`, from [Docker Official Images repo-info](https://github.com/docker-library/repo-info/blob/master/repos/postgres/remote/17.11.md), checked 13 September 2026. Image pinning does not pin subsequent operating-system package downloads.

Cite the exact tested source and CI run. A workflow definition or mock pass is not a passing PostgreSQL drill. Production Railway restore, deployment status and original release acceptance require separate evidence.

Primary references: [PostgreSQL 17 pg_dump](https://www.postgresql.org/docs/17/app-pgdump.html), [PostgreSQL 17 pg_restore](https://www.postgresql.org/docs/17/app-pgrestore.html). A database snapshot does not snapshot external capture storage, and restoring an archive can execute source-owner code.
