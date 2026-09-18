# PostgreSQL leaf certificate maintenance

Existing ARB-044 / #58. This is an explicit maintenance tool, not automatic
production activation, a backup service, or a completed research release.

## Current deployment boundary

On 18 September 2026 the production PostgreSQL public server certificate had
`CA:TRUE`, despite the correct private DNS SAN. PR148 requires `verify-full` in
the launcher and Rust session checker. Do not disable verification to mask the
certificate problem. The existing public CA is available through authenticated
Railway configuration; neither its private key nor the server private key belongs
in ChatGPT, GitHub, downloaded artifacts or service variables.

The connected Railway tools provide configuration/deployment changes and file
reads, but no verified container exec, SQL/reload or volume-backup capability.
A writable mount does not establish such a tool. The production repair has NOT
been applied by this implementation. Use an existing authorized operator
connection, after the review/CI and backup gates below. No new provider account
or Base RPC/session configuration is needed.

PostgreSQL rereads certificate files on configuration reload; a full restart is
not intrinsically required. This differs from the connected tool's inability to
send a reload. Railway pre-deploy containers do not mount volumes, so a
pre-deploy command cannot repair the database's mounted certificate files.

Sources checked 18 September 2026:
- https://www.postgresql.org/docs/18/ssl-tcp.html
- https://docs.railway.com/deployments/pre-deploy-command
- https://docs.railway.com/cli/ssh

## Gate before any production apply

Verify the exact service, mounted paths and current public certificate pins.
Record a successful current volume backup/restore route in #58 using an actual
backup operation. Neither documentation, plan backup limits nor a missing WAL
variable proves a backup exists. Preserve the currently deployed image/version,
PGDATA and Railway volume/runtime locks. Keep the observation launcher stopped.

Use the exact reviewed/CI-tested script bytes, run as the certificate file owner
(normally `postgres`). It needs Bash, OpenSSL 3, coreutils and flock. The staging
directory and its parents are trusted administration paths; this script does
not isolate a hostile filesystem owner, concurrent manual key rotation or a
privileged adversary. Coordinate exclusive certificate maintenance separately.

Default invocation and `--check` are inert. All operations take five arguments:

```text
bash scripts/postgres_leaf_repair.sh ACTION CERT_DIRECTORY ROOT_DER_SHA256 ORIGINAL_SERVER_DER_SHA256 DNS_HOST
```

Use lower-case 64-digit DER SHA256 hashes, not the text-file hash or subject-key
identifier. An unexpected current certificate or CA must be inspected, never
silently accepted as the new baseline.

## Three explicit operations

`--prepare` verifies the current root and server pins and matching server key,
then signs a 90-day `CA:FALSE`/`serverAuth` leaf with the EXISTING root and server
keys. The SAN includes localhost and the supplied private hostname. It creates
only a private `.arb-leaf-repair` record: exact original public certificate,
new public certificate, public CSR and anchors. It never rotates/copies private
keys, changes root.srl, edits PostgreSQL config or replaces the active leaf.
An existing record refuses re-preparation rather than overwriting rollback.

`--apply` rechecks the pins, recorded candidate, hostname, lifetime, chain and
server public key. It preserves the original certificate's owner/group/mode,
syncs staged data and atomically replaces only `server.crt`. Repeating apply of
the same candidate is safe. The result is `LEAF_REPLACED_RELOAD_REQUIRED`, NOT a
verified handshake or successful hosted activation.

`--rollback` accepts only the recorded candidate or original as the active
certificate and atomically restores the exact original public bytes. It does
not restore a changed key or unknown certificate. It explicitly restores the
original CA:TRUE weakness; the strict worker must remain stopped. The result is
`ORIGINAL_RESTORED_RELOAD_REQUIRED`. This certificate-only rollback is not a
full database backup or automatic recovery from an unobserved interruption.

## Exact existing production target

Project: `c7cea88d-24b8-45dc-9602-7c0c272f1da7`.
Environment: `5ff4044a-b0ac-4b02-b9cd-888cd91a3fee`.
Database service: `ff012117-92dc-433f-a2a3-ed61f3f33ce9`.
Volume: `744e7772-2b2c-4998-92f4-dabc529b0d9c`.
Certificate directory: `/var/lib/postgresql/data/certs`.
PGDATA: `/var/lib/postgresql/data/pgdata`.
Private hostname: `postgres.railway.internal`.

Authenticated public file reads were parsed independently with OpenSSL:

```text
ROOT=5385eb665eebc8edbd62aeb609319501ee1f788c5dd527b0b1f495e5f948e3f5
ORIGINAL=998b112f74c3a11b86cad451a2b7a757901f14c0b1a5b473b5b03977be3881a7
```

These are a dated reference, not permission to ignore a later certificate
rotation. No production private-key values are stored here.

## Reload and verify from the authorized runtime

After explicit apply, the authorized operator can use PostgreSQL's installed
`pg_ctl reload -D /var/lib/postgresql/data/pgdata` as postgres. The maintenance
script intentionally issues no SQL, signals, restart, worker START or network
request. Verify the ACTUAL served certificate, not only the on-disk path or the
reload exit code: PostgreSQL may keep its old certificate on a failed reload.

Compare the served SHA256 with `.arb-leaf-repair/next.sha256`, then test the
shipped `worker-session --status /data/runtime/base-v1` from the worker with its
existing trusted CA, profile, operator and database settings. Require the
original registered session receipt. Do not register a new session. Record the
postmaster start time and unchanged application state before/after. A public
certificate match alone does not prove Rustls/database authentication.

If verification fails, explicitly rollback and reload, verify the original peer
fingerprint and service health, and retain the failure evidence. Do not issue
research START, initialize/reset a source or rotate the CA to make checks pass.

## Renewal and limitations

The image's original init-ssl template remains unchanged. Its renewal path may
recreate the invalid certificate or rotate the CA. This is a one-time repair,
not a solved certificate lifecycle: review/reissue before the 30-day renewal
threshold. No recurring renewal has been installed. Stage records must not be
silently deleted or repurposed for a new certificate generation.

## Tests

`python scripts/test_postgres_leaf_repair.py -v` runs actual local OpenSSL tests
using generated synthetic keys: inert default, prepare/apply/rollback,
idempotency, immutable keys/CA/serial marker, mismatched anchors, tampered
staging, links, writable paths, input bounds and maintenance locking.

`bash scripts/test_postgres_leaf_repair_container.sh arb-worker:test` exercises
the shipped Rust verifier against isolated PostgreSQL 17.11. It proves the
CA:TRUE rejection, prepare without active change, apply + configuration reload,
verified database read, and rollback + rejection. Actual peer fingerprints,
unchanged postmaster start time, sentinel rows, keys/CA and zero session/stream
counts are compared. Production is PostgreSQL18; this isolated drill is not a
production handshake or production backup. Exact execution results belong on
the implementing PR and #58; authored tests alone are not a pass.
