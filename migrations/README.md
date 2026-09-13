# Control database migrations and recovery

`arb_storage::Store::migrate()` applies embedded SQLx migrations with PostgreSQL transaction and checksum tracking. Run migrations once during the controlled application startup phase before accepting HTTP requests; CI verifies PostgreSQL 17.11. The test suite requires `TEST_DATABASE_URL` and fails when it is absent.

Migration `0001_control.sql` creates immutable configuration snapshots, research sessions, creation idempotency keys, revisioned command receipts, and append-only audit events. Research modes are constrained in PostgreSQL; LIVE cannot be inserted. Session mode/network/config/experiment/strategy identity is immutable. SQLx migration checksums detect edited applied migration files. Add a new migration for changes; do not edit a migration already applied outside a disposable test database.

Migration `0002_decisions_and_paper.sql` adds immutable decision traces and bounded virtual-accounting journals. Migration `0003_collection_attempts.sql` adds durable pre-I/O collection batch starts, immutable terminal telemetry and unique decision-to-batch associations. Earlier migration checksums remain unchanged. Collection IN_PROGRESS rows survive restart and restoration as unknown outcomes; never erase or auto-resolve them to improve coverage statistics. Collection telemetry is independent of financial/drain attempts and must not block or fabricate their reconciliation.

Migration `0004_cost_assessments.sql` adds append-only manual cost assessments and a composite source-decision foreign key. It adds a supporting unique identity constraint to decision traces; earlier migration files and checksums remain unchanged. Assessments reference their original quote and retain scenario/report digests. The migration does not alter virtual balances, source quotes, worker state or execution capability. Apply migrations before starting the updated API. Deploy the API and dashboard together for export schema 1.1.0; older strict clients reject the added sixth dataset.

## Backup, rollback, and restore

1. Stop API writes and fence/stop all cooperating research workers. A browser outage does not prove that a worker stopped. Preserve unresolved attempts and retain the journal.
2. Take a verified PostgreSQL backup (`pg_dump --format=custom`) before upgrading. Store the binary backup separately with controlled access and verify restoration into an isolated database.
3. Apply migrations to a restored rehearsal database. Run the integration suite against a separate disposable test database and run the target binary against the rehearsal database to verify reads.
4. Deploy the compatible binary and migrate production research state. Save schema version, migration checksums, binary revision and a backup reference in the deployment record.
5. If application rollback is needed, use an older binary only if its recorded compatibility matrix supports the current schema. These migrations provide no destructive down migration. Do not DROP or truncate control/audit tables to make an old binary start.
6. If restoration is necessary, stop writers, restore into a new database, compare retained command/audit history and explicitly reconcile events after the backup cutoff from the preserved old database. Never resume automatically or overwrite the only surviving audit copy. Workers restart RECOVERING with closed local gates, then STOPPED after unresolved research work is reconciled.

The append-only trigger guards ordinary UPDATE/DELETE statements; a database administrator can still disable triggers or alter/drop tables. Use separate runtime/migration database roles and restricted backup access in deployment. This schema does not replace PostgreSQL durability/backup configuration and does not provide a live transaction/nonce/ledger journal.
