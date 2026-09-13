# Quiesced backup and isolated recovery

Tracking: #103, a bounded implementation slice of [ARB-044 / #58](https://github.com/makafeli/arbitrage-research/issues/58). This does not accept the broader research release, its prerequisites, production backup scheduling or worker recovery. Use Python 3.11+ on POSIX and matching PostgreSQL client/server majors. CI tests PostgreSQL 17.11 against the actual committed migrations with synthetic storage fixtures.

## Scope and trust

The CLI creates a PostgreSQL custom-format dump, byte-for-byte copies of one raw capture root, database table/sequence evidence, and `manifest.json` as its final completion marker. Keep the SHA-256 printed at successful backup in a **separate trusted record**, not only beside the bundle. Verification hashes that manifest and every listed file and rejects missing, extra, modified or unsafe files. Checksums detect changes relative to that trusted digest; they do not authenticate an attacker-supplied manifest or make an untrusted database archive safe.

The backup is unredacted and **not encrypted**. Raw captures, database rows, identifiers and credentials accidentally stored in application data can be sensitive. New output directories use mode 0700 and copied files/metadata use 0600. Use a protected encrypted volume, approved encrypted offsite storage and separate access controls. Never commit, attach to GitHub, or publish production bundles or evidence JSON. CI intentionally uploads no dumps, captured data, credentials or database rows.

PostgreSQL restores can execute code supplied by source database owners. Only restore an operator-trusted, reviewed source, in an isolated environment without application services, production credentials or unrestricted outbound network access. `--trusted-source` records that acknowledgement; it is not a code scanner. The `arb_restore_*` target naming restriction and empty-object check are additional safeguards, **not proof of environmental isolation**.

## Before backup: stop all writers

Stop API processes that can create sessions, commands, costs or paper runs. Stop both research workers, standalone capture processes, retention jobs, maintenance writers and anything else touching this database or capture volume. A dashboard **STOP is insufficient**: a stopped/paused worker may still collect readiness captures. Do not issue a backup while such processes are running.

The CLI requires `--quiesced` and compares source table/sequence contents and capture inventory before and after collection. These comparisons are not a distributed lock or a proof that no intermediate concurrent write occurred. PostgreSQL supplies its own transactionally consistent dump, but filesystem and database boundaries are not one atomic transaction. The operator must maintain quiescence throughout. DDL changes, other database users and concurrent filesystem writers are outside this supported operating procedure.

Use the correct libpq connection context from the deployment secret mechanism (`PGHOST`, `PGPORT`, `PGUSER`, and preferably a protected `PGPASSFILE` or `PGSERVICE`). Database arguments must be literal names, not connection strings; secrets are never placed in CLI arguments or tool output. The tool suppresses subprocess stderr and emits a generic failure rather than leaking private SQL/errors. Source PostgreSQL connections are read-only. It overrides PGOPTIONS for consistent UTC/ISO serialization and bounded statement execution; configure TLS using the usual libpq TLS environment/service settings.

With private Railway networking, run from an approved administrative context that can reach the private database and the relevant capture volume. No public database proxy is required or created. The existing Railway web/API/PostgreSQL deployment is already established; this runbook does not reprovision it.

## Backup and offline verification

Resolve symlinked parent paths first, for example `/private/tmp` rather than the macOS `/tmp` alias. Source/destination roots and their ancestors must be real directories. All capture names use letters, digits, `_`, `-`, `.`, and directory separators; symlinks, hardlinks, device files and pipes are rejected. The destination must be new and outside the capture root.

```sh
# Supply connection details privately before running these commands.
python3 scripts/recovery.py backup \
  --database railway \
  --captures /protected/research-captures \
  --output /protected/backups/research-20260913 \
  --source-commit ACTUAL_40_CHARACTER_DEPLOYED_COMMIT \
  --quiesced

# Use the exact 64-character manifest_sha256 returned by successful backup.
python3 scripts/recovery.py verify \
  --bundle /protected/backups/research-20260913 \
  --manifest-sha256 TRUSTED_64_CHARACTER_MANIFEST_DIGEST
```

Defaults: 50,000 artifact files, 10 GiB combined bundle content, 300 seconds **per subprocess**, at most 1,024 inventoried database relations and 16 MiB per metadata document. Use `--max-files`, `--max-bytes`, and `--timeout` only after capacity review. Metadata limits remain fixed. Table evidence uses sorted JSON row serialization, so full table scans and database sort/temp space are required. This is an offline maintenance tool, not a latency-safe hot-path operation. Provision enough free space for the dump, capture copy and PostgreSQL sort activity. The dump file's size is checked after pg_dump completes; the bundle limit is not an operating-system disk quota. Large deployments need a reviewed filesystem quota or a different backup design.

A failure may leave a private partial directory, without a valid final manifest/digest. Never use partial output as a complete backup. No existing directory is overwritten or automatically removed. Inspect and clean partial output explicitly after confirming its ownership. For power-loss durability, retain a validated copy in approved storage; the CLI does not claim distributed durability from a successful local write alone.

## Isolated restore

Create a **new empty database from `template0`** in a disposable isolated environment with the same PostgreSQL major, using an operator identity allowed to restore the reviewed source. Do not run this against the production application environment. Provision roles/extensions separately as required; ownership and ACLs are deliberately not reinstated by this drill.

```sh
# Connection environment must now point at the isolated restore service.
python3 scripts/recovery.py restore \
  --bundle /protected/backups/research-20260913 \
  --manifest-sha256 TRUSTED_64_CHARACTER_MANIFEST_DIGEST \
  --database arb_restore_drill_20260913 \
  --confirm-target arb_restore_drill_20260913 \
  --captures /protected/restore-drill/captures \
  --trusted-source
```

The bundle is verified before database access. The target name must match its explicit confirmation, differ from the saved source database name, and start with `arb_restore_`. The database must contain no application relations, functions, types, additional schemas, event triggers or extensions other than plpgsql. The capture destination must not exist. The tool copies capture bytes into that new root, reverifies the protected bundle, then uses pg_restore with `--single-transaction --exit-on-error --no-owner --no-acl`, **never `--clean` or `--create`**.

After restore, every inventoried table/materialized-view row and sequence state is compared with source evidence; capture hashes are compared again. The success record is `ISOLATED_RESTORE_VERIFIED`, with `workers_started: 0` and `production_accepted: false`. The tool neither normalizes worker states nor marks commands APPLIED. Historical states and pending commands are restored unchanged; application startup/recovery must separately establish safe STOPPED behavior before an operator starts research.

Filesystem copying and the PostgreSQL transaction are not atomic together. Failed/cancelled restores can leave copied captures or a restored database without a final success report. Keep that target quarantined; never connect workers to it automatically. Do not retry into a nonempty target. No rollback script deletes operator data. After inspection, create a fresh isolated target for the next drill.

## Retention and remaining acceptance

Raw manifest bytes and expiry values remain unchanged. File verification does **not** validate protocol semantics, prove market provenance, extend capture expiry or certify that every database capture reference exists. Missing raw dependencies that were already absent from the source remain a separate release acceptance task. Restore-time byte equality is not proof that an old calculation can still be replayed under its original retention policy.

The full dump includes the database's stored configuration, command/audit history, paper journal and capture references, rather than only the six dashboard export datasets. Cluster roles, server configuration, deployment secrets, external artifacts outside the selected root, offsite encryption, retention scheduling/deletion, application-aware replay and production crash/restart drills remain outside this slice. No research worker or live trading service is started. Do not close #58 merely because this CLI passed in CI.

## Tests and evidence

```sh
python3 scripts/test_recovery.py -v
```

These 24 unit/adversarial tests mock database operations; they verify file integrity, refusal paths, safe CLI error handling, subprocess limits and command construction, not real database restore. The separate **Recovery review** workflow runs `scripts/test_recovery_postgres.py` against disposable PostgreSQL 17.11, applies all real migrations, seeds clearly synthetic storage rows, backs up/restores, and compares bytes, sequences, exact 24-digit values, pending commands, uniqueness and append-only audit protections. Missing service prerequisites fail the drill rather than skipping it. Those storage fixtures are not valid market captures or a complete running worker lifecycle.

CI results must be cited with the exact tested source head and run ID. A workflow definition or successful mock test is not a passing PostgreSQL drill. Existing full Rust/PostgreSQL, browser, contract and container CI remains separate. Production Railway restore evidence and a new production deployment must be verified independently.

Primary behavior references: [PostgreSQL 17 pg_dump](https://www.postgresql.org/docs/17/app-pgdump.html), [PostgreSQL 17 pg_restore](https://www.postgresql.org/docs/17/app-pgrestore.html). In particular, pg_dump's database snapshot does not snapshot capture storage, and a pg_restore archive can execute source-owner code.
